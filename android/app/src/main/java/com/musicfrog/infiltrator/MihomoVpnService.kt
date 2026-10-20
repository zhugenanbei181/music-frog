package com.musicfrog.infiltrator

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.VpnService
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.ParcelFileDescriptor
import android.os.PowerManager
import android.os.SystemClock
import android.util.Log
import androidx.core.app.NotificationCompat
import infiltrator_android.AndroidControlIntent
import infiltrator_android.AndroidVpnConfigurationActivationLease
import infiltrator_android.AndroidVpnRoutingActivationLease
import infiltrator_android.AndroidVpnRoutingActivationReleaseResult
import infiltrator_android.AndroidVpnRoutingActivationResult
import infiltrator_android.FfiErrorCode
import infiltrator_android.androidBeginVpnConfigurationActivation
import infiltrator_android.androidBeginVpnRoutingActivation
import infiltrator_android.androidControl
import infiltrator_android.androidFinishVpnConfigurationActivation
import infiltrator_android.androidFinishVpnRoutingActivation
import infiltrator_android.cancelVpnPreparation
import infiltrator_android.prepareVpn
import infiltrator_android.startVpn
import infiltrator_android.stopVpnWorker
import kotlinx.coroutines.runBlocking
import java.io.IOException
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.ThreadPoolExecutor
import java.util.concurrent.TimeUnit

private fun <T> VpnOwnedResource<T>.toOwnerWireState(): VpnOwnerResourceStateWire = when (this) {
    VpnOwnedResource.Absent -> VpnOwnerResourceStateWire.Absent

    is VpnOwnedResource.Held -> VpnOwnerResourceStateWire.Held(generation)

    is VpnOwnedResource.CleanupPending -> VpnOwnerResourceStateWire.CleanupPending(
        generation,
        failure.message,
    )
}

private fun VpnOwnedResource<VpnRoutingLeaseState>.toRoutingLeaseOwnerWireState(): VpnOwnerResourceStateWire = when (this) {
    VpnOwnedResource.Absent -> VpnOwnerResourceStateWire.Absent

    is VpnOwnedResource.CleanupPending -> VpnOwnerResourceStateWire.CleanupPending(
        generation,
        failure.message,
    )

    is VpnOwnedResource.Held -> when (val leaseState = value) {
        VpnRoutingLeaseState.Released -> VpnOwnerResourceStateWire.Absent

        is VpnRoutingLeaseState.Held -> VpnOwnerResourceStateWire.Held(generation)

        is VpnRoutingLeaseState.ReleasePending -> VpnOwnerResourceStateWire.CleanupPending(
            generation,
            leaseState.releaseFailureMessage() ?: "VPN routing lease release is pending",
        )
    }
}

private typealias MihomoVpnOwnerResources = VpnOwnerResourceRecord<
    ParcelFileDescriptor,
    VpnWorkerOwnership,
    VpnRoutingLeaseState,
    VpnPreparedOwnership,
    >

class MihomoVpnService : VpnService() {
    internal val ownerResources: MihomoVpnOwnerResources
        get() = sharedOwnerResources
    private lateinit var vpnExecutor: ThreadPoolExecutor
    internal lateinit var systemPolicyExecutor: ThreadPoolExecutor
    internal val serviceAdmission = VpnServiceExecutorAdmission()
    private val queuedStartTasks = ConcurrentHashMap<Long, StartTransactionTask>()
    internal var notificationActionMode = VpnServiceNotificationActionMode.UserManaged
    internal var lifecycleWakeLock = VpnLifecycleWakeLock(
        UnavailableVpnWakeLockHandle,
        VpnLifecycleWakeLock.MAX_LEASE_DURATION_MILLIS,
    )

    override fun onCreate() {
        super.onCreate()
        lifecycleWakeLock = createLifecycleWakeLock()
        vpnExecutor = ThreadPoolExecutor(
            1,
            1,
            0L,
            TimeUnit.MILLISECONDS,
            ArrayBlockingQueue(1),
        ) { runnable -> Thread(runnable, "musicfrog-vpn-lifecycle") }
        systemPolicyExecutor = ThreadPoolExecutor(
            1,
            1,
            0L,
            TimeUnit.MILLISECONDS,
            ArrayBlockingQueue(1),
        ) { runnable -> Thread(runnable, "musicfrog-vpn-system-start") }
        createNotificationChannel()
        when (val retry = lifecycle.retryPendingCleanup()) {
            is VpnCleanupPlan.Retry -> queueCleanup(retry, stopId = null)

            is VpnCleanupPlan.Dispatch,
            is VpnCleanupPlan.Join,
            is VpnCleanupPlan.AlreadyClean,
            VpnCleanupPlan.Superseded,
            -> Unit
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (serviceAdmission.phase() != VpnServiceAdmissionPhase.Accepting) return START_NOT_STICKY
        val decision = decideVpnServiceStartup(
            command = readServiceCommand(intent),
            delivery = readServiceDelivery(flags),
            alwaysOn = readAlwaysOnObservation(),
        )
        notificationActionMode = when (decision) {
            is VpnServiceStartupDecision.ExecuteCommand -> decision.notificationMode

            is VpnServiceStartupDecision.RestoreSystemManagedProxy ->
                VpnServiceNotificationActionMode.SystemManaged

            is VpnServiceStartupDecision.Reject -> VpnServiceNotificationActionMode.UserManaged
        }
        return when (decision) {
            is VpnServiceStartupDecision.ExecuteCommand -> when (decision.command) {
                VpnServiceCommand.Start -> intent?.let { startForegroundAndQueue(it, startId) }
                    ?: failClosedUnrequestedRestart(startId, VpnServiceStartupRejection.MalformedCommand)

                VpnServiceCommand.Stop -> intent?.let { queueStop(it, startId) }
                    ?: failClosedUnrequestedRestart(startId, VpnServiceStartupRejection.MalformedCommand)
            }

            is VpnServiceStartupDecision.RestoreSystemManagedProxy ->
                startSystemManagedProxy(startId, decision)

            is VpnServiceStartupDecision.Reject ->
                failClosedUnrequestedRestart(startId, decision.reason)
        }
    }

    override fun onDestroy() {
        if (serviceAdmission.beginDestroy() == VpnServiceAdmissionPhase.Destroying) {
            val snapshot = lifecycle.snapshot()
            val cleanupPlan = when (snapshot.state) {
                VpnProcessState.Stopped -> when (ownerResources.snapshot().presence()) {
                    VpnOwnedResourcesPresence.Clear -> VpnCleanupPlan.AlreadyClean(snapshot.generation)

                    VpnOwnedResourcesPresence.Retained -> lifecycle.requestCleanup(
                        VpnCleanupIntent.Stop,
                        resources = VpnOwnedResourcesPresence.Retained,
                    )
                }

                VpnProcessState.Revoked -> when (ownerResources.snapshot().presence()) {
                    VpnOwnedResourcesPresence.Clear -> VpnCleanupPlan.AlreadyClean(snapshot.generation)

                    VpnOwnedResourcesPresence.Retained -> lifecycle.requestCleanup(
                        VpnCleanupIntent.Revoke,
                        resources = VpnOwnedResourcesPresence.Retained,
                    )
                }

                VpnProcessState.Stopping -> lifecycle.retryPendingCleanup()

                is VpnProcessState.Failed -> when {
                    lifecycle.hasPendingCleanup() -> lifecycle.retryPendingCleanup()

                    ownerResources.snapshot().presence() == VpnOwnedResourcesPresence.Retained ->
                        lifecycle.requestCleanup(
                            VpnCleanupIntent.Stop,
                            resources = VpnOwnedResourcesPresence.Retained,
                        )

                    else -> VpnCleanupPlan.AlreadyClean(snapshot.generation)
                }

                VpnProcessState.StartRequested,
                VpnProcessState.Starting,
                VpnProcessState.Foreground,
                VpnProcessState.TunnelEstablished,
                VpnProcessState.Running,
                -> lifecycle.requestCleanup(
                    VpnCleanupIntent.Stop,
                    resources = ownerResources.snapshot().presence(),
                )
            }
            queueCleanup(cleanupPlan, stopId = null)
            vpnExecutor.shutdown()
            systemPolicyExecutor.shutdownNow()
        }
        serviceAdmission.finishDestroy()
        super.onDestroy()
    }

    private fun startForegroundAndQueue(intent: Intent, startId: Int): Int {
        val generation = intent.getLongExtra(EXTRA_GENERATION, INVALID_GENERATION)
        val snapshot = lifecycle.snapshot()
        if (generation != snapshot.generation) {
            Log.w(TAG, "Ignoring stale or unrequested VPN start")
            return START_NOT_STICKY
        }
        when (snapshot.state) {
            VpnProcessState.StartRequested -> Unit

            VpnProcessState.Starting,
            VpnProcessState.Foreground,
            VpnProcessState.TunnelEstablished,
            VpnProcessState.Running,
            VpnProcessState.Stopping,
            -> return START_NOT_STICKY

            VpnProcessState.Stopped,
            VpnProcessState.Revoked,
            is VpnProcessState.Failed,
            -> {
                Log.w(TAG, "Ignoring VPN start without a matching start request")
                stopSelfResult(startId)
                return START_NOT_STICKY
            }
        }
        val deadline = readStartDeadline(intent)
        if (deadline == null) {
            cleanupFailedStart(generation, startId, "VPN start request omitted its deadline")
            return START_NOT_STICKY
        }
        when (lifecycle.transition(generation, VpnProcessState.Starting)) {
            VpnTransitionOutcome.Applied -> Unit

            VpnTransitionOutcome.StaleGeneration,
            VpnTransitionOutcome.RunningRequiresDataPlaneReceipt,
            -> return START_NOT_STICKY
        }
        try {
            when (
                val promotion = promoteToForeground(
                    createNotification(
                        getString(R.string.notification_starting),
                        notificationActionMode,
                    ),
                )
            ) {
                VpnForegroundPromotion.Promoted -> Unit
                is VpnForegroundPromotion.Failed -> throw IOException(promotion.reason)
            }
        } catch (e: Exception) {
            val message = e.message ?: "Failed to enter foreground"
            Log.e(TAG, message, e)
            when (lifecycle.transition(generation, VpnProcessState.Failed(message))) {
                VpnTransitionOutcome.Applied -> stopSelfResult(startId)

                VpnTransitionOutcome.StaleGeneration,
                VpnTransitionOutcome.RunningRequiresDataPlaneReceipt,
                -> Unit
            }
            return START_NOT_STICKY
        }
        when (lifecycle.transition(generation, VpnProcessState.Foreground)) {
            VpnTransitionOutcome.Applied -> Unit

            VpnTransitionOutcome.StaleGeneration,
            VpnTransitionOutcome.RunningRequiresDataPlaneReceipt,
            -> return START_NOT_STICKY
        }
        if (deadline.remainingAt(SystemClock.elapsedRealtime()) == VpnDeadlineState.Expired) {
            cleanupFailedStart(generation, startId, "VPN start request expired before service admission")
            return START_NOT_STICKY
        }
        val consentIntent = try {
            VpnService.prepare(this)
        } catch (error: Exception) {
            val message = error.message ?: "VPN authorization state could not be read"
            cleanupFailedStart(generation, startId, message)
            return START_NOT_STICKY
        }
        if (consentIntent != null) {
            // Authorization changed after preflight; revoke shares the same
            // cleanup transaction and is published only after owner readback.
            val revokePlan = lifecycle.requestCleanup(
                VpnCleanupIntent.Revoke,
                VpnStopTarget.Generation(generation),
                ownerResources.snapshot().presence(),
            )
            queueCleanup(revokePlan, stopId = startId)
            VpnAuthorizationBroadcast.requestForExplicitStart(this)
            stopSelfResult(startId)
            return START_NOT_STICKY
        }
        submitStartTransaction(generation, startId, deadline)
        return START_NOT_STICKY
    }

    private fun submitStartTransaction(generation: Long, startId: Int, deadline: VpnOwnerRequestDeadline) {
        val admission = serviceAdmission.admitStart(generation)
        when (admission) {
            VpnServiceTaskAdmission.Accepted -> {
                val task = StartTransactionTask(generation, startId, deadline)
                queuedStartTasks[generation] = task
                try {
                    vpnExecutor.execute(task)
                } catch (_: RejectedExecutionException) {
                    queuedStartTasks.remove(generation)
                    serviceAdmission.rejectStart(generation)
                    when (
                        lifecycle.transition(
                            generation,
                            VpnProcessState.Failed("VPN lifecycle executor queue is full"),
                        )
                    ) {
                        VpnTransitionOutcome.Applied -> removeForegroundAndStop(startId, generation)

                        VpnTransitionOutcome.StaleGeneration,
                        VpnTransitionOutcome.RunningRequiresDataPlaneReceipt,
                        -> Unit
                    }
                }
            }

            VpnServiceTaskAdmission.Joined,
            VpnServiceTaskAdmission.StaleGeneration,
            VpnServiceTaskAdmission.Destroying,
            VpnServiceTaskAdmission.Destroyed,
            -> Unit

            VpnServiceTaskAdmission.QueueFull -> when (
                lifecycle.transition(generation, VpnProcessState.Failed("VPN lifecycle queue is saturated"))
            ) {
                VpnTransitionOutcome.Applied -> removeForegroundAndStop(startId, generation)

                VpnTransitionOutcome.StaleGeneration,
                VpnTransitionOutcome.RunningRequiresDataPlaneReceipt,
                -> Unit
            }

            VpnServiceTaskAdmission.ReplacedQueuedStart -> Unit
        }
    }

    private inner class StartTransactionTask(
        private val generation: Long,
        private val startId: Int,
        private val deadline: VpnOwnerRequestDeadline,
    ) : Runnable {
        override fun run() {
            if (serviceAdmission.beginStart(generation) != VpnServiceTaskAdmission.Accepted) return
            queuedStartTasks.remove(generation)
            try {
                lifecycleWakeLock.withLease(
                    generation = generation,
                    elapsedRealtimeNow = { SystemClock.elapsedRealtime() },
                    onAcquire = { outcome -> logLifecycleWakeLockAcquire(generation, outcome) },
                    onRelease = { outcome -> logLifecycleWakeLockRelease(generation, outcome) },
                ) {
                    runStartTransaction(generation, startId, deadline)
                }
            } finally {
                serviceAdmission.finishStart(generation)
                serviceAdmission.finishDestroy()
            }
        }
    }

    private fun runStartTransaction(generation: Long, startId: Int, deadline: VpnOwnerRequestDeadline) {
        if (!lifecycle.isCurrent(generation)) return
        if (deadline.remainingAt(SystemClock.elapsedRealtime()) == VpnDeadlineState.Expired) {
            cleanupFailedStart(generation, startId, "VPN start request expired in the lifecycle queue")
            return
        }
        try {
            when (ownerResources.routingLeaseState() ?: VpnRoutingLeaseState.Released) {
                VpnRoutingLeaseState.Released -> Unit

                is VpnRoutingLeaseState.Held,
                is VpnRoutingLeaseState.ReleasePending,
                -> throw IOException(
                    "A prior VPN routing activation lease still needs cleanup",
                )
            }
            val preparation = prepareVpn(generation.toULong())
            if (preparation.status.code != FfiErrorCode.OK) {
                throw IOException(preparation.status.message ?: "Rust VPN preparation failed")
            }
            val preparationId = preparation.preparationId
                ?: throw IOException("Rust VPN preparation returned no transaction ID")
            if (
                ownerResources.recordPreparedConfiguration(
                    generation,
                    VpnPreparedOwnership(preparationId),
                ) ==
                VpnOwnerResourceMutation.StaleGeneration
            ) {
                throw IOException("A newer VPN preparation is still owned by the service")
            }
            if (!lifecycle.isCurrent(generation)) return

            val prepared = when (
                val outcome = takePendingConfiguration(generation.toULong(), preparationId)
            ) {
                is VpnPreparedConfigurationTakeOutcome.Taken -> outcome.configuration

                VpnPreparedConfigurationTakeOutcome.Missing ->
                    throw IOException("Rust VPN configuration was not prepared")

                VpnPreparedConfigurationTakeOutcome.NotReady ->
                    throw IOException("Rust VPN configuration is not ready")

                VpnPreparedConfigurationTakeOutcome.TransactionMismatch ->
                    throw IOException("VPN Builder transaction ID did not match Rust preparation")

                VpnPreparedConfigurationTakeOutcome.StaleGeneration ->
                    throw IOException("VPN Builder configuration belongs to an old generation")

                VpnPreparedConfigurationTakeOutcome.AlreadyConsumed ->
                    throw IOException("VPN Builder configuration was already consumed")
            }
            val routing = when (val loaded = loadVpnRoutingConfiguration()) {
                is VpnRoutingConfiguration.Ready -> loaded
                is VpnRoutingConfiguration.Failed -> throw IOException(loaded.message)
            }
            val routingPlan = when (val plan = buildVpnPackageRoutingPlan(routing, packageName)) {
                is VpnPackageRoutingPlan.Ready -> plan
                is VpnPackageRoutingPlan.Failed -> throw IOException(plan.message)
            }
            if (!lifecycle.isCurrent(generation)) return

            val configurationAdmission = androidBeginVpnConfigurationActivation(
                preparationId,
                generation.toULong(),
                prepared.ownerDataRevision.value,
            ).toAdmission()
            withVpnConfigurationActivation(
                prepared = prepared,
                admission = configurationAdmission,
                release = { lease ->
                    when (val state = releaseConfigurationActivation(generation, lease)) {
                        VpnConfigurationActivationLeaseState.Released -> Unit

                        is VpnConfigurationActivationLeaseState.Held -> Log.e(
                            TAG,
                            "VPN configuration activation lease remained held after release",
                        )

                        is VpnConfigurationActivationLeaseState.ReleasePending -> Log.e(
                            TAG,
                            "VPN configuration activation cleanup remains pending: ${state.problem}",
                        )
                    }
                },
                execute = { configuration, configurationLease ->
                    if (!lifecycle.isCurrent(generation)) return

                    val builder = Builder().setMtu(configuration.mtu)
                    builder.addAddress(IPV4_TUN_ADDRESS, 30)
                    if (configuration.ipv6) builder.addAddress(IPV6_TUN_ADDRESS, 126)
                    for (route in configuration.routes) {
                        if (configuration.ipv6 || !route.address.contains(":")) {
                            applyVpnRoute(builder, route)
                        }
                    }
                    for (server in configuration.dnsServers) builder.addDnsServer(server)
                    builder.setSession(getString(R.string.app_name))
                    applyVpnPackageRouting(builder, routingPlan)

                    val activation = androidBeginVpnRoutingActivation(routing.routingRevision)
                    val routingLease = when (activation) {
                        is AndroidVpnRoutingActivationResult.Acquired -> activation.lease

                        is AndroidVpnRoutingActivationResult.Stale -> throw IOException(
                            "VPN app-routing changed while configuring the tunnel " +
                                "(expected ${routing.routingRevision}, observed ${activation.observedRoutingRevision})",
                        )

                        AndroidVpnRoutingActivationResult.Busy -> throw IOException(
                            "Another VPN routing activation or update is in progress",
                        )

                        is AndroidVpnRoutingActivationResult.Failed -> throw IOException(
                            activation.status.message ?: "VPN routing activation could not be fenced",
                        )
                    }
                    if (
                        ownerResources.updateRoutingLease(
                            generation,
                            VpnRoutingLeaseState.Held(routingLease),
                        ) == VpnOwnerResourceMutation.StaleGeneration
                    ) {
                        throw IOException("A newer VPN routing activation lease is still owned by the service")
                    }

                    try {
                        if (routingLease.routingRevision != routing.routingRevision) {
                            throw IOException("VPN routing activation returned a mismatched revision")
                        }
                        if (!lifecycle.isCurrent(generation)) return
                        val established = builder.establish()
                            ?: throw IOException("Failed to establish VPN interface")
                        if (
                            ownerResources.recordTunnelDescriptor(generation, established) ==
                            VpnOwnerResourceMutation.StaleGeneration
                        ) {
                            closeInterface(established)
                            throw IOException("A newer VPN tunnel descriptor is still owned by the service")
                        }
                        when (lifecycle.transition(generation, VpnProcessState.TunnelEstablished)) {
                            VpnTransitionOutcome.Applied -> Unit

                            VpnTransitionOutcome.StaleGeneration -> {
                                cleanupStaleStart(generation)
                                return
                            }

                            VpnTransitionOutcome.RunningRequiresDataPlaneReceipt -> {
                                throw IOException("VPN Running requires a successful Rust data-plane receipt")
                            }
                        }
                        if (!lifecycle.isCurrent(generation)) {
                            cleanupStaleStart(generation)
                            return
                        }
                        // Rust borrows this fd and owns a duplicate; Kotlin keeps and closes this PFD.
                        if (
                            ownerResources.recordWorker(
                                generation,
                                VpnWorkerOwnership(
                                    generation = generation,
                                    mtu = configuration.mtu,
                                    routeCount = configuration.routes.size,
                                    dnsServers = configuration.dnsServers,
                                    ipv6 = configuration.ipv6,
                                ),
                            ) == VpnOwnerResourceMutation.StaleGeneration
                        ) {
                            throw IOException("A newer VPN worker is still owned by the service")
                        }
                        val status = startVpn(
                            established.fd,
                            generation.toULong(),
                            preparationId,
                            configurationLease.leaseId,
                        )
                        when (val receipt = status.toVpnDataPlaneStartReceipt()) {
                            VpnDataPlaneStartReceipt.Started -> {
                                when (
                                    val publication = lifecycle.completeDataPlaneStart(
                                        generation,
                                        receipt,
                                        ownerResources.routingLeaseState() ?: VpnRoutingLeaseState.Released,
                                    )
                                ) {
                                    VpnDataPlaneStartPublication.Published -> Unit

                                    VpnDataPlaneStartPublication.StaleGeneration -> {
                                        cleanupStaleStart(generation)
                                        return
                                    }

                                    VpnDataPlaneStartPublication.RoutingLeaseNotHeld -> {
                                        Log.e(TAG, "Rust start receipt arrived without a held routing activation lease")
                                        cleanupStaleStart(generation)
                                        return
                                    }

                                    is VpnDataPlaneStartPublication.UnexpectedState -> {
                                        Log.e(
                                            TAG,
                                            "Rust start receipt arrived outside TunnelEstablished: ${publication.state}",
                                        )
                                        cleanupStaleStart(generation)
                                        return
                                    }
                                }
                            }

                            is VpnDataPlaneStartReceipt.Failed -> {
                                when (
                                    val publication = lifecycle.completeDataPlaneStart(
                                        generation,
                                        receipt,
                                        ownerResources.routingLeaseState() ?: VpnRoutingLeaseState.Released,
                                    )
                                ) {
                                    VpnDataPlaneStartPublication.Published -> throw IOException(receipt.message)

                                    VpnDataPlaneStartPublication.StaleGeneration -> {
                                        cleanupStaleStart(generation)
                                        return
                                    }

                                    VpnDataPlaneStartPublication.RoutingLeaseNotHeld -> {
                                        Log.e(TAG, "Rust start failure arrived without a held routing activation lease")
                                        cleanupStaleStart(generation)
                                        return
                                    }

                                    is VpnDataPlaneStartPublication.UnexpectedState -> {
                                        Log.e(
                                            TAG,
                                            "Rust failure receipt arrived outside TunnelEstablished: ${publication.state}",
                                        )
                                        cleanupStaleStart(generation)
                                        return
                                    }
                                }
                            }
                        }
                    } finally {
                        val releaseFailure = releaseRoutingLease(generation, routingLease)
                        if (releaseFailure != null) {
                            Log.e(TAG, "VPN is active or failed with routing lease cleanup pending: $releaseFailure")
                        }
                    }
                },
            )
            try {
                getSystemService(NotificationManager::class.java).notify(
                    NOTIFICATION_ID,
                    createNotification(getString(R.string.notification_active), notificationActionMode),
                )
            } catch (error: Exception) {
                Log.w(TAG, "VPN is active but its notification update failed: ${error.message}", error)
            }
        } catch (error: Exception) {
            val message = error.message ?: "VPN startup failed"
            Log.e(TAG, message, error)
            cleanupFailedStart(generation, startId, message)
        } finally {
            val preparedFailure = clearPreparedVpnConfiguration(generation)
            if (preparedFailure != null) {
                Log.e(TAG, "VPN prepared transaction remains owned: ${preparedFailure.message}")
            }
        }
    }

    private fun queueStop(intent: Intent, startId: Int): Int {
        val intentGeneration = intent.getLongExtra(EXTRA_GENERATION, INVALID_GENERATION)
        val target = intentGeneration.takeIf { it != INVALID_GENERATION }
            ?.let(VpnStopTarget::Generation)
            ?: VpnStopTarget.Current
        val plan = lifecycle.requestCleanup(
            VpnCleanupIntent.Stop,
            target,
            ownerResources.snapshot().presence(),
        )
        return when (plan) {
            is VpnCleanupPlan.Dispatch,
            is VpnCleanupPlan.Retry,
            -> {
                queueCleanup(plan, stopId = startId)
                START_NOT_STICKY
            }

            is VpnCleanupPlan.Join -> {
                START_NOT_STICKY
            }

            is VpnCleanupPlan.AlreadyClean -> {
                stopSelfResult(startId)
                START_NOT_STICKY
            }

            VpnCleanupPlan.Superseded -> START_NOT_STICKY
        }
    }

    internal fun queueCleanup(plan: VpnCleanupPlan, stopId: Int?) {
        val request = when (plan) {
            is VpnCleanupPlan.Dispatch -> plan.request

            is VpnCleanupPlan.Retry -> plan.request

            is VpnCleanupPlan.Join,
            is VpnCleanupPlan.AlreadyClean,
            VpnCleanupPlan.Superseded,
            -> return
        }
        submitCleanupRequest(request, stopId)
    }

    private fun submitCleanupRequest(request: VpnCleanupRequest, stopId: Int?) {
        val queuedStart = serviceAdmission.pendingTask() as? VpnServiceTaskSlot.Start
        val admission = serviceAdmission.admitCleanup(request)
        when (admission) {
            VpnServiceTaskAdmission.Joined -> return

            VpnServiceTaskAdmission.Accepted,
            VpnServiceTaskAdmission.ReplacedQueuedStart,
            -> Unit

            VpnServiceTaskAdmission.QueueFull,
            VpnServiceTaskAdmission.Destroyed,
            VpnServiceTaskAdmission.Destroying,
            VpnServiceTaskAdmission.StaleGeneration,
            -> {
                rejectCleanupAdmission(request, "VPN cleanup could not enter the bounded lifecycle queue")
                return
            }
        }
        if (admission == VpnServiceTaskAdmission.ReplacedQueuedStart && queuedStart != null) {
            queuedStartTasks.remove(queuedStart.generation)?.let(vpnExecutor.queue::remove)
        }
        val task = CleanupTransactionTask(request, stopId)
        try {
            vpnExecutor.execute(task)
        } catch (_: RejectedExecutionException) {
            serviceAdmission.rejectCleanup(request)
            rejectCleanupAdmission(request, "VPN lifecycle executor rejected its bounded cleanup slot")
        }
    }

    private inner class CleanupTransactionTask(private val request: VpnCleanupRequest, private val stopId: Int?) : Runnable {
        override fun run() {
            if (serviceAdmission.beginCleanup(request) != VpnServiceTaskAdmission.Accepted) return
            try {
                lifecycleWakeLock.withLease(
                    generation = request.generation,
                    elapsedRealtimeNow = { SystemClock.elapsedRealtime() },
                    onAcquire = { outcome -> logLifecycleWakeLockAcquire(request.generation, outcome) },
                    onRelease = { outcome -> logLifecycleWakeLockRelease(request.generation, outcome) },
                ) {
                    finishCleanup(request, stopId)
                }
            } finally {
                serviceAdmission.finishCleanup(request)
                serviceAdmission.finishDestroy()
            }
        }
    }

    private fun rejectCleanupAdmission(request: VpnCleanupRequest, message: String) {
        ownerResources.markCleanupPending(
            request.resourceGeneration,
            VpnCleanupFailure(VpnCleanupResourceKind.ExecutorQueue, message),
        )
        when (val publication = lifecycle.cleanupQueueRejected(request, message)) {
            is VpnCleanupPublication.CleanupPending ->
                Log.e(TAG, "VPN cleanup remains pending: ${publication.failure.message}")

            is VpnCleanupPublication.Published,
            VpnCleanupPublication.StaleGeneration,
            -> Unit
        }
    }

    private fun finishCleanup(request: VpnCleanupRequest, stopId: Int?) {
        var firstFailure = clearPreparedVpnConfiguration(request.resourceGeneration)
        val workerFailure = stopRustWorker(request.resourceGeneration)
        if (firstFailure == null) firstFailure = workerFailure

        when (workerFailure) {
            null -> {
                val descriptorFailure = closeOwnedInterface(request.resourceGeneration)
                if (firstFailure == null) firstFailure = descriptorFailure
                when (descriptorFailure) {
                    null -> {
                        val leaseFailure = retryRoutingLeaseRelease(request.resourceGeneration)
                        if (firstFailure == null) firstFailure = leaseFailure
                    }

                    else -> Unit
                }
            }

            else -> Unit
        }

        val cleanupOutcome = when (val failure = firstFailure) {
            null -> VpnCleanupOutcome.Completed
            else -> VpnCleanupOutcome.CleanupPending(failure)
        }
        val ownerSnapshot = ownerResources.snapshot()
        val lifecycleSnapshot = lifecycle.snapshot()
        val ownerReadback = VpnCleanupOwnerReadback(
            generation = lifecycleSnapshot.generation,
            state = lifecycleSnapshot.state,
            resources = ownerSnapshot.presence(),
        )
        when (val publication = lifecycle.completeCleanup(request, cleanupOutcome, ownerReadback)) {
            is VpnCleanupPublication.Published -> {
                Log.i(TAG, "VPN cleanup reached owner terminal state: ${publication.state}")
                removeForegroundAndStop(stopId, request.generation)
            }

            is VpnCleanupPublication.CleanupPending ->
                Log.e(TAG, "VPN cleanup remains pending: ${publication.failure.message}")

            VpnCleanupPublication.StaleGeneration ->
                Log.w(TAG, "Ignoring stale VPN cleanup completion for generation ${request.generation}")
        }
    }

    override fun onRevoke() {
        queueCleanup(
            lifecycle.requestCleanup(
                VpnCleanupIntent.Revoke,
                resources = ownerResources.snapshot().presence(),
            ),
            stopId = null,
        )
        super.onRevoke()
    }

    companion object {
        internal const val TAG = "MihomoVpnService"
        internal const val CHANNEL_ID = "vpn_service_channel"
        internal const val NOTIFICATION_ID = 1
        private const val IPV4_TUN_ADDRESS = "172.19.0.1"
        private const val IPV6_TUN_ADDRESS = "fd00:fd00:fd00::1"
        internal const val EXTRA_COMMAND = "vpn_service_command"
        internal const val EXTRA_GENERATION = "vpn_generation"
        internal const val EXTRA_DEADLINE_ELAPSED_REALTIME = "vpn_deadline_elapsed_realtime"
        internal const val INVALID_GENERATION = Long.MIN_VALUE
        internal val lifecycle = VpnLifecycleCoordinator()
        internal val mainHandler = Handler(Looper.getMainLooper())
        internal val sharedOwnerResources = VpnOwnerResourceRecord<
            ParcelFileDescriptor,
            VpnWorkerOwnership,
            VpnRoutingLeaseState,
            VpnPreparedOwnership,
            >()

        internal val preparedConfigurationStore = VpnPreparedConfigurationStore()
        internal const val OWNER_REQUEST_TIMEOUT_MILLIS = 15_000L
        internal const val SYSTEM_POLICY_START_TIMEOUT_MILLIS = 30_000L
        internal const val OWNER_SESSION_POLL_MILLIS = 50L

        private fun readStartDeadline(intent: Intent): VpnOwnerRequestDeadline? {
            if (!intent.hasExtra(EXTRA_DEADLINE_ELAPSED_REALTIME)) return null
            val deadline = intent.getLongExtra(EXTRA_DEADLINE_ELAPSED_REALTIME, INVALID_GENERATION)
            if (deadline <= 0L) return null
            return VpnOwnerRequestDeadline(deadline)
        }

        internal fun lifecycleSnapshot(): VpnLifecycleSnapshot = lifecycle.snapshot()

        internal fun ownerResourceFacts(): List<VpnOwnerResourceFactWire> {
            val resources = sharedOwnerResources.snapshot()
            return listOf(
                VpnOwnerResourceFactWire(
                    VpnOwnerResourceKindWire.PreparedConfiguration,
                    resources.preparedConfiguration.toOwnerWireState(),
                ),
                VpnOwnerResourceFactWire(
                    VpnOwnerResourceKindWire.Worker,
                    resources.worker.toOwnerWireState(),
                ),
                VpnOwnerResourceFactWire(
                    VpnOwnerResourceKindWire.TunnelDescriptor,
                    resources.tunnelDescriptor.toOwnerWireState(),
                ),
                VpnOwnerResourceFactWire(
                    VpnOwnerResourceKindWire.RoutingLease,
                    resources.routingLease.toRoutingLeaseOwnerWireState(),
                ),
            )
        }

        internal fun ownerConfiguration(): VpnOwnerConfigurationWire? {
            val resources = sharedOwnerResources.snapshot()
            val worker = when (val owned = resources.worker) {
                VpnOwnedResource.Absent -> null
                is VpnOwnedResource.Held -> owned.value
                is VpnOwnedResource.CleanupPending -> owned.value
            } ?: return null
            return VpnOwnerConfigurationWire(
                mtu = worker.mtu,
                routeCount = worker.routeCount,
                dnsServers = worker.dnsServers,
                ipv6 = worker.ipv6,
            )
        }
    }
}

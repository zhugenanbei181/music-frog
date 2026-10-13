package com.musicfrog.infiltrator

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.net.VpnService
import android.os.Handler
import android.os.Looper
import android.os.ParcelFileDescriptor
import android.os.PowerManager
import android.os.SystemClock
import android.util.Log
import androidx.core.app.NotificationCompat
import infiltrator_android.AndroidControlIntent
import infiltrator_android.FfiErrorCode
import infiltrator_android.AndroidVpnConfigurationActivationLease
import infiltrator_android.AndroidVpnRoutingActivationLease
import infiltrator_android.AndroidVpnRoutingActivationReleaseResult
import infiltrator_android.AndroidVpnRoutingActivationResult
import infiltrator_android.androidBeginVpnRoutingActivation
import infiltrator_android.androidBeginVpnConfigurationActivation
import infiltrator_android.androidControl
import infiltrator_android.androidFinishVpnConfigurationActivation
import infiltrator_android.androidFinishVpnRoutingActivation
import infiltrator_android.cancelVpnPreparation
import infiltrator_android.prepareVpn
import infiltrator_android.startVpn
import infiltrator_android.stopVpnWorker
import java.io.IOException
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.ThreadPoolExecutor
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.runBlocking

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
    private val ownerResources: MihomoVpnOwnerResources
        get() = sharedOwnerResources
    private lateinit var vpnExecutor: ThreadPoolExecutor
    private lateinit var systemPolicyExecutor: ThreadPoolExecutor
    private val serviceAdmission = VpnServiceExecutorAdmission()
    private val queuedStartTasks = ConcurrentHashMap<Long, StartTransactionTask>()
    private var notificationActionMode = VpnServiceNotificationActionMode.UserManaged
    private var lifecycleWakeLock = VpnLifecycleWakeLock(
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
            VpnCleanupPlan.Superseded -> Unit
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

    private fun readServiceCommand(intent: Intent?): VpnServiceCommandExtra {
        if (intent == null || !intent.hasExtra(EXTRA_COMMAND)) return VpnServiceCommandExtra.Missing
        val command = try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                intent.getSerializableExtra(EXTRA_COMMAND, VpnServiceCommand::class.java)
            } else {
                @Suppress("DEPRECATION")
                intent.getSerializableExtra(EXTRA_COMMAND) as? VpnServiceCommand
            }
        } catch (_: Exception) {
            return VpnServiceCommandExtra.Malformed
        }
        return when (command) {
            VpnServiceCommand.Start -> VpnServiceCommandExtra.Present(VpnServiceCommand.Start)
            VpnServiceCommand.Stop -> VpnServiceCommandExtra.Present(VpnServiceCommand.Stop)
            null -> VpnServiceCommandExtra.Malformed
        }
    }

    private fun readServiceDelivery(flags: Int): VpnServiceDelivery = when {
        flags and android.app.Service.START_FLAG_REDELIVERY != 0 -> VpnServiceDelivery.Redelivered
        flags and android.app.Service.START_FLAG_RETRY != 0 -> VpnServiceDelivery.Retried
        else -> VpnServiceDelivery.Initial
    }

    private fun readAlwaysOnObservation(): VpnAlwaysOnObservation = when {
        Build.VERSION.SDK_INT < Build.VERSION_CODES.Q -> VpnAlwaysOnObservation.NotQueryable
        isAlwaysOn -> VpnAlwaysOnObservation.Enabled
        else -> VpnAlwaysOnObservation.Disabled
    }

    private fun startSystemManagedProxy(
        startId: Int,
        decision: VpnServiceStartupDecision.RestoreSystemManagedProxy,
    ): Int {
        notificationActionMode = VpnServiceNotificationActionMode.SystemManaged
        when (val promotion = promoteToForeground(getString(R.string.notification_starting))) {
            VpnForegroundPromotion.Promoted -> Unit
            is VpnForegroundPromotion.Failed -> {
                Log.e(TAG, "Always-on VPN service could not enter foreground: ${promotion.reason}")
                stopSelfResult(startId)
                return START_NOT_STICKY
            }
        }
        try {
            startService(Intent(this, MihomoControlService::class.java))
        } catch (error: Exception) {
            showSystemManagedStartupFailure(
                startId,
                "Core owner service could not be initialized: ${error.message ?: error.javaClass.simpleName}",
            )
            return START_STICKY
        }
        try {
            systemPolicyExecutor.execute {
                dispatchSystemManagedProxyStart(startId, decision)
            }
        } catch (_: RejectedExecutionException) {
            showSystemManagedStartupFailure(startId, "VPN system-start queue is full")
        }
        return START_STICKY
    }

    private fun dispatchSystemManagedProxyStart(
        startId: Int,
        decision: VpnServiceStartupDecision.RestoreSystemManagedProxy,
    ) {
        val deadline = VpnOwnerRequestDeadline.after(
            SystemClock.elapsedRealtime(),
            SYSTEM_POLICY_START_TIMEOUT_MILLIS,
        )
        val application = application as? MihomoApplication
        if (application == null) {
            showSystemManagedStartupFailure(startId, "Core owner application is unavailable")
            return
        }
        when (val readiness = awaitCoreOwnerSession(application, deadline)) {
            is VpnOwnerSessionWaitOutcome.Ready -> Unit
            is VpnOwnerSessionWaitOutcome.Failed -> {
                showSystemManagedStartupFailure(startId, readiness.reason)
                return
            }
            VpnOwnerSessionWaitOutcome.TimedOut -> {
                showSystemManagedStartupFailure(startId, "Core owner initialization timed out")
                return
            }
            VpnOwnerSessionWaitOutcome.Interrupted -> {
                showSystemManagedStartupFailure(startId, "Core owner initialization was interrupted")
                return
            }
        }
        if (serviceAdmission.phase() != VpnServiceAdmissionPhase.Accepting) return
        val status = try {
            runBlocking { androidControl(AndroidControlIntent.StartProxy) }
        } catch (error: Exception) {
            showSystemManagedStartupFailure(
                startId,
                "Always-on proxy start failed: ${error.message ?: error.javaClass.simpleName}",
            )
            return
        }
        when (status.code) {
            FfiErrorCode.OK -> showSystemManagedStartupStatus(getString(R.string.notification_active))
            else -> showSystemManagedStartupFailure(
                startId,
                status.message ?: "Always-on proxy start failed",
            )
        }
        Log.i(TAG, "System-managed VPN startup completed with ${decision.policy}/${decision.delivery}")
    }

    private fun awaitCoreOwnerSession(
        application: MihomoApplication,
        deadline: VpnOwnerRequestDeadline,
    ): VpnOwnerSessionWaitOutcome {
        while (true) {
            val budget = when (
                val remaining = deadline.remainingAt(SystemClock.elapsedRealtime())
            ) {
                VpnDeadlineState.Expired -> VpnOwnerSessionWaitBudget.Expired
                is VpnDeadlineState.Remaining -> VpnOwnerSessionWaitBudget.Remaining(remaining.millis)
            }
            when (
                val directive = decideVpnOwnerSessionWait(
                    readiness = application.vpnOwnerSessionReadiness(),
                    budget = budget,
                    pollIntervalMillis = OWNER_SESSION_POLL_MILLIS,
                )
            ) {
                is VpnOwnerSessionWaitDirective.Complete -> return directive.outcome
                is VpnOwnerSessionWaitDirective.Wait -> try {
                    Thread.sleep(directive.millis)
                } catch (_: InterruptedException) {
                    Thread.currentThread().interrupt()
                    return VpnOwnerSessionWaitOutcome.Interrupted
                }
            }
        }
    }

    private fun showSystemManagedStartupFailure(startId: Int, reason: String) {
        Log.e(TAG, reason)
        when (serviceAdmission.phase()) {
            VpnServiceAdmissionPhase.Accepting -> Unit
            VpnServiceAdmissionPhase.Destroying,
            VpnServiceAdmissionPhase.Destroyed -> return
        }
        val snapshot = lifecycle.snapshot()
        when (snapshot.state) {
            VpnProcessState.Running -> Unit
            VpnProcessState.StartRequested,
            VpnProcessState.Starting,
            VpnProcessState.Foreground,
            VpnProcessState.TunnelEstablished -> cleanupFailedStart(snapshot.generation, startId, reason)
            VpnProcessState.Stopping -> Unit
            VpnProcessState.Stopped,
            VpnProcessState.Revoked,
            is VpnProcessState.Failed -> lifecycle.failUnrequestedRestart(reason)
        }
        showSystemManagedStartupStatus("${getString(R.string.notification_start_failed)}: $reason")
    }

    private fun showSystemManagedStartupStatus(status: String) {
        mainHandler.post {
            if (serviceAdmission.phase() == VpnServiceAdmissionPhase.Destroyed) return@post
            val notification = createNotification(
                status,
                VpnServiceNotificationActionMode.SystemManaged,
            )
            val manager = getSystemService(NotificationManager::class.java)
            manager.notify(NOTIFICATION_ID, notification)
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
                VpnProcessState.Running -> lifecycle.requestCleanup(
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
            VpnProcessState.Stopping -> return START_NOT_STICKY
            VpnProcessState.Stopped,
            VpnProcessState.Revoked,
            is VpnProcessState.Failed -> {
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
            VpnTransitionOutcome.RunningRequiresDataPlaneReceipt -> return START_NOT_STICKY
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
                VpnTransitionOutcome.RunningRequiresDataPlaneReceipt -> Unit
            }
            return START_NOT_STICKY
        }
        when (lifecycle.transition(generation, VpnProcessState.Foreground)) {
            VpnTransitionOutcome.Applied -> Unit
            VpnTransitionOutcome.StaleGeneration,
            VpnTransitionOutcome.RunningRequiresDataPlaneReceipt -> return START_NOT_STICKY
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

    private fun createLifecycleWakeLock(): VpnLifecycleWakeLock {
        val handle = try {
            getSystemService(PowerManager::class.java)
                ?.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "$packageName:VpnLifecycle")
                ?.apply { setReferenceCounted(false) }
                ?.let(::AndroidVpnWakeLockHandle)
                ?: UnavailableVpnWakeLockHandle
        } catch (error: RuntimeException) {
            Log.w(TAG, "Android partial WakeLock is unavailable: ${error.message}")
            UnavailableVpnWakeLockHandle
        }
        return VpnLifecycleWakeLock(handle, VpnLifecycleWakeLock.MAX_LEASE_DURATION_MILLIS)
    }

    private fun logLifecycleWakeLockAcquire(generation: Long, outcome: VpnWakeLockAcquire) {
        when (outcome) {
            is VpnWakeLockAcquire.Acquired,
            is VpnWakeLockAcquire.Joined -> Unit
            is VpnWakeLockAcquire.Busy -> Log.w(
                TAG,
                "VPN generation $generation shares a bounded WakeLock held by generation ${outcome.heldGeneration}",
            )
            is VpnWakeLockAcquire.Unavailable -> Log.w(
                TAG,
                "VPN lifecycle continues without a WakeLock: ${outcome.reason}",
            )
            is VpnWakeLockAcquire.CleanupPending -> Log.e(
                TAG,
                "VPN WakeLock release remains pending; the platform timeout is the fallback: ${outcome.reason}",
            )
        }
    }

    private fun logLifecycleWakeLockRelease(generation: Long, outcome: VpnWakeLockRelease) {
        when (outcome) {
            VpnWakeLockRelease.Released,
            VpnWakeLockRelease.AlreadyReleased,
            VpnWakeLockRelease.Expired -> Unit
            VpnWakeLockRelease.StaleGeneration -> Log.w(
                TAG,
                "Ignoring stale WakeLock release for VPN generation $generation",
            )
            is VpnWakeLockRelease.CleanupPending -> Log.e(
                TAG,
                "VPN WakeLock release remains pending until its bounded timeout: ${outcome.reason}",
            )
        }
    }

    private fun submitStartTransaction(
        generation: Long,
        startId: Int,
        deadline: VpnOwnerRequestDeadline,
    ) {
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
                        VpnTransitionOutcome.RunningRequiresDataPlaneReceipt -> Unit
                    }
                }
            }
            VpnServiceTaskAdmission.Joined,
            VpnServiceTaskAdmission.StaleGeneration,
            VpnServiceTaskAdmission.Destroying,
            VpnServiceTaskAdmission.Destroyed -> Unit
            VpnServiceTaskAdmission.QueueFull -> when (
                lifecycle.transition(generation, VpnProcessState.Failed("VPN lifecycle queue is saturated"))
            ) {
                VpnTransitionOutcome.Applied -> removeForegroundAndStop(startId, generation)
                VpnTransitionOutcome.StaleGeneration,
                VpnTransitionOutcome.RunningRequiresDataPlaneReceipt -> Unit
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

    private fun runStartTransaction(
        generation: Long,
        startId: Int,
        deadline: VpnOwnerRequestDeadline,
    ) {
        if (!lifecycle.isCurrent(generation)) return
        if (deadline.remainingAt(SystemClock.elapsedRealtime()) == VpnDeadlineState.Expired) {
            cleanupFailedStart(generation, startId, "VPN start request expired in the lifecycle queue")
            return
        }
        try {
            when (ownerResources.routingLeaseState() ?: VpnRoutingLeaseState.Released) {
                VpnRoutingLeaseState.Released -> Unit
                is VpnRoutingLeaseState.Held,
                is VpnRoutingLeaseState.ReleasePending -> throw IOException(
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
                                        ownerResources.routingLeaseState(),
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
                                        ownerResources.routingLeaseState(),
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

    private fun cleanupStaleStart(generation: Long) {
        val snapshot = lifecycle.snapshot()
        val intent = when (val state = snapshot.state) {
            is VpnProcessState.Failed -> when {
                snapshot.generation == generation -> VpnCleanupIntent.StartFailure(state.message)
                else -> VpnCleanupIntent.Stop
            }
            else -> VpnCleanupIntent.Stop
        }
        val plan = lifecycle.requestCleanup(
            intent,
            VpnStopTarget.Generation(generation),
            ownerResources.snapshot().presence(),
        )
        queueCleanup(plan, stopId = null)
    }

    private fun cleanupFailedStart(generation: Long, startId: Int, message: String) {
        when (
            lifecycle.transition(generation, VpnProcessState.Failed(message))
        ) {
            VpnTransitionOutcome.Applied ->
                queueCleanup(
                    lifecycle.requestCleanup(
                        VpnCleanupIntent.StartFailure(message),
                        VpnStopTarget.Generation(generation),
                        ownerResources.snapshot().presence(),
                    ),
                    stopId = startId,
                )
            VpnTransitionOutcome.StaleGeneration,
            VpnTransitionOutcome.RunningRequiresDataPlaneReceipt -> Unit
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
            is VpnCleanupPlan.Retry -> {
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

    private fun queueCleanup(plan: VpnCleanupPlan, stopId: Int?) {
        val request = when (plan) {
            is VpnCleanupPlan.Dispatch,
            is VpnCleanupPlan.Retry -> plan.request
            is VpnCleanupPlan.Join,
            is VpnCleanupPlan.AlreadyClean,
            VpnCleanupPlan.Superseded -> return
        }
        submitCleanupRequest(request, stopId)
    }

    private fun submitCleanupRequest(request: VpnCleanupRequest, stopId: Int?) {
        val queuedStart = serviceAdmission.pendingTask() as? VpnServiceTaskSlot.Start
        val admission = serviceAdmission.admitCleanup(request)
        when (admission) {
            VpnServiceTaskAdmission.Joined -> return
            VpnServiceTaskAdmission.Accepted,
            VpnServiceTaskAdmission.ReplacedQueuedStart -> Unit
            VpnServiceTaskAdmission.QueueFull,
            VpnServiceTaskAdmission.Destroyed,
            VpnServiceTaskAdmission.Destroying,
            VpnServiceTaskAdmission.StaleGeneration -> {
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

    private inner class CleanupTransactionTask(
        private val request: VpnCleanupRequest,
        private val stopId: Int?,
    ) : Runnable {
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
            VpnCleanupPublication.StaleGeneration -> Unit
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

    private fun stopRustWorker(generation: Long): VpnCleanupFailure? {
        if (!ownerResources.hasWorker(generation)) return null
        val failure = try {
            val status = stopVpnWorker()
            if (status.code == FfiErrorCode.OK) null else status.message ?: "Rust VPN worker stop failed"
        } catch (error: Exception) {
            error.message ?: "Rust VPN worker stop failed"
        }
        return when (failure) {
            null -> when (ownerResources.clearWorker(generation)) {
                VpnOwnerResourceMutation.Cleared,
                VpnOwnerResourceMutation.AlreadyAbsent -> null
                VpnOwnerResourceMutation.Recorded,
                VpnOwnerResourceMutation.StaleGeneration -> VpnCleanupFailure(
                    VpnCleanupResourceKind.Worker,
                    "Rust VPN worker ownership changed during cleanup",
                )
            }
            else -> {
                val cleanupFailure = VpnCleanupFailure(VpnCleanupResourceKind.Worker, failure)
                ownerResources.markWorkerPending(generation, cleanupFailure)
                cleanupFailure
            }
        }
    }

    private fun clearPreparedVpnConfiguration(generation: Long): VpnCleanupFailure? {
        if (generation <= 0L) return null
        clearPendingConfigurationThrough(generation)
        var failure: String? = null
        try {
            val status = cancelVpnPreparation(generation.toULong())
            if (status.code != FfiErrorCode.OK) {
                failure = status.message ?: "Could not clear prepared VPN transaction"
            }
        } catch (error: Exception) {
            failure = error.message ?: "Could not clear prepared VPN transaction"
        }
        val cleanupFailure = failure?.let {
            VpnCleanupFailure(VpnCleanupResourceKind.PreparedConfiguration, it)
        }
        when (cleanupFailure) {
            null -> ownerResources.clearPreparedConfiguration(generation)
            else -> {
                if (ownerResources.snapshot().preparedConfiguration is VpnOwnedResource.Absent) {
                    ownerResources.recordPreparedConfiguration(
                        generation,
                        VpnPreparedOwnership(0uL),
                    )
                }
                ownerResources.markPreparedConfigurationPending(generation, cleanupFailure)
            }
        }
        return cleanupFailure
    }

    private fun releaseRoutingLease(
        generation: Long,
        lease: AndroidVpnRoutingActivationLease,
    ): String? {
        val current = ownerResources.routingLeaseState(generation)
            ?: return "VPN routing lease generation changed during release"
        return try {
            val result = androidFinishVpnRoutingActivation(lease.leaseId)
            val updated = current.afterReleaseAttempt(lease, result)
            val mutation = recordRoutingLeaseState(generation, updated)
            when (mutation) {
                VpnOwnerResourceMutation.StaleGeneration -> "VPN routing lease generation changed during release"
                VpnOwnerResourceMutation.Recorded,
                VpnOwnerResourceMutation.Cleared,
                VpnOwnerResourceMutation.AlreadyAbsent -> updated.releaseFailureMessage()
            }
        } catch (error: Exception) {
            val message = error.message ?: "VPN routing activation release failed"
            val updated = current.afterReleaseFailure(lease, message)
            val mutation = recordRoutingLeaseState(generation, updated)
            when (mutation) {
                VpnOwnerResourceMutation.StaleGeneration -> "VPN routing lease generation changed during release"
                VpnOwnerResourceMutation.Recorded,
                VpnOwnerResourceMutation.Cleared,
                VpnOwnerResourceMutation.AlreadyAbsent -> message
            }
        }
    }

    private fun releaseConfigurationActivation(
        generation: Long,
        lease: AndroidVpnConfigurationActivationLease,
    ): VpnConfigurationActivationLeaseState {
        val initial = VpnConfigurationActivationLeaseState.Held(lease)
        val afterFinish = try {
            initial.afterReleaseAttempt(
                lease,
                androidFinishVpnConfigurationActivation(lease.leaseId),
            )
        } catch (error: Exception) {
            initial.afterReleaseFailure(
                lease,
                error.message ?: "VPN configuration activation release failed",
            )
        }
        return when (afterFinish) {
            VpnConfigurationActivationLeaseState.Released -> afterFinish
            is VpnConfigurationActivationLeaseState.Held -> afterFinish
            is VpnConfigurationActivationLeaseState.ReleasePending -> {
                val cleanup = try {
                    val status = cancelVpnPreparation(generation.toULong())
                    when (status.code) {
                        FfiErrorCode.OK -> VpnConfigurationActivationCleanupResult.Cleared
                        else -> VpnConfigurationActivationCleanupResult.Failed(
                            status.message ?: "VPN activation cleanup did not clear owner lease",
                        )
                    }
                } catch (error: Exception) {
                    VpnConfigurationActivationCleanupResult.Failed(
                        error.message ?: "VPN activation cleanup failed",
                    )
                }
                afterFinish.afterCancellation(cleanup)
            }
        }
    }

    private fun recordRoutingLeaseState(
        generation: Long,
        state: VpnRoutingLeaseState,
    ): VpnOwnerResourceMutation = when (state) {
        VpnRoutingLeaseState.Released -> ownerResources.clearRoutingLease(generation)
        is VpnRoutingLeaseState.Held -> ownerResources.updateRoutingLease(generation, state)
        is VpnRoutingLeaseState.ReleasePending -> ownerResources.updateRoutingLease(
            generation,
            state,
            VpnCleanupFailure(
                VpnCleanupResourceKind.RoutingLease,
                state.releaseFailureMessage() ?: "VPN routing activation release is pending",
            ),
        )
    }

    private fun retryRoutingLeaseRelease(generation: Long): VpnCleanupFailure? {
        return when (val current = ownerResources.routingLeaseState(generation)) {
            null,
            VpnRoutingLeaseState.Released -> null
            is VpnRoutingLeaseState.Held -> releaseRoutingLease(generation, current.lease)?.let {
                VpnCleanupFailure(VpnCleanupResourceKind.RoutingLease, it)
            }
            is VpnRoutingLeaseState.ReleasePending -> releaseRoutingLease(generation, current.lease)?.let {
                VpnCleanupFailure(VpnCleanupResourceKind.RoutingLease, it)
            }
        }
    }

    private fun closeOwnedInterface(generation: Long): VpnCleanupFailure? {
        val descriptor = ownerResources.tunnelDescriptor(generation) ?: return null
        val failure = closeInterface(descriptor)
        return when (failure) {
            null -> when (ownerResources.clearTunnelDescriptor(generation)) {
                VpnOwnerResourceMutation.Cleared,
                VpnOwnerResourceMutation.AlreadyAbsent -> null
                VpnOwnerResourceMutation.Recorded,
                VpnOwnerResourceMutation.StaleGeneration -> VpnCleanupFailure(
                    VpnCleanupResourceKind.TunnelDescriptor,
                    "VPN tunnel descriptor ownership changed during cleanup",
                )
            }
            else -> {
                val cleanupFailure = VpnCleanupFailure(VpnCleanupResourceKind.TunnelDescriptor, failure)
                ownerResources.markTunnelDescriptorPending(generation, cleanupFailure)
                cleanupFailure
            }
        }
    }

    private fun closeInterface(descriptor: ParcelFileDescriptor?): String? = try {
        descriptor?.close()
        null
    } catch (error: Exception) {
        error.message ?: "VPN interface close failed"
    }

    private fun removeForegroundAndStop(startId: Int?, generation: Long) {
        mainHandler.post {
            if (serviceAdmission.phase() == VpnServiceAdmissionPhase.Destroyed) return@post
            if (lifecycle.snapshot().generation != generation) return@post
            stopForeground(STOP_FOREGROUND_REMOVE)
            if (startId == null || !stopSelfResult(startId)) stopSelf()
        }
    }

    private fun failClosedUnrequestedRestart(
        startId: Int,
        reason: VpnServiceStartupRejection,
    ): Int {
        val before = lifecycle.snapshot()
        when (before.state) {
            VpnProcessState.StartRequested,
            VpnProcessState.Starting,
            VpnProcessState.Foreground,
            VpnProcessState.TunnelEstablished,
            VpnProcessState.Running,
            VpnProcessState.Stopping -> {
                Log.w(TAG, "Ignoring duplicate service start without a typed command")
                return START_NOT_STICKY
            }
            VpnProcessState.Stopped,
            VpnProcessState.Revoked,
            is VpnProcessState.Failed -> Unit
        }
        val message = when (reason) {
            VpnServiceStartupRejection.MalformedCommand ->
                "VPN service start contained a malformed command"
            VpnServiceStartupRejection.AlwaysOnNotEnabled ->
                "VPN service started without an explicit command while always-on is disabled"
        }
        val snapshot = lifecycle.failUnrequestedRestart(
            message,
        )
        Log.w(TAG, "Stopping unrequested VPN service start at generation ${snapshot.generation}: $message")
        stopSelfResult(startId)
        return START_NOT_STICKY
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

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val serviceChannel = NotificationChannel(
                CHANNEL_ID,
                getString(R.string.notification_channel_name),
                NotificationManager.IMPORTANCE_LOW // Low priority to avoid annoying sounds
            )
            val manager = getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(serviceChannel)
        }
    }

    private fun promoteToForeground(statusText: String): VpnForegroundPromotion =
        promoteToForeground(createNotification(statusText, notificationActionMode))

    private fun promoteToForeground(notification: Notification): VpnForegroundPromotion = try {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(
                NOTIFICATION_ID,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_SYSTEM_EXEMPTED,
            )
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
        VpnForegroundPromotion.Promoted
    } catch (error: Exception) {
        VpnForegroundPromotion.Failed(error.message ?: "Failed to enter foreground")
    }

    private fun createNotification(
        statusText: String,
        actionMode: VpnServiceNotificationActionMode,
    ): Notification {
        val mainIntent = Intent().setClassName(this, getString(R.string.notification_activity))
        val pendingMainIntent = PendingIntent.getActivity(
            this, 0, mainIntent, PendingIntent.FLAG_IMMUTABLE
        )
        val builder = NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle(getString(R.string.notification_title))
            .setContentText(statusText)
            .setSmallIcon(R.drawable.ic_app_icon)
            .setContentIntent(pendingMainIntent)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
        when (actionMode) {
            VpnServiceNotificationActionMode.UserManaged -> {
                val stopIntent = Intent(this, MihomoVpnService::class.java).apply {
                    putExtra(EXTRA_COMMAND, VpnServiceCommand.Stop)
                    putExtra(EXTRA_GENERATION, lifecycle.snapshot().generation)
                }
                val pendingStopIntent = PendingIntent.getService(
                    this,
                    0,
                    stopIntent,
                    PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
                )
                builder.addAction(
                    android.R.drawable.ic_menu_close_clear_cancel,
                    getString(R.string.action_stop),
                    pendingStopIntent,
                )
            }
            VpnServiceNotificationActionMode.SystemManaged -> Unit
        }
        return builder.build()
    }

    companion object {
        private const val TAG = "MihomoVpnService"
        private const val CHANNEL_ID = "vpn_service_channel"
        private const val NOTIFICATION_ID = 1
        private const val IPV4_TUN_ADDRESS = "172.19.0.1"
        private const val IPV6_TUN_ADDRESS = "fd00:fd00:fd00::1"
        private const val EXTRA_COMMAND = "vpn_service_command"
        private const val EXTRA_GENERATION = "vpn_generation"
        private const val EXTRA_DEADLINE_ELAPSED_REALTIME = "vpn_deadline_elapsed_realtime"
        private const val INVALID_GENERATION = Long.MIN_VALUE
        private val lifecycle = VpnLifecycleCoordinator()
        private val mainHandler = Handler(Looper.getMainLooper())
        private val sharedOwnerResources = VpnOwnerResourceRecord<
            ParcelFileDescriptor,
            VpnWorkerOwnership,
            VpnRoutingLeaseState,
            VpnPreparedOwnership,
        >()

        private val preparedConfigurationStore = VpnPreparedConfigurationStore()
        private const val OWNER_REQUEST_TIMEOUT_MILLIS = 15_000L
        private const val SYSTEM_POLICY_START_TIMEOUT_MILLIS = 30_000L
        private const val OWNER_SESSION_POLL_MILLIS = 50L

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

        internal fun startAndAwaitRunning(context: android.content.Context): VpnStartResult =
            startAndAwaitRunning(
                context,
                VpnOwnerRequestDeadline.after(
                    SystemClock.elapsedRealtime(),
                    OWNER_REQUEST_TIMEOUT_MILLIS,
                ),
            )

        internal fun startAndAwaitRunning(
            context: android.content.Context,
            deadline: VpnOwnerRequestDeadline,
        ): VpnStartResult =
            startAndAwaitRunning(
                context,
                deadline,
                { VpnOwnerStartGenerationBinding.Bound },
                { VpnOwnerStartGenerationBinding.ExecutionAlreadyStarted },
                { VpnOwnerStartDispatchOutcome.Admitted },
                { VpnOwnerStartDispatchOutcome.ExecutionStarted },
                { VpnOwnerStartDispatchOutcome.Dispatched },
            )

        internal fun startAndAwaitRunning(
            context: android.content.Context,
            deadline: VpnOwnerRequestDeadline,
            generationSelected: (Long) -> VpnOwnerStartGenerationBinding,
            generationAlreadyExecuting: (Long) -> VpnOwnerStartGenerationBinding,
            generationDispatchPending: (Long) -> VpnOwnerStartDispatchOutcome,
            generationDispatchStarted: (Long) -> VpnOwnerStartDispatchOutcome,
            generationDispatched: (Long) -> VpnOwnerStartDispatchOutcome,
        ): VpnStartResult {
            if (deadline.remainingAt(SystemClock.elapsedRealtime()) == VpnDeadlineState.Expired) {
                return VpnStartResult.TimedOut
            }
            if (VpnService.prepare(context) != null) {
                VpnAuthorizationBroadcast.requestForExplicitStart(context)
                return VpnStartResult.PermissionRequired
            }
            if (deadline.remainingAt(SystemClock.elapsedRealtime()) == VpnDeadlineState.Expired) {
                return VpnStartResult.TimedOut
            }
            val plan = lifecycle.requestStart(sharedOwnerResources.snapshot().presence())
            val generation = when (plan) {
                VpnStartPlan.AlreadyRunning -> return VpnStartResult.Running
                is VpnStartPlan.Join -> plan.generation
                VpnStartPlan.StopInProgress ->
                    return VpnStartResult.Failed("VPN is still stopping")
            }
            val generationBinding = when (plan) {
                is VpnStartPlan.Dispatch -> generationSelected(generation)
                is VpnStartPlan.Join -> generationAlreadyExecuting(generation)
                VpnStartPlan.AlreadyRunning,
                VpnStartPlan.StopInProgress -> return VpnStartResult.Failed(
                    "VPN start plan changed before generation binding",
                )
            }
            when (plan) {
                is VpnStartPlan.Dispatch -> when (generationBinding) {
                    VpnOwnerStartGenerationBinding.Bound -> Unit
                    VpnOwnerStartGenerationBinding.Canceled -> {
                        cancelUndispatchedStart(generation)
                        return VpnStartResult.TimedOut
                    }
                    VpnOwnerStartGenerationBinding.ExecutionAlreadyStarted,
                    VpnOwnerStartGenerationBinding.Unknown -> {
                        cancelUndispatchedStart(generation)
                        return VpnStartResult.Failed("VPN start request was no longer admitted")
                    }
                }
                is VpnStartPlan.Join -> when (generationBinding) {
                    VpnOwnerStartGenerationBinding.ExecutionAlreadyStarted -> Unit
                    VpnOwnerStartGenerationBinding.Canceled -> return VpnStartResult.TimedOut
                    VpnOwnerStartGenerationBinding.Bound,
                    VpnOwnerStartGenerationBinding.Unknown ->
                        return VpnStartResult.Failed("VPN join request was no longer admitted")
                }
                VpnStartPlan.AlreadyRunning,
                VpnStartPlan.StopInProgress -> return VpnStartResult.Failed(
                    "VPN start plan changed before generation binding",
                )
            }
            if (plan is VpnStartPlan.Dispatch) {
                if (deadline.remainingAt(SystemClock.elapsedRealtime()) == VpnDeadlineState.Expired) {
                    cancelUndispatchedStart(generation)
                    return VpnStartResult.TimedOut
                }
                when (generationDispatchPending(generation)) {
                    VpnOwnerStartDispatchOutcome.Admitted -> Unit
                    VpnOwnerStartDispatchOutcome.Canceled -> {
                        cancelUndispatchedStart(generation)
                        return VpnStartResult.TimedOut
                    }
                    VpnOwnerStartDispatchOutcome.Dispatched,
                    VpnOwnerStartDispatchOutcome.ExecutionStarted,
                    VpnOwnerStartDispatchOutcome.Unknown -> {
                        cancelUndispatchedStart(generation)
                        return VpnStartResult.Failed("VPN start request lost its dispatch admission")
                    }
                }
                if (!lifecycle.isCurrent(generation)) {
                    cancelUndispatchedStart(generation)
                    return VpnStartResult.TimedOut
                }
                when (generationDispatchStarted(generation)) {
                    VpnOwnerStartDispatchOutcome.ExecutionStarted -> Unit
                    VpnOwnerStartDispatchOutcome.Canceled -> {
                        cancelUndispatchedStart(generation)
                        return VpnStartResult.TimedOut
                    }
                    VpnOwnerStartDispatchOutcome.Admitted,
                    VpnOwnerStartDispatchOutcome.Dispatched,
                    VpnOwnerStartDispatchOutcome.Unknown -> {
                        cancelUndispatchedStart(generation)
                        return VpnStartResult.Failed("VPN start dispatch lost its execution admission")
                    }
                }
                val intent = Intent(context, MihomoVpnService::class.java).apply {
                    putExtra(EXTRA_COMMAND, VpnServiceCommand.Start)
                    putExtra(EXTRA_GENERATION, generation)
                    putExtra(EXTRA_DEADLINE_ELAPSED_REALTIME, deadline.elapsedRealtimeMillis)
                }
                try {
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                        context.startForegroundService(intent)
                    } else {
                        context.startService(intent)
                    }
                } catch (error: Exception) {
                    val message = error.message ?: "Failed to start VPN service"
                    return when (lifecycle.transition(generation, VpnProcessState.Failed(message))) {
                        VpnTransitionOutcome.Applied -> VpnStartResult.Failed(message)
                        VpnTransitionOutcome.StaleGeneration ->
                            VpnStartResult.Failed("VPN start request was superseded")
                        VpnTransitionOutcome.RunningRequiresDataPlaneReceipt ->
                            VpnStartResult.Failed(
                                "VPN Running requires a successful Rust data-plane receipt",
                            )
                    }
                }
                when (generationDispatched(generation)) {
                    VpnOwnerStartDispatchOutcome.Dispatched -> Unit
                    VpnOwnerStartDispatchOutcome.Canceled -> {
                        return VpnStartResult.TimedOut
                    }
                    VpnOwnerStartDispatchOutcome.ExecutionStarted,
                    VpnOwnerStartDispatchOutcome.Admitted ->
                        return VpnStartResult.Failed("VPN start dispatch did not record acceptance")
                    VpnOwnerStartDispatchOutcome.Unknown -> {
                        return VpnStartResult.Failed("VPN start request lost its dispatch admission")
                    }
                }
            }
            return lifecycle.awaitStartUntil(generation, deadline, SystemClock::elapsedRealtime)
        }

        internal fun cancelUndispatchedStart(generation: Long) {
            val plan = lifecycle.requestCleanup(
                VpnCleanupIntent.Stop,
                VpnStopTarget.Generation(generation),
                sharedOwnerResources.snapshot().presence(),
            )
            val request = when (plan) {
                is VpnCleanupPlan.Dispatch,
                is VpnCleanupPlan.Retry -> plan.request
                is VpnCleanupPlan.Join -> return
                is VpnCleanupPlan.AlreadyClean,
                VpnCleanupPlan.Superseded -> return
            }
            val snapshot = lifecycle.snapshot()
            when (
                lifecycle.completeCleanup(
                    request,
                    VpnCleanupOutcome.Completed,
                    VpnCleanupOwnerReadback(
                        generation = snapshot.generation,
                        state = snapshot.state,
                        resources = sharedOwnerResources.snapshot().presence(),
                    ),
                )
            ) {
                is VpnCleanupPublication.Published,
                is VpnCleanupPublication.CleanupPending,
                VpnCleanupPublication.StaleGeneration -> Unit
            }
        }

        private fun requestCleanupWithoutWaiting(
            context: android.content.Context,
            target: VpnStopTarget,
        ) {
            val plan = lifecycle.requestCleanup(
                VpnCleanupIntent.Stop,
                target,
                sharedOwnerResources.snapshot().presence(),
            )
            val request = when (plan) {
                is VpnCleanupPlan.Dispatch,
                is VpnCleanupPlan.Retry,
                is VpnCleanupPlan.Join -> plan.request
                is VpnCleanupPlan.AlreadyClean,
                VpnCleanupPlan.Superseded -> return
            }
            when (plan) {
                is VpnCleanupPlan.Join -> Unit
                is VpnCleanupPlan.Dispatch,
                is VpnCleanupPlan.Retry -> {
                    val intent = Intent(context, MihomoVpnService::class.java).apply {
                        putExtra(EXTRA_COMMAND, VpnServiceCommand.Stop)
                        putExtra(EXTRA_GENERATION, request.generation)
                    }
                    try {
                        context.startService(intent)
                    } catch (error: Exception) {
                        val message = error.message ?: "Failed to queue VPN start cancellation cleanup"
                        sharedOwnerResources.markCleanupPending(
                            request.resourceGeneration,
                            VpnCleanupFailure(VpnCleanupResourceKind.ExecutorQueue, message),
                        )
                        lifecycle.cleanupQueueRejected(request, message, VpnCleanupResourceKind.ExecutorQueue)
                    }
                }
                is VpnCleanupPlan.AlreadyClean,
                VpnCleanupPlan.Superseded -> Unit
            }
        }

        internal fun stopAndAwaitStopped(
            context: android.content.Context,
            target: VpnStopTarget = VpnStopTarget.Current,
            deadline: VpnOwnerRequestDeadline = VpnOwnerRequestDeadline.after(
                SystemClock.elapsedRealtime(),
                OWNER_REQUEST_TIMEOUT_MILLIS,
            ),
        ): VpnStopResult {
            val plan = lifecycle.requestCleanup(
                VpnCleanupIntent.Stop,
                target,
                sharedOwnerResources.snapshot().presence(),
            )
            val request = when (plan) {
                is VpnCleanupPlan.AlreadyClean -> return VpnStopResult.Stopped
                VpnCleanupPlan.Superseded -> return VpnStopResult.Superseded
                is VpnCleanupPlan.Join -> plan.request
                is VpnCleanupPlan.Dispatch -> plan.request
                is VpnCleanupPlan.Retry -> plan.request
            }
            when (plan) {
                is VpnCleanupPlan.Dispatch,
                is VpnCleanupPlan.Retry -> {
                    val intent = Intent(context, MihomoVpnService::class.java).apply {
                        putExtra(EXTRA_COMMAND, VpnServiceCommand.Stop)
                        putExtra(EXTRA_GENERATION, request.generation)
                    }
                    try {
                        context.startService(intent)
                    } catch (error: Exception) {
                        val message = error.message ?: "Failed to stop VPN service"
                        sharedOwnerResources.markCleanupPending(
                            request.resourceGeneration,
                            VpnCleanupFailure(VpnCleanupResourceKind.ExecutorQueue, message),
                        )
                        return when (lifecycle.cleanupQueueRejected(
                            request,
                            message,
                            VpnCleanupResourceKind.ExecutorQueue,
                        )) {
                            is VpnCleanupPublication.CleanupPending -> VpnStopResult.Failed(message)
                            is VpnCleanupPublication.Published -> VpnStopResult.Superseded
                            VpnCleanupPublication.StaleGeneration -> VpnStopResult.Superseded
                        }
                    }
                }
                is VpnCleanupPlan.Join,
                is VpnCleanupPlan.AlreadyClean,
                VpnCleanupPlan.Superseded -> Unit
            }
            return lifecycle.awaitStopUntil(request.generation, deadline, SystemClock::elapsedRealtime)
        }

        @Synchronized
        @Suppress("UNUSED_PARAMETER")
        fun setPendingConfiguration(
            context: android.content.Context,
            configJson: String,
        ): VpnConfigurationApplyOutcomeWire {
            val prepared = NativePreparedVpnConfiguration.parse(configJson)
                ?: return VpnConfigurationApplyOutcomeWire.Rejected
            if (prepared.vpnGeneration != lifecycle.snapshot().generation.toULong()) {
                return VpnConfigurationApplyOutcomeWire.Rejected
            }
            when (lifecycle.preparedConfigurationGate(prepared.vpnGeneration.toLong())) {
                VpnPreparedConfigurationGate.Accepted -> Unit
                VpnPreparedConfigurationGate.CleanupPending,
                VpnPreparedConfigurationGate.StaleGeneration,
                is VpnPreparedConfigurationGate.UnexpectedState ->
                    return VpnConfigurationApplyOutcomeWire.Rejected
            }
            when (preparedConfigurationStore.install(prepared)) {
                VpnPreparedConfigurationInstallOutcome.RejectedCurrent ->
                    return VpnConfigurationApplyOutcomeWire.Rejected
                VpnPreparedConfigurationInstallOutcome.Installed,
                VpnPreparedConfigurationInstallOutcome.ReplacedOlder -> Unit
            }
            val recordOutcome = sharedOwnerResources.recordPreparedConfiguration(
                prepared.vpnGeneration.toLong(),
                VpnPreparedOwnership(prepared.preparationId),
            )
            val gateAfterInstall = lifecycle.preparedConfigurationGate(prepared.vpnGeneration.toLong())
            fun rejectPreparedConfiguration(): VpnConfigurationApplyOutcomeWire {
                preparedConfigurationStore.clearThrough(prepared.vpnGeneration)
                sharedOwnerResources.clearPreparedConfiguration(prepared.vpnGeneration.toLong())
                return VpnConfigurationApplyOutcomeWire.Rejected
            }
            return when (recordOutcome) {
                VpnOwnerResourceMutation.Recorded -> when (gateAfterInstall) {
                    VpnPreparedConfigurationGate.Accepted -> VpnConfigurationApplyOutcomeWire.Accepted
                    VpnPreparedConfigurationGate.CleanupPending,
                    VpnPreparedConfigurationGate.StaleGeneration,
                    is VpnPreparedConfigurationGate.UnexpectedState -> rejectPreparedConfiguration()
                }
                VpnOwnerResourceMutation.Cleared,
                VpnOwnerResourceMutation.AlreadyAbsent,
                VpnOwnerResourceMutation.StaleGeneration -> rejectPreparedConfiguration()
            }
        }

        @Synchronized
        private fun takePendingConfiguration(
            vpnGeneration: ULong,
            preparationId: ULong,
        ): VpnPreparedConfigurationTakeOutcome = preparedConfigurationStore.take(
            vpnGeneration,
            preparationId,
        )

        @Synchronized
        private fun clearPendingConfigurationThrough(
            vpnGeneration: Long,
        ): VpnPreparedConfigurationClearOutcome {
            if (vpnGeneration <= 0L) return VpnPreparedConfigurationClearOutcome.NotRequired
            return preparedConfigurationStore.clearThrough(vpnGeneration.toULong())
        }

    }
}

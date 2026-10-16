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


    internal fun MihomoVpnService.Companion.startAndAwaitRunning(context: android.content.Context): VpnStartResult =
        startAndAwaitRunning(
            context,
            VpnOwnerRequestDeadline.after(
                SystemClock.elapsedRealtime(),
                OWNER_REQUEST_TIMEOUT_MILLIS,
            ),
        )

    internal fun MihomoVpnService.Companion.startAndAwaitRunning(
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

    internal fun MihomoVpnService.Companion.startAndAwaitRunning(
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
            is VpnStartPlan.Dispatch -> plan.generation
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

    internal fun MihomoVpnService.Companion.cancelUndispatchedStart(generation: Long) {
        val plan = lifecycle.requestCleanup(
            VpnCleanupIntent.Stop,
            VpnStopTarget.Generation(generation),
            sharedOwnerResources.snapshot().presence(),
        )
        val request = when (plan) {
            is VpnCleanupPlan.Dispatch -> plan.request
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

    internal fun MihomoVpnService.Companion.requestCleanupWithoutWaiting(
        context: android.content.Context,
        target: VpnStopTarget,
    ) {
        val plan = lifecycle.requestCleanup(
            VpnCleanupIntent.Stop,
            target,
            sharedOwnerResources.snapshot().presence(),
        )
        val request = when (plan) {
            is VpnCleanupPlan.Dispatch -> plan.request
            is VpnCleanupPlan.Retry -> plan.request
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

    internal fun MihomoVpnService.Companion.stopAndAwaitStopped(
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
    internal fun MihomoVpnService.Companion.setPendingConfiguration(
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
    internal fun MihomoVpnService.Companion.takePendingConfiguration(
        vpnGeneration: ULong,
        preparationId: ULong,
    ): VpnPreparedConfigurationTakeOutcome = preparedConfigurationStore.take(
        vpnGeneration,
        preparationId,
    )

    @Synchronized
    internal fun MihomoVpnService.Companion.clearPendingConfigurationThrough(
        vpnGeneration: Long,
    ): VpnPreparedConfigurationClearOutcome {
        if (vpnGeneration <= 0L) return VpnPreparedConfigurationClearOutcome.NotRequired
        return preparedConfigurationStore.clearThrough(vpnGeneration.toULong())
    }


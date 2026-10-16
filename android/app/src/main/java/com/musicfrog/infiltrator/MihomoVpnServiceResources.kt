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


internal fun MihomoVpnService.createLifecycleWakeLock(): VpnLifecycleWakeLock {
    val handle = try {
        getSystemService(PowerManager::class.java)
            ?.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "$packageName:VpnLifecycle")
            ?.apply { setReferenceCounted(false) }
            ?.let(::AndroidVpnWakeLockHandle)
            ?: UnavailableVpnWakeLockHandle
    } catch (error: RuntimeException) {
        Log.w(MihomoVpnService.TAG, "Android partial WakeLock is unavailable: ${error.message}")
        UnavailableVpnWakeLockHandle
    }
    return VpnLifecycleWakeLock(handle, VpnLifecycleWakeLock.MAX_LEASE_DURATION_MILLIS)
}

internal fun MihomoVpnService.logLifecycleWakeLockAcquire(generation: Long, outcome: VpnWakeLockAcquire) {
    when (outcome) {
        is VpnWakeLockAcquire.Acquired,
        is VpnWakeLockAcquire.Joined -> Unit
        is VpnWakeLockAcquire.Busy -> Log.w(
            MihomoVpnService.TAG,
            "VPN generation $generation shares a bounded WakeLock held by generation ${outcome.heldGeneration}",
        )
        is VpnWakeLockAcquire.Unavailable -> Log.w(
            MihomoVpnService.TAG,
            "VPN MihomoVpnService.lifecycle continues without a WakeLock: ${outcome.reason}",
        )
        is VpnWakeLockAcquire.CleanupPending -> Log.e(
            MihomoVpnService.TAG,
            "VPN WakeLock release remains pending; the platform timeout is the fallback: ${outcome.reason}",
        )
    }
}

internal fun MihomoVpnService.logLifecycleWakeLockRelease(generation: Long, outcome: VpnWakeLockRelease) {
    when (outcome) {
        VpnWakeLockRelease.Released,
        VpnWakeLockRelease.AlreadyReleased,
        VpnWakeLockRelease.Expired -> Unit
        VpnWakeLockRelease.StaleGeneration -> Log.w(
            MihomoVpnService.TAG,
            "Ignoring stale WakeLock release for VPN generation $generation",
        )
        is VpnWakeLockRelease.CleanupPending -> Log.e(
            MihomoVpnService.TAG,
            "VPN WakeLock release remains pending until its bounded timeout: ${outcome.reason}",
        )
    }
}

internal fun MihomoVpnService.cleanupStaleStart(generation: Long) {
    val snapshot = MihomoVpnService.lifecycle.snapshot()
    val intent = when (val state = snapshot.state) {
        is VpnProcessState.Failed -> when {
            snapshot.generation == generation -> VpnCleanupIntent.StartFailure(state.message)
            else -> VpnCleanupIntent.Stop
        }
        else -> VpnCleanupIntent.Stop
    }
    val plan = MihomoVpnService.lifecycle.requestCleanup(
        intent,
        VpnStopTarget.Generation(generation),
        ownerResources.snapshot().presence(),
    )
    queueCleanup(plan, stopId = null)
}

internal fun MihomoVpnService.cleanupFailedStart(generation: Long, startId: Int, message: String) {
    when (
        MihomoVpnService.lifecycle.transition(generation, VpnProcessState.Failed(message))
    ) {
        VpnTransitionOutcome.Applied ->
            queueCleanup(
                MihomoVpnService.lifecycle.requestCleanup(
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

internal fun MihomoVpnService.stopRustWorker(generation: Long): VpnCleanupFailure? {
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

internal fun MihomoVpnService.clearPreparedVpnConfiguration(generation: Long): VpnCleanupFailure? {
    if (generation <= 0L) return null
    MihomoVpnService.clearPendingConfigurationThrough(generation)
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

internal fun MihomoVpnService.releaseRoutingLease(
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

internal fun MihomoVpnService.releaseConfigurationActivation(
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

internal fun MihomoVpnService.recordRoutingLeaseState(
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

internal fun MihomoVpnService.retryRoutingLeaseRelease(generation: Long): VpnCleanupFailure? {
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

internal fun MihomoVpnService.closeOwnedInterface(generation: Long): VpnCleanupFailure? {
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

internal fun MihomoVpnService.closeInterface(descriptor: ParcelFileDescriptor?): String? = try {
    descriptor?.close()
    null
} catch (error: Exception) {
    error.message ?: "VPN interface close failed"
}

internal fun MihomoVpnService.removeForegroundAndStop(startId: Int?, generation: Long) {
    MihomoVpnService.mainHandler.post {
        if (serviceAdmission.phase() == VpnServiceAdmissionPhase.Destroyed) return@post
        if (MihomoVpnService.lifecycle.snapshot().generation != generation) return@post
        stopForeground(android.app.Service.STOP_FOREGROUND_REMOVE)
        if (startId == null || !stopSelfResult(startId)) stopSelf()
    }
}

internal fun MihomoVpnService.failClosedUnrequestedRestart(
    startId: Int,
    reason: VpnServiceStartupRejection,
): Int {
    val before = MihomoVpnService.lifecycle.snapshot()
    when (before.state) {
        VpnProcessState.StartRequested,
        VpnProcessState.Starting,
        VpnProcessState.Foreground,
        VpnProcessState.TunnelEstablished,
        VpnProcessState.Running,
        VpnProcessState.Stopping -> {
            Log.w(MihomoVpnService.TAG, "Ignoring duplicate service start without a typed command")
            return android.app.Service.START_NOT_STICKY
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
    val snapshot = MihomoVpnService.lifecycle.failUnrequestedRestart(
        message,
    )
    Log.w(MihomoVpnService.TAG, "Stopping unrequested VPN service start at generation ${snapshot.generation}: $message")
    stopSelfResult(startId)
    return android.app.Service.START_NOT_STICKY
}


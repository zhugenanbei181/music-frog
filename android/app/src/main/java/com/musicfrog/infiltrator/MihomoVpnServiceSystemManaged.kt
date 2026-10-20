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

internal fun MihomoVpnService.readServiceCommand(intent: Intent?): VpnServiceCommandExtra {
    if (intent == null || !intent.hasExtra(MihomoVpnService.EXTRA_COMMAND)) return VpnServiceCommandExtra.Missing
    val command = try {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            intent.getSerializableExtra(MihomoVpnService.EXTRA_COMMAND, VpnServiceCommand::class.java)
        } else {
            @Suppress("DEPRECATION")
            intent.getSerializableExtra(MihomoVpnService.EXTRA_COMMAND) as? VpnServiceCommand
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

internal fun MihomoVpnService.readServiceDelivery(flags: Int): VpnServiceDelivery = when {
    flags and android.app.Service.START_FLAG_REDELIVERY != 0 -> VpnServiceDelivery.Redelivered
    flags and android.app.Service.START_FLAG_RETRY != 0 -> VpnServiceDelivery.Retried
    else -> VpnServiceDelivery.Initial
}

internal fun MihomoVpnService.readAlwaysOnObservation(): VpnAlwaysOnObservation = when {
    isAlwaysOn -> VpnAlwaysOnObservation.Enabled
    else -> VpnAlwaysOnObservation.Disabled
}

internal fun MihomoVpnService.startSystemManagedProxy(startId: Int, decision: VpnServiceStartupDecision.RestoreSystemManagedProxy): Int {
    notificationActionMode = VpnServiceNotificationActionMode.SystemManaged
    when (val promotion = promoteToForeground(getString(R.string.notification_starting))) {
        VpnForegroundPromotion.Promoted -> Unit

        is VpnForegroundPromotion.Failed -> {
            Log.e(MihomoVpnService.TAG, "Always-on VPN service could not enter foreground: ${promotion.reason}")
            stopSelfResult(startId)
            return android.app.Service.START_NOT_STICKY
        }
    }
    try {
        startService(Intent(this, MihomoControlService::class.java))
    } catch (error: Exception) {
        showSystemManagedStartupFailure(
            startId,
            "Core owner service could not be initialized: ${error.message ?: error.javaClass.simpleName}",
        )
        return android.app.Service.START_STICKY
    }
    try {
        systemPolicyExecutor.execute {
            dispatchSystemManagedProxyStart(startId, decision)
        }
    } catch (_: RejectedExecutionException) {
        showSystemManagedStartupFailure(startId, "VPN system-start queue is full")
    }
    return android.app.Service.START_STICKY
}

internal fun MihomoVpnService.dispatchSystemManagedProxyStart(
    startId: Int,
    decision: VpnServiceStartupDecision.RestoreSystemManagedProxy,
) {
    val deadline = VpnOwnerRequestDeadline.after(
        SystemClock.elapsedRealtime(),
        MihomoVpnService.SYSTEM_POLICY_START_TIMEOUT_MILLIS,
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
        runBlocking { androidControl(AndroidControlIntent.START_PROXY) }
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
    Log.i(MihomoVpnService.TAG, "System-managed VPN startup completed with ${decision.policy}/${decision.delivery}")
}

internal fun MihomoVpnService.awaitCoreOwnerSession(
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
                pollIntervalMillis = MihomoVpnService.OWNER_SESSION_POLL_MILLIS,
            )
        ) {
            is VpnOwnerSessionWaitDirective.Complete -> return directive.outcome

            is VpnOwnerSessionWaitDirective.Wait -> try {
                // ARCH-REDLINE: the typed owner wait directive is bounded and runs on the VPN worker thread
                Thread.sleep(directive.millis)
            } catch (_: InterruptedException) {
                Thread.currentThread().interrupt()
                return VpnOwnerSessionWaitOutcome.Interrupted
            }
        }
    }
}

internal fun MihomoVpnService.showSystemManagedStartupFailure(startId: Int, reason: String) {
    Log.e(MihomoVpnService.TAG, reason)
    when (serviceAdmission.phase()) {
        VpnServiceAdmissionPhase.Accepting -> Unit

        VpnServiceAdmissionPhase.Destroying,
        VpnServiceAdmissionPhase.Destroyed,
        -> return
    }
    val snapshot = MihomoVpnService.lifecycle.snapshot()
    when (snapshot.state) {
        VpnProcessState.Running -> Unit

        VpnProcessState.StartRequested,
        VpnProcessState.Starting,
        VpnProcessState.Foreground,
        VpnProcessState.TunnelEstablished,
        -> cleanupFailedStart(snapshot.generation, startId, reason)

        VpnProcessState.Stopping -> Unit

        VpnProcessState.Stopped,
        VpnProcessState.Revoked,
        is VpnProcessState.Failed,
        -> MihomoVpnService.lifecycle.failUnrequestedRestart(reason)
    }
    showSystemManagedStartupStatus("${getString(R.string.notification_start_failed)}: $reason")
}

internal fun MihomoVpnService.showSystemManagedStartupStatus(status: String) {
    MihomoVpnService.mainHandler.post {
        if (serviceAdmission.phase() == VpnServiceAdmissionPhase.Destroyed) return@post
        val notification = createNotification(
            status,
            VpnServiceNotificationActionMode.SystemManaged,
        )
        val manager = getSystemService(NotificationManager::class.java)
        manager.notify(MihomoVpnService.NOTIFICATION_ID, notification)
    }
}

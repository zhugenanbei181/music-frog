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

internal fun MihomoVpnService.createNotificationChannel() {
    val serviceChannel = NotificationChannel(
        MihomoVpnService.CHANNEL_ID,
        getString(R.string.notification_channel_name),
        NotificationManager.IMPORTANCE_LOW, // Low priority to avoid annoying sounds
    )
    val manager = getSystemService(NotificationManager::class.java)
    manager.createNotificationChannel(serviceChannel)
}

internal fun MihomoVpnService.promoteToForeground(statusText: String): VpnForegroundPromotion =
    promoteToForeground(createNotification(statusText, notificationActionMode))

internal fun MihomoVpnService.promoteToForeground(notification: Notification): VpnForegroundPromotion = try {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
        startForeground(
            MihomoVpnService.NOTIFICATION_ID,
            notification,
            ServiceInfo.FOREGROUND_SERVICE_TYPE_SYSTEM_EXEMPTED,
        )
    } else {
        startForeground(MihomoVpnService.NOTIFICATION_ID, notification)
    }
    VpnForegroundPromotion.Promoted
} catch (error: Exception) {
    VpnForegroundPromotion.Failed(error.message ?: "Failed to enter foreground")
}

internal fun MihomoVpnService.createNotification(statusText: String, actionMode: VpnServiceNotificationActionMode): Notification {
    val mainIntent = Intent().setClassName(this, getString(R.string.notification_activity))
    val pendingMainIntent = PendingIntent.getActivity(
        this,
        0,
        mainIntent,
        PendingIntent.FLAG_IMMUTABLE,
    )
    val builder = NotificationCompat.Builder(this, MihomoVpnService.CHANNEL_ID)
        .setContentTitle(getString(R.string.notification_title))
        .setContentText(statusText)
        .setSmallIcon(R.drawable.ic_app_icon)
        .setContentIntent(pendingMainIntent)
        .setOngoing(true)
        .setOnlyAlertOnce(true)
    when (actionMode) {
        VpnServiceNotificationActionMode.UserManaged -> {
            val stopIntent = Intent(this, MihomoVpnService::class.java).apply {
                putExtra(MihomoVpnService.EXTRA_COMMAND, VpnServiceCommand.Stop)
                putExtra(MihomoVpnService.EXTRA_GENERATION, MihomoVpnService.lifecycle.snapshot().generation)
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

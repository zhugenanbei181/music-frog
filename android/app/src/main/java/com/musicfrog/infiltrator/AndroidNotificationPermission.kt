package com.musicfrog.infiltrator

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.content.ContextCompat

internal enum class NotificationPermissionStatus {
    NotRequired,
    Granted,
    NotGranted,
}

internal enum class NotificationPermissionPromptResult {
    Granted,
    NotGranted,
}

internal object AndroidNotificationPermission {
    fun status(context: Context): NotificationPermissionStatus =
        status(
            Build.VERSION.SDK_INT,
            ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS),
        )

    internal fun status(sdkVersion: Int, permissionResult: Int): NotificationPermissionStatus =
        when {
            sdkVersion < Build.VERSION_CODES.TIRAMISU -> NotificationPermissionStatus.NotRequired
            permissionResult == PackageManager.PERMISSION_GRANTED -> NotificationPermissionStatus.Granted
            else -> NotificationPermissionStatus.NotGranted
        }

    internal fun fromActivityResult(granted: Boolean): NotificationPermissionPromptResult =
        if (granted) NotificationPermissionPromptResult.Granted
        else NotificationPermissionPromptResult.NotGranted

    internal fun fromPermissionResults(grantResults: IntArray): NotificationPermissionPromptResult =
        if (grantResults.firstOrNull() == PackageManager.PERMISSION_GRANTED) {
            NotificationPermissionPromptResult.Granted
        } else {
            // Android reports both denial and dismissal as a missing grant. The
            // Activity Result contract also exposes only a Boolean, so the app
            // must not claim to distinguish those platform outcomes.
            NotificationPermissionPromptResult.NotGranted
        }
}

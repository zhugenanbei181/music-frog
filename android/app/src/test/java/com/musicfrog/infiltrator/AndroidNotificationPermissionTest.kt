package com.musicfrog.infiltrator

import android.content.pm.PackageManager
import android.os.Build
import org.junit.Assert.assertEquals
import org.junit.Test

class AndroidNotificationPermissionTest {
    @Test
    fun notificationPermissionIsNotRequiredBeforeAndroidThirteen() {
        assertEquals(
            NotificationPermissionStatus.NotRequired,
            AndroidNotificationPermission.status(Build.VERSION_CODES.S, PackageManager.PERMISSION_DENIED),
        )
    }

    @Test
    fun notificationPermissionTracksThePlatformGrantResult() {
        assertEquals(
            NotificationPermissionStatus.Granted,
            AndroidNotificationPermission.status(Build.VERSION_CODES.TIRAMISU, PackageManager.PERMISSION_GRANTED),
        )
        assertEquals(
            NotificationPermissionStatus.NotGranted,
            AndroidNotificationPermission.status(Build.VERSION_CODES.TIRAMISU, PackageManager.PERMISSION_DENIED),
        )
    }

    @Test
    fun deniedAndDismissedNotificationPromptsShareThePlatformNotGrantedOutcome() {
        assertEquals(
            NotificationPermissionPromptResult.NotGranted,
            AndroidNotificationPermission.fromActivityResult(false),
        )
        assertEquals(
            NotificationPermissionPromptResult.NotGranted,
            AndroidNotificationPermission.fromPermissionResults(
                intArrayOf(PackageManager.PERMISSION_DENIED),
            ),
        )
        assertEquals(
            NotificationPermissionPromptResult.NotGranted,
            AndroidNotificationPermission.fromPermissionResults(intArrayOf()),
        )
    }
}

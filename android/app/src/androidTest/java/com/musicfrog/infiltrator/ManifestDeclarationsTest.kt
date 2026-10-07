package com.musicfrog.infiltrator

import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * BANDROID-022: instrumented assertions that the *installed* APK really declares
 * the permissions, VPN service and boot receiver the product depends on.
 *
 * This is the runtime counterpart of `scripts/quality/android-manifest-guard.py`,
 * which only reads the source manifest. It runs on an emulator or device via
 * `connectedDebugAndroidTest` and therefore cannot be replaced by the PR compile
 * gate.
 */
@RunWith(AndroidJUnit4::class)
class ManifestDeclarationsTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val packageName = context.packageName
    private val packageInfo = context.packageManager.getPackageInfo(
        packageName,
        PackageManager.GET_PERMISSIONS or PackageManager.GET_SERVICES or PackageManager.GET_RECEIVERS,
    )

    private fun service(name: String): ServiceInfo? =
        packageInfo.services?.firstOrNull { it.name == "$packageName.$name" }

    @Test
    fun declaresRequiredPermissions() {
        val declared = packageInfo.requestedPermissions?.toSet().orEmpty()
        val required = listOf(
            "android.permission.INTERNET",
            "android.permission.ACCESS_NETWORK_STATE",
            "android.permission.FOREGROUND_SERVICE",
            "android.permission.POST_NOTIFICATIONS",
            "android.permission.RECEIVE_BOOT_COMPLETED",
            "android.permission.FOREGROUND_SERVICE_SYSTEM_EXEMPTED",
        )
        for (permission in required) {
            assertTrue("manifest must declare $permission", permission in declared)
        }
    }

    @Test
    fun vpnServiceIsBoundAndTypedForForeground() {
        val vpn = requireNotNull(service("MihomoVpnService")) {
            "MihomoVpnService must be declared"
        }
        assertEquals("android.permission.BIND_VPN_SERVICE", vpn.permission)
        assertTrue(
            "MihomoVpnService must declare a foreground service type",
            vpn.foregroundServiceType != 0,
        )
    }

    @Test
    fun bootReceiverIsDeclared() {
        requireNotNull(
            packageInfo.receivers?.firstOrNull { it.name == "$packageName.BootReceiver" },
        ) { "BootReceiver must be declared" }
    }

    @Test
    fun quickSettingsTileServiceIsDeclared() {
        val tile = requireNotNull(service("InfiltratorTileService")) {
            "InfiltratorTileService must be declared"
        }
        assertEquals("android.permission.BIND_QUICK_SETTINGS_TILE", tile.permission)
    }
}

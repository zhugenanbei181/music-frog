package com.musicfrog.infiltrator

import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.net.VpnService
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
 * It runs on an emulator or device via `connectedDebugAndroidTest` and therefore
 * cannot be replaced by the PR compile gate.
 */
@RunWith(AndroidJUnit4::class)
class ManifestDeclarationsTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val packageName = context.packageName
    private val packageInfo = context.packageManager.getPackageInfo(
        packageName,
        PackageManager.GET_PERMISSIONS or
            PackageManager.GET_SERVICES or
            PackageManager.GET_RECEIVERS or
            PackageManager.GET_META_DATA,
    )

    private fun service(name: String): ServiceInfo? =
        packageInfo.services?.firstOrNull { it.name.substringAfterLast('.') == name }

    @Test
    fun launcherMatchesTheInstalledProductFlavor() {
        val expectedActivity = when (BuildConfig.FLAVOR) {
            "compose" -> "com.musicfrog.infiltrator.MainActivity"
            "bevy" -> "com.musicfrog.infiltrator.BevyHostActivity"
            "slint" -> "android.app.NativeActivity"
            else -> error("unregistered Android product flavor: ${BuildConfig.FLAVOR}")
        }
        val actualActivity = context.packageManager
            .getLaunchIntentForPackage(packageName)
            ?.resolveActivity(context.packageManager)
            ?.className
        assertEquals(expectedActivity, actualActivity)
    }

    @Test
    fun launcherNameAndIconArePresentForTheInstalledFlavor() {
        val label = context.applicationInfo.loadLabel(context.packageManager).toString()
        assertEquals(context.getString(R.string.app_name), label)
        assertTrue("application must have its registered icon resource", context.applicationInfo.icon != 0)
    }

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
        assertEquals(true, vpn.exported)
        assertTrue("MihomoVpnService must share the :vpn owner process", vpn.processName.endsWith(":vpn"))
        assertEquals(
            ServiceInfo.FOREGROUND_SERVICE_TYPE_SYSTEM_EXEMPTED,
            vpn.foregroundServiceType,
        )
        assertEquals(
            false,
            requireNotNull(vpn.metaData).getBoolean(
                VpnService.SERVICE_META_DATA_SUPPORTS_ALWAYS_ON,
                true,
            ),
        )
    }

    @Test
    fun controlServiceOwnsThePrivateVpnProcess() {
        val control = requireNotNull(service("MihomoControlService")) {
            "MihomoControlService must be declared"
        }
        assertTrue(control.processName.endsWith(":vpn"))
        assertEquals(false, control.exported)
    }

    @Test
    fun bootReceiverIsDeclared() {
        requireNotNull(
            packageInfo.receivers?.firstOrNull { it.name.endsWith(".BootReceiver") },
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

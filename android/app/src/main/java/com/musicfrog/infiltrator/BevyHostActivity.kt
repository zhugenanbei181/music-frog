package com.musicfrog.infiltrator

import android.app.NativeActivity
import android.os.Bundle
import android.util.Log

/**
 * BANDROID-003: Gradle-hosted Bevy product Activity.
 *
 * Extends [NativeActivity] because the Bevy/winit Android surface is owned by
 * the native driver (`android-activity`'s `android_main`), which installs
 * `bevy_android::ANDROID_APP` and calls
 * `infiltrator_android::launch_bevy_android_host`. Kotlin owns only the shared
 * `:vpn` service binding and the process lifecycle here; it does not own a
 * second swapchain, VPN state machine or bridge registration.
 *
 * The driver library name is declared in the manifest as
 * `android.app.lib_name`. The Activity is intentionally not the default
 * launcher until the native driver is packaged and BANDROID-003 selects the
 * shipping surface; see docs/android/BEVY_ANDROID_PRODUCT.md §4.
 */
class BevyHostActivity : NativeActivity() {
    private var vpnBinding: VpnServiceBinding? = null
    private var nativeHost: NativeHostAdapter? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // The `:vpn` process owns the service; this UI process only binds to it.
        vpnBinding = VpnServiceBinding(this, BindingLog).also { it.bind() }
        // BANDROID-005/006/008: push the real Activity lifecycle, insets and
        // clipboard into the shared typed seams.
        nativeHost = NativeHostAdapter(this).also { it.attach() }
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        nativeHost?.onWindowFocusChanged(hasFocus)
    }

    override fun onDestroy() {
        nativeHost?.detach()
        nativeHost = null
        vpnBinding?.unbind()
        vpnBinding = null
        super.onDestroy()
    }

    private object BindingLog : VpnServiceBinding.Listener {
        override fun onBindingStateChanged(state: VpnServiceBinding.State) {
            Log.i(TAG, "vpn service binding: $state")
        }
    }

    companion object {
        private const val TAG = "BevyHostActivity"

        /**
         * Native library packaged by the Bevy host Gradle module and declared
         * as `android.app.lib_name` in the manifest. Kept here so the Activity
         * and the packaging wiring share one source of truth.
         */
        const val DRIVER_LIBRARY = "infiltrator_bevy_android"
    }
}

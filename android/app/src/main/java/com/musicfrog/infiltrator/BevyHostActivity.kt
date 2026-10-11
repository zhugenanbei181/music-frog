package com.musicfrog.infiltrator

import android.app.NativeActivity
import android.Manifest
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.net.VpnService
import android.os.Bundle
import android.util.Log
import android.widget.Toast
import androidx.core.content.ContextCompat
import androidx.annotation.Keep
import infiltrator_android.AndroidControlIntent
import infiltrator_android.FfiErrorCode
import infiltrator_android.VpnAuthorizationResult
import infiltrator_android.FfiStatus
import infiltrator_android.androidControl
import infiltrator_android.androidExecuteProductCommandJson
import infiltrator_android.androidSurfaceSnapshotJson
import infiltrator_android.vpnAuthorizationResult
import kotlinx.coroutines.runBlocking

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
 * The Bevy flavor declares this Activity as its launcher and names the driver
 * with `android.app.lib_name`; the Compose flavor has its own launcher overlay.
 * Both flavors keep the same VPN service and Rust control owner.
 */
class BevyHostActivity : NativeActivity() {
    private var vpnBinding: VpnServiceBinding? = null
    private var nativeHost: NativeHostAdapter? = null
    private var pendingControlIntent: AndroidControlIntent? = null
    private lateinit var vpnAuthorizationFlow: AndroidVpnAuthorizationFlow

    /** JNI entry for the Bevy command sink; the payload is shared Rust enum JSON. */
    @Keep
    fun executeAndroidProductCommandJson(commandJson: String): String =
        runBlocking { androidExecuteProductCommandJson(commandJson) }

    /** Read the current owner-authored VPN/Core lifecycle snapshot for command readback. */
    @Keep
    fun readAndroidVpnOwnerSnapshotJson(): String? =
        MihomoControlClient(this).readVpnOwnerSnapshotJson()

    /** Stop the VPN through the :vpn owner and return its owner-authored terminal receipt. */
    @Keep
    fun stopAndroidVpnServiceCurrent(): String =
        MihomoControlClient(this).stopVpnService()
            ?: throw IllegalStateException("VPN owner returned no stop receipt")

    /** JNI entry for the Bevy reader; data remains authored by the `:vpn` process. */
    @Keep
    fun readAndroidSurfaceSnapshotJson(): String? {
        val result = runBlocking { androidSurfaceSnapshotJson() }
        return if (result.status.code == FfiErrorCode.OK) result.value else null
    }

    /** JNI entry for the typed Activity insets feed consumed by Bevy's UI shell. */
    @Keep
    fun readAndroidWindowInsetsJson(): String? = nativeHost?.currentWindowInsetsJson()

    @Keep
    private external fun nativeOnWindowInsetsJson(windowInsetsJson: String)

    /** Forward only changed host observations into the Bevy driver's Rust channel. */
    @Keep
    fun publishBevyWindowInsetsJson(windowInsetsJson: String) {
        try {
            nativeOnWindowInsetsJson(windowInsetsJson)
        } catch (error: LinkageError) {
            Log.e(TAG, "Bevy window-insets callback failed: ${error.message}", error)
        }
    }

    private val vpnAuthorizationReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            val command = readControlIntent(intent) ?: return
            when (command) {
                AndroidControlIntent.START_PROXY -> requestControl(command)
                AndroidControlIntent.STOP_PROXY,
                AndroidControlIntent.RESUME_KERNEL,
                AndroidControlIntent.PAUSE_KERNEL,
                AndroidControlIntent.RESTART_KERNEL ->
                    Log.w(TAG, "unexpected host authorization request: $command")
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        vpnAuthorizationFlow = AndroidVpnAuthorizationFlow(
            BevyVpnAuthorizationHost(),
            savedVpnAuthorizationFlowState(savedInstanceState),
        )
        handleControlIntent(intent)
        // The `:vpn` process owns the service; this UI process only binds to it.
        vpnBinding = VpnServiceBinding(this, AndroidVpnServiceBindingLog).also { it.bind() }
        // BANDROID-005/008: attach the lifecycle/clipboard host and the Bevy
        // Activity's typed inset snapshot source.
        nativeHost = NativeHostAdapter(
            this,
            NativeInsetsDelivery.BevyActivityFeed(::publishBevyWindowInsetsJson),
        ).also { it.attach() }
    }

    override fun onStart() {
        super.onStart()
        ContextCompat.registerReceiver(
            this,
            vpnAuthorizationReceiver,
            IntentFilter(VpnAuthorizationBroadcast.ACTION),
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )
    }

    override fun onStop() {
        unregisterReceiver(vpnAuthorizationReceiver)
        super.onStop()
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleControlIntent(intent)
        dispatchPendingControlIntent()
    }

    override fun onPostResume() {
        super.onPostResume()
        dispatchPendingControlIntent()
    }

    // NativeActivity is required by android-activity/winit. This override only
    // adapts its platform result into the shared typed authorization flow.
    @Deprecated("NativeActivity result adapter for the shared VPN authorization flow")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode != VPN_CONSENT_REQUEST) return
        vpnAuthorizationFlow.onVpnConsentPromptReturned()
    }

    // NativeActivity does not implement ComponentActivity's Activity Result
    // registry, so this platform callback forwards into the same shared flow.
    @Deprecated("NativeActivity permission adapter for the shared VPN authorization flow")
    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray,
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode != NOTIFICATION_PERMISSION_REQUEST) return
        vpnAuthorizationFlow.onNotificationPermissionPromptReturned(
            AndroidNotificationPermission.fromPermissionResults(grantResults),
        )
    }

    private fun readControlIntent(intent: Intent): AndroidControlIntent? {
        return if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
            intent.getSerializableExtra(
                AndroidControlIntentWire.EXTRA,
                AndroidControlIntent::class.java,
            )
        } else {
            @Suppress("DEPRECATION")
            intent.getSerializableExtra(AndroidControlIntentWire.EXTRA)
                as? AndroidControlIntent
        }
    }

    private fun handleControlIntent(intent: Intent?) {
        pendingControlIntent = intent?.let(::readControlIntent)
    }

    private fun dispatchPendingControlIntent() {
        val command = pendingControlIntent ?: return
        pendingControlIntent = null
        requestControl(command)
    }

    private fun requestControl(command: AndroidControlIntent) {
        when (command) {
            AndroidControlIntent.START_PROXY -> vpnAuthorizationFlow.requestStart()
            AndroidControlIntent.STOP_PROXY,
            AndroidControlIntent.RESUME_KERNEL,
            AndroidControlIntent.PAUSE_KERNEL,
            AndroidControlIntent.RESTART_KERNEL -> dispatchControl(command)
        }
    }

    private fun dispatchControl(command: AndroidControlIntent) {
        Thread({
            val status = try {
                runBlocking { androidControl(command) }
            } catch (error: Exception) {
                runOnUiThread {
                    Toast.makeText(
                        this,
                        error.message ?: "Android control command failed",
                        Toast.LENGTH_LONG,
                    ).show()
                }
                return@Thread
            }
            if (status.code != FfiErrorCode.OK) {
                runOnUiThread {
                    Toast.makeText(
                        this,
                        status.message ?: "Android control command failed",
                        Toast.LENGTH_LONG,
                    ).show()
                }
            }
        }, "musicfrog-bevy-control").start()
    }

    override fun onSaveInstanceState(outState: Bundle) {
        outState.putSerializable(
            VPN_AUTHORIZATION_FLOW_STATE_KEY,
            vpnAuthorizationFlow.stateForSaving(),
        )
        super.onSaveInstanceState(outState)
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

    private inner class BevyVpnAuthorizationHost : AndroidVpnAuthorizationHost {
        override fun authorizationAvailability(): VpnAuthorizationAvailability =
            readVpnAuthorizationAvailability(this@BevyHostActivity)

        override fun launchVpnConsentPrompt(): VpnConsentPromptLaunch {
            return try {
                val permissionIntent = VpnService.prepare(this@BevyHostActivity)
                    ?: return VpnConsentPromptLaunch.AuthorizationAlreadyGranted
                @Suppress("DEPRECATION")
                startActivityForResult(permissionIntent, VPN_CONSENT_REQUEST)
                VpnConsentPromptLaunch.Launched
            } catch (error: Exception) {
                VpnConsentPromptLaunch.Failed(
                    error.message ?: "Failed to request VPN authorization",
                )
            }
        }

        override fun notificationPermissionStatus(): NotificationPermissionStatus = try {
            AndroidNotificationPermission.status(this@BevyHostActivity)
        } catch (_: Exception) {
            NotificationPermissionStatus.NotGranted
        }

        override fun launchNotificationPermissionPrompt(): NotificationPromptLaunch {
            return try {
                if (AndroidNotificationPermission.status(this@BevyHostActivity) !=
                    NotificationPermissionStatus.NotGranted
                ) {
                    return NotificationPromptLaunch.PermissionAlreadyGranted
                }
                requestPermissions(
                    arrayOf(Manifest.permission.POST_NOTIFICATIONS),
                    NOTIFICATION_PERMISSION_REQUEST,
                )
                NotificationPromptLaunch.Launched
            } catch (error: Exception) {
                NotificationPromptLaunch.Failed(
                    error.message ?: "Failed to request notification permission",
                )
            }
        }

        override fun reportVpnAuthorization(result: VpnAuthorizationResult): FfiStatus =
            vpnAuthorizationResult(result)

        override fun onVpnAuthorizationNotGranted(status: FfiStatus) {
            val message = status.message ?: getString(R.string.status_permission_required)
            Log.w(TAG, message)
            Toast.makeText(this@BevyHostActivity, message, Toast.LENGTH_LONG).show()
        }

        override fun onNotificationPermissionNotGranted(result: NotificationPermissionPromptResult) {
            Toast.makeText(
                this@BevyHostActivity,
                R.string.notification_permission_denied,
                Toast.LENGTH_LONG,
            ).show()
        }

        override fun onNotificationPermissionPromptFailed(message: String) {
            Log.w(TAG, "Notification permission prompt could not be launched: $message")
            Toast.makeText(
                this@BevyHostActivity,
                R.string.notification_permission_prompt_failed,
                Toast.LENGTH_LONG,
            ).show()
        }

        override fun dispatchProxyStart(completion: (VpnProxyStartReadback) -> Unit) {
            Thread({
                val result = try {
                    runBlocking {
                        executeVpnStartAndReadOwnerState(
                            (application as MihomoApplication).ownerIpcClient,
                        )
                    }
                } catch (error: Exception) {
                    VpnProxyStartReadback.OwnerStateUnavailable(
                        error.message ?: "Android VPN start request failed",
                    )
                }
                runOnUiThread { completion(result) }
            }, "musicfrog-bevy-vpn-start").start()
        }

        override fun reportFailure(message: String) {
            Log.e(TAG, message)
            Toast.makeText(this@BevyHostActivity, message, Toast.LENGTH_LONG).show()
        }
    }

    companion object {
        private const val TAG = "BevyHostActivity"
        private const val VPN_CONSENT_REQUEST = 0x4d46
        private const val NOTIFICATION_PERMISSION_REQUEST = 0x4d47

        /**
         * Native library packaged by the Bevy host Gradle module and declared
         * as `android.app.lib_name` in the manifest. Kept here so the Activity
         * and the packaging wiring share one source of truth.
         */
        const val DRIVER_LIBRARY = "infiltrator_bevy_android"
    }
}

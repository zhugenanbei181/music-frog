package com.musicfrog.infiltrator

import com.musicfrog.infiltrator.ui.common.toLocalizedFailure
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.localizedFailure
import android.Manifest
import android.content.Intent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.IntentFilter
import android.net.VpnService
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.windowsizeclass.ExperimentalMaterial3WindowSizeClassApi
import androidx.compose.material3.windowsizeclass.calculateWindowSizeClass
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.musicfrog.infiltrator.ui.InfiltratorApp
import com.musicfrog.infiltrator.ui.theme.InfiltratorTheme
import infiltrator_android.AndroidControlIntent
import infiltrator_android.VpnAuthorizationResult
import infiltrator_android.androidControl
import infiltrator_android.vpnAuthorizationResult
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import androidx.lifecycle.lifecycleScope
import androidx.core.content.ContextCompat
import infiltrator_android.FfiStatus
import kotlinx.coroutines.withContext

class MainActivity : AppCompatActivity() {
    private var nativeHost: NativeHostAdapter? = null
    private var vpnBinding: VpnServiceBinding? = null
    private var pendingImportUrl = mutableStateOf<String?>(null)
    private var pendingControlIntent: AndroidControlIntent? = null
    private lateinit var vpnAuthorizationFlow: AndroidVpnAuthorizationFlow

    private val vpnAuthorizationReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            when (readControlIntent(intent)) {
                AndroidControlIntent.START_PROXY -> requestControl(AndroidControlIntent.START_PROXY)
                null -> Unit
                else -> Log.w("VpnAuth", "unexpected control intent in authorization request")
            }
        }
    }

    private val vpnConsentLauncher = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult()
    ) {
        vpnAuthorizationFlow.onVpnConsentPromptReturned()
    }

    private val notificationPermissionLauncher = registerForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        vpnAuthorizationFlow.onNotificationPermissionPromptReturned(
            AndroidNotificationPermission.fromActivityResult(granted),
        )
    }

    @OptIn(ExperimentalMaterial3WindowSizeClassApi::class)
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        vpnAuthorizationFlow = AndroidVpnAuthorizationFlow(
            ComposeVpnAuthorizationHost(),
            savedVpnAuthorizationFlowState(savedInstanceState),
        )
        vpnBinding = VpnServiceBinding(this, AndroidVpnServiceBindingLog).also { it.bind() }
        // BANDROID-005/006/008: push the real Activity lifecycle, insets and
        // clipboard into the shared typed seams.
        try {
        nativeHost = NativeHostAdapter(
            this,
            NativeInsetsDelivery.ComposeRustHost,
        ).also { it.attach() }
        } catch (t: Throwable) {
            Log.e("MainActivity", "Failed to attach NativeHostAdapter", t)
        }

        // Handle the initial Android intent. Commands enter Rust as generated
        // UniFFI enums; the String action channel is reserved for Android VIEW.
        handleIntent(intent)
        
        setContent {
            val windowSizeClass = calculateWindowSizeClass(this)
            val importUrl by pendingImportUrl
            
            val authorizationState by AndroidActivityState.vpnAuthorization.collectAsState()
            
            val requestVpnPermission: () -> Unit = {
                Log.d("VpnAuth", "Checking VPN permission for pkg=${this.packageName} uid=${android.os.Process.myUid()}")
                requestControl(AndroidControlIntent.START_PROXY)
            }

            InfiltratorTheme(darkTheme = isSystemInDarkTheme()) {
                InfiltratorApp(
                    windowSizeClass = windowSizeClass,
                    vpnAuthorization = authorizationState,
                    onRequestVpnPermission = requestVpnPermission,
                    onControlIntent = ::requestControl,
                    pendingImportUrl = importUrl,
                    onImportHandled = { pendingImportUrl.value = null }
                )
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleIntent(intent)
        dispatchPendingControlIntent()
    }

    override fun onResume() {
        super.onResume()
        AndroidActivityState.refreshVpnAuthorization(this)
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

    private fun handleIntent(intent: Intent?) {
        when (intent?.action) {
            Intent.ACTION_VIEW -> {
                val data = intent.data
                if (data?.scheme == "clash" && data.host == "install-config") {
                    val url = data.getQueryParameter("url")
                    if (!url.isNullOrBlank()) {
                        pendingImportUrl.value = url
                    }
                }
            }
        }
        pendingControlIntent = intent?.let(::readControlIntent)
    }

    override fun onPostResume() {
        super.onPostResume()
        dispatchPendingControlIntent()
    }

    private fun readControlIntent(intent: Intent): AndroidControlIntent? {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            intent.getSerializableExtra(
                AndroidControlIntentWire.EXTRA,
                AndroidControlIntent::class.java,
            )
        } else {
            @Suppress("DEPRECATION")
            intent.getSerializableExtra(AndroidControlIntentWire.EXTRA) as? AndroidControlIntent
        }
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
        lifecycleScope.launch(Dispatchers.IO) {
            try {
                val status = androidControl(command)
                if (status.code != infiltrator_android.FfiErrorCode.OK) {
                    AndroidActivityState.reportError(status.toLocalizedFailure())
                }
            } catch (error: Exception) {
                AndroidActivityState.reportError(error.toLocalizedFailure())
            }
        }
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

    private inner class ComposeVpnAuthorizationHost : AndroidVpnAuthorizationHost {
        override fun authorizationAvailability(): VpnAuthorizationAvailability {
            val availability = readVpnAuthorizationAvailability(this@MainActivity)
            AndroidActivityState.applyVpnAuthorizationAvailability(availability)
            return availability
        }

        override fun launchVpnConsentPrompt(): VpnConsentPromptLaunch {
            return try {
                val permissionIntent = VpnService.prepare(this@MainActivity)
                    ?: return VpnConsentPromptLaunch.AuthorizationAlreadyGranted
                vpnConsentLauncher.launch(permissionIntent)
                VpnConsentPromptLaunch.Launched
            } catch (error: Exception) {
                VpnConsentPromptLaunch.Failed(
                    error.message.orEmpty(),
                )
            }
        }

        override fun notificationPermissionStatus(): NotificationPermissionStatus = try {
            AndroidNotificationPermission.status(this@MainActivity)
        } catch (_: Exception) {
            NotificationPermissionStatus.NotGranted
        }

        override fun launchNotificationPermissionPrompt(): NotificationPromptLaunch {
            return try {
                if (AndroidNotificationPermission.status(this@MainActivity) !=
                    NotificationPermissionStatus.NotGranted
                ) {
                    return NotificationPromptLaunch.PermissionAlreadyGranted
                }
                notificationPermissionLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
                NotificationPromptLaunch.Launched
            } catch (error: Exception) {
                NotificationPromptLaunch.Failed(
                    error.message.orEmpty(),
                )
            }
        }

        override fun reportVpnAuthorization(result: VpnAuthorizationResult): FfiStatus {
            AndroidActivityState.applyVpnAuthorizationResult(result)
            return vpnAuthorizationResult(result)
        }

        override fun onVpnAuthorizationNotGranted(status: FfiStatus) {
            AndroidActivityState.reportError(
                status.toLocalizedFailure(),
            )
        }

        override fun onNotificationPermissionNotGranted(result: NotificationPermissionPromptResult) {
            Toast.makeText(
                this@MainActivity,
                R.string.notification_permission_denied,
                Toast.LENGTH_LONG,
            ).show()
        }

        override fun onNotificationPermissionPromptFailed(message: String) {
            Log.w("VpnAuth", "Notification permission prompt could not be launched: $message")
            Toast.makeText(
                this@MainActivity,
                R.string.notification_permission_prompt_failed,
                Toast.LENGTH_LONG,
            ).show()
        }

        override fun dispatchProxyStart(completion: (VpnProxyStartReadback) -> Unit) {
            lifecycleScope.launch(Dispatchers.IO) {
                val result = executeVpnStartAndReadOwnerState(
                    (application as MihomoApplication).ownerIpcClient,
                )
                withContext(Dispatchers.Main.immediate) {
                    completion(result)
                }
            }
        }

        override fun reportFailure(message: String) {
            AndroidActivityState.reportError(localizedFailure(FailureTextKey.Internal, message))
        }
    }

}

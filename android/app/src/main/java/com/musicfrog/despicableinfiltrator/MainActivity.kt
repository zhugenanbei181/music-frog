package com.musicfrog.despicableinfiltrator

import android.content.Intent
import android.net.VpnService
import android.os.Bundle
import android.util.Log
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.windowsizeclass.ExperimentalMaterial3WindowSizeClassApi
import androidx.compose.material3.windowsizeclass.calculateWindowSizeClass
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.platform.LocalContext
import com.musicfrog.despicableinfiltrator.ui.InfiltratorApp
import com.musicfrog.despicableinfiltrator.ui.theme.InfiltratorTheme

class MainActivity : AppCompatActivity() {
    private var bridgeHost: BridgeHost? = null
    private var pendingImportUrl = mutableStateOf<String?>(null)

    @OptIn(ExperimentalMaterial3WindowSizeClassApi::class)
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        initRustBridge()
        
        // Handle initial intent
        handleIntent(intent)
        
        // Register VpnStateManager receiver
        VpnStateManager.register(this)
        
        setContent {
            val windowSizeClass = calculateWindowSizeClass(this)
            val context = LocalContext.current
            val importUrl by pendingImportUrl
            
            // Use VpnStateManager for permission state
            val permissionState by VpnStateManager.permissionState.collectAsState()
            val vpnPermissionGranted = permissionState == VpnStateManager.PermissionState.GRANTED
            
            // Initial permission check
            LaunchedEffect(Unit) {
                VpnStateManager.checkPermission(context)
            }
            
            val launcher = rememberLauncherForActivityResult(
                ActivityResultContracts.StartActivityForResult()
            ) {
                if (VpnStateManager.checkPermission(context)) {
                    VpnStateManager.onPermissionGranted()
                } else {
                    VpnStateManager.onPermissionDenied()
                }
            }
            val requestVpnPermission: () -> Unit = {
                Log.d("VpnAuth", "Checking VPN permission for pkg=${this.packageName} uid=${android.os.Process.myUid()}")
                try {
                    val intent = VpnService.prepare(this)
                    if (intent != null) {
                        Log.d("VpnAuth", "Permission required, launching intent")
                        launcher.launch(intent)
                    } else {
                        Log.d("VpnAuth", "Permission already granted")
                        VpnStateManager.onPermissionGranted()
                    }
                } catch (e: Exception) {
                    Log.e("VpnAuth", "VpnService.prepare failed", e)
                }
            }

            InfiltratorTheme(darkTheme = isSystemInDarkTheme()) {
                InfiltratorApp(
                    windowSizeClass = windowSizeClass,
                    host = bridgeHost as? MihomoHost,
                    vpnPermissionGranted = vpnPermissionGranted,
                    onRequestVpnPermission = requestVpnPermission,
                    pendingImportUrl = importUrl,
                    onImportHandled = { pendingImportUrl.value = null }
                )
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleIntent(intent)
    }

    private fun handleIntent(intent: Intent?) {
        if (intent?.action == Intent.ACTION_VIEW) {
            val data = intent.data
            if (data?.scheme == "clash" && data.host == "install-config") {
                val url = data.getQueryParameter("url")
                if (!url.isNullOrBlank()) {
                    pendingImportUrl.value = url
                }
            }
        }
    }
    
    override fun onDestroy() {
        super.onDestroy()
        VpnStateManager.unregister(this)
    }

    private fun initRustBridge() {
        val initCode = RustBridge.init(filesDir.absolutePath, cacheDir.absolutePath)
        if (initCode != 0) {
            Log.w("RustBridge", "init failed: $initCode")
        }
        val host = MihomoHost(this)
        val registerCode = RustBridge.registerBridge(host)
        if (registerCode != 0) {
            Log.w("RustBridge", "register failed: $registerCode")
        } else {
            bridgeHost = host
            // Auto-start core runtime
            Thread {
                if (host.coreStart()) {
                    Log.i("MihomoCore", "Core runtime auto-started")
                    VpnStateManager.updateCoreState(true)
                } else {
                    Log.e("MihomoCore", "Failed to auto-start core runtime")
                    VpnStateManager.updateCoreState(false)
                }
            }.start()
        }
    }
}
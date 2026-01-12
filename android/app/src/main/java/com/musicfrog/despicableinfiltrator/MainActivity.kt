package com.musicfrog.despicableinfiltrator

import android.net.VpnService
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.windowsizeclass.ExperimentalMaterial3WindowSizeClassApi
import androidx.compose.material3.windowsizeclass.calculateWindowSizeClass
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.platform.LocalContext
import androidx.activity.compose.rememberLauncherForActivityResult
import com.musicfrog.despicableinfiltrator.ui.InfiltratorApp
import com.musicfrog.despicableinfiltrator.ui.theme.InfiltratorTheme

class MainActivity : ComponentActivity() {
    private var bridgeHost: BridgeHost? = null

    @OptIn(ExperimentalMaterial3WindowSizeClassApi::class)
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        initRustBridge()
        
        // Register VpnStateManager receiver
        VpnStateManager.register(this)
        
        setContent {
            val windowSizeClass = calculateWindowSizeClass(this)
            val context = LocalContext.current
            
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
            val requestVpnPermission = {
                val intent = VpnService.prepare(context)
                if (intent != null) {
                    launcher.launch(intent)
                } else {
                    VpnStateManager.onPermissionGranted()
                }
            }

            InfiltratorTheme(darkTheme = isSystemInDarkTheme()) {
                InfiltratorApp(
                    windowSizeClass = windowSizeClass,
                    host = bridgeHost as? MihomoHost,
                    vpnPermissionGranted = vpnPermissionGranted,
                    onRequestVpnPermission = requestVpnPermission
                )
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
        }
    }
}

package io.musicfrog.lurker.template

import android.app.Activity
import android.content.Intent
import android.net.VpnService
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import io.musicfrog.lurker.template.ui.TemplateScreen

class MainActivity : ComponentActivity() {

    private var isVpnRunning by mutableStateOf(false)
    private var routingMode by mutableStateOf("FollowRules")

    private val vpnPrepareLauncher = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult()
    ) { result ->
        if (result.resultCode == Activity.RESULT_OK) {
            launchVpnService()
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            TemplateScreen(
                isRunning = isVpnRunning,
                routingMode = routingMode,
                onStartVpn = { prepareAndStartVpn() },
                onStopVpn = { stopVpnService() },
                onSelectMode = { mode -> routingMode = mode }
            )
        }
    }

    private fun prepareAndStartVpn() {
        val prepareIntent = VpnService.prepare(this)
        if (prepareIntent != null) {
            vpnPrepareLauncher.launch(prepareIntent)
        } else {
            launchVpnService()
        }
    }

    private fun launchVpnService() {
        val intent = Intent(this, LurkerVpnService::class.java).apply {
            action = LurkerVpnService.ACTION_START_VPN
        }
        startService(intent)
        isVpnRunning = true
    }

    private fun stopVpnService() {
        val intent = Intent(this, LurkerVpnService::class.java).apply {
            action = LurkerVpnService.ACTION_STOP_VPN
        }
        startService(intent)
        isVpnRunning = false
    }
}

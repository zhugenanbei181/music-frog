package com.musicfrog.infiltrator

import android.app.PendingIntent
import android.content.Intent
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import androidx.annotation.RequiresApi
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch

@RequiresApi(Build.VERSION_CODES.N)
class InfiltratorTileService : TileService() {
    private val serviceScope = CoroutineScope(Dispatchers.Main + Job())
    private var stateJob: Job? = null

    override fun onStartListening() {
        super.onStartListening()
        stateJob?.cancel()
        stateJob = serviceScope.launch {
            VpnStateManager.vpnState.collectLatest { state ->
                updateTile(state)
            }
        }
    }

    override fun onStopListening() {
        super.onStopListening()
        stateJob?.cancel()
    }

    override fun onClick() {
        super.onClick()
        val currentState = VpnStateManager.vpnState.value
        when (currentState) {
            VpnStateManager.VpnState.RUNNING -> {
                MihomoVpnService.stop(this)
            }
            VpnStateManager.VpnState.STOPPED, VpnStateManager.VpnState.ERROR -> {
                if (VpnStateManager.checkPermission(this)) {
                    MihomoVpnService.start(this)
                } else {
                    // Start activity to request permission
                    val intent = Intent(this, MainActivity::class.java).apply {
                        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                    }
                    
                    val pendingIntent = PendingIntent.getActivity(
                        this,
                        0,
                        intent,
                        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
                    )
                    
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                        startActivityAndCollapse(pendingIntent)
                    } else {
                        // For older versions, we can still use the PendingIntent version if available,
                        // or fall back. TileService has had (PendingIntent) since API 24.
                        startActivityAndCollapse(pendingIntent)
                    }
                }
            }
            else -> {
                // Ignore while starting/stopping
            }
        }
    }

    private fun updateTile(state: VpnStateManager.VpnState) {
        val tile = qsTile ?: return
        when (state) {
            VpnStateManager.VpnState.RUNNING -> {
                tile.state = Tile.STATE_ACTIVE
                tile.label = getString(R.string.status_active)
            }
            VpnStateManager.VpnState.STARTING -> {
                tile.state = Tile.STATE_INACTIVE
                tile.label = getString(R.string.status_starting)
            }
            VpnStateManager.VpnState.STOPPING -> {
                tile.state = Tile.STATE_INACTIVE
                tile.label = getString(R.string.status_stopping)
            }
            else -> {
                tile.state = Tile.STATE_INACTIVE
                tile.label = getString(R.string.status_idle)
            }
        }
        tile.updateTile()
    }
}
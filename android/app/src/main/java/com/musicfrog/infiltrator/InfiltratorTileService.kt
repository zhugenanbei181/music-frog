package com.musicfrog.infiltrator

import android.app.PendingIntent
import android.content.Intent
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import androidx.annotation.RequiresApi
import infiltrator_android.AndroidControlIntent
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

@RequiresApi(Build.VERSION_CODES.N)
class InfiltratorTileService : TileService() {
    private val serviceScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val controlClient by lazy {
        (applicationContext as? MihomoApplication)?.ownerIpcClient
            ?: throw IllegalStateException("MihomoApplication must own the Android control client")
    }
    private var stateJob: Job? = null

    override fun onStartListening() {
        super.onStartListening()
        stateJob?.cancel()
        stateJob = serviceScope.launch { refreshTile() }
    }

    override fun onStopListening() {
        super.onStopListening()
        stateJob?.cancel()
    }

    override fun onDestroy() {
        stateJob?.cancel()
        serviceScope.cancel()
        super.onDestroy()
    }

    override fun onClick() {
        super.onClick()
        serviceScope.launch {
            val state = withContext(Dispatchers.IO) { readOwnerState() } ?: return@launch
            val command = when (state.tileState()) {
                TileVpnState.Active,
                TileVpnState.CleanupPending -> AndroidControlIntent.STOP_PROXY
                TileVpnState.Starting,
                TileVpnState.Stopping,
                TileVpnState.Unavailable -> return@launch
                TileVpnState.Idle -> AndroidControlIntent.START_PROXY
            }
            val intent = Intent().setClassName(
                this@InfiltratorTileService,
                getString(R.string.notification_activity),
            ).apply {
                putExtra(AndroidControlIntentWire.EXTRA, command)
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            val pendingIntent = PendingIntent.getActivity(
                this@InfiltratorTileService,
                command.ordinal,
                intent,
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                startActivityAndCollapse(pendingIntent)
            } else {
                @Suppress("DEPRECATION")
                startActivityAndCollapse(intent)
            }
        }
    }

    private suspend fun refreshTile() {
        val state = withContext(Dispatchers.IO) { readOwnerState() }
        val tile = qsTile ?: return
        when (state?.tileState() ?: TileVpnState.Unavailable) {
            TileVpnState.Active -> {
                tile.state = Tile.STATE_ACTIVE
                tile.label = getString(R.string.status_active)
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                    tile.subtitle = getString(R.string.app_name)
                }
            }
            TileVpnState.Starting -> {
                tile.state = Tile.STATE_INACTIVE
                tile.label = getString(R.string.status_starting)
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) tile.subtitle = null
            }
            TileVpnState.Stopping,
            TileVpnState.CleanupPending -> {
                tile.state = Tile.STATE_INACTIVE
                tile.label = getString(R.string.status_stopping)
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) tile.subtitle = null
            }
            TileVpnState.Idle -> {
                tile.state = Tile.STATE_INACTIVE
                tile.label = getString(R.string.status_idle)
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) tile.subtitle = null
            }
            TileVpnState.Unavailable -> {
                tile.state = Tile.STATE_UNAVAILABLE
                tile.label = getString(R.string.status_unknown)
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) tile.subtitle = null
            }
        }
        tile.updateTile()
    }

    private fun readOwnerState(): VpnOwnerSnapshotWire? = controlClient.readVpnOwnerSnapshot()

    private fun VpnOwnerSnapshotWire.tileState(): TileVpnState = when {
        readiness == VpnOwnerReadinessWire.CleanupPending -> TileVpnState.CleanupPending
        readiness == VpnOwnerReadinessWire.Ready && vpnState == VpnOwnerLifecycleWire.Running ->
            TileVpnState.Active
        readiness == VpnOwnerReadinessWire.Stopping || vpnState == VpnOwnerLifecycleWire.Stopping ->
            TileVpnState.Stopping
        vpnState == VpnOwnerLifecycleWire.StartRequested ||
            vpnState == VpnOwnerLifecycleWire.Starting ||
            vpnState == VpnOwnerLifecycleWire.Foreground ||
            vpnState == VpnOwnerLifecycleWire.TunnelEstablished -> TileVpnState.Starting
        vpnState == VpnOwnerLifecycleWire.PermissionRequired ||
            vpnState == VpnOwnerLifecycleWire.Stopped ||
            vpnState == VpnOwnerLifecycleWire.Revoked -> TileVpnState.Idle
        else -> TileVpnState.Unavailable
    }

    private enum class TileVpnState {
        Active,
        Starting,
        Stopping,
        CleanupPending,
        Idle,
        Unavailable,
    }
}

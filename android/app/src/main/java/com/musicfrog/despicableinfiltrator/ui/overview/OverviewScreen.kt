package com.musicfrog.despicableinfiltrator.ui.overview

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Description
import androidx.compose.material.icons.outlined.PowerSettingsNew
import androidx.compose.material.icons.outlined.Speed
import androidx.compose.material.icons.outlined.Tune
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ElevatedCard
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedCard
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.musicfrog.despicableinfiltrator.MihomoHost
import com.musicfrog.despicableinfiltrator.R
import com.musicfrog.despicableinfiltrator.VpnStateManager
import com.musicfrog.despicableinfiltrator.ui.common.ErrorDialog
import infiltrator_android.LogEntry
import infiltrator_android.LogLevel
import infiltrator_android.TrafficSnapshot

@Composable
fun OverviewScreen(
    host: MihomoHost?,
    vpnPermissionGranted: Boolean,
    onRequestVpnPermission: () -> Unit,
    isExpanded: Boolean,
    viewModel: OverviewViewModel = viewModel()
) {
    val state by viewModel.state.collectAsState()
    val vpnState by VpnStateManager.vpnState.collectAsState()
    val coreRunning by VpnStateManager.coreRunning.collectAsState()
    val vpnErrorMessage by VpnStateManager.errorMessage.collectAsState()
    
    val context = LocalContext.current

    val vpnRunning = vpnState == VpnStateManager.VpnState.RUNNING
    val vpnStarting = vpnState == VpnStateManager.VpnState.STARTING
    val vpnStopping = vpnState == VpnStateManager.VpnState.STOPPING

    val displayError = state.error ?: vpnErrorMessage

    if (displayError != null) {
        ErrorDialog(
            message = displayError,
            onDismiss = { viewModel.clearError() }
        )
    }

    val cards = remember(state, coreRunning, vpnRunning, vpnStarting, vpnStopping, vpnPermissionGranted) {
        overviewCards(
            coreRunning = coreRunning,
            vpnRunning = vpnRunning,
            vpnStarting = vpnStarting,
            vpnStopping = vpnStopping,
            vpnPermissionGranted = vpnPermissionGranted,
            currentMode = state.currentMode,
            traffic = state.traffic,
            trafficLoading = state.trafficLoading,
            logs = state.logs,
            logsLoading = state.logsLoading,
            onRequestVpnPermission = onRequestVpnPermission,
            onToggleVpn = { viewModel.toggleVpn(host) },
            onRestartCore = { viewModel.restartCore(host) },
            onModeChange = { mode -> viewModel.changeMode(mode, context) }
        )
    }

    if (isExpanded) {
        val leftCards = cards.filterIndexed { index, _ -> index % 2 == 0 }
        val rightCards = cards.filterIndexed { index, _ -> index % 2 == 1 }
        Row(
            horizontalArrangement = Arrangement.spacedBy(20.dp),
            modifier = Modifier.fillMaxSize()
        ) {
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(16.dp)
            ) {
                leftCards.forEach { card -> card() }
            }
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(16.dp)
            ) {
                rightCards.forEach { card -> card() }
            }
        }
    } else {
        LazyColumn(
            modifier = Modifier.fillMaxWidth(),
            contentPadding = PaddingValues(bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            items(cards) { card ->
                card()
            }
        }
    }
}

private fun overviewCards(
    coreRunning: Boolean,
    vpnRunning: Boolean,
    vpnStarting: Boolean,
    vpnStopping: Boolean,
    vpnPermissionGranted: Boolean,
    currentMode: String,
    traffic: TrafficSnapshot?,
    trafficLoading: Boolean,
    logs: List<LogEntry>,
    logsLoading: Boolean,
    onRequestVpnPermission: () -> Unit,
    onToggleVpn: () -> Unit,
    onRestartCore: () -> Unit,
    onModeChange: (String) -> Unit
): List<@Composable () -> Unit> {
    return listOf(
        {
            // Compact TUN Service Card with Switch
            val vpnSubtitle = when {
                !vpnPermissionGranted -> stringResource(R.string.status_permission_required)
                vpnStarting -> stringResource(R.string.status_starting)
                vpnStopping -> stringResource(R.string.status_stopping)
                vpnRunning -> stringResource(R.string.status_active)
                else -> stringResource(R.string.status_idle)
            }
            
            // Interaction logic
            val isBusy = vpnStarting || vpnStopping
            val onSwitch = { checked: Boolean ->
                if (!vpnPermissionGranted) {
                    onRequestVpnPermission()
                } else {
                    onToggleVpn()
                }
            }

            OutlinedCard(
                modifier = Modifier.fillMaxWidth(),
                colors = androidx.compose.material3.CardDefaults.outlinedCardColors(
                    containerColor = if (vpnRunning) 
                        MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.4f)
                    else 
                        MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.3f)
                )
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 20.dp, vertical = 16.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Icon(
                                Icons.Outlined.PowerSettingsNew,
                                contentDescription = null,
                                tint = if (vpnRunning) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant
                            )
                            Spacer(modifier = Modifier.width(12.dp))
                            Text(
                                text = stringResource(R.string.status_tun_service),
                                style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold)
                            )
                        }
                        Text(
                            text = vpnSubtitle,
                            style = MaterialTheme.typography.bodyMedium,
                            color = if (vpnRunning) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(start = 36.dp, top = 4.dp)
                        )
                    }
                    
                    Switch(
                        checked = vpnRunning,
                        onCheckedChange = onSwitch,
                        enabled = !isBusy
                    )
                }
            }
        },
        {
            var expanded by remember { mutableStateOf(false) }
            StatusCard(
                title = stringResource(R.string.status_proxy_mode),
                subtitle = currentMode.uppercase(),
                icon = Icons.Outlined.Tune,
                actions = {
                    Box {
                        OutlinedButton(onClick = { expanded = true }) {
                            Text(stringResource(R.string.action_change_mode))
                        }
                        DropdownMenu(
                            expanded = expanded,
                            onDismissRequest = { expanded = false }
                        ) {
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.mode_rule)) },
                                onClick = { onModeChange("rule"); expanded = false }
                            )
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.mode_global)) },
                                onClick = { onModeChange("global"); expanded = false }
                            )
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.mode_direct)) },
                                onClick = { onModeChange("direct"); expanded = false }
                            )
                        }
                    }
                }
            )
        },
        {
            TrafficCard(traffic, trafficLoading)
        },
        {
            // Core Runtime (Hidden unless stopped/error)
            val coreSubtitle = if (coreRunning) stringResource(R.string.status_running) else stringResource(R.string.status_stopped)
            // Only show actions if NOT running
            if (!coreRunning) {
                StatusCard(
                    title = stringResource(R.string.status_core_runtime),
                    subtitle = coreSubtitle,
                    actions = {
                        TextButton(onClick = onRestartCore) {
                            Text(text = stringResource(R.string.action_restart))
                        }
                    }
                )
            } else {
                // If running, we can optionally hide it completely or show a minimalist info
                // User asked to put it above logs. Let's keep it but make it very small if running?
                // Or just standard StatusCard without actions.
                // Let's use a standard card but minimal.
                StatusCard(
                    title = stringResource(R.string.status_core_runtime),
                    subtitle = coreSubtitle
                )
            }
        },
        {
            LogPreviewCard(logs, logsLoading)
        }
    )
}

@Composable
private fun StatusCard(
    title: String,
    subtitle: String,
    icon: ImageVector? = null,
    actions: @Composable (() -> Unit)? = null
) {
    OutlinedCard(
        modifier = Modifier
            .fillMaxWidth()
            .animateContentSize(),
        colors = androidx.compose.material3.CardDefaults.outlinedCardColors(
            containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.3f)
        )
    ) {
        Column(modifier = Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (icon != null) {
                    Icon(
                        icon,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.primary
                    )
                    Spacer(modifier = Modifier.width(12.dp))
                }
                Text(
                    text = title,
                    style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold)
                )
            }
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodyMedium,
                modifier = if (icon != null) Modifier.padding(start = 36.dp) else Modifier
            )
            if (actions != null) {
                Spacer(modifier = Modifier.height(4.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    actions()
                }
            }
        }
    }
}

@Composable
private fun TrafficCard(
    snapshot: TrafficSnapshot?,
    isLoading: Boolean
) {
    OutlinedCard(
        modifier = Modifier
            .fillMaxWidth()
            .animateContentSize(),
        colors = androidx.compose.material3.CardDefaults.outlinedCardColors(
            containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.3f)
        )
    ) {
        Column(
            modifier = Modifier.padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    Icons.Outlined.Speed,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.primary
                )
                Spacer(modifier = Modifier.width(12.dp))
                Text(
                    text = stringResource(R.string.card_traffic),
                    style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold)
                )
            }
            
            if (isLoading || snapshot == null) {
                Text(
                    text = stringResource(R.string.text_loading),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 36.dp)
                )
            } else {
                // Speed row
                Row(
                    modifier = Modifier.fillMaxWidth().padding(start = 36.dp),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Column {
                        Text(
                            text = "↑ ${formatSpeed(snapshot.upRate.toLong())}",
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.primary
                        )
                        Text(
                            text = stringResource(R.string.label_upload),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                    Column(horizontalAlignment = Alignment.End) {
                        Text(
                            text = "↓ ${formatSpeed(snapshot.downRate.toLong())}",
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.secondary
                        )
                        Text(
                            text = stringResource(R.string.label_download),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
                
                // Total row
                Row(
                    modifier = Modifier.fillMaxWidth().padding(start = 36.dp),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(
                        text = stringResource(
                            R.string.label_total,
                            formatBytes(snapshot.upTotal.toLong()),
                            formatBytes(snapshot.downTotal.toLong())
                        ),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Text(
                        text = stringResource(R.string.label_connections, snapshot.connections),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }
        }
    }
}

@Composable
private fun LogPreviewCard(
    logs: List<LogEntry>,
    isLoading: Boolean
) {
    OutlinedCard(
        modifier = Modifier
            .fillMaxWidth()
            .animateContentSize(),
        colors = androidx.compose.material3.CardDefaults.outlinedCardColors(
            containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.3f)
        )
    ) {
        Column(
            modifier = Modifier.padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    Icons.Outlined.Description,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.primary
                )
                Spacer(modifier = Modifier.width(12.dp))
                Text(
                    text = stringResource(R.string.card_recent_logs),
                    style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold)
                )
            }
            
            if (isLoading) {
                Text(
                    text = stringResource(R.string.text_loading),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 36.dp)
                )
            } else if (logs.isEmpty()) {
                Text(
                    text = stringResource(R.string.text_no_logs),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 36.dp)
                )
            } else {
                Column(modifier = Modifier.padding(start = 36.dp)) {
                    logs.reversed().forEach { entry ->
                        val levelColor = when (entry.level) {
                            LogLevel.ERROR -> MaterialTheme.colorScheme.error
                            LogLevel.WARNING -> MaterialTheme.colorScheme.tertiary
                            else -> MaterialTheme.colorScheme.onSurfaceVariant
                        }
                        Text(
                            text = entry.message,
                            style = MaterialTheme.typography.bodySmall,
                            fontFamily = FontFamily.Monospace,
                            fontSize = 11.sp,
                            color = levelColor,
                            maxLines = 1
                        )
                    }
                }
            }
        }
    }
}

private fun formatSpeed(bytesPerSec: Long): String {
    return when {
        bytesPerSec >= 1_000_000_000 -> String.format("%.1f GB/s", bytesPerSec / 1_000_000_000.0)
        bytesPerSec >= 1_000_000 -> String.format("%.1f MB/s", bytesPerSec / 1_000_000.0)
        bytesPerSec >= 1_000 -> String.format("%.1f KB/s", bytesPerSec / 1_000.0)
        else -> "$bytesPerSec B/s"
    }
}

private fun formatBytes(bytes: Long): String {
    return when {
        bytes >= 1_000_000_000 -> String.format("%.1f GB", bytes / 1_000_000_000.0)
        bytes >= 1_000_000 -> String.format("%.1f MB", bytes / 1_000_000.0)
        bytes >= 1_000 -> String.format("%.1f KB", bytes / 1_000.0)
        else -> "$bytes B"
    }
}

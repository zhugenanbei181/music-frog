package com.musicfrog.infiltrator.ui.overview

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.*
import androidx.compose.material.icons.rounded.ArrowDownward
import androidx.compose.material.icons.rounded.ArrowUpward
import androidx.compose.material.icons.rounded.PowerSettingsNew
import androidx.compose.material3.*
import androidx.compose.runtime.*
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
import com.musicfrog.infiltrator.MihomoHost
import com.musicfrog.infiltrator.R
import com.musicfrog.infiltrator.VpnStateManager
import com.musicfrog.infiltrator.ui.common.ErrorDialog
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
            egressIp = state.egressIp,
            egressLocation = state.egressLocation,
            ipLoading = state.ipLoading,
            onRequestVpnPermission = onRequestVpnPermission,
            onToggleVpn = { viewModel.toggleVpn(host) },
            onRestartCore = { viewModel.restartCore(host) },
            onModeChange = { mode -> viewModel.changeMode(mode, context) },
            onRefreshIp = { viewModel.refreshIpCheck() },
        )
    }

    // Fix: Proper padding handling instead of global box
    val contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp)

    if (isExpanded) {
        val leftCards = cards.filterIndexed { index, _ -> index % 2 == 0 }
        val rightCards = cards.filterIndexed { index, _ -> index % 2 == 1 }
        Row(
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            modifier = Modifier
                .fillMaxSize()
                .padding(contentPadding)
        ) {
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                leftCards.forEach { card -> card() }
            }
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                rightCards.forEach { card -> card() }
            }
        }
    } else {
        LazyColumn(
            modifier = Modifier.fillMaxWidth(),
            contentPadding = contentPadding,
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            items(cards) { card ->
                card()
            }
            item { Spacer(modifier = Modifier.height(16.dp)) }
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
    egressIp: String?,
    egressLocation: String?,
    ipLoading: Boolean,
    onRequestVpnPermission: () -> Unit,
    onToggleVpn: () -> Unit,
    onRestartCore: () -> Unit,
    onModeChange: (String) -> Unit,
    onRefreshIp: () -> Unit,
): List<@Composable () -> Unit> {
    return listOf(
        {
            // Hero VPN Card
            VpnHeroCard(
                vpnRunning = vpnRunning,
                vpnStarting = vpnStarting,
                vpnStopping = vpnStopping,
                vpnPermissionGranted = vpnPermissionGranted,
                onRequestVpnPermission = onRequestVpnPermission,
                onToggleVpn = onToggleVpn
            )
        },
        {
            var expanded by remember { mutableStateOf(false) }
            StatusCard(
                title = stringResource(R.string.status_proxy_mode),
                subtitle = currentMode.uppercase(),
                icon = Icons.Outlined.Tune,
                actions = {
                    Box {
                        FilledTonalButton(
                            onClick = { expanded = true },
                            contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp)
                        ) {
                            Text(stringResource(R.string.action_change_mode))
                            Spacer(Modifier.width(8.dp))
                            Icon(Icons.Outlined.ArrowDropDown, null, modifier = Modifier.size(18.dp))
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
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.mode_script)) },
                                onClick = { onModeChange("script"); expanded = false }
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
            // Core Runtime
            if (!coreRunning) {
               Card(
                   colors = CardDefaults.cardColors(
                       containerColor = MaterialTheme.colorScheme.errorContainer,
                       contentColor = MaterialTheme.colorScheme.onErrorContainer
                   )
               ) {
                   Column(
                       modifier = Modifier.padding(16.dp).fillMaxWidth(),
                       verticalArrangement = Arrangement.spacedBy(8.dp)
                   ) {
                       Row(verticalAlignment = Alignment.CenterVertically) {
                           Icon(Icons.Outlined.Warning, null)
                           Spacer(Modifier.width(8.dp))
                           Text(
                               stringResource(R.string.status_core_runtime), 
                               style = MaterialTheme.typography.titleMedium,
                               fontWeight = FontWeight.Bold
                           )
                       }
                       Text(stringResource(R.string.status_stopped))
                       Button(
                           onClick = onRestartCore, 
                           colors = ButtonDefaults.buttonColors(
                               containerColor = MaterialTheme.colorScheme.onErrorContainer,
                               contentColor = MaterialTheme.colorScheme.errorContainer
                           )
                       ) {
                           Text(stringResource(R.string.action_restart))
                       }
                   }
               }
            } else {
                 StatusCard(
                    title = stringResource(R.string.status_core_runtime),
                    subtitle = stringResource(R.string.status_running),
                    icon = Icons.Outlined.Memory
                 )
            }
        },
        {
            val subtitle = when {
                ipLoading -> stringResource(R.string.text_loading)
                egressIp.isNullOrBlank() -> stringResource(R.string.text_ip_unavailable)
                egressLocation.isNullOrBlank() -> stringResource(R.string.label_exit_ip, egressIp)
                else -> "${stringResource(R.string.label_exit_ip, egressIp)}\n${stringResource(R.string.label_ip_location, egressLocation)}"
            }
            StatusCard(
                title = stringResource(R.string.card_network_diagnostics),
                subtitle = subtitle,
                icon = Icons.Outlined.Public,
                actions = {
                    FilledTonalButton(onClick = onRefreshIp) {
                        Text(stringResource(R.string.action_refresh_ip))
                    }
                }
            )
        },
        {
            LogPreviewCard(logs, logsLoading)
        }
    )
}

@Composable
private fun VpnHeroCard(
    vpnRunning: Boolean,
    vpnStarting: Boolean,
    vpnStopping: Boolean,
    vpnPermissionGranted: Boolean,
    onRequestVpnPermission: () -> Unit,
    onToggleVpn: () -> Unit
) {
    val isBusy = vpnStarting || vpnStopping
    
    val containerColor by animateColorAsState(
        targetValue = if (vpnRunning) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surfaceContainerHigh,
        animationSpec = tween(500),
        label = "cardColor"
    )
    
    val contentColor by animateColorAsState(
        targetValue = if (vpnRunning) MaterialTheme.colorScheme.onPrimary else MaterialTheme.colorScheme.onSurface,
        animationSpec = tween(500),
        label = "contentColor"
    )

    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = MaterialTheme.shapes.large,
        colors = CardDefaults.cardColors(
            containerColor = containerColor,
            contentColor = contentColor
        ),
        elevation = CardDefaults.cardElevation(defaultElevation = if (vpnRunning) 8.dp else 2.dp)
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            val statusText = when {
                !vpnPermissionGranted -> stringResource(R.string.status_permission_required)
                vpnStarting -> stringResource(R.string.status_starting)
                vpnStopping -> stringResource(R.string.status_stopping)
                vpnRunning -> stringResource(R.string.status_active)
                else -> stringResource(R.string.status_idle)
            }
            
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                 Column {
                     Text(
                        text = stringResource(R.string.status_tun_service),
                        style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.Bold)
                     )
                     Text(
                        text = statusText,
                        style = MaterialTheme.typography.bodyLarge.copy(
                            color = contentColor.copy(alpha = 0.8f)
                        )
                     )
                 }
                 
                 FilledIconButton(
                     onClick = { 
                        if (!vpnPermissionGranted) onRequestVpnPermission() else onToggleVpn()
                     },
                     enabled = !isBusy,
                     modifier = Modifier.size(64.dp),
                     colors = IconButtonDefaults.filledIconButtonColors(
                         containerColor = if (vpnRunning) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.primary,
                         contentColor = if (vpnRunning) MaterialTheme.colorScheme.onPrimaryContainer else MaterialTheme.colorScheme.onPrimary
                     )
                 ) {
                     Icon(
                         Icons.Rounded.PowerSettingsNew,
                         contentDescription = null,
                         modifier = Modifier.size(32.dp)
                     )
                 }
            }
        }
    }
}

@Composable
private fun StatusCard(
    title: String,
    subtitle: String,
    icon: ImageVector? = null,
    actions: @Composable (() -> Unit)? = null
) {
    ElevatedCard(
        modifier = Modifier.fillMaxWidth().animateContentSize(),
        colors = CardDefaults.elevatedCardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainerLow
        )
    ) {
        Column(modifier = Modifier.padding(20.dp)) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.fillMaxWidth()
            ) {
                if (icon != null) {
                    Icon(
                        icon,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.size(24.dp)
                    )
                    Spacer(modifier = Modifier.width(16.dp))
                }
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = title,
                        style = MaterialTheme.typography.titleMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Text(
                        text = subtitle,
                        style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.SemiBold),
                        color = MaterialTheme.colorScheme.onSurface
                    )
                }
            }
            if (actions != null) {
                Spacer(modifier = Modifier.height(16.dp))
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.End
                ) {
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
    ElevatedCard(
        modifier = Modifier.fillMaxWidth().animateContentSize(),
        colors = CardDefaults.elevatedCardColors(
             containerColor = MaterialTheme.colorScheme.surfaceContainerLow
        )
    ) {
        Column(
            modifier = Modifier.padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
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
                 Box(Modifier.fillMaxWidth().height(80.dp), contentAlignment = Alignment.Center) {
                     CircularProgressIndicator()
                 }
            } else {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceEvenly
                ) {
                    // Upload
                    Column(horizontalAlignment = Alignment.CenterHorizontally) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Icon(Icons.Rounded.ArrowUpward, null, tint = MaterialTheme.colorScheme.secondary, modifier = Modifier.size(16.dp))
                            Text(stringResource(R.string.label_upload), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        Text(
                            text = formatSpeed(snapshot.upRate.toLong()),
                            style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.Bold),
                            color = MaterialTheme.colorScheme.onSurface
                        )
                        Text(
                            text = formatBytes(snapshot.upTotal.toLong()),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                    
                    VerticalDivider(modifier = Modifier.height(60.dp))
                    
                    // Download
                    Column(horizontalAlignment = Alignment.CenterHorizontally) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Icon(Icons.Rounded.ArrowDownward, null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(16.dp))
                            Text(stringResource(R.string.label_download), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        Text(
                            text = formatSpeed(snapshot.downRate.toLong()),
                            style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.Bold),
                            color = MaterialTheme.colorScheme.primary
                        )
                        Text(
                            text = formatBytes(snapshot.downTotal.toLong()),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
                
                HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f))
                
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                     Text(
                        text = stringResource(R.string.label_connections, snapshot.connections),
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.onSurface
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
    ElevatedCard(
        modifier = Modifier.fillMaxWidth().animateContentSize(),
        colors = CardDefaults.elevatedCardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainerLow
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
                    logs.reversed().take(5).forEach { entry ->
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

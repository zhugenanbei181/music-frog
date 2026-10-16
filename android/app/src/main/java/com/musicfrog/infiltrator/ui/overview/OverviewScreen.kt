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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.platform.testTag
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.musicfrog.infiltrator.AndroidActivityState
import com.musicfrog.infiltrator.AndroidVpnAuthorizationUiState
import com.musicfrog.infiltrator.R
import com.musicfrog.infiltrator.ui.common.ErrorDialog
import com.musicfrog.infiltrator.ui.common.toLocalizedFailure
import com.musicfrog.infiltrator.ui.common.asDisplayString
import com.musicfrog.infiltrator.ui.common.LogEntry
import com.musicfrog.infiltrator.ui.common.LogLevel
import com.musicfrog.infiltrator.ui.common.TrafficSnapshot
import com.musicfrog.infiltrator.ui.common.OwnerVpnObservation
import com.musicfrog.infiltrator.VpnOwnerDataPlaneState
import infiltrator_android.AndroidCoreLifecycle
import infiltrator_android.AndroidControlIntent

@Composable
internal fun OverviewScreen(
    vpnAuthorization: AndroidVpnAuthorizationUiState,
    onRequestVpnPermission: () -> Unit,
    onControlIntent: (AndroidControlIntent) -> Unit,
    isExpanded: Boolean,
    viewModel: OverviewViewModel = viewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val activityError by AndroidActivityState.error.collectAsStateWithLifecycle()

    val displayError = state.error ?: state.coreLifecycleError ?: state.vpnStateError ?: activityError

    if (displayError != null) {
        ErrorDialog(
            failure = displayError,
            onDismiss = { viewModel.clearError() }
        )
    }

    val cards = remember(state, vpnAuthorization) {
        overviewCards(
            coreLifecycle = state.coreLifecycle,
            vpnOwnerObservation = state.vpnOwnerObservation,
            vpnAuthorization = vpnAuthorization,
            routingIntent = state.routingIntent,
            traffic = state.traffic,
            logs = state.logs,
            egressIp = state.egressIp,
            onRequestVpnPermission = onRequestVpnPermission,
            onToggleVpn = {
                when (val observation = state.vpnOwnerObservation) {
                    is ObservationState.Loading,
                    ObservationState.Empty,
                    is ObservationState.Unavailable -> Unit
                    is ObservationState.Observed -> when (val vpn = observation.value) {
                        is OwnerVpnObservation.AndroidOwner -> when (vpn.snapshot.dataPlaneState()) {
                            VpnOwnerDataPlaneState.Running ->
                                onControlIntent(AndroidControlIntent.STOP_PROXY)
                            VpnOwnerDataPlaneState.PermissionRequired,
                            VpnOwnerDataPlaneState.Stopped,
                            VpnOwnerDataPlaneState.Revoked -> onRequestVpnPermission()
                            is VpnOwnerDataPlaneState.Failed -> if (
                                canStartVpnSession(observation, vpnAuthorization)
                            ) {
                                onRequestVpnPermission()
                            }
                            VpnOwnerDataPlaneState.Starting,
                            VpnOwnerDataPlaneState.Stopping,
                            is VpnOwnerDataPlaneState.CleanupPending,
                            is VpnOwnerDataPlaneState.Unavailable -> Unit
                        }
                        is OwnerVpnObservation.Unsupported,
                        OwnerVpnObservation.NotApplicable,
                        is OwnerVpnObservation.Unavailable -> Unit
                    }
                }
            },
            onResumeCore = { onControlIntent(AndroidControlIntent.RESUME_KERNEL) },
            onRestartCore = { onControlIntent(AndroidControlIntent.RESTART_KERNEL) },
            onRoutingIntentChange = viewModel::changeRoutingIntent,
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
            modifier = Modifier.fillMaxSize(),
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
    coreLifecycle: ObservationState<AndroidCoreLifecycle>,
    vpnOwnerObservation: ObservationState<OwnerVpnObservation>,
    vpnAuthorization: AndroidVpnAuthorizationUiState,
    routingIntent: RoutingIntentUiState,
    traffic: ObservationState<TrafficSnapshot>,
    logs: ObservationState<List<LogEntry>>,
    egressIp: ObservationState<EgressIpObservation>,
    onRequestVpnPermission: () -> Unit,
    onToggleVpn: () -> Unit,
    onResumeCore: () -> Unit,
    onRestartCore: () -> Unit,
    onRoutingIntentChange: (RoutingIntentOption) -> Unit,
    onRefreshIp: () -> Unit,
): List<@Composable () -> Unit> {
    return listOf(
        {
            // Hero VPN Card
            VpnHeroCard(
                vpnOwnerObservation = vpnOwnerObservation,
                vpnAuthorization = vpnAuthorization,
                onRequestVpnPermission = onRequestVpnPermission,
                onToggleVpn = onToggleVpn
            )
        },
        {
            var expanded by remember { mutableStateOf(false) }
            val currentIntent = when (routingIntent) {
                is RoutingIntentUiState.Ready -> routingIntent.current
                is RoutingIntentUiState.Updating -> routingIntent.current
                is RoutingIntentUiState.Pending -> routingIntent.snapshot.current
                is RoutingIntentUiState.Unsupported -> routingIntent.snapshot.current
                is RoutingIntentUiState.Failed,
                RoutingIntentUiState.Unobserved,
                RoutingIntentUiState.Loading,
                RoutingIntentUiState.Unavailable -> null
            }
            val currentOption = RoutingIntentOption.fromObserved(currentIntent)
            val canChangeIntent = routingIntent is RoutingIntentUiState.Ready
            val subtitle = when {
                routingIntent is RoutingIntentUiState.Updating ->
                    sharedRoutingIntentCopy(infiltrator_android.AndroidRoutingIntentCopyKey.UPDATING)
                routingIntent is RoutingIntentUiState.Pending ->
                    sharedRoutingIntentCopy(infiltrator_android.AndroidRoutingIntentCopyKey.UPDATING)
                routingIntent is RoutingIntentUiState.Unsupported ->
                    sharedRoutingIntentCopy(infiltrator_android.AndroidRoutingIntentCopyKey.UNSUPPORTED)
                routingIntent is RoutingIntentUiState.Failed ->
                    routingIntent.failure?.toLocalizedFailure()?.asDisplayString()
                        ?: sharedRoutingIntentCopy(infiltrator_android.AndroidRoutingIntentCopyKey.UNAVAILABLE)
                routingIntent == RoutingIntentUiState.Unavailable ->
                    sharedRoutingIntentCopy(infiltrator_android.AndroidRoutingIntentCopyKey.UNAVAILABLE)
                currentOption != null -> sharedRoutingIntentCopy(currentOption.copyKey)
                currentIntent is com.musicfrog.infiltrator.ui.common.RoutingIntentObservation.EngineDefined ->
                    sharedEngineDefinedRoutingIntentCopy(currentIntent.extensionId)
                else -> stringResource(R.string.status_unknown)
            }
            StatusCard(
                title = sharedRoutingIntentCopy(infiltrator_android.AndroidRoutingIntentCopyKey.TITLE),
                subtitle = subtitle,
                icon = Icons.Outlined.Tune,
                actions = {
                    Box {
                        FilledTonalButton(
                            onClick = { expanded = true },
                            enabled = canChangeIntent,
                            contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp)
                        ) {
                            Text(sharedRoutingIntentCopy(infiltrator_android.AndroidRoutingIntentCopyKey.CHANGE_ACTION))
                            Spacer(Modifier.width(8.dp))
                            Icon(Icons.Outlined.ArrowDropDown, null, modifier = Modifier.size(18.dp))
                        }
                        DropdownMenu(
                            expanded = expanded,
                            onDismissRequest = { expanded = false }
                        ) {
                            RoutingIntentOption.commonChoices.forEach { intent ->
                                DropdownMenuItem(
                                    text = { Text(sharedRoutingIntentCopy(intent.copyKey)) },
                                    enabled = canChangeIntent && currentOption != intent,
                                    onClick = {
                                        onRoutingIntentChange(intent)
                                        expanded = false
                                    }
                                )
                            }
                        }
                    }
                }
            )
        },
        {
            TrafficCard(traffic)
        },
        {
            CoreRuntimeCard(
                observation = coreLifecycle,
                onResumeCore = onResumeCore,
                onRestartCore = onRestartCore,
            )
        },
        {
            val subtitle = when (egressIp) {
                is ObservationState.Loading -> stringResource(R.string.text_loading)
                ObservationState.Empty,
                is ObservationState.Unavailable -> stringResource(R.string.text_ip_unavailable)
                is ObservationState.Observed -> when {
                    egressIp.value.location == null -> stringResource(
                        R.string.label_exit_ip,
                        egressIp.value.ip,
                    )
                    else -> "${stringResource(R.string.label_exit_ip, egressIp.value.ip)}\n" +
                        stringResource(R.string.label_ip_location, egressIp.value.location)
                }
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
            LogPreviewCard(logs)
        }
    )
}

@Composable
private fun VpnHeroCard(
    vpnOwnerObservation: ObservationState<OwnerVpnObservation>,
    vpnAuthorization: AndroidVpnAuthorizationUiState,
    onRequestVpnPermission: () -> Unit,
    onToggleVpn: () -> Unit
) {
    val observedOwner = (vpnOwnerObservation as? ObservationState.Observed)?.value
        as? OwnerVpnObservation.AndroidOwner
    val observedState = observedOwner?.snapshot?.dataPlaneState()
    val vpnRunning = vpnSessionIsRunning(vpnOwnerObservation, vpnAuthorization)
    val canAct = vpnRunning || canStartVpnSession(vpnOwnerObservation, vpnAuthorization)
    
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
                vpnAuthorization == AndroidVpnAuthorizationUiState.Required ->
                    stringResource(R.string.status_permission_required)
                vpnAuthorization == AndroidVpnAuthorizationUiState.Unknown ->
                    stringResource(R.string.status_unknown)
                vpnAuthorization == AndroidVpnAuthorizationUiState.Unavailable ->
                    stringResource(R.string.status_unknown)
                vpnOwnerObservation is ObservationState.Loading ->
                    stringResource(R.string.text_loading)
                vpnOwnerObservation is ObservationState.Unavailable ||
                    vpnOwnerObservation == ObservationState.Empty -> stringResource(R.string.status_unknown)
                observedState == VpnOwnerDataPlaneState.PermissionRequired ->
                    stringResource(R.string.status_permission_required)
                observedState == VpnOwnerDataPlaneState.Starting -> stringResource(R.string.status_starting)
                observedState == VpnOwnerDataPlaneState.Stopping ||
                    observedState is VpnOwnerDataPlaneState.CleanupPending -> stringResource(R.string.status_stopping)
                vpnRunning -> stringResource(R.string.status_active)
                observedState is VpnOwnerDataPlaneState.Unavailable ||
                    observedState is VpnOwnerDataPlaneState.Failed || observedState == null ->
                    stringResource(R.string.status_unknown)
                else -> stringResource(R.string.status_stopped)
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
                        if (vpnRunning) onToggleVpn() else onRequestVpnPermission()
                     },
                     enabled = canAct,
                     modifier = Modifier.size(64.dp).testTag("vpn-action"),
                     colors = IconButtonDefaults.filledIconButtonColors(
                         containerColor = if (vpnRunning) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.primary,
                         contentColor = if (vpnRunning) MaterialTheme.colorScheme.onPrimaryContainer else MaterialTheme.colorScheme.onPrimary
                     )
                 ) {
                     Icon(
                         Icons.Rounded.PowerSettingsNew,
                         contentDescription = stringResource(
                             if (vpnRunning) R.string.action_stop else R.string.action_start,
                         ),
                         modifier = Modifier.size(32.dp)
                     )
                 }
            }
        }
    }
}

@Composable
private fun CoreRuntimeCard(
    observation: ObservationState<AndroidCoreLifecycle>,
    onResumeCore: () -> Unit,
    onRestartCore: () -> Unit,
) {
    val title = stringResource(R.string.status_core_runtime)
    when (observation) {
        is ObservationState.Loading -> StatusCard(
            title = title,
            subtitle = stringResource(R.string.text_loading),
            icon = Icons.Outlined.Memory,
        )
        ObservationState.Empty,
        is ObservationState.Unavailable -> StatusCard(
            title = title,
            subtitle = stringResource(R.string.status_unknown),
            icon = Icons.Outlined.Warning,
        )
        is ObservationState.Observed -> when (observation.value) {
            AndroidCoreLifecycle.READY,
            AndroidCoreLifecycle.RUNNING -> StatusCard(
                title = title,
                subtitle = stringResource(R.string.status_running),
                icon = Icons.Outlined.Memory,
            )
            AndroidCoreLifecycle.STARTING -> StatusCard(
                title = title,
                subtitle = stringResource(R.string.status_starting),
                icon = Icons.Outlined.Memory,
            )
            AndroidCoreLifecycle.STOPPING -> StatusCard(
                title = title,
                subtitle = stringResource(R.string.status_stopping),
                icon = Icons.Outlined.Memory,
            )
            AndroidCoreLifecycle.STOPPED -> StoppedCoreCard(
                statusLabel = R.string.status_stopped,
                actionLabel = R.string.action_start,
                onAction = onResumeCore,
            )
            AndroidCoreLifecycle.FAILED -> StoppedCoreCard(
                statusLabel = R.string.status_unknown,
                actionLabel = R.string.action_restart,
                onAction = onRestartCore,
            )
        }
    }
}

@Composable
private fun StoppedCoreCard(
    statusLabel: Int,
    actionLabel: Int,
    onAction: () -> Unit,
) {
    Card(
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.errorContainer,
            contentColor = MaterialTheme.colorScheme.onErrorContainer,
        ),
    ) {
        Column(
            modifier = Modifier.padding(16.dp).fillMaxWidth(),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.Outlined.Warning, contentDescription = null)
                Spacer(Modifier.width(8.dp))
                Text(
                    stringResource(R.string.status_core_runtime),
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Bold,
                )
            }
            Text(stringResource(statusLabel))
            Button(
                onClick = onAction,
                colors = ButtonDefaults.buttonColors(
                    containerColor = MaterialTheme.colorScheme.onErrorContainer,
                    contentColor = MaterialTheme.colorScheme.errorContainer,
                ),
            ) {
                Text(stringResource(actionLabel))
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
    observation: ObservationState<TrafficSnapshot>,
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
            
            when (observation) {
                is ObservationState.Loading -> Box(
                    Modifier.fillMaxWidth().height(80.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    CircularProgressIndicator()
                }
                ObservationState.Empty,
                is ObservationState.Unavailable -> Box(
                    Modifier.fillMaxWidth().height(80.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = stringResource(R.string.status_unknown),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                is ObservationState.Observed -> TrafficSnapshotContent(observation.value)
            }
        }
    }
}

@Composable
private fun TrafficSnapshotContent(snapshot: TrafficSnapshot) {
    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
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
                            text = formatBytes(snapshot.upTotal),
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
                            text = formatBytes(snapshot.downTotal),
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

@Composable
private fun LogPreviewCard(
    observation: ObservationState<List<LogEntry>>,
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
            
            when (observation) {
                is ObservationState.Loading -> ObservationLabel(R.string.text_loading)
                ObservationState.Empty -> ObservationLabel(R.string.text_no_logs)
                is ObservationState.Unavailable -> ObservationLabel(R.string.status_unknown)
                is ObservationState.Observed -> Column(modifier = Modifier.padding(start = 36.dp)) {
                    observation.value.reversed().take(5).forEach { entry ->
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

@Composable
private fun ObservationLabel(resourceId: Int) {
    Text(
        text = stringResource(resourceId),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(start = 36.dp),
    )
}

private fun formatSpeed(bytesPerSec: Long): String {
    return when {
        bytesPerSec >= 1_000_000_000 -> String.format("%.1f GB/s", bytesPerSec / 1_000_000_000.0)
        bytesPerSec >= 1_000_000 -> String.format("%.1f MB/s", bytesPerSec / 1_000_000.0)
        bytesPerSec >= 1_000 -> String.format("%.1f KB/s", bytesPerSec / 1_000.0)
        else -> "$bytesPerSec B/s"
    }
}

private fun formatBytes(bytes: ULong?): String {
    if (bytes == null) return "—"
    val value = bytes.toDouble()
    return when {
        value >= 1_000_000_000 -> String.format("%.1f GB", value / 1_000_000_000.0)
        value >= 1_000_000 -> String.format("%.1f MB", value / 1_000_000.0)
        value >= 1_000 -> String.format("%.1f KB", value / 1_000.0)
        else -> "$bytes B"
    }
}

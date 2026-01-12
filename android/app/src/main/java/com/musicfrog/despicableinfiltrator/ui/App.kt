package com.musicfrog.despicableinfiltrator.ui

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
import androidx.compose.material.icons.outlined.CloudSync
import androidx.compose.material.icons.outlined.Description
import androidx.compose.material.icons.outlined.NetworkCheck
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.Shield
import androidx.compose.material.icons.outlined.Storage
import androidx.compose.material.icons.outlined.Tune
import androidx.compose.material3.CenterAlignedTopAppBar
import androidx.compose.material3.ElevatedCard
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationRail
import androidx.compose.material3.NavigationRailItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.windowsizeclass.WindowSizeClass
import androidx.compose.material3.windowsizeclass.WindowWidthSizeClass
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.musicfrog.despicableinfiltrator.MihomoHost
import com.musicfrog.despicableinfiltrator.VpnStateManager

import androidx.compose.material3.OutlinedButton
import infiltrator_android.configPatchMode
import kotlinx.coroutines.launch
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton

// Import updated packages
import com.musicfrog.despicableinfiltrator.ui.profiles.ProfilesScreen
import com.musicfrog.despicableinfiltrator.ui.proxies.ProxiesScreen
import com.musicfrog.despicableinfiltrator.ui.settings.SettingsScreen
import com.musicfrog.despicableinfiltrator.ui.settings.routing.AppRoutingScreen
import com.musicfrog.despicableinfiltrator.ui.settings.dns.DnsScreen
import com.musicfrog.despicableinfiltrator.ui.settings.fakeip.FakeIpScreen
import com.musicfrog.despicableinfiltrator.ui.settings.rules.RulesScreen
import com.musicfrog.despicableinfiltrator.ui.settings.tun.TunScreen
import com.musicfrog.despicableinfiltrator.ui.sync.SyncScreen
import com.musicfrog.despicableinfiltrator.ui.logs.LogsScreen
import com.musicfrog.despicableinfiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.despicableinfiltrator.ui.common.runFfiCall
import com.musicfrog.despicableinfiltrator.ui.common.userMessage
import infiltrator_android.FfiErrorCode
import com.musicfrog.despicableinfiltrator.ui.common.ErrorDialog
import com.musicfrog.despicableinfiltrator.ui.common.showToast
import infiltrator_android.trafficSnapshot
import infiltrator_android.logsGet
import infiltrator_android.logsStartStreaming
import infiltrator_android.LogEntry
import infiltrator_android.LogLevel
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.material.icons.outlined.Speed
import androidx.compose.material.icons.outlined.Article
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun InfiltratorApp(
    windowSizeClass: WindowSizeClass,
    host: MihomoHost?,
    vpnPermissionGranted: Boolean,
    onRequestVpnPermission: () -> Unit
) {
    var section by remember { mutableStateOf(AppSection.Overview) }
    // Settings sub-navigation state
    var settingsSubScreen by remember { mutableStateOf<String?>(null) }

    val navigationType = when (windowSizeClass.widthSizeClass) {
        WindowWidthSizeClass.Expanded -> NavigationType.Rail
        WindowWidthSizeClass.Medium -> NavigationType.Rail
        else -> NavigationType.Bottom
    }
    val overviewExpanded = windowSizeClass.widthSizeClass == WindowWidthSizeClass.Expanded

    // Reset settings nav when switching main tabs
    val onSectionChange = { newSection: AppSection ->
        section = newSection
        if (newSection != AppSection.AppRouting) {
            settingsSubScreen = null
        }
    }

    Scaffold(
        topBar = {
            CenterAlignedTopAppBar(
                title = { 
                    val title = when {
                        section == AppSection.AppRouting && settingsSubScreen != null -> settingsSubScreen!!
                        else -> section.title
                    }
                    Text(text = title) 
                },
                navigationIcon = {
                    if (section == AppSection.AppRouting && settingsSubScreen != null) {
                        IconButton(onClick = { settingsSubScreen = null }) {
                            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                        }
                    }
                }
            )
        },
        bottomBar = {
            if (navigationType == NavigationType.Bottom) {
                NavigationBar {
                    AppSection.values().forEach { item ->
                        NavigationBarItem(
                            selected = item == section,
                            onClick = { onSectionChange(item) },
                            icon = { androidx.compose.material3.Icon(item.icon, null) },
                            label = { Text(text = item.title) }
                        )
                    }
                }
            }
        }
    ) { innerPadding ->
        Row(modifier = Modifier.fillMaxSize().padding(innerPadding)) {
            if (navigationType == NavigationType.Rail) {
                NavigationRail {
                    AppSection.values().forEach { item ->
                        NavigationRailItem(
                            selected = item == section,
                            onClick = { onSectionChange(item) },
                            icon = { androidx.compose.material3.Icon(item.icon, null) },
                            label = { Text(text = item.title) }
                        )
                    }
                }
            }

            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(20.dp)
            ) {
                when (section) {
                    AppSection.Overview -> OverviewScreen(
                        host = host,
                        vpnPermissionGranted = vpnPermissionGranted,
                        onRequestVpnPermission = onRequestVpnPermission,
                        isExpanded = overviewExpanded
                    )
                    AppSection.Profiles -> ProfilesScreen()
                    AppSection.Proxies -> ProxiesScreen()
                    AppSection.AppRouting -> {
                        // Settings & Routing Tab
                        if (settingsSubScreen == null) {
                            SettingsScreen(
                                onNavigateToRouting = { settingsSubScreen = "App Routing" },
                                onNavigateToTun = { settingsSubScreen = "TUN" },
                                onNavigateToDns = { settingsSubScreen = "DNS" },
                                onNavigateToFakeIp = { settingsSubScreen = "Fake-IP" },
                                onNavigateToRules = { settingsSubScreen = "Rules" },
                                onNavigateToLogs = { settingsSubScreen = "Logs" }
                            )
                        } else {
                            when (settingsSubScreen) {
                                "App Routing" -> AppRoutingScreen()
                                "TUN" -> TunScreen()
                                "DNS" -> DnsScreen()
                                "Fake-IP" -> FakeIpScreen()
                                "Rules" -> RulesScreen()
                                "Logs" -> LogsScreen()
                            }
                        }
                    }
                    AppSection.Sync -> SyncScreen() // Added Sync support if enum updated
                }
            }
        }
    }
}


@Composable
private fun OverviewScreen(
    host: MihomoHost?,
    vpnPermissionGranted: Boolean,
    onRequestVpnPermission: () -> Unit,
    isExpanded: Boolean
) {
    // Subscribe to VPN state from StateFlow
    val vpnState by VpnStateManager.vpnState.collectAsState()
    val coreRunning by VpnStateManager.coreRunning.collectAsState()
    val vpnErrorMessage by VpnStateManager.errorMessage.collectAsState()
    
    val vpnRunning = vpnState == VpnStateManager.VpnState.RUNNING
    val vpnStarting = vpnState == VpnStateManager.VpnState.STARTING
    val vpnStopping = vpnState == VpnStateManager.VpnState.STOPPING
    
    val scope = rememberCoroutineScope()
    var currentMode by remember { mutableStateOf("rule") } // Default display
    var errorMessage by remember { mutableStateOf<String?>(null) }
    val context = androidx.compose.ui.platform.LocalContext.current

    // Show VPN error if any
    val displayError = errorMessage ?: vpnErrorMessage

    val contentPadding = PaddingValues(bottom = 24.dp)

    // Helper to change mode
    val changeMode: (String) -> Unit = { mode ->
        scope.launch {
            try {
                val call = runFfiCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) {
                    configPatchMode(mode)
                }
                if (call.error != null) {
                    errorMessage = call.error
                    return@launch
                }
                val status = call.value!!
                if (status.code == FfiErrorCode.OK) {
                    currentMode = mode
                    showToast(context, "Mode switched to $mode")
                } else {
                    errorMessage = status.userMessage("Failed to switch mode")
                }
            } catch (e: Exception) {
                errorMessage = "Failed to switch mode: ${e.message}"
            }
        }
        Unit
    }

    val onToggleCore = {
        scope.launch {
            if (coreRunning) {
                val stopped = host?.coreStop() == true
                if (stopped) {
                    VpnStateManager.updateCoreState(false)
                    showToast(context, "Core stopped")
                } else {
                    errorMessage = "Failed to stop Core"
                }
            } else {
                val started = host?.coreStart() == true
                if (started) {
                    VpnStateManager.updateCoreState(true)
                    showToast(context, "Core started")
                } else {
                    errorMessage = "Failed to start Core"
                }
            }
        }
        Unit
    }

    val onToggleVpn = {
        if (!vpnStarting && !vpnStopping) {
            scope.launch {
                if (vpnRunning) {
                    val stopped = host?.vpnStop() == true
                    if (stopped) {
                        showToast(context, "VPN stopping...")
                    } else {
                        errorMessage = "Failed to stop VPN"
                    }
                } else {
                    val started = host?.vpnStart() == true
                    if (started) {
                        showToast(context, "VPN starting...")
                    } else {
                        errorMessage = "Failed to start VPN"
                    }
                }
            }
        }
        Unit
    }

    if (displayError != null) {
        ErrorDialog(
            message = displayError,
            onDismiss = { 
                errorMessage = null
                VpnStateManager.clearError()
            }
        )
    }

    val cards = overviewCards(
        coreRunning = coreRunning,
        vpnRunning = vpnRunning,
        vpnStarting = vpnStarting,
        vpnStopping = vpnStopping,
        vpnPermissionGranted = vpnPermissionGranted,
        currentMode = currentMode,
        onRequestVpnPermission = onRequestVpnPermission,
        onToggleCore = onToggleCore,
        onToggleVpn = onToggleVpn,
        onModeChange = changeMode
    )

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
            contentPadding = contentPadding,
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
    vpnStarting: Boolean = false,
    vpnStopping: Boolean = false,
    vpnPermissionGranted: Boolean,
    currentMode: String,
    onRequestVpnPermission: () -> Unit,
    onToggleCore: () -> Unit,
    onToggleVpn: () -> Unit,
    onModeChange: (String) -> Unit
): List<@Composable () -> Unit> {
    return listOf(
        {
            StatusCard(
                title = "Core Runtime",
                subtitle = if (coreRunning) "Running" else "Stopped",
                actions = {
                    val label = if (coreRunning) "Stop" else "Start"
                    TextButton(onClick = onToggleCore) {
                        Text(text = label)
                    }
                }
            )
        },
        {
            val vpnSubtitle = when {
                !vpnPermissionGranted -> "Permission required"
                vpnStarting -> "Starting..."
                vpnStopping -> "Stopping..."
                vpnRunning -> "Active"
                else -> "Idle"
            }
            val vpnButtonEnabled = vpnPermissionGranted && !vpnStarting && !vpnStopping
            StatusCard(
                title = "VPN / TUN",
                subtitle = vpnSubtitle,
                actions = {
                    if (!vpnPermissionGranted) {
                        TextButton(onClick = onRequestVpnPermission) {
                            Text(text = "Grant permission")
                        }
                    } else {
                        val label = when {
                            vpnStarting -> "Starting..."
                            vpnStopping -> "Stopping..."
                            vpnRunning -> "Stop"
                            else -> "Start"
                        }
                        TextButton(
                            onClick = onToggleVpn,
                            enabled = vpnButtonEnabled
                        ) {
                            Text(text = label)
                        }
                    }
                }
            )
        },
        {
            var expanded by remember { mutableStateOf(false) }
            StatusCard(
                title = "Proxy Mode",
                subtitle = "Current: ${currentMode.uppercase()}",
                icon = Icons.Outlined.Tune,
                actions = {
                    Box {
                        OutlinedButton(onClick = { expanded = true }) {
                            Text("Change Mode")
                        }
                        DropdownMenu(
                            expanded = expanded,
                            onDismissRequest = { expanded = false }
                        ) {
                            DropdownMenuItem(
                                text = { Text("Rule") },
                                onClick = { onModeChange("rule"); expanded = false }
                            )
                            DropdownMenuItem(
                                text = { Text("Global") },
                                onClick = { onModeChange("global"); expanded = false }
                            )
                            DropdownMenuItem(
                                text = { Text("Direct") },
                                onClick = { onModeChange("direct"); expanded = false }
                            )
                        }
                    }
                }
            )
        },
        {
            TrafficCard()
        },
        {
            LogPreviewCard()
        }
    )
}


@Composable
private fun StatusCard(
    title: String,
    subtitle: String,
    icon: androidx.compose.ui.graphics.vector.ImageVector? = null,
    actions: @Composable (() -> Unit)? = null
) {
    ElevatedCard(
        modifier = Modifier
            .fillMaxWidth()
            .animateContentSize()
    ) {
        Column(modifier = Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (icon != null) {
                    androidx.compose.material3.Icon(
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
                style = MaterialTheme.typography.bodyMedium
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
private fun PlaceholderScreen(title: String, description: String) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(text = title, style = MaterialTheme.typography.displaySmall)
        Text(text = description, style = MaterialTheme.typography.bodyMedium)
    }
}

/**
 * Traffic Card showing real-time upload/download speeds and totals
 */
@Composable
private fun TrafficCard() {
    var upRate by remember { mutableStateOf(0L) }
    var downRate by remember { mutableStateOf(0L) }
    var upTotal by remember { mutableStateOf(0L) }
    var downTotal by remember { mutableStateOf(0L) }
    var connections by remember { mutableStateOf(0) }
    var isLoading by remember { mutableStateOf(true) }
    
    // Refresh traffic every 2 seconds
    LaunchedEffect(Unit) {
        while (true) {
            try {
                val result = trafficSnapshot()
                if (result.status.code == FfiErrorCode.OK && result.snapshot != null) {
                    val snapshot = result.snapshot!!
                    upRate = snapshot.upRate.toLong()
                    downRate = snapshot.downRate.toLong()
                    upTotal = snapshot.upTotal.toLong()
                    downTotal = snapshot.downTotal.toLong()
                    connections = snapshot.connections.toInt()
                }
                isLoading = false
            } catch (e: Exception) {
                // Ignore errors, keep last values
            }
            delay(2000)
        }
    }
    
    ElevatedCard(
        modifier = Modifier
            .fillMaxWidth()
            .animateContentSize()
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
                    text = "Traffic",
                    style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold)
                )
            }
            
            if (isLoading) {
                Text(
                    text = "Loading...",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            } else {
                // Speed row
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Column {
                        Text(
                            text = "↑ ${formatSpeed(upRate)}",
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.primary
                        )
                        Text(
                            text = "Upload",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                    Column(horizontalAlignment = Alignment.End) {
                        Text(
                            text = "↓ ${formatSpeed(downRate)}",
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.secondary
                        )
                        Text(
                            text = "Download",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
                
                // Total row
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(
                        text = "Total: ${formatBytes(upTotal)} ↑ / ${formatBytes(downTotal)} ↓",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Text(
                        text = "$connections conn",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
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

/**
 * Log Preview Card showing recent log entries
 */
@Composable
private fun LogPreviewCard() {
    var logs by remember { mutableStateOf<List<LogEntry>>(emptyList()) }
    var isLoading by remember { mutableStateOf(true) }
    
    // Start streaming and poll for logs
    LaunchedEffect(Unit) {
        logsStartStreaming()
        isLoading = false
        
        while (true) {
            val result = logsGet(5u)
            if (result.status.code == FfiErrorCode.OK) {
                logs = result.entries.takeLast(5)
            }
            delay(3000)
        }
    }
    
    ElevatedCard(
        modifier = Modifier
            .fillMaxWidth()
            .animateContentSize()
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
                    text = "Recent Logs",
                    style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold)
                )
            }
            
            if (isLoading) {
                Text(
                    text = "Loading...",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            } else if (logs.isEmpty()) {
                Text(
                    text = "No logs yet",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            } else {
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

private enum class NavigationType {
    Bottom,
    Rail
}

private enum class AppSection(
    val title: String,
    val icon: ImageVector
) {
    Overview("Overview", Icons.Outlined.Shield),
    Profiles("Profiles", Icons.Outlined.Storage),
    Proxies("Proxies", Icons.Outlined.Tune),
    AppRouting("Settings", Icons.Outlined.Settings),
    Sync("Sync", Icons.Outlined.CloudSync),
}

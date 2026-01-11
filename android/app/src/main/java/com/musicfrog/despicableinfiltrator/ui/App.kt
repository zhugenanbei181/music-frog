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
import com.musicfrog.despicableinfiltrator.ui.sync.SyncScreen

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
                                onNavigateToDns = { settingsSubScreen = "DNS" },
                                onNavigateToFakeIp = { settingsSubScreen = "Fake-IP" },
                                onNavigateToRules = { settingsSubScreen = "Rules" }
                            )
                        } else {
                            when (settingsSubScreen) {
                                "App Routing" -> AppRoutingScreen()
                                "DNS" -> DnsScreen()
                                "Fake-IP" -> FakeIpScreen()
                                "Rules" -> RulesScreen()
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
    var coreRunning by remember { mutableStateOf(host?.coreIsRunning() == true) }
    var vpnRunning by remember { mutableStateOf(host?.vpnIsRunning() == true) }
    val scope = rememberCoroutineScope()
    var currentMode by remember { mutableStateOf("rule") } // Default display
    val context = androidx.compose.ui.platform.LocalContext.current

    val contentPadding = PaddingValues(bottom = 24.dp)

    // Helper to change mode
    val changeMode: (String) -> Unit = { mode ->
        scope.launch {
            try {
                configPatchMode(mode)
                currentMode = mode
                android.widget.Toast.makeText(context, "Mode switched to $mode", android.widget.Toast.LENGTH_SHORT).show()
            } catch (e: Exception) {
                android.widget.Toast.makeText(context, "Failed to switch mode: ${e.message}", android.widget.Toast.LENGTH_SHORT).show()
            }
        }
        Unit
    }

    val onToggleCore = {
        scope.launch {
            if (coreRunning) {
                val stopped = host?.coreStop() == true
                if (stopped) {
                    coreRunning = false
                    android.widget.Toast.makeText(context, "Core stopped", android.widget.Toast.LENGTH_SHORT).show()
                } else {
                    android.widget.Toast.makeText(context, "Failed to stop Core", android.widget.Toast.LENGTH_SHORT).show()
                }
            } else {
                val started = host?.coreStart() == true
                if (started) {
                    coreRunning = true
                    android.widget.Toast.makeText(context, "Core started", android.widget.Toast.LENGTH_SHORT).show()
                } else {
                    android.widget.Toast.makeText(context, "Failed to start Core", android.widget.Toast.LENGTH_SHORT).show()
                }
            }
        }
        Unit
    }

    val onToggleVpn = {
        scope.launch {
            if (vpnRunning) {
                val stopped = host?.vpnStop() == true
                if (stopped) {
                    vpnRunning = false
                    android.widget.Toast.makeText(context, "VPN stopped", android.widget.Toast.LENGTH_SHORT).show()
                } else {
                    android.widget.Toast.makeText(context, "Failed to stop VPN", android.widget.Toast.LENGTH_SHORT).show()
                }
            } else {
                val started = host?.vpnStart() == true
                if (started) {
                    vpnRunning = true
                    android.widget.Toast.makeText(context, "VPN started", android.widget.Toast.LENGTH_SHORT).show()
                } else {
                    android.widget.Toast.makeText(context, "Failed to start VPN", android.widget.Toast.LENGTH_SHORT).show()
                }
            }
        }
        Unit
    }

    val cards = overviewCards(
        coreRunning = coreRunning,
        vpnRunning = vpnRunning,
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
            StatusCard(
                title = "VPN / TUN",
                subtitle = when {
                    !vpnPermissionGranted -> "Permission required"
                    vpnRunning -> "Active"
                    else -> "Idle"
                },
                actions = {
                    if (!vpnPermissionGranted) {
                        TextButton(onClick = onRequestVpnPermission) {
                            Text(text = "Grant permission")
                        }
                    } else {
                        val label = if (vpnRunning) "Stop" else "Start"
                        TextButton(onClick = onToggleVpn) {
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

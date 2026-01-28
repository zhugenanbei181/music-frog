package com.musicfrog.despicableinfiltrator.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.outlined.CloudSync
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.Shield
import androidx.compose.material.icons.outlined.Storage
import androidx.compose.material.icons.outlined.Tune
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationRail
import androidx.compose.material3.NavigationRailItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.windowsizeclass.WindowSizeClass
import androidx.compose.material3.windowsizeclass.WindowWidthSizeClass
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.musicfrog.despicableinfiltrator.MihomoHost
import com.musicfrog.despicableinfiltrator.R
import com.musicfrog.despicableinfiltrator.ui.logs.LogsScreen
import com.musicfrog.despicableinfiltrator.ui.overview.OverviewScreen
import com.musicfrog.despicableinfiltrator.ui.profiles.ProfilesScreen
import com.musicfrog.despicableinfiltrator.ui.proxies.ProxiesScreen
import com.musicfrog.despicableinfiltrator.ui.settings.SettingsScreen
import com.musicfrog.despicableinfiltrator.ui.settings.dns.DnsScreen
import com.musicfrog.despicableinfiltrator.ui.settings.fakeip.FakeIpScreen
import com.musicfrog.despicableinfiltrator.ui.settings.routing.AppRoutingScreen
import com.musicfrog.despicableinfiltrator.ui.settings.rules.RulesScreen
import com.musicfrog.despicableinfiltrator.ui.settings.tun.TunScreen
import com.musicfrog.despicableinfiltrator.ui.sync.SyncScreen

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun InfiltratorApp(
    windowSizeClass: WindowSizeClass,
    host: MihomoHost?,
    vpnPermissionGranted: Boolean,
    onRequestVpnPermission: () -> Unit,
    pendingImportUrl: String? = null,
    onImportHandled: () -> Unit = {}
) {
    var section by remember { mutableStateOf(AppSection.Overview) }
    // Settings sub-navigation state
    var settingsSubScreen by remember { mutableStateOf<String?>(null) }

    // Handle deep link redirect
    LaunchedEffect(pendingImportUrl) {
        if (pendingImportUrl != null) {
            section = AppSection.Profiles
        }
    }

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

    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()

    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        topBar = {
            val titleText = when {
                section == AppSection.AppRouting && settingsSubScreen != null -> settingsSubScreen!!
                else -> stringResource(section.titleRes)
            }
            
            LargeTopAppBar(
                title = { Text(text = titleText) },
                navigationIcon = {
                    if (section == AppSection.AppRouting && settingsSubScreen != null) {
                        IconButton(onClick = { settingsSubScreen = null }) {
                            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                        }
                    }
                },
                scrollBehavior = scrollBehavior
            )
        },
        bottomBar = {
            if (navigationType == NavigationType.Bottom) {
                NavigationBar {
                    AppSection.values().forEach { item ->
                        NavigationBarItem(
                            selected = item == section,
                            onClick = { onSectionChange(item) },
                            icon = { Icon(item.icon, null) },
                            label = { Text(text = stringResource(item.titleRes)) }
                        )
                    }
                }
            }
        }
    ) { innerPadding ->
        Row(modifier = Modifier.fillMaxSize().padding(innerPadding)) {
            if (navigationType == NavigationType.Rail) {
                NavigationRail {
                    Column(
                        modifier = Modifier
                            .fillMaxHeight()
                            .verticalScroll(rememberScrollState()),
                        verticalArrangement = Arrangement.Center
                    ) {
                        AppSection.values().forEach { item ->
                            NavigationRailItem(
                                selected = item == section,
                                onClick = { onSectionChange(item) },
                                icon = { Icon(item.icon, null) },
                                label = { Text(text = stringResource(item.titleRes)) }
                            )
                        }
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
                    AppSection.Profiles -> ProfilesScreen(
                        initialImportUrl = pendingImportUrl,
                        onImportHandled = onImportHandled
                    )
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
                    AppSection.Sync -> SyncScreen()
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
    val titleRes: Int,
    val icon: ImageVector
) {
    Overview(R.string.nav_overview, Icons.Outlined.Shield),
    Profiles(R.string.nav_profiles, Icons.Outlined.Storage),
    Proxies(R.string.nav_proxies, Icons.Outlined.Tune),
    AppRouting(R.string.nav_settings, Icons.Outlined.Settings),
    Sync(R.string.nav_sync, Icons.Outlined.CloudSync),
}

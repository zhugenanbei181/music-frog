package com.musicfrog.infiltrator.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.activity.compose.BackHandler
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationRail
import androidx.compose.material3.NavigationRailItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.windowsizeclass.WindowSizeClass
import androidx.compose.material3.windowsizeclass.WindowWidthSizeClass
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.platform.testTag
import com.musicfrog.infiltrator.R
import com.musicfrog.infiltrator.AndroidVpnAuthorizationUiState
import com.musicfrog.infiltrator.ui.connections.ConnectionsScreen
import com.musicfrog.infiltrator.ui.logs.LogsScreen
import com.musicfrog.infiltrator.ui.overview.OverviewScreen
import com.musicfrog.infiltrator.ui.profiles.ProfilesScreen
import com.musicfrog.infiltrator.ui.proxies.ProxiesScreen
import com.musicfrog.infiltrator.ui.settings.SettingsScreen
import com.musicfrog.infiltrator.ui.settings.dns.DnsScreen
import com.musicfrog.infiltrator.ui.settings.fakeip.FakeIpScreen
import com.musicfrog.infiltrator.ui.settings.routing.AppRoutingScreen
import com.musicfrog.infiltrator.ui.settings.rules.RulesScreen
import com.musicfrog.infiltrator.ui.settings.tun.TunScreen
import com.musicfrog.infiltrator.ui.sync.SyncScreen
import infiltrator_android.AndroidControlIntent

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun InfiltratorApp(
    windowSizeClass: WindowSizeClass,
    vpnAuthorization: AndroidVpnAuthorizationUiState,
    onRequestVpnPermission: () -> Unit,
    onControlIntent: (AndroidControlIntent) -> Unit,
    pendingImportUrl: String? = null,
    onImportHandled: () -> Unit = {}
) {
    var route by rememberSaveable(stateSaver = AppRouteSaver) {
        mutableStateOf<AppRoute>(AppRoute.Main(AppSection.Overview))
    }
    val routeStateHolder = rememberSaveableStateHolder()

    // Handle deep link redirect
    LaunchedEffect(pendingImportUrl) {
        if (pendingImportUrl != null) {
            route = AppRoute.Main(AppSection.Profiles)
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
        route = AppRoute.Main(newSection)
    }

    BackHandler(enabled = route.canGoBack) {
        route = route.back()
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Text(
                        text = stringResource(route.titleRes),
                        modifier = Modifier.testTag("screen-title"),
                    )
                },
                navigationIcon = {
                    if (route is AppRoute.SettingsDetail) {
                        IconButton(onClick = { route = route.back() }) {
                            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                        }
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = androidx.compose.ui.graphics.Color.Transparent,
                    scrolledContainerColor = androidx.compose.ui.graphics.Color.Transparent
                )
            )
        },
        bottomBar = {
            if (navigationType == NavigationType.Bottom && route is AppRoute.Main) {
                NavigationBar {
                    AppSection.values().forEach { item ->
                        NavigationBarItem(
                            selected = item == route.selectedSection,
                            onClick = { onSectionChange(item) },
                            icon = { Icon(item.icon, null) },
                            label = { Text(text = stringResource(item.titleRes)) },
                            modifier = Modifier.testTag("nav_${item.name.lowercase()}"),
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
                                selected = item == route.selectedSection,
                                onClick = { onSectionChange(item) },
                                icon = { Icon(item.icon, null) },
                                label = { Text(text = stringResource(item.titleRes)) },
                                modifier = Modifier.testTag("nav_${item.name.lowercase()}"),
                            )
                        }
                    }
                }
            }

            Box(
                modifier = Modifier
                    .fillMaxSize()
            ) {
                routeStateHolder.SaveableStateProvider(route.saveableStateKey) {
                    when (val activeRoute = route) {
                        is AppRoute.Main -> when (activeRoute.section) {
                            AppSection.Overview -> OverviewScreen(
                                vpnAuthorization = vpnAuthorization,
                                onRequestVpnPermission = onRequestVpnPermission,
                                onControlIntent = onControlIntent,
                                isExpanded = overviewExpanded
                            )
                            AppSection.Profiles -> ProfilesScreen(
                                initialImportUrl = pendingImportUrl,
                                onImportHandled = onImportHandled
                            )
                            AppSection.Proxies -> ProxiesScreen()
                            AppSection.AppRouting -> SettingsScreen(
                                onNavigateToRouting = {
                                    route = AppRoute.SettingsDetail(SettingsDestination.Routing)
                                },
                                onNavigateToTun = {
                                    route = AppRoute.SettingsDetail(SettingsDestination.Tun)
                                },
                                onNavigateToDns = {
                                    route = AppRoute.SettingsDetail(SettingsDestination.Dns)
                                },
                                onNavigateToFakeIp = {
                                    route = AppRoute.SettingsDetail(SettingsDestination.FakeIp)
                                },
                                onNavigateToRules = {
                                    route = AppRoute.SettingsDetail(SettingsDestination.Rules)
                                },
                                onNavigateToConnections = {
                                    route = AppRoute.SettingsDetail(SettingsDestination.Connections)
                                },
                                onNavigateToLogs = {
                                    route = AppRoute.SettingsDetail(SettingsDestination.Logs)
                                }
                            )
                            AppSection.Sync -> SyncScreen()
                        }
                        is AppRoute.SettingsDetail -> when (activeRoute.destination) {
                            SettingsDestination.Routing -> AppRoutingScreen()
                            SettingsDestination.Tun -> TunScreen()
                            SettingsDestination.Dns -> DnsScreen()
                            SettingsDestination.FakeIp -> FakeIpScreen()
                            SettingsDestination.Rules -> RulesScreen()
                            SettingsDestination.Connections -> ConnectionsScreen()
                            SettingsDestination.Logs -> LogsScreen()
                        }
                    }
                }
            }
        }
    }
}

private enum class NavigationType {
    Bottom,
    Rail
}

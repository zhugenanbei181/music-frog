package com.musicfrog.despicableinfiltrator.ui.settings

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ListItem
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

@Composable
fun SettingsScreen(
    onNavigateToRouting: () -> Unit,
    onNavigateToTun: () -> Unit,
    onNavigateToDns: () -> Unit,
    onNavigateToFakeIp: () -> Unit,
    onNavigateToRules: () -> Unit,
    onNavigateToLogs: () -> Unit = {}
) {
    Column(modifier = Modifier.fillMaxSize()) {
        ListItem(
            headlineContent = { Text("App Routing") },
            supportingContent = { Text("Select which apps use the VPN") },
            modifier = Modifier.clickable { onNavigateToRouting() }
        )
        HorizontalDivider()
        ListItem(
            headlineContent = { Text("TUN") },
            supportingContent = { Text("Configure MTU, routes, and DNS for VPN") },
            modifier = Modifier.clickable { onNavigateToTun() }
        )
        HorizontalDivider()
        ListItem(
            headlineContent = { Text("DNS") },
            supportingContent = { Text("Configure DNS servers and policies") },
            modifier = Modifier.clickable { onNavigateToDns() }
        )
        HorizontalDivider()
        ListItem(
            headlineContent = { Text("Fake-IP") },
            supportingContent = { Text("Manage Fake-IP pool and filters") },
            modifier = Modifier.clickable { onNavigateToFakeIp() }
        )
        HorizontalDivider()
        ListItem(
            headlineContent = { Text("Rule Sets") },
            supportingContent = { Text("Manage rule providers and priorities") },
            modifier = Modifier.clickable { onNavigateToRules() }
        )
        HorizontalDivider()
        ListItem(
            headlineContent = { Text("Logs") },
            supportingContent = { Text("View core runtime logs") },
            modifier = Modifier.clickable { onNavigateToLogs() }
        )
    }
}

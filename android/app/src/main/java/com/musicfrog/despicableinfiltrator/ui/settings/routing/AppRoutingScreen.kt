package com.musicfrog.despicableinfiltrator.ui.settings.routing

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.core.graphics.drawable.toBitmap
import androidx.lifecycle.viewmodel.compose.viewModel
import com.musicfrog.despicableinfiltrator.R
import com.musicfrog.despicableinfiltrator.ui.common.StandardListItem

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AppRoutingScreen(viewModel: AppRoutingViewModel = viewModel()) {
    val apps by viewModel.uiState.collectAsState()
    val routingMode by viewModel.routingMode.collectAsState()
    val isLoading by viewModel.isLoading.collectAsState()
    var searchQuery by remember { mutableStateOf("") }
    var selectedTab by remember { mutableStateOf(0) } // 0: User, 1: System
    var expandedMode by remember { mutableStateOf(false) }

    Scaffold { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
        ) {
            // Mode Selection
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(16.dp)
            ) {
                ExposedDropdownMenuBox(
                    expanded = expandedMode,
                    onExpandedChange = { expandedMode = !expandedMode }
                ) {
                    OutlinedTextField(
                        value = getModeLabel(routingMode),
                        onValueChange = {},
                        readOnly = true,
                        label = { Text(stringResource(R.string.routing_mode_label)) },
                        trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = expandedMode) },
                        modifier = Modifier.fillMaxWidth().menuAnchor()
                    )
                    ExposedDropdownMenu(
                        expanded = expandedMode,
                        onDismissRequest = { expandedMode = false }
                    ) {
                        RoutingMode.values().forEach { mode ->
                            DropdownMenuItem(
                                text = { Text(getModeLabel(mode)) },
                                onClick = {
                                    viewModel.setRoutingMode(mode)
                                    expandedMode = false
                                },
                                leadingIcon = if (mode == routingMode) {
                                    { Icon(Icons.Default.Check, null) }
                                } else null
                            )
                        }
                    }
                }
            }

            // Search
            OutlinedTextField(
                value = searchQuery,
                onValueChange = { 
                    searchQuery = it
                    viewModel.search(it)
                },
                label = { Text(stringResource(R.string.search_apps_hint)) },
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 8.dp),
                singleLine = true
            )

            // User/System Tabs
            TabRow(selectedTabIndex = selectedTab) {
                Tab(
                    selected = selectedTab == 0,
                    onClick = { selectedTab = 0 },
                    text = { Text(stringResource(R.string.tab_user_apps)) }
                )
                Tab(
                    selected = selectedTab == 1,
                    onClick = { selectedTab = 1 },
                    text = { Text(stringResource(R.string.tab_system_apps)) }
                )
            }

            if (isLoading) {
                Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    CircularProgressIndicator()
                }
            } else {
                val filteredApps = apps.filter { app ->
                    if (selectedTab == 0) !app.isSystem else app.isSystem
                }

                if (filteredApps.isEmpty()) {
                    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        Text(
                            text = stringResource(R.string.text_no_apps),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                } else {
                    LazyColumn(modifier = Modifier.fillMaxSize()) {
                        items(filteredApps, key = { it.packageName }) { app ->
                            AppRow(
                                app = app,
                                onToggle = { viewModel.toggleApp(app.packageName) },
                                enabled = routingMode != RoutingMode.ProxyAll
                            )
                            HorizontalDivider()
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun AppRow(app: AppItem, onToggle: () -> Unit, enabled: Boolean) {
    StandardListItem(
        headline = app.name,
        supporting = app.packageName,
        leadingContent = {
            if (app.icon != null) {
                Image(
                    bitmap = app.icon.toBitmap().asImageBitmap(),
                    contentDescription = null,
                    modifier = Modifier.size(40.dp)
                )
            }
        },
        trailingContent = {
            Switch(
                checked = app.isSelected,
                onCheckedChange = { onToggle() },
                enabled = enabled
            )
        },
        onClick = if (enabled) onToggle else null
    )
}

@Composable
private fun getModeLabel(mode: RoutingMode): String {
    return when (mode) {
        RoutingMode.ProxyAll -> stringResource(R.string.routing_mode_proxy_all)
        RoutingMode.ProxySelected -> stringResource(R.string.routing_mode_proxy_selected)
        RoutingMode.BypassSelected -> stringResource(R.string.routing_mode_bypass_selected)
    }
}
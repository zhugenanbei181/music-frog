package com.musicfrog.infiltrator.ui.settings.routing

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
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
import androidx.compose.material3.ExposedDropdownMenuAnchorType
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.PrimaryTabRow
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Tab
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
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.core.graphics.drawable.toBitmap
import androidx.lifecycle.viewmodel.compose.viewModel
import infiltrator_android.AndroidAppRoutingMode
import com.musicfrog.infiltrator.R
import com.musicfrog.infiltrator.ui.common.LocalizedFailureText
import com.musicfrog.infiltrator.ui.common.StandardListItem
import com.musicfrog.infiltrator.ui.common.toLocalizedFailure

private enum class RoutingModeMenuState {
    Expanded,
    Collapsed,
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun AppRoutingScreen(viewModel: AppRoutingViewModel = viewModel()) {
    val apps by viewModel.uiState.collectAsState()
    val routingMode by viewModel.routingMode.collectAsState()
    val operation by viewModel.operation.collectAsState()
    val canEditSelection = when (val mode = routingMode) {
        AppRoutingModeState.Unavailable -> false
        is AppRoutingModeState.Available -> operation == AppRoutingOperationState.Ready &&
            mode.freshness == AppRoutingSnapshotFreshness.Current &&
            mode.mode != AndroidAppRoutingMode.PROXY_ALL
    }
    val canChangeMode = when (val mode = routingMode) {
        AppRoutingModeState.Unavailable -> false
        is AppRoutingModeState.Available -> mode.freshness == AppRoutingSnapshotFreshness.Current &&
            operation == AppRoutingOperationState.Ready
    }
    var searchQuery by remember { mutableStateOf("") }
    var selectedTab by remember { mutableStateOf(AppGroup.User) }
    var modeMenuState by remember { mutableStateOf(RoutingModeMenuState.Collapsed) }

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
                    expanded = modeMenuState == RoutingModeMenuState.Expanded,
                    onExpandedChange = { expanded ->
                        modeMenuState = when (expanded) {
                            true -> RoutingModeMenuState.Expanded
                            false -> RoutingModeMenuState.Collapsed
                        }
                    }
                ) {
                    OutlinedTextField(
                        value = when (val mode = routingMode) {
                            AppRoutingModeState.Unavailable -> stringResource(R.string.routing_mode_unavailable)
                            is AppRoutingModeState.Available -> getModeLabel(mode.mode)
                        },
                        onValueChange = {},
                        readOnly = true,
                        label = { Text(stringResource(R.string.routing_mode_label)) },
                        trailingIcon = {
                            ExposedDropdownMenuDefaults.TrailingIcon(
                                expanded = modeMenuState == RoutingModeMenuState.Expanded,
                            )
                        },
                        modifier = Modifier.fillMaxWidth().menuAnchor(ExposedDropdownMenuAnchorType.PrimaryNotEditable),
                        enabled = canChangeMode,
                    )
                    ExposedDropdownMenu(
                        expanded = modeMenuState == RoutingModeMenuState.Expanded,
                        onDismissRequest = { modeMenuState = RoutingModeMenuState.Collapsed }
                    ) {
                        listOf(
                            AndroidAppRoutingMode.PROXY_ALL,
                            AndroidAppRoutingMode.PROXY_SELECTED,
                            AndroidAppRoutingMode.BYPASS_SELECTED,
                        ).forEach { mode ->
                            DropdownMenuItem(
                                text = { Text(getModeLabel(mode)) },
                                onClick = {
                                    viewModel.setRoutingMode(mode)
                                    modeMenuState = RoutingModeMenuState.Collapsed
                                },
                                leadingIcon = when (val currentMode = routingMode) {
                                    AppRoutingModeState.Unavailable -> null
                                    is AppRoutingModeState.Available -> if (currentMode.mode == mode) {
                                        { Icon(Icons.Default.Check, null) }
                                    } else {
                                        null
                                    }
                                }
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
            PrimaryTabRow(
                selectedTabIndex = when (selectedTab) {
                    AppGroup.User -> 0
                    AppGroup.System -> 1
                },
            ) {
                Tab(
                    selected = selectedTab == AppGroup.User,
                    onClick = { selectedTab = AppGroup.User },
                    text = { Text(stringResource(R.string.tab_user_apps)) }
                )
                Tab(
                    selected = selectedTab == AppGroup.System,
                    onClick = { selectedTab = AppGroup.System },
                    text = { Text(stringResource(R.string.tab_system_apps)) }
                )
            }

            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 4.dp),
                horizontalArrangement = Arrangement.End
            ) {
                TextButton(
                    onClick = { viewModel.selectAll(selectedTab) },
                    enabled = canEditSelection
                ) {
                    Text(stringResource(R.string.action_select_all))
                }
                TextButton(
                    onClick = { viewModel.clearAll(selectedTab) },
                    enabled = canEditSelection
                ) {
                    Text(stringResource(R.string.action_clear_all))
                }
            }

            when (val state = operation) {
                AppRoutingOperationState.Loading -> {
                    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        CircularProgressIndicator()
                    }
                }
                is AppRoutingOperationState.Failed -> {
                    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        Column(horizontalAlignment = Alignment.CenterHorizontally) {
                            LocalizedFailureText(state.failure.toLocalizedFailure())
                            TextButton(onClick = viewModel::load) {
                                Text(stringResource(R.string.action_retry))
                            }
                        }
                    }
                }
                is AppRoutingOperationState.Cancelled -> {
                    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        Column(horizontalAlignment = Alignment.CenterHorizontally) {
                            LocalizedFailureText(state.failure.toLocalizedFailure())
                            TextButton(onClick = viewModel::load) {
                                Text(stringResource(R.string.action_retry))
                            }
                        }
                    }
                }
                is AppRoutingOperationState.TimedOut -> {
                    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        Column(horizontalAlignment = Alignment.CenterHorizontally) {
                            LocalizedFailureText(state.failure.toLocalizedFailure())
                            TextButton(onClick = viewModel::load) {
                                Text(stringResource(R.string.action_retry))
                            }
                        }
                    }
                }
                AppRoutingOperationState.Ready,
                AppRoutingOperationState.Updating -> {
                val filteredApps = apps.filter { app ->
                    when (selectedTab) {
                        AppGroup.User -> app.classification == AppClassification.User
                        AppGroup.System -> app.classification == AppClassification.System
                    }
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
                    LazyColumn(
                        modifier = Modifier.fillMaxSize(),
                        contentPadding = androidx.compose.foundation.layout.PaddingValues(bottom = 16.dp)
                    ) {
                        items(filteredApps, key = { it.packageName }) { app ->
                            AppRow(
                                app = app,
                                onToggle = { viewModel.toggleApp(app.packageName) },
                                enabled = canEditSelection
                            )
                            HorizontalDivider()
                        }
                    }
                }
            }
            }
        }
    }
}

@Composable
internal fun AppRow(app: AppItem, onToggle: () -> Unit, enabled: Boolean) {
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
                checked = app.selection == AppPackageSelection.Selected,
                onCheckedChange = { onToggle() },
                enabled = enabled
            )
        },
        onClick = if (enabled) onToggle else null
    )
}

@Composable
private fun getModeLabel(mode: AndroidAppRoutingMode): String {
    return when (mode) {
        AndroidAppRoutingMode.PROXY_ALL -> stringResource(R.string.routing_mode_proxy_all)
        AndroidAppRoutingMode.PROXY_SELECTED -> stringResource(R.string.routing_mode_proxy_selected)
        AndroidAppRoutingMode.BYPASS_SELECTED -> stringResource(R.string.routing_mode_bypass_selected)
    }
}

package com.musicfrog.infiltrator.ui.settings.tun

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.musicfrog.infiltrator.R
import com.musicfrog.infiltrator.ui.common.ErrorDialog
import com.musicfrog.infiltrator.ui.common.InputDialog
import com.musicfrog.infiltrator.ui.common.SelectionDialog
import com.musicfrog.infiltrator.ui.common.StandardListItem
import com.musicfrog.infiltrator.ui.common.toLocalizedFailure

@Composable
internal fun TunScreen(viewModel: TunViewModel = viewModel()) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    var showMtuDialog by remember { mutableStateOf(false) }
    var showStackDialog by remember { mutableStateOf(false) }

    if (showMtuDialog) {
        InputDialog(
            title = stringResource(R.string.label_mtu),
            initialValue = state.draft.mtu,
            onDismiss = { showMtuDialog = false },
            onConfirm = {
                viewModel.updateMtu(it)
                showMtuDialog = false
            },
            isNumeric = true
        )
    }

    if (showStackDialog) {
        SelectionDialog(
            title = stringResource(R.string.label_stack),
            options = listOf(
                "system" to stringResource(R.string.option_stack_system),
                "gvisor" to stringResource(R.string.option_stack_gvisor)
            ),
            selectedOption = state.draft.stack,
            onDismiss = { showStackDialog = false },
            onSelect = {
                viewModel.updateStack(it)
                showStackDialog = false
            }
        )
    }

    Scaffold(
        bottomBar = {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(16.dp),
                horizontalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(12.dp)
            ) {
                Button(
                    onClick = { viewModel.save() },
                    enabled = !state.isBusy,
                    modifier = Modifier.weight(1f)
                ) {
                    Text(stringResource(R.string.action_save))
                }
                TextButton(
                    onClick = { viewModel.load() },
                    enabled = !state.isBusy
                ) {
                    Text(stringResource(R.string.action_reload))
                }
            }
        }
    ) { padding ->
        Box(modifier = Modifier.padding(padding).fillMaxSize()) {
            if (state.isBusy) {
                CircularProgressIndicator(modifier = Modifier.align(Alignment.Center))
            }

            state.failure?.let { failure ->
                ErrorDialog(
                    failure = failure.toLocalizedFailure(),
                    onDismiss = { viewModel.clearError() }
                )
            }

            LazyColumn(
                modifier = Modifier.fillMaxSize(),
            ) {
                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_mtu),
                        supporting = if (state.draft.mtu.isEmpty()) "Default (1500)" else state.draft.mtu,
                        onClick = { if (!state.isBusy) showMtuDialog = true }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_stack),
                        supporting = when (state.draft.stack) {
                            "system" -> stringResource(R.string.option_stack_system)
                            "gvisor" -> stringResource(R.string.option_stack_gvisor)
                            else -> stringResource(R.string.option_stack_auto)
                        },
                        onClick = { if (!state.isBusy) showStackDialog = true }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.tun_auto_route),
                        supporting = stringResource(R.string.tun_auto_route_desc),
                        trailingContent = {
                            Switch(
                                checked = state.draft.autoRoute,
                                onCheckedChange = { viewModel.updateAutoRoute(it) },
                                enabled = !state.isBusy
                            )
                        },
                        onClick = { if (!state.isBusy) viewModel.updateAutoRoute(!state.draft.autoRoute) }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.tun_strict_route),
                        supporting = stringResource(R.string.tun_strict_route_desc),
                        trailingContent = {
                            Switch(
                                checked = state.draft.strictRoute,
                                onCheckedChange = { viewModel.updateStrictRoute(it) },
                                enabled = !state.isBusy
                            )
                        },
                        onClick = { if (!state.isBusy) viewModel.updateStrictRoute(!state.draft.strictRoute) }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.tun_auto_detect_interface),
                        supporting = stringResource(R.string.tun_auto_detect_interface_desc),
                        trailingContent = {
                            Switch(
                                checked = state.draft.autoDetectInterface,
                                onCheckedChange = { viewModel.updateAutoDetectInterface(it) },
                                enabled = !state.isBusy
                            )
                        },
                        onClick = {
                            if (!state.isBusy) {
                                viewModel.updateAutoDetectInterface(!state.draft.autoDetectInterface)
                            }
                        }
                    )
                    HorizontalDivider()
                }

                if (state.operation is TunOperationState.Saved) {
                    item {
                        Box(modifier = Modifier.fillMaxWidth().padding(16.dp), contentAlignment = Alignment.Center) {
                            Text(
                                text = stringResource(R.string.text_saved),
                                color = MaterialTheme.colorScheme.primary,
                                style = MaterialTheme.typography.labelLarge
                            )
                        }
                    }
                }
            }
        }
    }
}

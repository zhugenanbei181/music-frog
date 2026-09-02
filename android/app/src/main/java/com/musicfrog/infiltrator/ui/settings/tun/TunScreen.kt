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
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.musicfrog.infiltrator.R
import com.musicfrog.infiltrator.ui.common.ErrorDialog
import com.musicfrog.infiltrator.ui.common.InputDialog
import com.musicfrog.infiltrator.ui.common.SelectionDialog
import com.musicfrog.infiltrator.ui.common.StandardListItem

@Composable
fun TunScreen(viewModel: TunViewModel = viewModel()) {
    val state by viewModel.state.collectAsState()
    var showMtuDialog by remember { mutableStateOf(false) }
    var showStackDialog by remember { mutableStateOf(false) }
    var showDnsDialog by remember { mutableStateOf(false) }

    if (showMtuDialog) {
        InputDialog(
            title = stringResource(R.string.label_mtu),
            initialValue = state.mtu,
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
                "" to stringResource(R.string.option_stack_auto),
                "system" to stringResource(R.string.option_stack_system),
                "gvisor" to stringResource(R.string.option_stack_gvisor)
            ),
            selectedOption = state.stack,
            onDismiss = { showStackDialog = false },
            onSelect = {
                viewModel.updateStack(it)
                showStackDialog = false
            }
        )
    }

    if (showDnsDialog) {
        InputDialog(
            title = stringResource(R.string.label_dns_servers),
            initialValue = state.dnsServers,
            onDismiss = { showDnsDialog = false },
            onConfirm = {
                viewModel.updateDnsServers(it)
                showDnsDialog = false
            },
            singleLine = false
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
                    enabled = !state.isLoading,
                    modifier = Modifier.weight(1f)
                ) {
                    Text(stringResource(R.string.action_save))
                }
                TextButton(
                    onClick = { viewModel.load() },
                    enabled = !state.isLoading
                ) {
                    Text(stringResource(R.string.action_reload))
                }
            }
        }
    ) { padding ->
        Box(modifier = Modifier.padding(padding).fillMaxSize()) {
            if (state.isLoading) {
                CircularProgressIndicator(modifier = Modifier.align(Alignment.Center))
            }

            if (state.error != null) {
                ErrorDialog(
                    message = state.error ?: "",
                    onDismiss = { viewModel.clearError() }
                )
            }

            LazyColumn(
                modifier = Modifier.fillMaxSize(),
            ) {
                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_mtu),
                        supporting = if (state.mtu.isEmpty()) "Default (1500)" else state.mtu,
                        onClick = { if (!state.isLoading) showMtuDialog = true }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_stack),
                        supporting = when (state.stack) {
                            "system" -> stringResource(R.string.option_stack_system)
                            "gvisor" -> stringResource(R.string.option_stack_gvisor)
                            else -> stringResource(R.string.option_stack_auto)
                        },
                        onClick = { if (!state.isLoading) showStackDialog = true }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.tun_auto_route),
                        supporting = stringResource(R.string.tun_auto_route_desc),
                        trailingContent = {
                            Switch(
                                checked = state.autoRoute,
                                onCheckedChange = { viewModel.updateAutoRoute(it) },
                                enabled = !state.isLoading
                            )
                        },
                        onClick = { if (!state.isLoading) viewModel.updateAutoRoute(!state.autoRoute) }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.tun_strict_route),
                        supporting = stringResource(R.string.tun_strict_route_desc),
                        trailingContent = {
                            Switch(
                                checked = state.strictRoute,
                                onCheckedChange = { viewModel.updateStrictRoute(it) },
                                enabled = !state.isLoading
                            )
                        },
                        onClick = { if (!state.isLoading) viewModel.updateStrictRoute(!state.strictRoute) }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.tun_auto_detect_interface),
                        supporting = stringResource(R.string.tun_auto_detect_interface_desc),
                        trailingContent = {
                            Switch(
                                checked = state.autoDetectInterface,
                                onCheckedChange = { viewModel.updateAutoDetectInterface(it) },
                                enabled = !state.isLoading
                            )
                        },
                        onClick = {
                            if (!state.isLoading) {
                                viewModel.updateAutoDetectInterface(!state.autoDetectInterface)
                            }
                        }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_ipv6),
                        supporting = stringResource(R.string.tun_ipv6_desc),
                        trailingContent = {
                            Switch(
                                checked = state.ipv6,
                                onCheckedChange = { viewModel.updateIpv6(it) },
                                enabled = !state.isLoading
                            )
                        },
                        onClick = { if (!state.isLoading) viewModel.updateIpv6(!state.ipv6) }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_dns_servers),
                        supporting = if (state.dnsServers.isEmpty()) "System DNS" else state.dnsServers.replace("\n", ", "),
                        onClick = { if (!state.isLoading) showDnsDialog = true }
                    )
                    HorizontalDivider()
                }
                
                if (state.saved) {
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

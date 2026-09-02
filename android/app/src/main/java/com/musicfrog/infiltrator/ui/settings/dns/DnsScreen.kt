package com.musicfrog.infiltrator.ui.settings.dns

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
fun DnsScreen(viewModel: DnsViewModel = viewModel()) {
    val state by viewModel.state.collectAsState()
    var showModeDialog by remember { mutableStateOf(false) }
    var showNameserverDialog by remember { mutableStateOf(false) }
    var showDefaultDialog by remember { mutableStateOf(false) }
    var showFallbackDialog by remember { mutableStateOf(false) }
    var showFallbackFilterGeoipDialog by remember { mutableStateOf(false) }
    var showFallbackFilterGeoipCodeDialog by remember { mutableStateOf(false) }
    var showFallbackFilterIpcidrDialog by remember { mutableStateOf(false) }
    var showFallbackFilterDomainDialog by remember { mutableStateOf(false) }
    var showFallbackFilterDomainSuffixDialog by remember { mutableStateOf(false) }

    if (showModeDialog) {
        SelectionDialog(
            title = stringResource(R.string.label_enhanced_mode),
            options = listOf(
                "" to "Disabled",
                "fake-ip" to "Fake-IP",
                "redir-host" to "Redir-Host"
            ),
            selectedOption = state.enhancedMode,
            onDismiss = { showModeDialog = false },
            onSelect = {
                viewModel.updateEnhancedMode(it)
                showModeDialog = false
            }
        )
    }

    if (showNameserverDialog) {
        InputDialog(
            title = stringResource(R.string.label_nameserver),
            initialValue = state.nameserver,
            onDismiss = { showNameserverDialog = false },
            onConfirm = {
                viewModel.updateNameserver(it)
                showNameserverDialog = false
            },
            singleLine = false
        )
    }

    if (showDefaultDialog) {
        InputDialog(
            title = stringResource(R.string.label_default_nameserver),
            initialValue = state.defaultNameserver,
            onDismiss = { showDefaultDialog = false },
            onConfirm = {
                viewModel.updateDefaultNameserver(it)
                showDefaultDialog = false
            },
            singleLine = false
        )
    }

    if (showFallbackDialog) {
        InputDialog(
            title = stringResource(R.string.label_fallback),
            initialValue = state.fallback,
            onDismiss = { showFallbackDialog = false },
            onConfirm = {
                viewModel.updateFallback(it)
                showFallbackDialog = false
            },
            singleLine = false
        )
    }

    if (showFallbackFilterGeoipDialog) {
        SelectionDialog(
            title = stringResource(R.string.label_fallback_filter_geoip),
            options = listOf(
                "" to stringResource(R.string.option_auto),
                "true" to stringResource(R.string.option_enabled),
                "false" to stringResource(R.string.option_disabled)
            ),
            selectedOption = state.fallbackFilterGeoip,
            onDismiss = { showFallbackFilterGeoipDialog = false },
            onSelect = {
                viewModel.updateFallbackFilterGeoip(it)
                showFallbackFilterGeoipDialog = false
            }
        )
    }

    if (showFallbackFilterGeoipCodeDialog) {
        InputDialog(
            title = stringResource(R.string.label_fallback_filter_geoip_code),
            initialValue = state.fallbackFilterGeoipCode,
            onDismiss = { showFallbackFilterGeoipCodeDialog = false },
            onConfirm = {
                viewModel.updateFallbackFilterGeoipCode(it)
                showFallbackFilterGeoipCodeDialog = false
            }
        )
    }

    if (showFallbackFilterIpcidrDialog) {
        InputDialog(
            title = stringResource(R.string.label_fallback_filter_ipcidr),
            initialValue = state.fallbackFilterIpcidr,
            onDismiss = { showFallbackFilterIpcidrDialog = false },
            onConfirm = {
                viewModel.updateFallbackFilterIpcidr(it)
                showFallbackFilterIpcidrDialog = false
            },
            singleLine = false
        )
    }

    if (showFallbackFilterDomainDialog) {
        InputDialog(
            title = stringResource(R.string.label_fallback_filter_domain),
            initialValue = state.fallbackFilterDomain,
            onDismiss = { showFallbackFilterDomainDialog = false },
            onConfirm = {
                viewModel.updateFallbackFilterDomain(it)
                showFallbackFilterDomainDialog = false
            },
            singleLine = false
        )
    }

    if (showFallbackFilterDomainSuffixDialog) {
        InputDialog(
            title = stringResource(R.string.label_fallback_filter_domain_suffix),
            initialValue = state.fallbackFilterDomainSuffix,
            onDismiss = { showFallbackFilterDomainSuffixDialog = false },
            onConfirm = {
                viewModel.updateFallbackFilterDomainSuffix(it)
                showFallbackFilterDomainSuffixDialog = false
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

            LazyColumn(modifier = Modifier.fillMaxSize()) {
                item {
                    StandardListItem(
                        headline = stringResource(R.string.dns_enable),
                        supporting = stringResource(R.string.dns_enable_desc),
                        trailingContent = {
                            Switch(
                                checked = state.enabled,
                                onCheckedChange = { viewModel.updateEnabled(it) },
                                enabled = !state.isLoading
                            )
                        },
                        onClick = { if (!state.isLoading) viewModel.updateEnabled(!state.enabled) }
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
                        headline = stringResource(R.string.label_enhanced_mode),
                        supporting = state.enhancedMode.ifBlank { "Disabled" },
                        onClick = { if (!state.isLoading) showModeDialog = true }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_nameserver),
                        supporting = state.nameserver.replace("\n", ", "),
                        onClick = { if (!state.isLoading) showNameserverDialog = true }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_default_nameserver),
                        supporting = state.defaultNameserver.replace("\n", ", "),
                        onClick = { if (!state.isLoading) showDefaultDialog = true }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_fallback),
                        supporting = state.fallback.replace("\n", ", "),
                        onClick = { if (!state.isLoading) showFallbackDialog = true }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_fallback_filter_geoip),
                        supporting = when (state.fallbackFilterGeoip) {
                            "true" -> stringResource(R.string.option_enabled)
                            "false" -> stringResource(R.string.option_disabled)
                            else -> stringResource(R.string.option_auto)
                        },
                        onClick = { if (!state.isLoading) showFallbackFilterGeoipDialog = true }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_fallback_filter_geoip_code),
                        supporting = state.fallbackFilterGeoipCode,
                        onClick = { if (!state.isLoading) showFallbackFilterGeoipCodeDialog = true }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_fallback_filter_ipcidr),
                        supporting = state.fallbackFilterIpcidr.replace("\n", ", "),
                        onClick = { if (!state.isLoading) showFallbackFilterIpcidrDialog = true }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_fallback_filter_domain),
                        supporting = state.fallbackFilterDomain.replace("\n", ", "),
                        onClick = { if (!state.isLoading) showFallbackFilterDomainDialog = true }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_fallback_filter_domain_suffix),
                        supporting = state.fallbackFilterDomainSuffix.replace("\n", ", "),
                        onClick = { if (!state.isLoading) showFallbackFilterDomainSuffixDialog = true }
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

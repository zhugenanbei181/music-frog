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
import com.musicfrog.infiltrator.ui.common.LocalizedFailureText
import com.musicfrog.infiltrator.ui.common.SelectionDialog
import com.musicfrog.infiltrator.ui.common.StandardListItem
import com.musicfrog.infiltrator.ui.common.toLocalizedFailure

@Composable
internal fun DnsScreen(viewModel: DnsViewModel = viewModel()) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val observation = state.observation
    val operationFailure = when (val operation = state.operation) {
        is DnsSettingsOperationState.Failed -> operation.failure
        is DnsSettingsOperationState.Unconfirmed -> operation.failure
        is DnsSettingsOperationState.Cancelled -> operation.failure
        is DnsSettingsOperationState.TimedOut -> operation.failure
        DnsSettingsOperationState.Idle,
        DnsSettingsOperationState.Reading,
        DnsSettingsOperationState.Saving,
        is DnsSettingsOperationState.Saved -> null
    }
    var dialog by remember { mutableStateOf(DnsDialogState.Closed) }

    if (dialog == DnsDialogState.EnhancedMode) {
        SelectionDialog(
            title = stringResource(R.string.label_enhanced_mode),
            options = listOf(
                "" to stringResource(R.string.enhanced_mode_disabled),
                "fake-ip" to stringResource(R.string.enhanced_mode_fake_ip),
                "redir-host" to stringResource(R.string.enhanced_mode_redir_host),
            ),
            selectedOption = state.enhancedMode.selectionValue(),
            onDismiss = { dialog = DnsDialogState.Closed },
            onSelect = {
                viewModel.updateEnhancedMode(dnsEnhancedModeFromSelection(it))
                dialog = DnsDialogState.Closed
            }
        )
    }

    if (dialog == DnsDialogState.Nameserver) {
        InputDialog(
            title = stringResource(R.string.label_nameserver),
            initialValue = state.nameserver,
            onDismiss = { dialog = DnsDialogState.Closed },
            onConfirm = {
                viewModel.updateNameserver(it)
                dialog = DnsDialogState.Closed
            },
            singleLine = false
        )
    }

    if (dialog == DnsDialogState.DefaultNameserver) {
        InputDialog(
            title = stringResource(R.string.label_default_nameserver),
            initialValue = state.defaultNameserver,
            onDismiss = { dialog = DnsDialogState.Closed },
            onConfirm = {
                viewModel.updateDefaultNameserver(it)
                dialog = DnsDialogState.Closed
            },
            singleLine = false
        )
    }

    if (dialog == DnsDialogState.Fallback) {
        InputDialog(
            title = stringResource(R.string.label_fallback),
            initialValue = state.fallback,
            onDismiss = { dialog = DnsDialogState.Closed },
            onConfirm = {
                viewModel.updateFallback(it)
                dialog = DnsDialogState.Closed
            },
            singleLine = false
        )
    }

    if (dialog == DnsDialogState.FallbackGeoIp) {
        SelectionDialog(
            title = stringResource(R.string.label_fallback_filter_geoip),
            options = listOf(
                "" to stringResource(R.string.option_auto),
                "true" to stringResource(R.string.option_enabled),
                "false" to stringResource(R.string.option_disabled)
            ),
            selectedOption = state.fallbackFilterGeoip.selectionValue,
            onDismiss = { dialog = DnsDialogState.Closed },
            onSelect = {
                viewModel.updateFallbackFilterGeoip(DnsGeoIpPolicy.fromSelection(it))
                dialog = DnsDialogState.Closed
            }
        )
    }

    if (dialog == DnsDialogState.FallbackGeoIpCode) {
        InputDialog(
            title = stringResource(R.string.label_fallback_filter_geoip_code),
            initialValue = state.fallbackFilterGeoipCode,
            onDismiss = { dialog = DnsDialogState.Closed },
            onConfirm = {
                viewModel.updateFallbackFilterGeoipCode(it)
                dialog = DnsDialogState.Closed
            }
        )
    }

    if (dialog == DnsDialogState.FallbackIpcidr) {
        InputDialog(
            title = stringResource(R.string.label_fallback_filter_ipcidr),
            initialValue = state.fallbackFilterIpcidr,
            onDismiss = { dialog = DnsDialogState.Closed },
            onConfirm = {
                viewModel.updateFallbackFilterIpcidr(it)
                dialog = DnsDialogState.Closed
            },
            singleLine = false
        )
    }

    if (dialog == DnsDialogState.FallbackDomain) {
        InputDialog(
            title = stringResource(R.string.label_fallback_filter_domain),
            initialValue = state.fallbackFilterDomain,
            onDismiss = { dialog = DnsDialogState.Closed },
            onConfirm = {
                viewModel.updateFallbackFilterDomain(it)
                dialog = DnsDialogState.Closed
            },
            singleLine = false
        )
    }

    if (dialog == DnsDialogState.FallbackDomainSuffix) {
        InputDialog(
            title = stringResource(R.string.label_fallback_filter_domain_suffix),
            initialValue = state.fallbackFilterDomainSuffix,
            onDismiss = { dialog = DnsDialogState.Closed },
            onConfirm = {
                viewModel.updateFallbackFilterDomainSuffix(it)
                dialog = DnsDialogState.Closed
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
                    enabled = state.activity == DnsActivity.Interactive &&
                        state.availability == DnsSettingsAvailability.Available,
                    modifier = Modifier.weight(1f)
                ) {
                    Text(stringResource(R.string.action_save))
                }
                TextButton(
                    onClick = { viewModel.load() },
                    enabled = state.activity == DnsActivity.Interactive
                ) {
                    Text(stringResource(R.string.action_reload))
                }
            }
        }
    ) { padding ->
        Box(modifier = Modifier.padding(padding).fillMaxSize()) {
            if (operationFailure != null) {
                ErrorDialog(
                    failure = operationFailure.toLocalizedFailure(),
                    onDismiss = { viewModel.clearError() }
                )
            }

            if (state.availability == DnsSettingsAvailability.Available) {
                if (state.activity == DnsActivity.Busy) {
                    CircularProgressIndicator(modifier = Modifier.align(Alignment.Center))
                }
                when (val read = observation) {
                    is DnsSettingsReadState.Unavailable -> LocalizedFailureText(
                        failure = read.failure.toLocalizedFailure(),
                        modifier = Modifier.align(Alignment.TopCenter).padding(16.dp),
                    )
                    is DnsSettingsReadState.Loading,
                    is DnsSettingsReadState.Observed -> Unit
                }
                LazyColumn(modifier = Modifier.fillMaxSize()) {
                item {
                    StandardListItem(
                        headline = stringResource(R.string.dns_enable),
                        supporting = if (state.enabled == DnsToggleState.Unspecified) {
                            stringResource(R.string.setting_not_explicit)
                        } else {
                            stringResource(R.string.dns_enable_desc)
                        },
                        trailingContent = {
                            Switch(
                                checked = state.enabled == DnsToggleState.Enabled,
                                onCheckedChange = {
                                    viewModel.updateEnabled(
                                        if (it) DnsToggleState.Enabled else DnsToggleState.Disabled,
                                    )
                                },
                                enabled = state.activity == DnsActivity.Interactive &&
                                    state.availability == DnsSettingsAvailability.Available
                            )
                        },
                        onClick = {
                            if (state.activity == DnsActivity.Interactive &&
                                state.availability == DnsSettingsAvailability.Available
                            ) {
                                viewModel.updateEnabled(
                                    if (state.enabled == DnsToggleState.Enabled) {
                                        DnsToggleState.Disabled
                                    } else {
                                        DnsToggleState.Enabled
                                    },
                                )
                            }
                        }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_ipv6),
                        supporting = if (state.ipv6 == DnsToggleState.Unspecified) {
                            stringResource(R.string.setting_not_explicit)
                        } else {
                            stringResource(R.string.tun_ipv6_desc)
                        },
                        trailingContent = {
                            Switch(
                                checked = state.ipv6 == DnsToggleState.Enabled,
                                onCheckedChange = {
                                    viewModel.updateIpv6(
                                        if (it) DnsToggleState.Enabled else DnsToggleState.Disabled,
                                    )
                                },
                                enabled = state.activity == DnsActivity.Interactive &&
                                    state.availability == DnsSettingsAvailability.Available
                            )
                        },
                        onClick = {
                            if (state.activity == DnsActivity.Interactive &&
                                state.availability == DnsSettingsAvailability.Available
                            ) {
                                viewModel.updateIpv6(
                                    if (state.ipv6 == DnsToggleState.Enabled) {
                                        DnsToggleState.Disabled
                                    } else {
                                        DnsToggleState.Enabled
                                    },
                                )
                            }
                        }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_enhanced_mode),
                        supporting = when (val mode = state.enhancedMode) {
                            DnsEnhancedMode.Unspecified -> stringResource(R.string.setting_not_explicit)
                            DnsEnhancedMode.Disabled -> stringResource(R.string.enhanced_mode_disabled)
                            DnsEnhancedMode.FakeIp -> stringResource(R.string.enhanced_mode_fake_ip)
                            DnsEnhancedMode.RedirHost -> stringResource(R.string.enhanced_mode_redir_host)
                            is DnsEnhancedMode.Custom -> mode.value
                        },
                        onClick = {
                            if (state.activity == DnsActivity.Interactive &&
                                state.availability == DnsSettingsAvailability.Available
                            ) dialog = DnsDialogState.EnhancedMode
                        }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_nameserver),
                        supporting = state.nameserver.replace("\n", ", "),
                        onClick = {
                            if (state.activity == DnsActivity.Interactive &&
                                state.availability == DnsSettingsAvailability.Available
                            ) dialog = DnsDialogState.Nameserver
                        }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_default_nameserver),
                        supporting = state.defaultNameserver.replace("\n", ", "),
                        onClick = {
                            if (state.activity == DnsActivity.Interactive &&
                                state.availability == DnsSettingsAvailability.Available
                            ) dialog = DnsDialogState.DefaultNameserver
                        }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_fallback),
                        supporting = state.fallback.replace("\n", ", "),
                        onClick = {
                            if (state.activity == DnsActivity.Interactive &&
                                state.availability == DnsSettingsAvailability.Available
                            ) dialog = DnsDialogState.Fallback
                        }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_fallback_filter_geoip),
                        supporting = when (state.fallbackFilterGeoip) {
                            DnsGeoIpPolicy.Enabled -> stringResource(R.string.option_enabled)
                            DnsGeoIpPolicy.Disabled -> stringResource(R.string.option_disabled)
                            DnsGeoIpPolicy.Automatic -> stringResource(R.string.option_auto)
                        },
                        onClick = {
                            if (state.activity == DnsActivity.Interactive &&
                                state.availability == DnsSettingsAvailability.Available
                            ) dialog = DnsDialogState.FallbackGeoIp
                        }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_fallback_filter_geoip_code),
                        supporting = state.fallbackFilterGeoipCode,
                        onClick = {
                            if (state.activity == DnsActivity.Interactive &&
                                state.availability == DnsSettingsAvailability.Available
                            ) dialog = DnsDialogState.FallbackGeoIpCode
                        }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_fallback_filter_ipcidr),
                        supporting = state.fallbackFilterIpcidr.replace("\n", ", "),
                        onClick = {
                            if (state.activity == DnsActivity.Interactive &&
                                state.availability == DnsSettingsAvailability.Available
                            ) dialog = DnsDialogState.FallbackIpcidr
                        }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_fallback_filter_domain),
                        supporting = state.fallbackFilterDomain.replace("\n", ", "),
                        onClick = {
                            if (state.activity == DnsActivity.Interactive &&
                                state.availability == DnsSettingsAvailability.Available
                            ) dialog = DnsDialogState.FallbackDomain
                        }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_fallback_filter_domain_suffix),
                        supporting = state.fallbackFilterDomainSuffix.replace("\n", ", "),
                        onClick = {
                            if (state.activity == DnsActivity.Interactive &&
                                state.availability == DnsSettingsAvailability.Available
                            ) dialog = DnsDialogState.FallbackDomainSuffix
                        }
                    )
                    HorizontalDivider()
                }
                
                if (state.operation is DnsSettingsOperationState.Saved) {
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
            } else {
                Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    when (val observation = state.observation) {
                        is DnsSettingsReadState.Loading -> CircularProgressIndicator()
                        is DnsSettingsReadState.Unavailable -> LocalizedFailureText(
                            failure = observation.failure.toLocalizedFailure(),
                        )
                        is DnsSettingsReadState.Observed -> Text(stringResource(R.string.status_unknown))
                    }
                }
            }
        }
    }
}

private enum class DnsDialogState {
    Closed,
    EnhancedMode,
    Nameserver,
    DefaultNameserver,
    Fallback,
    FallbackGeoIp,
    FallbackGeoIpCode,
    FallbackIpcidr,
    FallbackDomain,
    FallbackDomainSuffix,
}

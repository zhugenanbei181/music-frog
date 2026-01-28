package com.musicfrog.despicableinfiltrator.ui.settings.dns

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.musicfrog.despicableinfiltrator.R
import com.musicfrog.despicableinfiltrator.ui.common.ErrorDialog
import com.musicfrog.despicableinfiltrator.ui.common.StandardListItem

@Composable
fun DnsScreen(viewModel: DnsViewModel = viewModel()) {
    val state by viewModel.state.collectAsState()

    Scaffold(
        bottomBar = {
            if (state.saved) {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(16.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = stringResource(R.string.text_saved),
                        color = MaterialTheme.colorScheme.primary,
                        style = MaterialTheme.typography.labelLarge
                    )
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
                    OutlinedTextField(
                        value = state.enhancedMode,
                        onValueChange = { viewModel.updateEnhancedMode(it) },
                        label = { Text(stringResource(R.string.label_enhanced_mode)) },
                        supportingText = { Text(stringResource(R.string.desc_enhanced_mode)) },
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(16.dp),
                        enabled = !state.isLoading,
                        singleLine = true
                    )
                }

                item {
                    OutlinedTextField(
                        value = state.nameserver,
                        onValueChange = { viewModel.updateNameserver(it) },
                        label = { Text(stringResource(R.string.label_nameserver)) },
                        supportingText = { Text(stringResource(R.string.desc_one_per_line)) },
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 16.dp),
                        enabled = !state.isLoading,
                        minLines = 3,
                        maxLines = 6
                    )
                }

                item {
                    OutlinedTextField(
                        value = state.defaultNameserver,
                        onValueChange = { viewModel.updateDefaultNameserver(it) },
                        label = { Text(stringResource(R.string.label_default_nameserver)) },
                        supportingText = { Text(stringResource(R.string.desc_one_per_line)) },
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(16.dp),
                        enabled = !state.isLoading,
                        minLines = 3,
                        maxLines = 6
                    )
                }

                item {
                    OutlinedTextField(
                        value = state.fallback,
                        onValueChange = { viewModel.updateFallback(it) },
                        label = { Text(stringResource(R.string.label_fallback)) },
                        supportingText = { Text(stringResource(R.string.desc_one_per_line)) },
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 16.dp),
                        enabled = !state.isLoading,
                        minLines = 3,
                        maxLines = 6
                    )
                }

                item {
                    androidx.compose.foundation.layout.Row(
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
            }
        }
    }
}

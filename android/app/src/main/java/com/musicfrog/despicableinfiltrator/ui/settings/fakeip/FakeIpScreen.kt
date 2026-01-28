package com.musicfrog.despicableinfiltrator.ui.settings.fakeip

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
fun FakeIpScreen(viewModel: FakeIpViewModel = viewModel()) {
    val state by viewModel.state.collectAsState()

    Scaffold(
        bottomBar = {
            if (state.saved || state.cacheMessage != null) {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(16.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = state.cacheMessage ?: stringResource(R.string.text_saved),
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
                        headline = stringResource(R.string.fakeip_store),
                        supporting = stringResource(R.string.fakeip_store_desc),
                        trailingContent = {
                            Switch(
                                checked = state.storeFakeIp,
                                onCheckedChange = { viewModel.updateStoreFakeIp(it) },
                                enabled = !state.isLoading
                            )
                        },
                        onClick = { if (!state.isLoading) viewModel.updateStoreFakeIp(!state.storeFakeIp) }
                    )
                    HorizontalDivider()
                }

                item {
                    OutlinedTextField(
                        value = state.fakeIpRange,
                        onValueChange = { viewModel.updateRange(it) },
                        label = { Text(stringResource(R.string.label_fakeip_range)) },
                        supportingText = { Text(stringResource(R.string.desc_cidr)) },
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(16.dp),
                        enabled = !state.isLoading,
                        singleLine = true
                    )
                }

                item {
                    OutlinedTextField(
                        value = state.fakeIpFilter,
                        onValueChange = { viewModel.updateFilter(it) },
                        label = { Text(stringResource(R.string.label_fakeip_filter)) },
                        supportingText = { Text(stringResource(R.string.desc_fakeip_filter)) },
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

                item {
                    androidx.compose.foundation.layout.Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 16.dp),
                        horizontalArrangement = androidx.compose.foundation.layout.Arrangement.Center
                    ) {
                        TextButton(
                            onClick = { viewModel.clearCache() },
                            enabled = !state.isLoading,
                            colors = androidx.compose.material3.ButtonDefaults.textButtonColors(
                                contentColor = MaterialTheme.colorScheme.error
                            )
                        ) {
                            Text(stringResource(R.string.action_clear_cache))
                        }
                    }
                }
            }
        }
    }
}

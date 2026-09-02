package com.musicfrog.infiltrator.ui.settings.fakeip

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
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
import com.musicfrog.infiltrator.ui.common.StandardListItem

@Composable
fun FakeIpScreen(viewModel: FakeIpViewModel = viewModel()) {
    val state by viewModel.state.collectAsState()
    var showRangeDialog by remember { mutableStateOf(false) }
    var showFilterDialog by remember { mutableStateOf(false) }

    if (showRangeDialog) {
        InputDialog(
            title = stringResource(R.string.label_fakeip_range),
            initialValue = state.fakeIpRange,
            onDismiss = { showRangeDialog = false },
            onConfirm = {
                viewModel.updateRange(it)
                showRangeDialog = false
            }
        )
    }

    if (showFilterDialog) {
        InputDialog(
            title = stringResource(R.string.label_fakeip_filter),
            initialValue = state.fakeIpFilter,
            onDismiss = { showFilterDialog = false },
            onConfirm = {
                viewModel.updateFilter(it)
                showFilterDialog = false
            },
            singleLine = false
        )
    }

    Scaffold(
        bottomBar = {
            Column(modifier = Modifier.padding(16.dp)) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
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
                TextButton(
                    onClick = { viewModel.clearCache() },
                    enabled = !state.isLoading,
                    modifier = Modifier.align(Alignment.CenterHorizontally),
                    colors = androidx.compose.material3.ButtonDefaults.textButtonColors(
                        contentColor = MaterialTheme.colorScheme.error
                    )
                ) {
                    Text(stringResource(R.string.action_clear_cache))
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
                    StandardListItem(
                        headline = stringResource(R.string.label_fakeip_range),
                        supporting = state.fakeIpRange.ifBlank { "Default (198.18.0.1/16)" },
                        onClick = { if (!state.isLoading) showRangeDialog = true }
                    )
                    HorizontalDivider()
                }

                item {
                    StandardListItem(
                        headline = stringResource(R.string.label_fakeip_filter),
                        supporting = state.fakeIpFilter.replace("\n", ", "),
                        onClick = { if (!state.isLoading) showFilterDialog = true }
                    )
                    HorizontalDivider()
                }
                
                if (state.saved || state.cacheMessage != null) {
                    item {
                        Box(modifier = Modifier.fillMaxWidth().padding(16.dp), contentAlignment = Alignment.Center) {
                            Text(
                                text = state.cacheMessage ?: stringResource(R.string.text_saved),
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

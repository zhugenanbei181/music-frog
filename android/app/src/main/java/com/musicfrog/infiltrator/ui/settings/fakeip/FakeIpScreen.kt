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
import com.musicfrog.infiltrator.ui.common.StandardListItem
import com.musicfrog.infiltrator.ui.common.toLocalizedFailure

@Composable
internal fun FakeIpScreen(viewModel: FakeIpViewModel = viewModel()) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val observation = state.observation
    val settingsBusy = state.settingsActivity == FakeIpSettingsActivity.Busy
    val cacheBusy = state.hostCacheOperation is HostFakeIpFileCacheState.Clearing
    val canEdit = lastAcceptedFakeIp(state.observation) != null
    val operationFailure = when (val operation = state.settingsOperation) {
        is FakeIpSettingsOperationState.Failed -> operation.failure

        is FakeIpSettingsOperationState.Cancelled -> operation.failure

        is FakeIpSettingsOperationState.TimedOut -> operation.failure

        is FakeIpSettingsOperationState.Unconfirmed -> operation.failure

        FakeIpSettingsOperationState.Idle,
        FakeIpSettingsOperationState.Reading,
        FakeIpSettingsOperationState.Saving,
        is FakeIpSettingsOperationState.Saved,
        -> (state.hostCacheOperation as? HostFakeIpFileCacheState.Failed)?.failure
    }
    val resultText = when (state.hostCacheOperation) {
        HostFakeIpFileCacheState.Cleared -> stringResource(R.string.fakeip_host_cache_cleared)

        HostFakeIpFileCacheState.AlreadyEmpty -> stringResource(R.string.fakeip_host_cache_already_empty)

        HostFakeIpFileCacheState.Idle,
        HostFakeIpFileCacheState.Clearing,
        is HostFakeIpFileCacheState.Failed,
        -> null
    } ?: if (state.settingsOperation is FakeIpSettingsOperationState.Saved) {
        stringResource(R.string.text_saved)
    } else {
        null
    }
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
            },
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
            singleLine = false,
        )
    }

    Scaffold(
        bottomBar = {
            Column(modifier = Modifier.padding(16.dp)) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(12.dp),
                ) {
                    Button(
                        onClick = { viewModel.save() },
                        enabled = canEdit && !settingsBusy,
                        modifier = Modifier.weight(1f),
                    ) {
                        Text(stringResource(R.string.action_save))
                    }
                    TextButton(
                        onClick = { viewModel.load() },
                        enabled = !settingsBusy,
                    ) {
                        Text(stringResource(R.string.action_reload))
                    }
                }
                TextButton(
                    onClick = { viewModel.clearHostFakeIpFileCache() },
                    enabled = !cacheBusy,
                    modifier = Modifier.align(Alignment.CenterHorizontally),
                    colors = androidx.compose.material3.ButtonDefaults.textButtonColors(
                        contentColor = MaterialTheme.colorScheme.error,
                    ),
                ) {
                    Text(stringResource(R.string.action_clear_host_fakeip_file_cache))
                }
            }
        },
    ) { padding ->
        Box(modifier = Modifier.padding(padding).fillMaxSize()) {
            if (operationFailure != null) {
                ErrorDialog(
                    failure = operationFailure.toLocalizedFailure(),
                    onDismiss = { viewModel.clearError() },
                )
            }

            if (canEdit) {
                if (settingsBusy || cacheBusy) {
                    CircularProgressIndicator(modifier = Modifier.align(Alignment.Center))
                }
                when (val read = observation) {
                    is FakeIpSettingsReadState.Unavailable -> LocalizedFailureText(
                        failure = read.failure.toLocalizedFailure(),
                        modifier = Modifier.align(Alignment.TopCenter).padding(16.dp),
                    )

                    is FakeIpSettingsReadState.Loading,
                    is FakeIpSettingsReadState.Observed,
                    -> Unit
                }
                LazyColumn(modifier = Modifier.fillMaxSize()) {
                    item {
                        StandardListItem(
                            headline = stringResource(R.string.fakeip_store),
                            supporting = if (state.storeFakeIp == FakeIpBooleanSetting.Unspecified) {
                                stringResource(R.string.setting_not_explicit)
                            } else {
                                stringResource(R.string.fakeip_store_desc)
                            },
                            trailingContent = {
                                Switch(
                                    checked = state.storeFakeIp == FakeIpBooleanSetting.Enabled,
                                    onCheckedChange = {
                                        viewModel.updateStoreFakeIp(
                                            if (it) FakeIpBooleanSetting.Enabled else FakeIpBooleanSetting.Disabled,
                                        )
                                    },
                                    enabled = canEdit && !settingsBusy,
                                )
                            },
                            onClick = {
                                if (canEdit && !settingsBusy) {
                                    viewModel.updateStoreFakeIp(
                                        if (state.storeFakeIp == FakeIpBooleanSetting.Enabled) {
                                            FakeIpBooleanSetting.Disabled
                                        } else {
                                            FakeIpBooleanSetting.Enabled
                                        },
                                    )
                                }
                            },
                        )
                        HorizontalDivider()
                    }

                    item {
                        StandardListItem(
                            headline = stringResource(R.string.label_fakeip_range),
                            supporting = state.fakeIpRange.ifBlank { stringResource(R.string.fakeip_default_range) },
                            onClick = { if (canEdit && !settingsBusy) showRangeDialog = true },
                        )
                        HorizontalDivider()
                    }

                    item {
                        StandardListItem(
                            headline = stringResource(R.string.label_fakeip_filter),
                            supporting = state.fakeIpFilter.replace("\n", ", "),
                            onClick = { if (canEdit && !settingsBusy) showFilterDialog = true },
                        )
                        HorizontalDivider()
                    }

                    if (resultText != null) {
                        item {
                            Box(modifier = Modifier.fillMaxWidth().padding(16.dp), contentAlignment = Alignment.Center) {
                                Text(
                                    text = resultText,
                                    color = MaterialTheme.colorScheme.primary,
                                    style = MaterialTheme.typography.labelLarge,
                                )
                            }
                        }
                    }
                }
            } else {
                Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    when (val observation = state.observation) {
                        is FakeIpSettingsReadState.Loading -> CircularProgressIndicator()

                        is FakeIpSettingsReadState.Unavailable -> LocalizedFailureText(
                            failure = observation.failure.toLocalizedFailure(),
                        )

                        is FakeIpSettingsReadState.Observed -> Text(stringResource(R.string.status_unknown))
                    }
                }
            }
        }
    }
}

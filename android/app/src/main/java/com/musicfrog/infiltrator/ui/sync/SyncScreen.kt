package com.musicfrog.infiltrator.ui.sync

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.musicfrog.infiltrator.R
import com.musicfrog.infiltrator.ui.common.ErrorDialog

@Composable
fun SyncScreen() {
    val viewModel = remember { SyncViewModel() }
    val state by viewModel.state.collectAsStateWithLifecycle()
    val form = state.settings.displayForm()
    val draft = form?.draft ?: SyncSettingsDraft(
        enabled = false,
        url = "",
        username = "",
        password = "",
        intervalMinutes = "60",
        syncOnStartup = false,
    )
    val inputEnabled = state.inputAvailability() == SyncInputAvailability.Enabled
    val reloadEnabled = state.requestAvailability() == SyncRequestAvailability.Available

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        item {
            Text(text = stringResource(R.string.title_webdav_sync), style = MaterialTheme.typography.titleLarge)
        }

        when (state.screenActivity()) {
            SyncScreenActivity.Loading,
            SyncScreenActivity.Busy -> item {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    CircularProgressIndicator(modifier = Modifier.height(20.dp))
                    Spacer(modifier = Modifier.width(8.dp))
                    Text(text = stringResource(R.string.text_sync_loading))
                }
            }
            SyncScreenActivity.Ready,
            SyncScreenActivity.Unavailable -> Unit
        }

        when (val action = state.action) {
            is SyncActionState.Failed -> item {
                ErrorDialog(failure = action.failure, onDismiss = viewModel::clearError)
            }
            is SyncActionState.Cancelled -> item {
                ErrorDialog(failure = action.failure, onDismiss = viewModel::clearError)
            }
            is SyncActionState.TimedOut -> item {
                ErrorDialog(failure = action.failure, onDismiss = viewModel::clearError)
            }
            is SyncActionState.Unconfirmed -> item {
                ErrorDialog(failure = action.failure, onDismiss = viewModel::clearError)
            }
            SyncActionState.Idle,
            is SyncActionState.Working,
            SyncActionState.Saved,
            SyncActionState.ConnectionTested,
            is SyncActionState.Synchronized -> Unit
        }

        item {
            androidx.compose.material3.ElevatedCard(
                colors = androidx.compose.material3.CardDefaults.elevatedCardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceContainerLow,
                ),
            ) {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    ToggleRow(
                        title = stringResource(R.string.label_enable_webdav),
                        checked = draft.enabled,
                        onCheckedChange = viewModel::updateEnabled,
                        enabled = inputEnabled,
                    )
                    OutlinedTextField(
                        value = draft.url,
                        onValueChange = viewModel::updateUrl,
                        label = { Text(stringResource(R.string.label_webdav_url)) },
                        modifier = Modifier.fillMaxWidth(),
                        enabled = inputEnabled,
                    )
                    OutlinedTextField(
                        value = draft.username,
                        onValueChange = viewModel::updateUsername,
                        label = { Text(stringResource(R.string.label_username)) },
                        modifier = Modifier.fillMaxWidth(),
                        enabled = inputEnabled,
                    )
                    OutlinedTextField(
                        value = draft.password,
                        onValueChange = viewModel::updatePassword,
                        label = { Text(stringResource(R.string.label_password)) },
                        modifier = Modifier.fillMaxWidth(),
                        enabled = inputEnabled,
                        visualTransformation = PasswordVisualTransformation(),
                    )
                    OutlinedTextField(
                        value = draft.intervalMinutes,
                        onValueChange = viewModel::updateSyncInterval,
                        label = { Text(stringResource(R.string.label_sync_interval)) },
                        modifier = Modifier.fillMaxWidth(),
                        enabled = inputEnabled,
                    )
                    ToggleRow(
                        title = stringResource(R.string.label_sync_on_startup),
                        checked = draft.syncOnStartup,
                        onCheckedChange = viewModel::updateSyncOnStartup,
                        enabled = inputEnabled,
                    )
                }
            }
        }

        item {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Button(
                    onClick = viewModel::save,
                    enabled = inputEnabled,
                    modifier = Modifier.weight(1f),
                ) {
                    Text(text = stringResource(R.string.action_save))
                }
                androidx.compose.material3.OutlinedButton(
                    onClick = viewModel::testConnection,
                    enabled = inputEnabled,
                ) {
                    Text(text = stringResource(R.string.action_test))
                }
            }
        }

        item {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                androidx.compose.material3.FilledTonalButton(
                    onClick = viewModel::syncNow,
                    enabled = inputEnabled,
                    modifier = Modifier.weight(1f),
                ) {
                    Text(text = stringResource(R.string.action_sync_now))
                }
                androidx.compose.material3.TextButton(
                    onClick = viewModel::load,
                    enabled = reloadEnabled,
                ) {
                    Text(text = stringResource(R.string.action_reload))
                }
            }
        }

        when (val action = state.action) {
            SyncActionState.Saved -> item {
                Text(
                    text = stringResource(R.string.text_sync_saved),
                    color = MaterialTheme.colorScheme.primary,
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
            SyncActionState.ConnectionTested -> item {
                Text(
                    text = stringResource(R.string.text_sync_connection_tested),
                    color = MaterialTheme.colorScheme.primary,
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
            is SyncActionState.Synchronized -> item {
                Text(
                    text = stringResource(
                        R.string.text_sync_summary,
                        action.summary.successCount.toString(),
                        action.summary.totalActions.toString(),
                        action.summary.failedCount.toString(),
                    ),
                    color = MaterialTheme.colorScheme.primary,
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
            SyncActionState.Idle,
            is SyncActionState.Working,
            is SyncActionState.Failed,
            is SyncActionState.Cancelled,
            is SyncActionState.TimedOut,
            is SyncActionState.Unconfirmed -> Unit
        }
    }
}

@Composable
private fun ToggleRow(
    title: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    enabled: Boolean,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = title,
            modifier = Modifier.weight(1f),
            style = MaterialTheme.typography.bodyMedium,
        )
        Switch(
            checked = checked,
            onCheckedChange = onCheckedChange,
            enabled = enabled,
        )
    }
}

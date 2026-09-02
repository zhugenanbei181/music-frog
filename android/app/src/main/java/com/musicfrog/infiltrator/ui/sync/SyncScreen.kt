package com.musicfrog.infiltrator.ui.sync

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
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
    val state by viewModel.state.collectAsState()

    androidx.compose.foundation.lazy.LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = androidx.compose.foundation.layout.PaddingValues(16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        item {
            Text(text = stringResource(R.string.title_webdav_sync), style = MaterialTheme.typography.titleLarge)
        }

        if (state.isLoading) {
            item {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    CircularProgressIndicator(modifier = Modifier.height(20.dp))
                    Spacer(modifier = Modifier.width(8.dp))
                    Text(text = stringResource(R.string.text_sync_loading))
                }
            }
        }

        if (state.error != null) {
            item {
                ErrorDialog(
                    message = state.error ?: "",
                    onDismiss = { viewModel.clearError() }
                )
            }
        }

        item {
            androidx.compose.material3.ElevatedCard(
                 colors = androidx.compose.material3.CardDefaults.elevatedCardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceContainerLow
                )
            ) {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp)
                ) {
                    ToggleRow(
                        title = stringResource(R.string.label_enable_webdav),
                        checked = state.enabled,
                        onCheckedChange = { viewModel.updateEnabled(it) },
                        enabled = !state.isLoading
                    )

                    OutlinedTextField(
                        value = state.url,
                        onValueChange = { viewModel.updateUrl(it) },
                        label = { Text(stringResource(R.string.label_webdav_url)) },
                        modifier = Modifier.fillMaxWidth(),
                        enabled = !state.isLoading
                    )

                    OutlinedTextField(
                        value = state.username,
                        onValueChange = { viewModel.updateUsername(it) },
                        label = { Text(stringResource(R.string.label_username)) },
                        modifier = Modifier.fillMaxWidth(),
                        enabled = !state.isLoading
                    )

                    OutlinedTextField(
                        value = state.password,
                        onValueChange = { viewModel.updatePassword(it) },
                        label = { Text(stringResource(R.string.label_password)) },
                        modifier = Modifier.fillMaxWidth(),
                        enabled = !state.isLoading,
                        visualTransformation = PasswordVisualTransformation()
                    )

                    OutlinedTextField(
                        value = state.syncInterval,
                        onValueChange = { viewModel.updateSyncInterval(it) },
                        label = { Text(stringResource(R.string.label_sync_interval)) },
                        modifier = Modifier.fillMaxWidth(),
                        enabled = !state.isLoading
                    )

                    ToggleRow(
                        title = stringResource(R.string.label_sync_on_startup),
                        checked = state.syncOnStartup,
                        onCheckedChange = { viewModel.updateSyncOnStartup(it) },
                        enabled = !state.isLoading
                    )
                }
            }
        }

        item {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Button(
                    onClick = { viewModel.save() },
                    enabled = !state.isLoading,
                    modifier = Modifier.weight(1f)
                ) {
                    Text(text = stringResource(R.string.action_save))
                }
                androidx.compose.material3.OutlinedButton(
                    onClick = { viewModel.testConnection() },
                    enabled = !state.isLoading
                ) {
                    Text(text = stringResource(R.string.action_test))
                }
            }
        }
        
        item {
             Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                androidx.compose.material3.FilledTonalButton(
                    onClick = { viewModel.syncNow() },
                    enabled = !state.isLoading,
                    modifier = Modifier.weight(1f)
                ) {
                    Text(text = stringResource(R.string.action_sync_now))
                }
                androidx.compose.material3.TextButton(
                    onClick = { viewModel.load() },
                    enabled = !state.isLoading
                ) {
                    Text(text = stringResource(R.string.action_reload))
                }
            }
        }

        if (state.saved) {
            item {
                Text(
                    text = stringResource(R.string.text_sync_saved),
                    color = MaterialTheme.colorScheme.primary,
                    style = MaterialTheme.typography.bodyMedium
                )
            }
        }

        if (state.testMessage != null) {
            item {
                Text(
                    text = state.testMessage ?: "",
                    color = MaterialTheme.colorScheme.primary,
                    style = MaterialTheme.typography.bodyMedium
                )
            }
        }

        if (state.syncSummary != null) {
            item {
                Text(
                    text = state.syncSummary ?: "",
                    color = MaterialTheme.colorScheme.primary,
                    style = MaterialTheme.typography.bodyMedium
                )
            }
        }
    }
}

@Composable
private fun ToggleRow(
    title: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    enabled: Boolean
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            text = title,
            modifier = Modifier.weight(1f),
            style = MaterialTheme.typography.bodyMedium
        )
        Switch(
            checked = checked,
            onCheckedChange = onCheckedChange,
            enabled = enabled
        )
    }
}

package com.musicfrog.despicableinfiltrator.ui.sync

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
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.musicfrog.despicableinfiltrator.ui.common.ErrorDialog

@Composable
fun SyncScreen() {
    val viewModel = remember { SyncViewModel() }
    val state by viewModel.state.collectAsState()

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text(text = "WebDAV Sync", style = MaterialTheme.typography.titleLarge)

        if (state.isLoading) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                CircularProgressIndicator(modifier = Modifier.height(20.dp))
                Spacer(modifier = Modifier.width(8.dp))
                Text(text = "Loading...")
            }
        }

        if (state.error != null) {
            ErrorDialog(
                message = state.error ?: "",
                onDismiss = { viewModel.clearError() }
            )
        }

        ToggleRow(
            title = "Enable WebDAV Sync",
            checked = state.enabled,
            onCheckedChange = { viewModel.updateEnabled(it) },
            enabled = !state.isLoading
        )

        OutlinedTextField(
            value = state.url,
            onValueChange = { viewModel.updateUrl(it) },
            label = { Text("WebDAV URL") },
            modifier = Modifier.fillMaxWidth(),
            enabled = !state.isLoading
        )

        OutlinedTextField(
            value = state.username,
            onValueChange = { viewModel.updateUsername(it) },
            label = { Text("Username") },
            modifier = Modifier.fillMaxWidth(),
            enabled = !state.isLoading
        )

        OutlinedTextField(
            value = state.password,
            onValueChange = { viewModel.updatePassword(it) },
            label = { Text("Password") },
            modifier = Modifier.fillMaxWidth(),
            enabled = !state.isLoading,
            visualTransformation = PasswordVisualTransformation()
        )

        OutlinedTextField(
            value = state.syncInterval,
            onValueChange = { viewModel.updateSyncInterval(it) },
            label = { Text("Sync interval (minutes)") },
            modifier = Modifier.fillMaxWidth(),
            enabled = !state.isLoading
        )

        ToggleRow(
            title = "Sync on startup",
            checked = state.syncOnStartup,
            onCheckedChange = { viewModel.updateSyncOnStartup(it) },
            enabled = !state.isLoading
        )

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Button(
                onClick = { viewModel.save() },
                enabled = !state.isLoading
            ) {
                Text(text = "Save")
            }
            TextButton(
                onClick = { viewModel.testConnection() },
                enabled = !state.isLoading
            ) {
                Text(text = "Test")
            }
            TextButton(
                onClick = { viewModel.syncNow() },
                enabled = !state.isLoading
            ) {
                Text(text = "Sync Now")
            }
            TextButton(
                onClick = { viewModel.load() },
                enabled = !state.isLoading
            ) {
                Text(text = "Reload")
            }
        }

        if (state.saved) {
            Text(
                text = "Saved",
                color = MaterialTheme.colorScheme.primary,
                style = MaterialTheme.typography.bodyMedium
            )
        }

        if (state.testMessage != null) {
            Text(
                text = state.testMessage ?: "",
                color = MaterialTheme.colorScheme.primary,
                style = MaterialTheme.typography.bodyMedium
            )
        }

        if (state.syncSummary != null) {
            Text(
                text = state.syncSummary ?: "",
                color = MaterialTheme.colorScheme.primary,
                style = MaterialTheme.typography.bodyMedium
            )
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

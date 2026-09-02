package com.musicfrog.infiltrator.ui.connections

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
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
import com.musicfrog.infiltrator.R
import com.musicfrog.infiltrator.ui.common.ErrorDialog
import com.musicfrog.infiltrator.ui.common.StandardListItem
import infiltrator_android.ConnectionRecord

@Composable
fun ConnectionsScreen(viewModel: ConnectionsViewModel = viewModel()) {
    val state by viewModel.uiState.collectAsState()
    val hostFilter by viewModel.hostFilter.collectAsState()
    val processFilter by viewModel.processFilter.collectAsState()

    if (state.error != null) {
        ErrorDialog(
            message = state.error.orEmpty(),
            onDismiss = { viewModel.clearError() },
        )
    }

    Scaffold { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Text(
                    text = stringResource(R.string.text_connections_total, state.connections.size),
                    style = MaterialTheme.typography.titleMedium,
                )
                Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    TextButton(onClick = { viewModel.refresh() }) {
                        Text(stringResource(R.string.action_refresh))
                    }
                    TextButton(onClick = { viewModel.closeAllConnections() }) {
                        Text(stringResource(R.string.action_close_all))
                    }
                }
            }

            OutlinedTextField(
                value = hostFilter,
                onValueChange = { viewModel.setHostFilter(it) },
                label = { Text(stringResource(R.string.label_host_filter)) },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )

            OutlinedTextField(
                value = processFilter,
                onValueChange = { viewModel.setProcessFilter(it) },
                label = { Text(stringResource(R.string.label_process_filter)) },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )

            if (state.isLoading && state.connections.isEmpty()) {
                Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    CircularProgressIndicator()
                }
            } else if (state.connections.isEmpty()) {
                Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Text(
                        text = stringResource(R.string.text_no_connections),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            } else {
                LazyColumn(
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(bottom = 16.dp),
                ) {
                    items(state.connections, key = { it.id }) { connection ->
                        ConnectionRow(
                            connection = connection,
                            onClose = { viewModel.closeConnection(connection.id) },
                        )
                        HorizontalDivider()
                    }
                }
            }
        }
    }
}

@Composable
private fun ConnectionRow(connection: ConnectionRecord, onClose: () -> Unit) {
    val headline = connection.host.ifBlank { connection.id }
    val supporting = buildSupportingText(connection)

    StandardListItem(
        headline = headline,
        supporting = supporting,
        onClick = null,
        trailingContent = {
            TextButton(onClick = onClose) {
                Text(stringResource(R.string.action_disconnect))
            }
        },
    )
}

@Composable
private fun buildSupportingText(connection: ConnectionRecord): String {
    val process = connection.processPath.ifBlank { "-" }
    val rule = connection.rule.ifBlank { "-" }
    val chains = if (connection.chains.isEmpty()) {
        "-"
    } else {
        connection.chains.joinToString(" > ")
    }
    val upload = formatBytes(connection.upload.toLong())
    val download = formatBytes(connection.download.toLong())

    return listOf(
        stringResource(R.string.text_connection_rule, rule),
        stringResource(R.string.text_connection_process, process),
        stringResource(R.string.text_connection_traffic, upload, download),
        stringResource(R.string.text_connection_chains, chains),
    ).joinToString("\n")
}

private fun formatBytes(bytes: Long): String {
    val units = listOf("B", "KB", "MB", "GB", "TB")
    var value = bytes.toDouble()
    var unitIndex = 0

    while (value >= 1024.0 && unitIndex < units.lastIndex) {
        value /= 1024.0
        unitIndex++
    }

    return String.format("%.1f %s", value, units[unitIndex])
}

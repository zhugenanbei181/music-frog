package com.musicfrog.infiltrator.ui.connections

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.lifecycle.compose.collectAsStateWithLifecycle
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
import com.musicfrog.infiltrator.ui.common.ConnectionRecord
import com.musicfrog.infiltrator.ui.common.ErrorDialog
import com.musicfrog.infiltrator.ui.common.LocalizedFailureText
import com.musicfrog.infiltrator.ui.common.StandardListItem
import com.musicfrog.infiltrator.ui.common.toLocalizedFailure

@Composable
fun ConnectionsScreen(viewModel: ConnectionsViewModel = viewModel()) {
    val state by viewModel.uiState.collectAsStateWithLifecycle()
    val hostFilter by viewModel.hostFilter.collectAsStateWithLifecycle()
    val processFilter by viewModel.processFilter.collectAsStateWithLifecycle()
    var selectedConnection by remember { mutableStateOf<ConnectionRecord?>(null) }

    val commandFailure = (state.actionState as? ConnectionActionState.Failed)?.failure
    if (commandFailure != null) {
        ErrorDialog(
            failure = commandFailure.toLocalizedFailure(),
            onDismiss = { viewModel.clearError() },
        )
    }

    if (selectedConnection != null) {
        ConnectionDetailsDialog(
            connection = selectedConnection!!,
            onDismiss = { selectedConnection = null },
            onClose = { viewModel.closeConnection(selectedConnection!!.id) },
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
            Column(
                modifier = Modifier.fillMaxWidth(),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Text(
                    text = when (state.listStatus) {
                        ConnectionsListStatus.Loading -> stringResource(R.string.text_loading)
                        ConnectionsListStatus.Unavailable -> stringResource(R.string.status_unknown)
                        ConnectionsListStatus.Empty,
                        ConnectionsListStatus.NoMatches,
                        ConnectionsListStatus.Content -> stringResource(
                            R.string.text_connections_total,
                            state.totalConnectionCount,
                        )
                    },
                    style = MaterialTheme.typography.titleMedium,
                )
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(4.dp, Alignment.End),
                ) {
                    TextButton(
                        onClick = { viewModel.refresh() },
                        enabled = state.actionState == ConnectionActionState.Idle,
                    ) {
                        Text(stringResource(R.string.action_refresh))
                    }
                    TextButton(
                        onClick = { viewModel.closeAllConnections() },
                        enabled = state.actionState == ConnectionActionState.Idle,
                    ) {
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

            when (state.listStatus) {
                ConnectionsListStatus.Loading -> Box(
                    modifier = Modifier.weight(1f).fillMaxWidth(),
                    contentAlignment = Alignment.Center,
                ) {
                    CircularProgressIndicator()
                }
                ConnectionsListStatus.Unavailable -> Box(
                    modifier = Modifier.weight(1f).fillMaxWidth(),
                ) {
                    Column(modifier = Modifier.fillMaxSize()) {
                        Column(
                            modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp),
                            horizontalAlignment = Alignment.CenterHorizontally,
                        ) {
                            Text(
                                text = stringResource(R.string.status_unknown),
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                            state.failure?.let { failure ->
                                LocalizedFailureText(
                                    failure = failure.toLocalizedFailure(),
                                    modifier = Modifier.padding(top = 8.dp),
                                )
                            }
                        }
                        if (state.connections.isNotEmpty()) {
                            LazyColumn(
                                modifier = Modifier.weight(1f).fillMaxWidth(),
                                contentPadding = PaddingValues(bottom = 16.dp),
                            ) {
                                items(state.connections, key = { it.id }) { connection ->
                                    ConnectionRow(
                                        connection = connection,
                                        onClick = { selectedConnection = connection },
                                        onClose = { viewModel.closeConnection(connection.id) },
                                        actionEnabled = state.actionState == ConnectionActionState.Idle,
                                    )
                                    HorizontalDivider()
                                }
                            }
                        }
                    }
                }
                ConnectionsListStatus.Empty -> Box(
                    modifier = Modifier.weight(1f).fillMaxWidth(),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = stringResource(R.string.text_no_connections),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                ConnectionsListStatus.NoMatches -> Spacer(Modifier.weight(1f))
                ConnectionsListStatus.Content -> LazyColumn(
                    modifier = Modifier.weight(1f).fillMaxWidth(),
                    contentPadding = PaddingValues(bottom = 16.dp),
                ) {
                    items(state.connections, key = { it.id }) { connection ->
                        ConnectionRow(
                            connection = connection,
                            onClick = { selectedConnection = connection },
                            onClose = { viewModel.closeConnection(connection.id) },
                            actionEnabled = state.actionState == ConnectionActionState.Idle,
                        )
                        HorizontalDivider()
                    }
                }
            }
        }
    }
}

@Composable
private fun ConnectionRow(
    connection: ConnectionRecord,
    onClick: () -> Unit,
    onClose: () -> Unit,
    actionEnabled: Boolean,
) {
    val headline = connection.host.ifBlank { connection.id }
    val supporting = buildSupportingText(connection)

    StandardListItem(
        headline = headline,
        supporting = supporting,
        onClick = onClick,
        trailingContent = {
            TextButton(onClick = onClose, enabled = actionEnabled) {
                Text(stringResource(R.string.action_disconnect))
            }
        },
    )
}

@Composable
private fun ConnectionDetailsDialog(
    connection: ConnectionRecord,
    onDismiss: () -> Unit,
    onClose: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(text = connection.host.ifBlank { connection.id }) },
        text = {
            Column(
                modifier = Modifier.verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                DetailItem(label = "ID", value = connection.id)
                DetailItem(label = "Network", value = connection.network.uppercase())
                DetailItem(label = "Type", value = connection.connectionType ?: "—")
                DetailItem(label = "Rule", value = connection.rule.ifBlank { "-" })
                DetailItem(label = "Process", value = connection.processPath.ifBlank { "-" })
                DetailItem(label = "Chains", value = connection.chains.joinToString(" > ").ifBlank { "-" })
                if (connection.destinationIpAsn.isNotBlank()) {
                    DetailItem(label = "Kernel ASN", value = connection.destinationIpAsn)
                }
                if (!connection.destinationGeoIp.isNullOrEmpty()) {
                    DetailItem(label = "Kernel GeoIP", value = connection.destinationGeoIp!!.joinToString(", "))
                }
                DetailItem(
                    label = "Traffic",
                    value = "↑ ${formatBytes(connection.upload.toLong())}  ↓ ${formatBytes(connection.download.toLong())}",
                )
            }
        },
        confirmButton = {
            TextButton(onClick = {
                onClose()
                onDismiss()
            }) {
                Text(stringResource(R.string.action_disconnect))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.action_cancel))
            }
        },
    )
}

@Composable
private fun DetailItem(label: String, value: String) {
    Column {
        Text(text = label, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary)
        Text(text = value, style = MaterialTheme.typography.bodyMedium)
    }
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

    val lines = mutableListOf(
        stringResource(R.string.text_connection_rule, rule),
        stringResource(R.string.text_connection_process, process),
        stringResource(R.string.text_connection_traffic, upload, download),
        stringResource(R.string.text_connection_chains, chains),
    )
    if (connection.destinationIpAsn.isNotBlank()) {
        lines.add("ASN: ${connection.destinationIpAsn}")
    }
    if (!connection.destinationGeoIp.isNullOrEmpty()) {
        lines.add("GeoIP: ${connection.destinationGeoIp!!.joinToString(", ")}")
    }
    return lines.joinToString("\n")
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

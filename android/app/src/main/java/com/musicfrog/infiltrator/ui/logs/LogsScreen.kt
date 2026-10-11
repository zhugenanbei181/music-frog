package com.musicfrog.infiltrator.ui.logs

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material3.ElevatedCard
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.musicfrog.infiltrator.R
import com.musicfrog.infiltrator.ui.common.LocalizedFailureText
import com.musicfrog.infiltrator.ui.common.LogEntry
import com.musicfrog.infiltrator.ui.common.LogLevel
import com.musicfrog.infiltrator.ui.common.OwnerLogStreamState
import com.musicfrog.infiltrator.ui.common.showToast
import com.musicfrog.infiltrator.ui.common.toLocalizedFailure

@Composable
fun LogsScreen() {
    val viewModel: LogsViewModel = viewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    val lifecycleOwner = LocalLifecycleOwner.current
    val context = LocalContext.current
    val clipboardManager = LocalClipboardManager.current
    val listState = rememberLazyListState()

    DisposableEffect(lifecycleOwner, viewModel) {
        val observer = LifecycleEventObserver { _, event ->
            when (event) {
                Lifecycle.Event.ON_START,
                Lifecycle.Event.ON_RESUME -> viewModel.dispatch(LogsIntent.PageVisible)
                Lifecycle.Event.ON_STOP,
                Lifecycle.Event.ON_DESTROY -> viewModel.dispatch(LogsIntent.PageHidden)
                else -> Unit
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        if (lifecycleOwner.lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED)) {
            viewModel.dispatch(LogsIntent.PageVisible)
        }
        onDispose {
            lifecycleOwner.lifecycle.removeObserver(observer)
            viewModel.dispatch(LogsIntent.PageHidden)
        }
    }

    val observation = state.observation
    val point = observation.lastAccepted()
    val logs = point?.logs?.entries.orEmpty()
    val streamState = when (observation) {
        is LogsObservationState.Content -> observation.point.logs.stream.toLogsStreamState()
        is LogsObservationState.Loading -> observation.lastAccepted?.logs?.stream
            ?.toLogsStreamState() ?: LogsStreamState.Starting
        is LogsObservationState.Unavailable -> LogsStreamState.Unavailable
        is LogsObservationState.Cancelled -> observation.lastAccepted?.logs?.stream
            ?.toLogsStreamState() ?: LogsStreamState.Unavailable
    }
    val streamSummary = when (streamState) {
        LogsStreamState.Starting -> stringResource(R.string.text_loading)
        LogsStreamState.Streaming -> stringResource(R.string.logs_streaming, logs.size)
        LogsStreamState.Paused -> stringResource(R.string.logs_paused, logs.size)
        LogsStreamState.Reconnecting,
        LogsStreamState.Failed,
        LogsStreamState.Unavailable -> stringResource(R.string.status_unknown)
    }

    Scaffold { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Box(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
                ElevatedCard(
                    modifier = Modifier.fillMaxWidth(),
                    colors = androidx.compose.material3.CardDefaults.elevatedCardColors(
                        containerColor = MaterialTheme.colorScheme.surfaceContainerLow,
                    ),
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(16.dp),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Column {
                            Text(
                                text = stringResource(R.string.logs_title),
                                style = MaterialTheme.typography.titleMedium,
                            )
                            Text(
                                text = streamSummary,
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                            IconButton(onClick = {
                                val text = logs.joinToString("\n") { entry ->
                                    "[${formatLogLevel(entry.level)}] ${entry.message}"
                                }
                                clipboardManager.setText(AnnotatedString(text))
                                showToast(context, context.getString(R.string.toast_logs_copied))
                            }) {
                                Icon(
                                    Icons.Outlined.ContentCopy,
                                    contentDescription = stringResource(R.string.action_copy),
                                )
                            }
                            IconButton(
                                enabled = state.operation !is LogsOperationState.Clearing,
                                onClick = { viewModel.dispatch(LogsIntent.Clear) },
                            ) {
                                Icon(
                                    Icons.Outlined.Delete,
                                    contentDescription = stringResource(R.string.action_clear),
                                )
                            }
                        }
                    }
                }
            }

            when (val operation = state.operation) {
                LogsOperationState.Idle -> Unit
                LogsOperationState.Clearing -> StatusLine(stringResource(R.string.text_loading))
                is LogsOperationState.Cleared -> StatusLine(stringResource(R.string.toast_logs_cleared))
                is LogsOperationState.Failed -> LocalizedFailureText(operation.failure.toLocalizedFailure())
                LogsOperationState.Cancelled -> StatusLine(stringResource(R.string.status_unknown))
            }

            when {
                point == null && observation is LogsObservationState.Loading -> {
                    StatusPanel(
                        modifier = Modifier.weight(1f),
                        text = stringResource(R.string.text_loading),
                    )
                }
                point == null && observation is LogsObservationState.Unavailable -> {
                    Box(
                        modifier = Modifier.weight(1f).fillMaxWidth(),
                        contentAlignment = Alignment.Center,
                    ) {
                        LocalizedFailureText(observation.failure.toLocalizedFailure())
                    }
                }
                point == null -> {
                    StatusPanel(
                        modifier = Modifier.weight(1f),
                        text = stringResource(R.string.status_unknown),
                    )
                }
                logs.isEmpty() -> {
                    StatusPanel(
                        modifier = Modifier.weight(1f),
                        text = stringResource(R.string.text_no_logs),
                    )
                }
                else -> {
                    LazyColumn(
                        state = listState,
                        modifier = Modifier.weight(1f).fillMaxWidth(),
                        contentPadding = PaddingValues(bottom = 16.dp),
                        verticalArrangement = Arrangement.spacedBy(0.dp),
                    ) {
                        items(logs.reversed()) { entry ->
                            LogEntryItem(entry)
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun StatusPanel(modifier: Modifier = Modifier, text: String) {
    Box(modifier = modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
        StatusLine(text)
    }
}

@Composable
private fun StatusLine(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

private fun OwnerLogStreamState.toLogsStreamState(): LogsStreamState = when (this) {
    OwnerLogStreamState.IDLE -> LogsStreamState.Paused
    OwnerLogStreamState.CONNECTING -> LogsStreamState.Starting
    OwnerLogStreamState.LIVE -> LogsStreamState.Streaming
    OwnerLogStreamState.RECONNECTING -> LogsStreamState.Reconnecting
    OwnerLogStreamState.FAILED -> LogsStreamState.Failed
    OwnerLogStreamState.UNKNOWN -> LogsStreamState.Unavailable
}

@Composable
private fun LogEntryItem(entry: LogEntry) {
    val levelColor = when (entry.level) {
        LogLevel.ERROR -> MaterialTheme.colorScheme.error
        LogLevel.WARNING -> Color(0xFFFF9800)
        LogLevel.INFO -> MaterialTheme.colorScheme.primary
        LogLevel.DEBUG -> MaterialTheme.colorScheme.tertiary
        LogLevel.SILENT,
        LogLevel.UNKNOWN -> MaterialTheme.colorScheme.onSurfaceVariant
    }

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 20.dp, vertical = 4.dp),
        verticalAlignment = Alignment.Top,
    ) {
        Text(
            text = formatLogLevel(entry.level),
            style = MaterialTheme.typography.labelSmall,
            color = levelColor,
            fontFamily = FontFamily.Monospace,
            modifier = Modifier.width(32.dp).padding(top = 2.dp),
        )
        Spacer(modifier = Modifier.width(8.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = entry.message,
                style = MaterialTheme.typography.bodySmall,
                fontFamily = FontFamily.Monospace,
                fontSize = 12.sp,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text = formatTimestamp(entry.timestamp),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                fontSize = 10.sp,
            )
        }
    }
}

private fun formatLogLevel(level: LogLevel): String = when (level) {
    LogLevel.DEBUG -> "DBG"
    LogLevel.INFO -> "INF"
    LogLevel.WARNING -> "WRN"
    LogLevel.ERROR -> "ERR"
    LogLevel.SILENT -> "SIL"
    LogLevel.UNKNOWN -> "???"
}

private fun formatTimestamp(timestamp: String): String = when {
    timestamp.isBlank() -> "—"
    'T' in timestamp -> timestamp.substringAfter('T').take(8)
    else -> timestamp.take(8)
}

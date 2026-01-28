package com.musicfrog.despicableinfiltrator.ui.logs

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.PlayArrow
import androidx.compose.material3.ElevatedCard
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.musicfrog.despicableinfiltrator.R
import com.musicfrog.despicableinfiltrator.ui.common.showToast
import infiltrator_android.FfiErrorCode
import infiltrator_android.LogEntry
import infiltrator_android.LogLevel
import infiltrator_android.logsClear
import infiltrator_android.logsGet
import infiltrator_android.logsIsStreaming
import infiltrator_android.logsStartStreaming
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@Composable
fun LogsScreen() {
    var logs by remember { mutableStateOf<List<LogEntry>>(emptyList()) }
    var isStreaming by remember { mutableStateOf(false) }
    var isLoading by remember { mutableStateOf(true) }
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val clipboardManager = LocalClipboardManager.current
    val listState = rememberLazyListState()

    // Start streaming and poll for logs
    LaunchedEffect(Unit) {
        // Start log streaming
        val startResult = logsStartStreaming()
        if (startResult.code == FfiErrorCode.OK) {
            isStreaming = true
        }
        isLoading = false

        // Poll for new logs every 2 seconds
        while (true) {
            val result = logsGet(200u)
            if (result.status.code == FfiErrorCode.OK) {
                logs = result.entries
                isStreaming = logsIsStreaming()
            }
            delay(2000)
        }
    }

    Scaffold { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            // Header with actions
            ElevatedCard(
                modifier = Modifier.fillMaxWidth()
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(16.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column {
                    Text(
                        text = stringResource(R.string.logs_title),
                        style = MaterialTheme.typography.titleMedium
                    )
                    Text(
                        text = if (isStreaming) 
                            stringResource(R.string.logs_streaming, logs.size) 
                            else stringResource(R.string.logs_paused, logs.size),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
                Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    if (!isStreaming) {
                        IconButton(
                            onClick = {
                                scope.launch {
                                    logsStartStreaming()
                                    isStreaming = logsIsStreaming()
                                }
                            }
                        ) {
                            Icon(Icons.Outlined.PlayArrow, contentDescription = stringResource(R.string.action_start))
                        }
                    } 
                    
                    IconButton(onClick = {
                        val text = logs.joinToString("\n") { entry ->
                            "[${formatLogLevel(entry.level)}] ${entry.message}"
                        }
                        clipboardManager.setText(AnnotatedString(text))
                        showToast(context, context.getString(R.string.toast_logs_copied))
                    }) {
                        Icon(Icons.Outlined.ContentCopy, contentDescription = stringResource(R.string.action_copy))
                    }
                    IconButton(onClick = {
                        scope.launch {
                            logsClear()
                            logs = emptyList()
                            showToast(context, context.getString(R.string.toast_logs_cleared))
                        }
                    }) {
                        Icon(Icons.Outlined.Delete, contentDescription = stringResource(R.string.action_clear))
                    }
                }
            }
        }

            // Log list
            if (isLoading) {
                Box(
                    modifier = Modifier.fillMaxSize(),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = stringResource(R.string.text_loading),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            } else if (logs.isEmpty()) {
                Box(
                    modifier = Modifier.fillMaxSize(),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = stringResource(R.string.text_no_logs),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            } else {
                LazyColumn(
                    state = listState,
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(bottom = 16.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    items(logs.reversed()) { entry ->
                        LogEntryItem(entry)
                    }
                }
            }
        }
    }
}

@Composable
private fun LogEntryItem(entry: LogEntry) {
    val levelColor = when (entry.level) {
        LogLevel.ERROR -> MaterialTheme.colorScheme.error
        LogLevel.WARNING -> Color(0xFFFF9800)
        LogLevel.INFO -> MaterialTheme.colorScheme.primary
        LogLevel.DEBUG -> MaterialTheme.colorScheme.tertiary
        LogLevel.SILENT -> MaterialTheme.colorScheme.onSurfaceVariant
    }

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 4.dp, vertical = 2.dp),
        verticalAlignment = Alignment.Top
    ) {
        // Level indicator
        Text(
            text = formatLogLevel(entry.level),
            style = MaterialTheme.typography.labelSmall,
            color = levelColor,
            fontFamily = FontFamily.Monospace,
            modifier = Modifier.width(32.dp).padding(top=2.dp)
        )
        
        Spacer(modifier = Modifier.width(8.dp))
        
        // Time and message
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = entry.message,
                style = MaterialTheme.typography.bodySmall,
                fontFamily = FontFamily.Monospace,
                fontSize = 12.sp,
                color = MaterialTheme.colorScheme.onSurface
            )
            Text(
                text = formatTimestamp(entry.timestamp),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                fontSize = 10.sp
            )
        }
    }
}

private fun formatLogLevel(level: LogLevel): String {
    return when (level) {
        LogLevel.DEBUG -> "DBG"
        LogLevel.INFO -> "INF"
        LogLevel.WARNING -> "WRN"
        LogLevel.ERROR -> "ERR"
        LogLevel.SILENT -> "SIL"
    }
}

private fun formatTimestamp(epochSeconds: ULong): String {
    return try {
        val date = Date(epochSeconds.toLong() * 1000)
        SimpleDateFormat("HH:mm:ss", Locale.getDefault()).format(date)
    } catch (e: Exception) {
        ""
    }
}

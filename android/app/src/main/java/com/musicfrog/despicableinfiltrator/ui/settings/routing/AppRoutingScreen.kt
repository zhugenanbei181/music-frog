package com.musicfrog.despicableinfiltrator.ui.settings.routing

import androidx.compose.foundation.Image
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.core.graphics.drawable.toBitmap

@Composable
fun AppRoutingScreen() {
    val context = LocalContext.current
    val viewModel = remember { AppRoutingViewModel(context) }
    val apps by viewModel.uiState.collectAsState()
    val routingMode by viewModel.routingMode.collectAsState()
    var searchQuery by remember { mutableStateOf("") }

    androidx.compose.foundation.layout.Column(modifier = Modifier.fillMaxSize()) {
        val proxySelected = routingMode == RoutingMode.ProxySelected
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                text = "Proxy Selected Only",
                modifier = Modifier.weight(1f),
                style = MaterialTheme.typography.bodyMedium
            )
            Switch(
                checked = proxySelected,
                onCheckedChange = { enabled ->
                    val mode = if (enabled) RoutingMode.ProxySelected else RoutingMode.ProxyAll
                    viewModel.setRoutingMode(mode)
                }
            )
        }
        OutlinedTextField(
            value = searchQuery,
            onValueChange = { 
                searchQuery = it
                viewModel.search(it)
            },
            label = { Text("Search Apps") },
            modifier = Modifier
                .fillMaxWidth()
                .padding(bottom = 8.dp)
        )

        LazyColumn {
            items(apps, key = { it.packageName }) { app ->
                AppRow(app = app, onToggle = { viewModel.toggleApp(app.packageName) })
                HorizontalDivider()
            }
        }
    }
}

@Composable
fun AppRow(app: AppItem, onToggle: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onToggle() }
            .padding(8.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        if (app.icon != null) {
            Image(
                bitmap = app.icon.toBitmap().asImageBitmap(),
                contentDescription = null,
                modifier = Modifier.size(40.dp)
            )
        } else {
            Spacer(modifier = Modifier.size(40.dp))
        }
        Spacer(modifier = Modifier.width(12.dp))
        Text(
            text = app.name,
            modifier = Modifier.weight(1f),
            style = MaterialTheme.typography.bodyLarge
        )
        Switch(
            checked = app.isSelected,
            onCheckedChange = { onToggle() }
        )
    }
}

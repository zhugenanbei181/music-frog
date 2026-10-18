package io.musicfrog.lurker.template.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

@Composable
fun TemplateScreen(
    isRunning: Boolean,
    routingMode: String,
    onStartVpn: () -> Unit,
    onStopVpn: () -> Unit,
    onSelectMode: (String) -> Unit
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(20.dp, Alignment.CenterVertically)
    ) {
        Text(
            text = "Lurker Android VPN Template",
            style = MaterialTheme.typography.headlineMedium
        )

        Card(modifier = Modifier.padding(8.dp)) {
            Column(
                modifier = Modifier.padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Text(text = "Core Status: " + if (isRunning) "Running" else "Stopped")
                Text(text = "Routing Intent: $routingMode")
            }
        }

        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Button(
                onClick = onStartVpn,
                enabled = !isRunning
            ) {
                Text("Start VPN")
            }

            Button(
                onClick = onStopVpn,
                enabled = isRunning
            ) {
                Text("Stop VPN")
            }
        }

        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = { onSelectMode("rules") }) {
                Text("Rules")
            }
            Button(onClick = { onSelectMode("global") }) {
                Text("Global")
            }
            Button(onClick = { onSelectMode("direct") }) {
                Text("Direct")
            }
        }
    }
}

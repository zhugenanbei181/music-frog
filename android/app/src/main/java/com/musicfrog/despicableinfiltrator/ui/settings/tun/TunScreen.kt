package com.musicfrog.despicableinfiltrator.ui.settings.tun

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
import androidx.compose.ui.unit.dp
import com.musicfrog.despicableinfiltrator.ui.common.ErrorDialog

@Composable
fun TunScreen() {
    val viewModel = remember { TunViewModel() }
    val state by viewModel.state.collectAsState()

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text(text = "TUN Settings", style = MaterialTheme.typography.titleLarge)

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

        OutlinedTextField(
            value = state.mtu,
            onValueChange = { viewModel.updateMtu(it) },
            label = { Text("MTU") },
            modifier = Modifier.fillMaxWidth(),
            enabled = !state.isLoading
        )

        ToggleRow(
            title = "Auto Route",
            checked = state.autoRoute,
            onCheckedChange = { viewModel.updateAutoRoute(it) },
            enabled = !state.isLoading
        )

        ToggleRow(
            title = "Strict Route",
            checked = state.strictRoute,
            onCheckedChange = { viewModel.updateStrictRoute(it) },
            enabled = !state.isLoading
        )

        ToggleRow(
            title = "IPv6",
            checked = state.ipv6,
            onCheckedChange = { viewModel.updateIpv6(it) },
            enabled = !state.isLoading
        )

        OutlinedTextField(
            value = state.dnsServers,
            onValueChange = { viewModel.updateDnsServers(it) },
            label = { Text("DNS Servers (one per line)") },
            modifier = Modifier
                .fillMaxWidth()
                .height(140.dp),
            enabled = !state.isLoading,
            maxLines = 6
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

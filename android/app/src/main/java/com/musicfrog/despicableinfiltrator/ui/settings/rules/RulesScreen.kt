package com.musicfrog.despicableinfiltrator.ui.settings.rules

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
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
fun RulesScreen() {
    val viewModel = remember { RulesViewModel() }
    val state by viewModel.state.collectAsState()
    val scrollState = rememberScrollState()

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(scrollState)
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text(text = "Rules", style = MaterialTheme.typography.titleLarge)

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
            value = state.newRule,
            onValueChange = { viewModel.updateNewRule(it) },
            label = { Text("New Rule") },
            modifier = Modifier.fillMaxWidth(),
            enabled = !state.isLoading
        )

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Button(
                onClick = { viewModel.addRule() },
                enabled = !state.isLoading
            ) {
                Text(text = "Add Rule")
            }
            TextButton(
                onClick = { viewModel.load() },
                enabled = !state.isLoading
            ) {
                Text(text = "Reload")
            }
        }

        if (state.rules.isEmpty() && !state.isLoading) {
            Text(
                text = "No rules configured",
                style = MaterialTheme.typography.bodyMedium
            )
        } else {
            state.rules.forEachIndexed { index, rule ->
                RuleRow(
                    rule = rule.rule,
                    enabled = rule.enabled,
                    onToggle = { viewModel.toggleRule(index, it) },
                    onRemove = { viewModel.removeRule(index) },
                    enabledUi = !state.isLoading
                )
            }
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Button(
                onClick = { viewModel.saveRules() },
                enabled = !state.isLoading
            ) {
                Text(text = "Save Rules")
            }
        }

        if (state.rulesSaved) {
            Text(
                text = "Rules saved",
                color = MaterialTheme.colorScheme.primary,
                style = MaterialTheme.typography.bodyMedium
            )
        }

        Spacer(modifier = Modifier.height(12.dp))
        Text(text = "Rule Providers (JSON)", style = MaterialTheme.typography.titleMedium)

        OutlinedTextField(
            value = state.providersJson,
            onValueChange = { viewModel.updateProvidersJson(it) },
            label = { Text("Providers JSON") },
            modifier = Modifier
                .fillMaxWidth()
                .height(200.dp),
            enabled = !state.isLoading,
            maxLines = 10
        )

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Button(
                onClick = { viewModel.saveProviders() },
                enabled = !state.isLoading
            ) {
                Text(text = "Save Providers")
            }
        }

        if (state.providersSaved) {
            Text(
                text = "Providers saved",
                color = MaterialTheme.colorScheme.primary,
                style = MaterialTheme.typography.bodyMedium
            )
        }
    }
}

@Composable
private fun RuleRow(
    rule: String,
    enabled: Boolean,
    onToggle: (Boolean) -> Unit,
    onRemove: () -> Unit,
    enabledUi: Boolean
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Switch(
            checked = enabled,
            onCheckedChange = onToggle,
            enabled = enabledUi
        )
        Text(
            text = rule,
            modifier = Modifier
                .weight(1f)
                .padding(start = 8.dp),
            style = MaterialTheme.typography.bodyMedium
        )
        TextButton(
            onClick = onRemove,
            enabled = enabledUi
        ) {
            Text(text = "Remove")
        }
    }
}

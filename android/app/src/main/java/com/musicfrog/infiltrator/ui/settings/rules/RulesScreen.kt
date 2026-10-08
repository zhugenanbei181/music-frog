package com.musicfrog.infiltrator.ui.settings.rules

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
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
import com.musicfrog.infiltrator.ui.common.ErrorDialog
import com.musicfrog.infiltrator.ui.common.StandardListItem
import infiltrator_android.RuleEntryRecord

@Composable
fun RulesScreen(viewModel: RulesViewModel = viewModel()) {
    val state by viewModel.state.collectAsState()
    var tracerTarget by remember { mutableStateOf("") }
    var tracerResult by remember { mutableStateOf<String?>(null) }

    Scaffold(
        bottomBar = {
            if (state.rulesSaved || state.providersSaved) {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(16.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = stringResource(if (state.rulesSaved) R.string.text_saved else R.string.text_providers_saved),
                        color = MaterialTheme.colorScheme.primary,
                        style = MaterialTheme.typography.labelLarge
                    )
                }
            }
        }
    ) { padding ->
        Box(modifier = Modifier.padding(padding).fillMaxSize()) {
            if (state.isLoading) {
                CircularProgressIndicator(modifier = Modifier.align(Alignment.Center))
            }

            if (state.error != null) {
                ErrorDialog(
                    message = state.error ?: "",
                    onDismiss = { viewModel.clearError() }
                )
            }

            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = androidx.compose.foundation.layout.PaddingValues(vertical = 16.dp)
            ) {
                item {
                    Text(
                        text = stringResource(R.string.section_add_rule),
                        style = MaterialTheme.typography.titleMedium,
                        modifier = Modifier.padding(16.dp, 16.dp, 16.dp, 8.dp),
                        color = MaterialTheme.colorScheme.primary
                    )
                }

                item {
                    androidx.compose.foundation.layout.Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 16.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(8.dp)
                    ) {
                        OutlinedTextField(
                            value = state.newRule,
                            onValueChange = { viewModel.updateNewRule(it) },
                            label = { Text(stringResource(R.string.label_new_rule)) },
                            modifier = Modifier.weight(1f),
                            enabled = !state.isLoading,
                            singleLine = true
                        )
                        Button(
                            onClick = { viewModel.addRule() },
                            enabled = !state.isLoading
                        ) {
                            Text(stringResource(R.string.action_add))
                        }
                    }
                }

                item {
                    Text(
                        text = stringResource(R.string.section_active_rules),
                        style = MaterialTheme.typography.titleMedium,
                        modifier = Modifier.padding(16.dp, 24.dp, 16.dp, 8.dp),
                        color = MaterialTheme.colorScheme.primary
                    )
                }

                if (state.rules.isEmpty() && !state.isLoading) {
                    item {
                        Text(
                            text = stringResource(R.string.text_no_rules),
                            style = MaterialTheme.typography.bodyMedium,
                            modifier = Modifier.padding(16.dp),
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }

                itemsIndexed(state.rules) { index, rule ->
                    StandardListItem(
                        headline = rule.rule,
                        leadingContent = {
                            Switch(
                                checked = rule.enabled,
                                onCheckedChange = { viewModel.toggleRule(index, it) },
                                enabled = !state.isLoading
                            )
                        },
                        trailingContent = {
                            IconButton(
                                onClick = { viewModel.removeRule(index) },
                                enabled = !state.isLoading
                            ) {
                                Icon(Icons.Outlined.Delete, contentDescription = stringResource(R.string.action_remove))
                            }
                        },
                        onClick = { if (!state.isLoading) viewModel.toggleRule(index, !rule.enabled) }
                    )
                    HorizontalDivider()
                }

                item {
                    Button(
                        onClick = { viewModel.saveRules() },
                        enabled = !state.isLoading,
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(16.dp)
                    ) {
                        Text(stringResource(R.string.action_save_rules))
                    }
                }

                item {
                    Text(
                        text = stringResource(R.string.section_providers),
                        style = MaterialTheme.typography.titleMedium,
                        modifier = Modifier.padding(16.dp, 24.dp, 16.dp, 8.dp),
                        color = MaterialTheme.colorScheme.primary
                    )
                }

                item {
                    OutlinedTextField(
                        value = state.providersJson,
                        onValueChange = { viewModel.updateProvidersJson(it) },
                        label = { Text(stringResource(R.string.label_providers_json)) },
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 16.dp),
                        enabled = !state.isLoading,
                        minLines = 5,
                        maxLines = 15
                    )
                }

                item {
                    androidx.compose.foundation.layout.Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(16.dp),
                        horizontalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(12.dp)
                    ) {
                        Button(
                            onClick = { viewModel.saveProviders() },
                            enabled = !state.isLoading,
                            modifier = Modifier.weight(1f)
                        ) {
                            Text(stringResource(R.string.action_save_providers))
                        }
                        TextButton(
                            onClick = { viewModel.load() },
                            enabled = !state.isLoading
                        ) {
                            Text(stringResource(R.string.action_reload_all))
                        }
                    }
                }

                item {
                    Text(
                        text = "规则测试器 / 追踪 (Rule Tracer)",
                        style = MaterialTheme.typography.titleMedium,
                        modifier = Modifier.padding(16.dp, 24.dp, 16.dp, 8.dp),
                        color = MaterialTheme.colorScheme.primary,
                    )
                }

                item {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 16.dp),
                        verticalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(8.dp),
                    ) {
                        OutlinedTextField(
                            value = tracerTarget,
                            onValueChange = { tracerTarget = it },
                            label = { Text("测试目标 (如 google.com 或 8.8.8.8)") },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                        )
                        Button(
                            onClick = {
                                tracerResult = simulateRuleMatch(tracerTarget, state.rules)
                            },
                            enabled = tracerTarget.isNotBlank(),
                        ) {
                            Text("开始规则匹配")
                        }
                        if (tracerResult != null) {
                            Text(
                                text = tracerResult!!,
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.tertiary,
                            )
                        }
                    }
                }
            }
        }
    }
}

private fun simulateRuleMatch(target: String, rules: List<RuleEntryRecord>): String {
    val trimmed = target.trim().lowercase()
    for (entry in rules) {
        if (!entry.enabled) continue
        val parts = entry.rule.split(",").map { it.trim() }
        if (parts.isEmpty()) continue
        val ruleType = parts[0].uppercase()
        val payload = if (parts.size > 1) parts[1].lowercase() else ""
        val targetProxy = if (parts.size > 2) parts[2] else "DIRECT"

        when (ruleType) {
            "MATCH" -> return "命中 MATCH 兜底规则 -> $targetProxy"
            "DOMAIN" -> {
                if (trimmed == payload) return "命中 DOMAIN: $payload -> $targetProxy"
            }
            "DOMAIN-SUFFIX" -> {
                if (trimmed == payload || trimmed.endsWith(".$payload")) return "命中 DOMAIN-SUFFIX: $payload -> $targetProxy"
            }
            "DOMAIN-KEYWORD" -> {
                if (trimmed.contains(payload)) return "命中 DOMAIN-KEYWORD: $payload -> $targetProxy"
            }
            "IP-CIDR", "IP-CIDR6" -> {
                if (trimmed == payload || trimmed.startsWith(payload.substringBefore("/"))) return "命中 $ruleType: $payload -> $targetProxy"
            }
            "GEOIP" -> {
                if (trimmed.contains(payload)) return "命中 GEOIP: $payload -> $targetProxy"
            }
        }
    }
    return "未命中任何显式规则，将使用内核默认直连 (DIRECT)"
}

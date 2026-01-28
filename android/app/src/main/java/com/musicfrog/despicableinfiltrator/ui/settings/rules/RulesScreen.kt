package com.musicfrog.despicableinfiltrator.ui.settings.rules

import androidx.compose.foundation.layout.Box
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
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.musicfrog.despicableinfiltrator.R
import com.musicfrog.despicableinfiltrator.ui.common.ErrorDialog
import com.musicfrog.despicableinfiltrator.ui.common.StandardListItem

@Composable
fun RulesScreen(viewModel: RulesViewModel = viewModel()) {
    val state by viewModel.state.collectAsState()

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

            LazyColumn(modifier = Modifier.fillMaxSize()) {
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
            }
        }
    }
}

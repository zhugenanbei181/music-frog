package com.musicfrog.infiltrator.ui.settings.rules

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
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.musicfrog.infiltrator.R
import com.musicfrog.infiltrator.ui.common.ErrorDialog
import com.musicfrog.infiltrator.ui.common.StandardListItem
import com.musicfrog.infiltrator.ui.common.toLocalizedFailure

@Composable
fun RulesScreen(viewModel: RulesViewModel = viewModel()) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val savedMessage = when (state.operation) {
        RulesOperationState.RulesSaved -> stringResource(R.string.text_saved)
        RulesOperationState.ProvidersSaved -> stringResource(R.string.text_providers_saved)
        else -> null
    }

    Scaffold(
        bottomBar = {
            if (savedMessage != null) {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(16.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = savedMessage,
                        color = MaterialTheme.colorScheme.primary,
                        style = MaterialTheme.typography.labelLarge,
                    )
                }
            }
        },
    ) { padding ->
        Box(modifier = Modifier.padding(padding).fillMaxSize()) {
            if (state.activity == RulesActivity.Busy) {
                CircularProgressIndicator(modifier = Modifier.align(Alignment.Center))
            }

            when (val failure = state.failure) {
                RulesFailureState.None -> Unit
                is RulesFailureState.Present -> ErrorDialog(
                    failure = failure.failure.toLocalizedFailure(),
                    onDismiss = viewModel::clearError,
                )
            }

            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = androidx.compose.foundation.layout.PaddingValues(vertical = 16.dp),
            ) {
                item {
                    Text(
                        text = stringResource(R.string.section_add_rule),
                        style = MaterialTheme.typography.titleMedium,
                        modifier = Modifier.padding(16.dp, 16.dp, 16.dp, 8.dp),
                        color = MaterialTheme.colorScheme.primary,
                    )
                }

                item {
                    androidx.compose.foundation.layout.Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 16.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(8.dp),
                    ) {
                        OutlinedTextField(
                            value = state.newRule,
                            onValueChange = viewModel::updateNewRule,
                            label = { Text(stringResource(R.string.label_new_rule)) },
                            modifier = Modifier.weight(1f),
                            enabled = state.activity == RulesActivity.Interactive &&
                                state.rulesAvailability == RulesObservationAvailability.Available,
                            singleLine = true,
                        )
                        Button(
                            onClick = viewModel::addRule,
                            enabled = state.activity == RulesActivity.Interactive &&
                                state.rulesAvailability == RulesObservationAvailability.Available,
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
                        color = MaterialTheme.colorScheme.primary,
                    )
                }

                if (state.rulesAvailability == RulesObservationAvailability.Available &&
                    state.rulesDraft.isEmpty() && state.activity == RulesActivity.Interactive
                ) {
                    item {
                        Text(
                            text = stringResource(R.string.text_no_rules),
                            style = MaterialTheme.typography.bodyMedium,
                            modifier = Modifier.padding(16.dp),
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }

                if (state.rulesAvailability == RulesObservationAvailability.Unavailable &&
                    state.activity == RulesActivity.Interactive
                ) {
                    item {
                        Text(
                            text = stringResource(R.string.text_rules_unavailable),
                            style = MaterialTheme.typography.bodyMedium,
                            modifier = Modifier.padding(16.dp),
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }

                itemsIndexed(state.rulesDraft) { index, rule ->
                    StandardListItem(
                        headline = rule.rule,
                        leadingContent = {
                            Switch(
                                checked = rule.enabled,
                                onCheckedChange = { viewModel.toggleRule(index, it) },
                                enabled = state.activity == RulesActivity.Interactive,
                            )
                        },
                        trailingContent = {
                            IconButton(
                                onClick = { viewModel.removeRule(index) },
                                enabled = state.activity == RulesActivity.Interactive,
                            ) {
                                Icon(
                                    Icons.Outlined.Delete,
                                    contentDescription = stringResource(R.string.action_remove),
                                )
                            }
                        },
                        onClick = {
                            if (state.activity == RulesActivity.Interactive) {
                                viewModel.toggleRule(index, !rule.enabled)
                            }
                        },
                    )
                    HorizontalDivider()
                }

                item {
                    Button(
                        onClick = viewModel::saveRules,
                        enabled = state.activity == RulesActivity.Interactive &&
                            state.rulesAvailability == RulesObservationAvailability.Available,
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(16.dp),
                    ) {
                        Text(stringResource(R.string.action_save_rules))
                    }
                }

                item {
                    Text(
                        text = stringResource(R.string.section_providers),
                        style = MaterialTheme.typography.titleMedium,
                        modifier = Modifier.padding(16.dp, 24.dp, 16.dp, 8.dp),
                        color = MaterialTheme.colorScheme.primary,
                    )
                }

                if (state.providersAvailability == RulesObservationAvailability.Available) {
                    item {
                        OutlinedTextField(
                            value = state.providersDraft,
                            onValueChange = viewModel::updateProvidersJson,
                            label = { Text(stringResource(R.string.label_providers_json)) },
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 16.dp),
                            enabled = state.activity == RulesActivity.Interactive,
                            minLines = 5,
                            maxLines = 15,
                        )
                    }
                } else if (state.activity == RulesActivity.Interactive) {
                    item {
                        Text(
                            text = stringResource(R.string.text_rule_providers_unavailable),
                            style = MaterialTheme.typography.bodyMedium,
                            modifier = Modifier.padding(horizontal = 16.dp),
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }

                item {
                    androidx.compose.foundation.layout.Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(16.dp),
                        horizontalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(12.dp),
                    ) {
                        Button(
                            onClick = viewModel::saveProviders,
                            enabled = state.activity == RulesActivity.Interactive &&
                                state.providersAvailability == RulesObservationAvailability.Available,
                            modifier = Modifier.weight(1f),
                        ) {
                            Text(stringResource(R.string.action_save_providers))
                        }
                        TextButton(
                            onClick = viewModel::load,
                            enabled = state.reloadAvailability == RulesReloadAvailability.Available,
                        ) {
                            Text(stringResource(R.string.action_reload_all))
                        }
                    }
                }

                if (state.reloadAvailability == RulesReloadAvailability.UnsavedChanges) {
                    item {
                        Text(
                            text = stringResource(R.string.text_save_before_reload),
                            style = MaterialTheme.typography.bodySmall,
                            modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
        }
    }
}

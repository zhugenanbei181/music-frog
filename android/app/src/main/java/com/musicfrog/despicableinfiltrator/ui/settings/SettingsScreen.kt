package com.musicfrog.despicableinfiltrator.ui.settings

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.List
import androidx.compose.material.icons.outlined.Apps
import androidx.compose.material.icons.outlined.Description
import androidx.compose.material.icons.outlined.Dns
import androidx.compose.material.icons.outlined.Language
import androidx.compose.material.icons.outlined.Public
import androidx.compose.material.icons.outlined.Router
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
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
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.musicfrog.despicableinfiltrator.R
import com.musicfrog.despicableinfiltrator.ui.common.StandardListItem

@Composable
fun SettingsScreen(
    viewModel: SettingsViewModel = viewModel(),
    onNavigateToRouting: () -> Unit,
    onNavigateToTun: () -> Unit,
    onNavigateToDns: () -> Unit,
    onNavigateToFakeIp: () -> Unit,
    onNavigateToRules: () -> Unit,
    onNavigateToLogs: () -> Unit = {}
) {
    val currentLocale by viewModel.currentLocale.collectAsState()
    var showLanguageDialog by remember { mutableStateOf(false) }

    Scaffold { padding ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
        ) {
            item {
                StandardListItem(
                    headline = stringResource(R.string.setting_language_title),
                    supporting = getLocaleLabel(currentLocale),
                    leadingIcon = Icons.Outlined.Language,
                    onClick = { showLanguageDialog = true }
                )
                HorizontalDivider()
            }
            item {
                StandardListItem(
                    headline = stringResource(R.string.setting_routing_title),
                    supporting = stringResource(R.string.setting_routing_desc),
                    leadingIcon = Icons.Outlined.Apps,
                    onClick = onNavigateToRouting
                )
                HorizontalDivider()
            }
            item {
                StandardListItem(
                    headline = stringResource(R.string.setting_tun_title),
                    supporting = stringResource(R.string.setting_tun_desc),
                    leadingIcon = Icons.Outlined.Router,
                    onClick = onNavigateToTun
                )
                HorizontalDivider()
            }
            item {
                StandardListItem(
                    headline = stringResource(R.string.setting_dns_title),
                    supporting = stringResource(R.string.setting_dns_desc),
                    leadingIcon = Icons.Outlined.Dns,
                    onClick = onNavigateToDns
                )
                HorizontalDivider()
            }
            item {
                StandardListItem(
                    headline = stringResource(R.string.setting_fakeip_title),
                    supporting = stringResource(R.string.setting_fakeip_desc),
                    leadingIcon = Icons.Outlined.Public,
                    onClick = onNavigateToFakeIp
                )
                HorizontalDivider()
            }
            item {
                StandardListItem(
                    headline = stringResource(R.string.setting_rules_title),
                    supporting = stringResource(R.string.setting_rules_desc),
                    leadingIcon = Icons.AutoMirrored.Outlined.List,
                    onClick = onNavigateToRules
                )
                HorizontalDivider()
            }
            item {
                StandardListItem(
                    headline = stringResource(R.string.setting_logs_title),
                    supporting = stringResource(R.string.setting_logs_desc),
                    leadingIcon = Icons.Outlined.Description,
                    onClick = onNavigateToLogs
                )
            }
        }

        if (showLanguageDialog) {
            LanguageDialog(
                currentLocale = currentLocale,
                onDismiss = { showLanguageDialog = false },
                onSelect = { tag ->
                    viewModel.setLocale(tag)
                    showLanguageDialog = false
                }
            )
        }
    }
}

@Composable
fun LanguageDialog(
    currentLocale: String,
    onDismiss: () -> Unit,
    onSelect: (String) -> Unit
) {
    val options = listOf(
        "system" to stringResource(R.string.setting_language_system),
        "en" to stringResource(R.string.setting_language_en),
        "zh-CN" to stringResource(R.string.setting_language_zh_cn),
        "zh-TW" to stringResource(R.string.setting_language_zh_tw)
    )

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.setting_language_title)) },
        text = {
            Column(Modifier.selectableGroup()) {
                options.forEach { (tag, label) ->
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .height(56.dp)
                            .selectable(
                                selected = (tag == currentLocale),
                                onClick = { onSelect(tag) },
                                role = Role.RadioButton
                            )
                            .padding(horizontal = 16.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        RadioButton(
                            selected = (tag == currentLocale),
                            onClick = null // handled by row
                        )
                        Text(
                            text = label,
                            style = MaterialTheme.typography.bodyLarge,
                            modifier = Modifier.padding(start = 16.dp)
                        )
                    }
                }
            }
        },
        confirmButton = {},
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.action_cancel)) }
        }
    )
}

@Composable
private fun getLocaleLabel(tag: String): String {
    return when (tag) {
        "en" -> stringResource(R.string.setting_language_en)
        "zh-CN" -> stringResource(R.string.setting_language_zh_cn)
        "zh-TW" -> stringResource(R.string.setting_language_zh_tw)
        else -> stringResource(R.string.setting_language_system)
    }
}

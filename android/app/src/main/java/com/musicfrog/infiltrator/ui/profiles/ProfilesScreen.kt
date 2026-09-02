package com.musicfrog.infiltrator.ui.profiles

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.UploadFile
import androidx.compose.material.icons.outlined.Description
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.FloatingActionButtonDefaults
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.musicfrog.infiltrator.R
import com.musicfrog.infiltrator.ui.common.ErrorDialog
import com.musicfrog.infiltrator.ui.common.StandardListItem
import infiltrator_android.ProfileDetail
import infiltrator_android.ProfileSummary
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private data class LocalImportDraft(
    val suggestedName: String,
    val content: String,
)

@Composable
fun ProfilesScreen(
    viewModel: ProfilesViewModel = viewModel(),
    initialImportUrl: String? = null,
    onImportHandled: () -> Unit = {}
) {
    val profiles by viewModel.profiles.collectAsState()
    val profileDetail by viewModel.profileDetail.collectAsState()
    val isLoading by viewModel.isLoading.collectAsState()
    val error by viewModel.error.collectAsState()
    val emptyMessage by viewModel.emptyMessage.collectAsState()

    var showAddDialog by remember { mutableStateOf(false) }
    var prefilledUrl by remember { mutableStateOf<String?>(null) }
    var deletingProfile by remember { mutableStateOf<ProfileSummary?>(null) }
    var subscriptionProfile by remember { mutableStateOf<ProfileSummary?>(null) }
    var localImportDraft by remember { mutableStateOf<LocalImportDraft?>(null) }
    val scope = rememberCoroutineScope()
    val context = androidx.compose.ui.platform.LocalContext.current

    val importLocalLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.GetContent(),
    ) { uri: Uri? ->
        if (uri == null) {
            return@rememberLauncherForActivityResult
        }
        scope.launch {
            val fileContent = readTextFromUri(context, uri)
            if (fileContent.isNullOrBlank()) {
                return@launch
            }
            val suggestedName = resolveDisplayName(context, uri)
                ?.substringBeforeLast(".")
                ?.takeIf { it.isNotBlank() }
                ?: "imported"
            localImportDraft = LocalImportDraft(
                suggestedName = suggestedName,
                content = fileContent,
            )
        }
    }

    LaunchedEffect(initialImportUrl) {
        if (initialImportUrl != null) {
            prefilledUrl = initialImportUrl
            showAddDialog = true
        }
    }

    Scaffold(
        floatingActionButton = {
            Column(
                verticalArrangement = Arrangement.spacedBy(12.dp),
                horizontalAlignment = Alignment.End
            ) {
                FloatingActionButton(
                    onClick = { importLocalLauncher.launch("*/*") },
                    containerColor = MaterialTheme.colorScheme.secondaryContainer,
                    elevation = FloatingActionButtonDefaults.elevation(defaultElevation = 2.dp),
                ) {
                    Icon(
                        Icons.Default.UploadFile,
                        contentDescription = stringResource(R.string.action_import_local),
                    )
                }
                FloatingActionButton(
                    onClick = {
                        prefilledUrl = null
                        showAddDialog = true
                    },
                    containerColor = MaterialTheme.colorScheme.primaryContainer,
                ) {
                    Icon(Icons.Default.Add, contentDescription = stringResource(R.string.title_add_profile))
                }
            }
        }
    ) { padding ->
        Box(modifier = Modifier
            .fillMaxSize()
            .padding(padding)) {
            LazyColumn(contentPadding = PaddingValues(bottom = 140.dp)) {
                items(profiles) { profile ->
                    ProfileRow(
                        profile = profile,
                        onSelect = { viewModel.selectProfile(profile.name) },
                        onUpdate = { viewModel.updateProfile(profile.name) },
                        onEdit = { viewModel.loadProfileDetail(profile.name) },
                        onSubscription = { subscriptionProfile = profile },
                        onDelete = { deletingProfile = profile },
                    )
                    HorizontalDivider()
                }
            }

            if (isLoading && profiles.isEmpty()) {
                CircularProgressIndicator(modifier = Modifier.align(Alignment.Center))
            }

            if (!isLoading && profiles.isEmpty() && emptyMessage != null) {
                Text(
                    text = emptyMessage.orEmpty(),
                    modifier = Modifier.align(Alignment.Center),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }

            if (error != null) {
                ErrorDialog(
                    message = error.orEmpty(),
                    onDismiss = { viewModel.clearError() }
                )
            }

            if (showAddDialog) {
                AddProfileDialog(
                    initialUrl = prefilledUrl ?: "",
                    onDismiss = {
                        showAddDialog = false
                        onImportHandled()
                    },
                    onConfirm = { name, url ->
                        viewModel.addProfile(name, url)
                        showAddDialog = false
                        onImportHandled()
                    }
                )
            }

            if (profileDetail != null) {
                SaveProfileDialog(
                    title = stringResource(R.string.title_edit_profile),
                    initialName = profileDetail!!.name,
                    nameEditable = false,
                    initialContent = profileDetail!!.content,
                    initialActivate = profileDetail!!.active,
                    onDismiss = { viewModel.clearProfileDetail() },
                    onConfirm = { name, content, activate ->
                        viewModel.saveProfileContent(name, content, activate)
                    },
                )
            }

            if (localImportDraft != null) {
                SaveProfileDialog(
                    title = stringResource(R.string.title_import_local_profile),
                    initialName = localImportDraft!!.suggestedName,
                    nameEditable = true,
                    initialContent = localImportDraft!!.content,
                    initialActivate = false,
                    onDismiss = { localImportDraft = null },
                    onConfirm = { name, content, activate ->
                        viewModel.saveProfileContent(name, content, activate)
                        localImportDraft = null
                    },
                )
            }

            if (subscriptionProfile != null) {
                SubscriptionSettingsDialog(
                    profile = subscriptionProfile!!,
                    onDismiss = { subscriptionProfile = null },
                    onSave = { url, autoUpdate, intervalHours ->
                        viewModel.saveSubscriptionSettings(
                            name = subscriptionProfile!!.name,
                            url = url,
                            autoUpdateEnabled = autoUpdate,
                            updateIntervalHours = intervalHours,
                        )
                        subscriptionProfile = null
                    },
                    onClear = {
                        viewModel.clearSubscriptionSettings(subscriptionProfile!!.name)
                        subscriptionProfile = null
                    }
                )
            }

            if (deletingProfile != null) {
                ConfirmDeleteDialog(
                    profileName = deletingProfile!!.name,
                    onDismiss = { deletingProfile = null },
                    onConfirm = {
                        viewModel.deleteProfile(deletingProfile!!.name)
                        deletingProfile = null
                    }
                )
            }
        }
    }
}

@Composable
private fun ProfileRow(
    profile: ProfileSummary,
    onSelect: () -> Unit,
    onUpdate: () -> Unit,
    onEdit: () -> Unit,
    onSubscription: () -> Unit,
    onDelete: () -> Unit,
) {
    val updatedText = profile.lastUpdated?.let {
        stringResource(R.string.text_updated_at, it.take(10))
    }

    StandardListItem(
        headline = profile.name,
        supporting = updatedText,
        leadingIcon = Icons.Outlined.Description,
        onClick = onSelect,
        trailingContent = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = onUpdate) {
                    Icon(Icons.Default.Refresh, contentDescription = stringResource(R.string.action_update_now))
                }
                IconButton(onClick = onEdit) {
                    Icon(Icons.Default.Edit, contentDescription = stringResource(R.string.action_edit))
                }
                IconButton(onClick = onSubscription) {
                    Icon(Icons.Default.Settings, contentDescription = stringResource(R.string.action_subscription_settings))
                }
                IconButton(onClick = onDelete) {
                    Icon(Icons.Default.Delete, contentDescription = stringResource(R.string.action_delete))
                }
                if (profile.active) {
                    Icon(
                        Icons.Default.Check,
                        contentDescription = stringResource(R.string.status_active),
                        tint = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(start = 8.dp, end = 12.dp)
                    )
                }
            }
        }
    )
}

@Composable
private fun AddProfileDialog(
    initialUrl: String = "",
    onDismiss: () -> Unit,
    onConfirm: (String, String) -> Unit
) {
    var name by remember { mutableStateOf("") }
    var url by remember { mutableStateOf(initialUrl) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.title_add_profile)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    label = { Text(stringResource(R.string.label_name)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
                OutlinedTextField(
                    value = url,
                    onValueChange = { url = it },
                    label = { Text(stringResource(R.string.label_url)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
            }
        },
        confirmButton = {
            TextButton(
                onClick = { if (name.isNotBlank() && url.isNotBlank()) onConfirm(name, url) },
                enabled = name.isNotBlank() && url.isNotBlank()
            ) {
                Text(stringResource(R.string.action_add))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.action_cancel))
            }
        }
    )
}

@Composable
private fun SaveProfileDialog(
    title: String,
    initialName: String,
    nameEditable: Boolean,
    initialContent: String,
    initialActivate: Boolean,
    onDismiss: () -> Unit,
    onConfirm: (String, String, Boolean) -> Unit,
) {
    var name by remember(initialName) { mutableStateOf(initialName) }
    var content by remember(initialContent) { mutableStateOf(initialContent) }
    var activate by remember(initialActivate) { mutableStateOf(initialActivate) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    label = { Text(stringResource(R.string.label_name)) },
                    enabled = nameEditable,
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
                OutlinedTextField(
                    value = content,
                    onValueChange = { content = it },
                    label = { Text(stringResource(R.string.label_profile_content)) },
                    modifier = Modifier.fillMaxWidth(),
                    minLines = 8,
                )
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(text = stringResource(R.string.label_activate_after_save))
                    Switch(
                        checked = activate,
                        onCheckedChange = { activate = it },
                    )
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = {
                    if (name.isNotBlank() && content.isNotBlank()) {
                        onConfirm(name, content, activate)
                    }
                },
                enabled = name.isNotBlank() && content.isNotBlank(),
            ) {
                Text(stringResource(R.string.action_save))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.action_cancel))
            }
        }
    )
}

@Composable
private fun SubscriptionSettingsDialog(
    profile: ProfileSummary,
    onDismiss: () -> Unit,
    onSave: (String, Boolean, Int?) -> Unit,
    onClear: () -> Unit,
) {
    var url by remember(profile.name) { mutableStateOf(profile.subscriptionUrl.orEmpty()) }
    var autoUpdateEnabled by remember(profile.name) { mutableStateOf(profile.autoUpdateEnabled) }
    var intervalHours by remember(profile.name) {
        mutableStateOf(profile.updateIntervalHours?.toString().orEmpty())
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.action_subscription_settings)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    text = profile.name,
                    style = MaterialTheme.typography.titleSmall
                )
                OutlinedTextField(
                    value = url,
                    onValueChange = { url = it },
                    label = { Text(stringResource(R.string.label_subscription_url)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(text = stringResource(R.string.label_auto_update))
                    Switch(
                        checked = autoUpdateEnabled,
                        onCheckedChange = { autoUpdateEnabled = it },
                    )
                }
                if (autoUpdateEnabled) {
                    OutlinedTextField(
                        value = intervalHours,
                        onValueChange = { intervalHours = it },
                        label = { Text(stringResource(R.string.label_update_interval_hours)) },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth()
                    )
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = {
                    val interval = intervalHours.trim().toIntOrNull()
                    onSave(url.trim(), autoUpdateEnabled, interval)
                },
                enabled = url.isNotBlank(),
            ) {
                Text(stringResource(R.string.action_save))
            }
        },
        dismissButton = {
            Row {
                TextButton(onClick = onClear) {
                    Text(stringResource(R.string.action_clear_subscription))
                }
                TextButton(onClick = onDismiss) {
                    Text(stringResource(R.string.action_cancel))
                }
            }
        },
    )
}

@Composable
private fun ConfirmDeleteDialog(
    profileName: String,
    onDismiss: () -> Unit,
    onConfirm: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.title_delete_profile)) },
        text = { Text(stringResource(R.string.text_delete_profile_confirm, profileName)) },
        confirmButton = {
            TextButton(onClick = onConfirm) {
                Text(stringResource(R.string.action_delete))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.action_cancel))
            }
        }
    )
}

private suspend fun readTextFromUri(context: Context, uri: Uri): String? {
    return withContext(Dispatchers.IO) {
        context.contentResolver.openInputStream(uri)?.bufferedReader()?.use { reader ->
            reader.readText()
        }
    }
}

private fun resolveDisplayName(context: Context, uri: Uri): String? {
    val cursor = context.contentResolver.query(uri, null, null, null, null) ?: return null
    cursor.use {
        val index = it.getColumnIndex(OpenableColumns.DISPLAY_NAME)
        if (index >= 0 && it.moveToFirst()) {
            return it.getString(index)
        }
    }
    return null
}

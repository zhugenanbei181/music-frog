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
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.musicfrog.infiltrator.R
import com.musicfrog.infiltrator.ui.common.EmptyStateText
import com.musicfrog.infiltrator.ui.common.EmptyStateTextKey
import com.musicfrog.infiltrator.ui.common.ErrorDialog
import com.musicfrog.infiltrator.ui.common.LocalizedFailureText
import com.musicfrog.infiltrator.ui.common.StandardListItem
import com.musicfrog.infiltrator.ui.common.toLocalizedFailure
import infiltrator_android.ProfileDetail
import infiltrator_android.ProfileSummary
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

@Composable
internal fun ProfilesScreen(
    viewModel: ProfilesViewModel = viewModel(),
    initialImportUrl: String? = null,
    onImportHandled: () -> Unit = {},
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val drafts = state.drafts
    val collection = state.collection
    val profiles = when (collection) {
        is ProfilesReadState.Loading -> collection.lastAccepted?.value?.profiles.orEmpty()
        is ProfilesReadState.Observed -> collection.point.value.profiles
        is ProfilesReadState.Unavailable -> collection.lastAccepted?.value?.profiles.orEmpty()
    }
    val profileDetail = when (val detail = state.detail) {
        ProfileDetailReadState.Idle -> null
        is ProfileDetailReadState.Loading -> detail.lastAccepted?.value?.profile
        is ProfileDetailReadState.Observed -> detail.point.value.profile
        is ProfileDetailReadState.Unavailable -> detail.lastAccepted?.value?.profile
    }
    val isLoading = state.action is ProfileActionState.Working ||
        (collection is ProfilesReadState.Loading && collection.lastAccepted == null)
    val emptyState = when (collection) {
        is ProfilesReadState.Observed -> if (collection.point.value.profiles.isEmpty()) EmptyStateTextKey.Profiles else null

        is ProfilesReadState.Loading,
        is ProfilesReadState.Unavailable,
        -> null
    }
    val visibleFailure = when (val action = state.action) {
        is ProfileActionState.Failed -> action.failure

        is ProfileActionState.Cancelled -> action.failure

        is ProfileActionState.TimedOut -> action.failure

        is ProfileActionState.Unconfirmed -> action.failure

        is ProfileActionState.Idle,
        is ProfileActionState.Working,
        -> (state.detail as? ProfileDetailReadState.Unavailable)?.failure
    }

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
            viewModel.setLocalImportDraft(
                LocalImportDraft(
                    suggestedName = suggestedName,
                    content = fileContent,
                ),
            )
        }
    }

    LaunchedEffect(initialImportUrl) {
        if (initialImportUrl != null) {
            viewModel.openAddDialog(initialImportUrl)
        }
    }

    Scaffold(
        floatingActionButton = {
            Column(
                verticalArrangement = Arrangement.spacedBy(12.dp),
                horizontalAlignment = Alignment.End,
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
                        viewModel.openAddDialog(null)
                    },
                    containerColor = MaterialTheme.colorScheme.primaryContainer,
                ) {
                    Icon(Icons.Default.Add, contentDescription = stringResource(R.string.title_add_profile))
                }
            }
        },
    ) { padding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
        ) {
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(bottom = 140.dp),
            ) {
                items(profiles, key = { it.name }) { profile ->
                    ProfileRow(
                        profile = profile,
                        onSelect = { viewModel.selectProfile(profile.name) },
                        onUpdate = { viewModel.updateProfile(profile.name) },
                        onEdit = { viewModel.loadProfileDetail(profile.name) },
                        onSubscription = { viewModel.setSubscriptionProfile(profile) },
                        onDelete = { viewModel.setDeletingProfile(profile) },
                    )
                    HorizontalDivider()
                }
            }

            if (isLoading && profiles.isEmpty()) {
                CircularProgressIndicator(modifier = Modifier.align(Alignment.Center))
            }

            if (!isLoading && profiles.isEmpty() && emptyState != null) {
                EmptyStateText(
                    key = emptyState,
                    modifier = Modifier.align(Alignment.Center),
                )
            }

            if (collection is ProfilesReadState.Unavailable) {
                collection.failure.let { failure ->
                    LocalizedFailureText(
                        failure = failure.toLocalizedFailure(),
                        modifier = Modifier.align(Alignment.TopCenter).padding(16.dp),
                    )
                }
            }

            if (visibleFailure != null) {
                ErrorDialog(
                    failure = visibleFailure.toLocalizedFailure(),
                    onDismiss = { viewModel.clearError() },
                )
            }

            if (drafts.showAddDialog) {
                AddProfileDialog(
                    initialUrl = drafts.prefilledUrl ?: "",
                    onDismiss = {
                        viewModel.closeAddDialog()
                        onImportHandled()
                    },
                    onConfirm = { name, url ->
                        viewModel.addProfile(name, url)
                        viewModel.closeAddDialog()
                        onImportHandled()
                    },
                )
            }

            if (profileDetail != null) {
                SaveProfileDialog(
                    title = stringResource(R.string.title_edit_profile),
                    initialName = profileDetail.name,
                    nameEditable = false,
                    initialContent = profileDetail.content,
                    initialActivate = profileDetail.active,
                    onDismiss = { viewModel.clearProfileDetail() },
                    onConfirm = { name, content, activate ->
                        viewModel.saveProfileContent(name, content, activate.toProfileActivation())
                    },
                )
            }

            val localDraft = drafts.localImportDraft
            if (localDraft != null) {
                SaveProfileDialog(
                    title = stringResource(R.string.title_import_local_profile),
                    initialName = localDraft.suggestedName,
                    nameEditable = true,
                    initialContent = localDraft.content,
                    initialActivate = false,
                    onDismiss = { viewModel.setLocalImportDraft(null) },
                    onConfirm = { name, content, activate ->
                        viewModel.saveProfileContent(name, content, activate.toProfileActivation())
                        viewModel.setLocalImportDraft(null)
                    },
                )
            }

            val subProfile = drafts.subscriptionProfile
            if (subProfile != null) {
                SubscriptionSettingsDialog(
                    profile = subProfile,
                    onDismiss = { viewModel.setSubscriptionProfile(null) },
                    onSave = { url, autoUpdate, intervalHours ->
                        viewModel.saveSubscriptionSettings(
                            name = subProfile.name,
                            url = url,
                            updatePolicy = autoUpdate.toSubscriptionUpdatePolicy(),
                            updateIntervalHours = intervalHours,
                        )
                        viewModel.setSubscriptionProfile(null)
                    },
                    onClear = {
                        viewModel.clearSubscriptionSettings(subProfile.name)
                        viewModel.setSubscriptionProfile(null)
                    },
                )
            }

            val delProfile = drafts.deletingProfile
            if (delProfile != null) {
                ConfirmDeleteDialog(
                    profileName = delProfile.name,
                    onDismiss = { viewModel.setDeletingProfile(null) },
                    onConfirm = {
                        viewModel.deleteProfile(delProfile.name)
                        viewModel.setDeletingProfile(null)
                    },
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
    var actionsExpanded by remember(profile.name) { mutableStateOf(false) }
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
                if (profile.active) {
                    Icon(
                        Icons.Default.Check,
                        contentDescription = stringResource(R.string.status_active),
                        tint = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(start = 8.dp, end = 12.dp),
                    )
                }
                Box {
                    IconButton(onClick = { actionsExpanded = true }) {
                        Icon(
                            Icons.Default.Settings,
                            contentDescription = stringResource(R.string.action_subscription_settings),
                        )
                    }
                    DropdownMenu(
                        expanded = actionsExpanded,
                        onDismissRequest = { actionsExpanded = false },
                    ) {
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.action_update_now)) },
                            leadingIcon = { Icon(Icons.Default.Refresh, contentDescription = null) },
                            onClick = {
                                actionsExpanded = false
                                onUpdate()
                            },
                        )
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.action_edit)) },
                            leadingIcon = { Icon(Icons.Default.Edit, contentDescription = null) },
                            onClick = {
                                actionsExpanded = false
                                onEdit()
                            },
                        )
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.action_subscription_settings)) },
                            leadingIcon = { Icon(Icons.Default.Settings, contentDescription = null) },
                            onClick = {
                                actionsExpanded = false
                                onSubscription()
                            },
                        )
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.action_delete)) },
                            leadingIcon = { Icon(Icons.Default.Delete, contentDescription = null) },
                            onClick = {
                                actionsExpanded = false
                                onDelete()
                            },
                        )
                    }
                }
            }
        },
    )
}

@Composable
private fun AddProfileDialog(initialUrl: String = "", onDismiss: () -> Unit, onConfirm: (String, String) -> Unit) {
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
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = url,
                    onValueChange = { url = it },
                    label = { Text(stringResource(R.string.label_url)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        },
        confirmButton = {
            TextButton(
                onClick = { if (name.isNotBlank() && url.isNotBlank()) onConfirm(name, url) },
                enabled = name.isNotBlank() && url.isNotBlank(),
            ) {
                Text(stringResource(R.string.action_add))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.action_cancel))
            }
        },
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
                    modifier = Modifier.fillMaxWidth(),
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
                    horizontalArrangement = Arrangement.SpaceBetween,
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
        },
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
                    style = MaterialTheme.typography.titleSmall,
                )
                OutlinedTextField(
                    value = url,
                    onValueChange = { url = it },
                    label = { Text(stringResource(R.string.label_subscription_url)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
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
                        modifier = Modifier.fillMaxWidth(),
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
private fun ConfirmDeleteDialog(profileName: String, onDismiss: () -> Unit, onConfirm: () -> Unit) {
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
        },
    )
}

private suspend fun readTextFromUri(context: Context, uri: Uri): String? = withContext(Dispatchers.IO) {
    context.contentResolver.openInputStream(uri)?.bufferedReader()?.use { reader ->
        reader.readText()
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

private fun Boolean.toProfileActivation(): ProfileActivation = when (this) {
    true -> ProfileActivation.Active
    false -> ProfileActivation.Inactive
}

private fun Boolean.toSubscriptionUpdatePolicy(): SubscriptionUpdatePolicy = when (this) {
    true -> SubscriptionUpdatePolicy.Enabled
    false -> SubscriptionUpdatePolicy.Disabled
}

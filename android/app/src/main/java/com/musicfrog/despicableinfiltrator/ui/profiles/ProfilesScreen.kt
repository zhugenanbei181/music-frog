package com.musicfrog.despicableinfiltrator.ui.profiles

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.outlined.Description
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
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
import infiltrator_android.ProfileSummary
import com.musicfrog.despicableinfiltrator.R
import com.musicfrog.despicableinfiltrator.ui.common.ErrorDialog
import com.musicfrog.despicableinfiltrator.ui.common.StandardListItem

@Composable
fun ProfilesScreen(
    viewModel: ProfilesViewModel = viewModel(),
    initialImportUrl: String? = null,
    onImportHandled: () -> Unit = {}
) {
    val profiles by viewModel.profiles.collectAsState()
    val isLoading by viewModel.isLoading.collectAsState()
    val error by viewModel.error.collectAsState()
    val emptyMessage by viewModel.emptyMessage.collectAsState()
    
    var showAddDialog by remember { mutableStateOf(false) }
    var prefilledUrl by remember { mutableStateOf<String?>(null) }

    // Handle initial URL from deep link
    LaunchedEffect(initialImportUrl) {
        if (initialImportUrl != null) {
            prefilledUrl = initialImportUrl
            showAddDialog = true
        }
    }

    Scaffold(
        floatingActionButton = {
            FloatingActionButton(
                onClick = { 
                    prefilledUrl = null
                    showAddDialog = true 
                },
                containerColor = MaterialTheme.colorScheme.primaryContainer
            ) {
                Icon(Icons.Default.Add, contentDescription = stringResource(R.string.title_add_profile))
            }
        }
    ) { padding ->
        Box(modifier = Modifier.padding(padding).fillMaxSize()) {
            if (isLoading && profiles.isEmpty()) {
                CircularProgressIndicator(modifier = Modifier.align(Alignment.Center))
            }
            
            LazyColumn {
                items(profiles) { profile ->
                    ProfileRow(
                        profile = profile,
                        onSelect = { viewModel.selectProfile(profile.name) },
                        onUpdate = { viewModel.updateProfile(profile.name) }
                    )
                    HorizontalDivider()
                }
            }

            if (!isLoading && profiles.isEmpty() && emptyMessage != null) {
                Text(
                    text = emptyMessage ?: "",
                    modifier = Modifier.align(Alignment.Center),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }

            if (error != null) {
                ErrorDialog(
                    message = error ?: "",
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
        }
    }
}

@Composable
fun ProfileRow(
    profile: ProfileSummary,
    onSelect: () -> Unit,
    onUpdate: () -> Unit
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
                    Icon(Icons.Default.Refresh, contentDescription = "Update")
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
fun AddProfileDialog(
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

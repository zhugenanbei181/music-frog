package com.musicfrog.despicableinfiltrator.ui.common

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier

@Composable
fun ComingSoonScreen(
    title: String,
    errorMessage: String? = null,
    onDismissError: () -> Unit = {}
) {
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Text("$title (Coming Soon)")
    }

    if (errorMessage != null) {
        ErrorDialog(message = errorMessage, onDismiss = onDismissError)
    }
}

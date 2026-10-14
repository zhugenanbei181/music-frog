package com.musicfrog.infiltrator.ui.common

import android.content.Context
import android.widget.Toast
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import com.musicfrog.infiltrator.R

@Composable
internal fun ErrorDialog(
    failure: LocalizedFailure,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        confirmButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.action_ok)) }
        },
        title = { Text(stringResource(R.string.error_title)) },
        text = { LocalizedFailureText(failure) },
    )
}

fun showToast(context: Context, message: String, long: Boolean = false) {
    val duration = if (long) Toast.LENGTH_LONG else Toast.LENGTH_SHORT
    Toast.makeText(context, message, duration).show()
}

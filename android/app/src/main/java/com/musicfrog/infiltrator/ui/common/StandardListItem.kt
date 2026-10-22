package com.musicfrog.infiltrator.ui.common

import androidx.compose.foundation.clickable
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector

@Composable
fun StandardListItem(
    headline: String,
    supporting: String? = null,
    leadingContent: @Composable (() -> Unit)? = null,
    trailingContent: @Composable (() -> Unit)? = null,
    onClick: (() -> Unit)? = null,
    containerColor: Color = Color.Transparent,
) {
    // EFFECTS-047 (#191): the row action goes through the typed gesture
    // arbitration, so scrolling a list never triggers the row action.
    val gestureArbiter = rememberRowGestureArbiter()
    ListItem(
        headlineContent = { Text(headline) },
        supportingContent = supporting?.let { { Text(it) } },
        leadingContent = leadingContent,
        trailingContent = trailingContent,
        modifier = Modifier.then(
            if (onClick != null) {
                Modifier
                    .arbitratedRowGesture(gestureArbiter)
                    .clickable(
                        onClick = {
                            if (!gestureArbiter.blocksRowAction) {
                                onClick()
                            }
                        },
                    )
            } else {
                Modifier
            },
        ),
        colors = ListItemDefaults.colors(
            containerColor = containerColor,
        ),
    )
}

// Convenience wrapper for Icon
@Composable
fun StandardListItem(
    headline: String,
    supporting: String? = null,
    leadingIcon: ImageVector, // Not nullable, to avoid ambiguity
    trailingContent: @Composable (() -> Unit)? = null,
    onClick: (() -> Unit)? = null,
    containerColor: Color = Color.Transparent,
) {
    StandardListItem(
        headline = headline,
        supporting = supporting,
        leadingContent = { Icon(leadingIcon, contentDescription = null) },
        trailingContent = trailingContent,
        onClick = onClick,
        containerColor = containerColor,
    )
}

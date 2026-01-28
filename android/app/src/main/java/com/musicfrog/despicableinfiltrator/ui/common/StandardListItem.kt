package com.musicfrog.despicableinfiltrator.ui.common

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
    containerColor: Color = Color.Transparent
) {
    ListItem(
        headlineContent = { Text(headline) },
        supportingContent = supporting?.let { { Text(it) } },
        leadingContent = leadingContent,
        trailingContent = trailingContent,
        modifier = Modifier.then(
            if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier
        ),
        colors = ListItemDefaults.colors(
            containerColor = containerColor
        )
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
    containerColor: Color = Color.Transparent
) {
    StandardListItem(
        headline = headline,
        supporting = supporting,
        leadingContent = { Icon(leadingIcon, contentDescription = null) },
        trailingContent = trailingContent,
        onClick = onClick,
        containerColor = containerColor
    )
}
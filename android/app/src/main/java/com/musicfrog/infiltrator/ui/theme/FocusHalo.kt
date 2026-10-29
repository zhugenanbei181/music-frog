package com.musicfrog.infiltrator.ui.theme

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.border
import androidx.compose.foundation.focusable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * Focus halo and scale modifier for Android TV remote controller navigation (FLPARITY-027).
 *
 * Expands element by 1.05x on focus, adds an elevated glow shadow, and renders a crisp accent border ring.
 */
@Composable
fun Modifier.tvFocusHalo(
    shape: Shape = AppShapes.medium,
    scaleOnFocus: Float = 1.05f,
    borderWidth: Dp = 2.dp,
    focusedBorderColor: Color = MaterialTheme.colorScheme.primary,
    focusedShadowElevation: Dp = 8.dp,
    interactionSource: MutableInteractionSource = remember { MutableInteractionSource() },
): Modifier {
    val isFocused by interactionSource.collectIsFocusedAsState()
    val scale by animateFloatAsState(
        targetValue = if (isFocused) scaleOnFocus else 1.0f,
        animationSpec = tween(durationMillis = 150),
        label = "tv_focus_scale",
    )

    return this
        .scale(scale)
        .then(
            if (isFocused) {
                Modifier
                    .shadow(elevation = focusedShadowElevation, shape = shape)
                    .border(width = borderWidth, color = focusedBorderColor, shape = shape)
            } else {
                Modifier
            },
        )
        .focusable(interactionSource = interactionSource)
}

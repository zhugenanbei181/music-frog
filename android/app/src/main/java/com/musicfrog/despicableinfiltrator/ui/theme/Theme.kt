package com.musicfrog.despicableinfiltrator.ui.theme

import android.os.Build
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.graphics.Color

private val LightColors = lightColorScheme(
    primary = Teal,
    onPrimary = Canvas,
    primaryContainer = Color(0xFFBFE8EA),
    onPrimaryContainer = TealDark,
    secondary = Coral,
    onSecondary = Canvas,
    secondaryContainer = Sand,
    onSecondaryContainer = WarmGrey,
    tertiary = Amber,
    onTertiary = Canvas,
    surface = Canvas,
    onSurface = Ink,
    surfaceVariant = SurfaceWarm,
    onSurfaceVariant = WarmGrey,
    background = Canvas,
    onBackground = Ink
)

private val DarkColors = darkColorScheme(
    primary = Color(0xFF7EDCE1),
    onPrimary = Color(0xFF002B2E),
    secondary = Color(0xFFFFB59B),
    onSecondary = Color(0xFF3B1B13),
    tertiary = Color(0xFFE0B868),
    onTertiary = Color(0xFF2B2008),
    surface = Color(0xFF12110F),
    onSurface = Color(0xFFF4F0EB),
    surfaceVariant = Color(0xFF2A2623),
    onSurfaceVariant = Color(0xFFD6CCC4),
    background = Color(0xFF0F0E0D),
    onBackground = Color(0xFFF4F0EB)
)

@Composable
fun InfiltratorTheme(
    darkTheme: Boolean,
    content: @Composable () -> Unit
) {
    val colors = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        val context = LocalContext.current
        if (darkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
    } else {
        if (darkTheme) DarkColors else LightColors
    }

    MaterialTheme(
        colorScheme = colors,
        typography = AppTypography,
        shapes = AppShapes,
        content = content
    )
}

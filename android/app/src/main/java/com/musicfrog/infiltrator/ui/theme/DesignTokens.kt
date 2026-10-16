package com.musicfrog.infiltrator.ui.theme

import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * BANDROID-033.1: Canonical design tokens mapped from `infiltrator_contract::design_tokens`.
 *
 * Provides dp/sp values, 48dp minimum touch targets, and safe area rules
 * identical to the shared contract authority.
 */
object DesignTokens {
    object Space {
        val XS: Dp = 4.dp
        val SM: Dp = 8.dp
        val MD: Dp = 12.dp
        val LG: Dp = 16.dp
        val XL: Dp = 20.dp
        val XXL: Dp = 24.dp
        val ScreenMarginMobile: Dp = 16.dp
        val ScreenMarginTablet: Dp = 24.dp
    }

    object Radius {
        val XS: Dp = 4.dp
        val SM: Dp = 8.dp
        val Control: Dp = 10.dp
        val Card: Dp = 16.dp
        val Modal: Dp = 24.dp
        val BottomSheet: Dp = 28.dp
        val Dialog: Dp = 28.dp
        val Pill: Dp = 999.dp
    }

    object Metrics {
        val Hairline: Dp = 1.dp
        val MinTouchTarget: Dp = 48.dp
        val TouchTargetCompact: Dp = 44.dp
        val TouchTargetComfortable: Dp = 48.dp
        val BottomNavHeight: Dp = 56.dp
        val TopAppBarHeight: Dp = 64.dp
    }

    object TypeScale {
        val Display: TextUnit = 22.sp
        val Title: TextUnit = 20.sp
        val Heading: TextUnit = 20.sp
        val BodyLarge: TextUnit = 16.sp
        val Body: TextUnit = 15.sp
        val LabelLarge: TextUnit = 14.sp
        val Caption: TextUnit = 12.sp
        val LabelSmall: TextUnit = 11.sp
        val Tag: TextUnit = 10.sp
        val Mono: TextUnit = 13.sp
    }
}

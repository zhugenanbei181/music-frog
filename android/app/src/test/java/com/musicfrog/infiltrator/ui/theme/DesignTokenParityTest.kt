package com.musicfrog.infiltrator.ui.theme

import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * BANDROID-033.1: Token parity verification between Kotlin DesignTokens
 * and the shared Rust `infiltrator_contract::design_tokens` specification.
 */
class DesignTokenParityTest {
    @Test
    fun spaceLadderMatchesSharedContract() {
        assertEquals(4.dp, DesignTokens.Space.XS)
        assertEquals(8.dp, DesignTokens.Space.SM)
        assertEquals(12.dp, DesignTokens.Space.MD)
        assertEquals(16.dp, DesignTokens.Space.LG)
        assertEquals(20.dp, DesignTokens.Space.XL)
        assertEquals(24.dp, DesignTokens.Space.XXL)
        assertEquals(16.dp, DesignTokens.Space.ScreenMarginMobile)
        assertEquals(24.dp, DesignTokens.Space.ScreenMarginTablet)
    }

    @Test
    fun radiusLadderMatchesSharedContract() {
        assertEquals(4.dp, DesignTokens.Radius.XS)
        assertEquals(8.dp, DesignTokens.Radius.SM)
        assertEquals(10.dp, DesignTokens.Radius.Control)
        assertEquals(16.dp, DesignTokens.Radius.Card)
        assertEquals(24.dp, DesignTokens.Radius.Modal)
        assertEquals(28.dp, DesignTokens.Radius.BottomSheet)
        assertEquals(28.dp, DesignTokens.Radius.Dialog)
        assertEquals(999.dp, DesignTokens.Radius.Pill)
    }

    @Test
    fun metricsMatchSharedContractAndWcagTouchTarget() {
        assertEquals(1.dp, DesignTokens.Metrics.Hairline)
        assertEquals(48.dp, DesignTokens.Metrics.MinTouchTarget)
        assertEquals(44.dp, DesignTokens.Metrics.TouchTargetCompact)
        assertEquals(48.dp, DesignTokens.Metrics.TouchTargetComfortable)
        assertEquals(56.dp, DesignTokens.Metrics.BottomNavHeight)
        assertEquals(64.dp, DesignTokens.Metrics.TopAppBarHeight)
    }

    @Test
    fun typeScaleMatchesSharedContract() {
        assertEquals(22.sp, DesignTokens.TypeScale.Display)
        assertEquals(20.sp, DesignTokens.TypeScale.Title)
        assertEquals(20.sp, DesignTokens.TypeScale.Heading)
        assertEquals(16.sp, DesignTokens.TypeScale.BodyLarge)
        assertEquals(15.sp, DesignTokens.TypeScale.Body)
        assertEquals(14.sp, DesignTokens.TypeScale.LabelLarge)
        assertEquals(12.sp, DesignTokens.TypeScale.Caption)
        assertEquals(11.sp, DesignTokens.TypeScale.LabelSmall)
        assertEquals(10.sp, DesignTokens.TypeScale.Tag)
        assertEquals(13.sp, DesignTokens.TypeScale.Mono)
    }
}

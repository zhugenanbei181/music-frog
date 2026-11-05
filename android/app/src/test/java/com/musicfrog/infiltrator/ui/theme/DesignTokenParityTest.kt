package com.musicfrog.infiltrator.ui.theme

import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

private val AUTHORITY_CANDIDATES = listOf(
    "crates/infiltrator-contract/assets/design-authority.json",
    "../crates/infiltrator-contract/assets/design-authority.json",
    "../../crates/infiltrator-contract/assets/design-authority.json",
)

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
    fun generatedTypographyMatchesTheContractAuthority() {
        val workingDirectory = File(System.getProperty("user.dir"))
        val authority = AUTHORITY_CANDIDATES
            .asSequence()
            .map { workingDirectory.resolve(it) }
            .firstOrNull(File::isFile)
            ?.readText()
            ?: error("design-authority.json is missing from $workingDirectory")
        val weights = Regex("\"font_weights\"\\s*:\\s*\\[([^]]+)]")
            .find(authority)?.groupValues?.get(1)
            ?.replace(Regex("\\s+"), "")
            ?.trim()
        assertEquals("400,500,600", weights)
        assertTrue(authority.contains("\"android_resource\": \"inter_semibold\""))
        assertTrue(authority.contains("\"role\": \"BodyStrong\""))
        assertTrue(authority.contains("\"semantic\": \"Settings\""))
        assertEquals(22f, TypeScale.Display.value)
        assertEquals(15f, TypeScale.Body.value)
        assertEquals(13f, TypeScale.Mono.value)
    }
}

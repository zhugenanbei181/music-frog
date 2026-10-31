package com.musicfrog.infiltrator.ui.theme

import com.musicfrog.infiltrator.ui.common.parsePublishedThemeOverride
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class PublishedThemeOverrideTest {
    @Test
    fun captureSeedPinsTheFixtureSkinAndUnknownSeedsYieldNull() {
        assertEquals(ThemeOverride.Forest, captureThemeOverride("Forest"))
        assertEquals(ThemeOverride.Amoled, captureThemeOverride("amoled"))
        assertNull(captureThemeOverride(null))
        assertNull(captureThemeOverride("neon"))
    }

    @Test
    fun resolvePrefersCaptureSeedThenPublishedReadbackThenPlatformFollow() {
        assertEquals(ThemeOverride.Light, resolveThemeOverride("light", ThemeOverride.Forest))
        assertEquals(ThemeOverride.Forest, resolveThemeOverride(null, ThemeOverride.Forest))
        assertEquals(ThemeOverride.None, resolveThemeOverride(null, null))
        assertEquals(ThemeOverride.None, resolveThemeOverride(null, ThemeOverride.None))
    }

    @Test
    fun publishedReadbackParsesTheSystemPreferenceAndPinnedSkins() {
        assertEquals(
            ThemeOverride.None,
            parsePublishedThemeOverride(
                """{"origin":"live","theme_readback":{"preference":"system"}}""",
            ),
        )
        assertEquals(
            ThemeOverride.Forest,
            parsePublishedThemeOverride(
                """{"origin":"live","theme_readback":{"preference":{"fixed":"forest"}}}""",
            ),
        )
        assertEquals(
            ThemeOverride.Amoled,
            parsePublishedThemeOverride(
                """{"origin":"live","theme_readback":{"preference":{"fixed":"amoled"}}}""",
            ),
        )
    }

    @Test
    fun unparsableOrNonLiveSnapshotsLeaveTheThemeUnobserved() {
        assertNull(
            parsePublishedThemeOverride(
                """{"origin":"live","theme_readback":{"preference":{"fixed":"neon"}}}""",
            ),
        )
        assertNull(parsePublishedThemeOverride("""{"origin":"live"}"""))
        assertNull(
            parsePublishedThemeOverride(
                """{"origin":"stale","theme_readback":{"preference":"system"}}""",
            ),
        )
        assertNull(parsePublishedThemeOverride("not json"))
    }
}

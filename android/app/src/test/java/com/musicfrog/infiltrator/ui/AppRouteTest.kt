package com.musicfrog.infiltrator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AppRouteTest {
    @Test
    fun systemBackFromSettingsDetailReturnsToSettingsRoot() {
        val detail = AppRoute.SettingsDetail(SettingsDestination.Dns)

        assertEquals(AppRoute.Main(AppSection.AppRouting), detail.back())
        assertEquals(AppSection.AppRouting, detail.selectedSection)
        assertTrue(detail.back().canGoBack)
        assertEquals(AppRoute.Main(AppSection.Overview), detail.back().back())
    }

    @Test
    fun systemBackFromAnotherMainSectionReturnsToOverview() {
        val profiles = AppRoute.Main(AppSection.Profiles)

        assertTrue(profiles.canGoBack)
        assertEquals(AppRoute.Main(AppSection.Overview), profiles.back())
    }

    @Test
    fun systemBackAtOverviewKeepsTheRootRoute() {
        val overview = AppRoute.Main(AppSection.Overview)

        assertFalse(overview.canGoBack)
        assertEquals(overview, overview.back())
    }

    @Test
    fun eachMainAndSettingsRouteHasAnIndependentScrollStateKey() {
        val overview = AppRoute.Main(AppSection.Overview)
        val proxies = AppRoute.Main(AppSection.Proxies)
        val settings = AppRoute.Main(AppSection.AppRouting)
        val dns = AppRoute.SettingsDetail(SettingsDestination.Dns)

        assertEquals("main-Overview", overview.saveableStateKey)
        assertEquals("main-Proxies", proxies.saveableStateKey)
        assertEquals("main-AppRouting", settings.saveableStateKey)
        assertEquals("settings-Dns", dns.saveableStateKey)
        assertFalse(overview.saveableStateKey == proxies.saveableStateKey)
        assertFalse(settings.saveableStateKey == dns.saveableStateKey)
    }
}

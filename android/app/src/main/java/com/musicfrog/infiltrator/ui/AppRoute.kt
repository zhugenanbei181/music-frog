package com.musicfrog.infiltrator.ui

import androidx.compose.runtime.saveable.Saver
import androidx.compose.ui.graphics.vector.ImageVector
import com.musicfrog.infiltrator.R
import com.musicfrog.infiltrator.ui.common.AppIcon
import com.musicfrog.infiltrator.ui.common.AppIconSemantic

internal enum class AppSection(val titleRes: Int, val icon: ImageVector) {
    Overview(R.string.nav_overview, AppIcon.image(AppIconSemantic.Shield)),
    Profiles(R.string.nav_profiles, AppIcon.image(AppIconSemantic.Server)),
    Proxies(R.string.nav_proxies, AppIcon.image(AppIconSemantic.Settings)),
    AppRouting(R.string.nav_settings, AppIcon.image(AppIconSemantic.Settings)),
    Sync(R.string.nav_sync, AppIcon.image(AppIconSemantic.Refresh)),
}

internal enum class SettingsDestination(val titleRes: Int) {
    Routing(R.string.setting_routing_title),
    Tun(R.string.setting_tun_title),
    Dns(R.string.setting_dns_title),
    FakeIp(R.string.setting_fakeip_title),
    Rules(R.string.setting_rules_title),
    Connections(R.string.setting_connections_title),
    Logs(R.string.setting_logs_title),
    About(R.string.setting_about_title),
}

/** Explicit native navigation states and their Android back-stack transition. */
internal sealed interface AppRoute {
    val selectedSection: AppSection
    val titleRes: Int
    val canGoBack: Boolean
    val saveableStateKey: String

    fun back(): AppRoute

    data class Main(val section: AppSection) : AppRoute {
        override val saveableStateKey: String
            get() = "main-${section.name}"

        override val selectedSection: AppSection
            get() = section

        override val titleRes: Int
            get() = section.titleRes

        override val canGoBack: Boolean
            get() = section != AppSection.Overview

        override fun back(): AppRoute = when (section) {
            AppSection.Overview -> this

            AppSection.Profiles,
            AppSection.Proxies,
            AppSection.AppRouting,
            AppSection.Sync,
            -> Main(AppSection.Overview)
        }
    }

    data class SettingsDetail(val destination: SettingsDestination) : AppRoute {
        override val saveableStateKey: String
            get() = "settings-${destination.name}"

        override val selectedSection: AppSection = AppSection.AppRouting
        override val titleRes: Int
            get() = destination.titleRes
        override val canGoBack: Boolean = true

        override fun back(): AppRoute = Main(AppSection.AppRouting)
    }
}

/** Save and restore the exhaustive navigation state across Activity recreation. */
internal val AppRouteSaver: Saver<AppRoute, String> = Saver(
    save = { route -> route.toSaveableValue() },
    restore = ::restoreAppRoute,
)

internal fun AppRoute.toSaveableValue(): String = when (this) {
    is AppRoute.Main -> "main:${section.name}"
    is AppRoute.SettingsDetail -> "settings:${destination.name}"
}

internal fun restoreAppRoute(value: String): AppRoute? {
    val parts = value.split(':', limit = 2)
    if (parts.size != 2) return null
    val (kind, key) = parts
    return when (kind) {
        "main" -> AppSection.values()
            .firstOrNull { it.name == key }
            ?.let(AppRoute::Main)

        "settings" -> SettingsDestination.values()
            .firstOrNull { it.name == key }
            ?.let(AppRoute::SettingsDetail)

        else -> null
    }
}

package com.musicfrog.infiltrator.ui

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.CloudSync
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.Shield
import androidx.compose.material.icons.outlined.Storage
import androidx.compose.material.icons.outlined.Tune
import androidx.compose.runtime.saveable.Saver
import androidx.compose.ui.graphics.vector.ImageVector
import com.musicfrog.infiltrator.R

internal enum class AppSection(val titleRes: Int, val icon: ImageVector) {
    Overview(R.string.nav_overview, Icons.Outlined.Shield),
    Profiles(R.string.nav_profiles, Icons.Outlined.Storage),
    Proxies(R.string.nav_proxies, Icons.Outlined.Tune),
    AppRouting(R.string.nav_settings, Icons.Outlined.Settings),
    Sync(R.string.nav_sync, Icons.Outlined.CloudSync),
}

internal enum class SettingsDestination(val titleRes: Int) {
    Routing(R.string.setting_routing_title),
    Tun(R.string.setting_tun_title),
    Dns(R.string.setting_dns_title),
    FakeIp(R.string.setting_fakeip_title),
    Rules(R.string.setting_rules_title),
    Connections(R.string.setting_connections_title),
    Logs(R.string.setting_logs_title),
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

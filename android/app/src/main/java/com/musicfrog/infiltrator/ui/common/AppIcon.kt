package com.musicfrog.infiltrator.ui.common

import androidx.compose.ui.graphics.vector.ImageVector

/**
 * The only icon resolver on Android. Business screens name an [AppIconSemantic];
 * the canonical Lucide source is the one vector inventory behind every surface.
 */
object AppIcon {
    fun image(semantic: AppIconSemantic): ImageVector = when (semantic) {
        AppIconSemantic.Overview, AppIconSemantic.LayoutGrid -> CanonicalIcons.LAYOUT_GRID
        AppIconSemantic.Proxy, AppIconSemantic.Globe -> CanonicalIcons.GLOBE
        AppIconSemantic.Config, AppIconSemantic.Logs, AppIconSemantic.FileText -> CanonicalIcons.FILE_TEXT
        AppIconSemantic.Settings -> CanonicalIcons.SETTINGS
        AppIconSemantic.Connections, AppIconSemantic.Network -> CanonicalIcons.NETWORK
        AppIconSemantic.Rules, AppIconSemantic.ListChecks -> CanonicalIcons.LIST_CHECKS
        AppIconSemantic.Traffic, AppIconSemantic.Activity -> CanonicalIcons.ACTIVITY
        AppIconSemantic.Search -> CanonicalIcons.SEARCH
        AppIconSemantic.Add, AppIconSemantic.Plus -> CanonicalIcons.PLUS
        AppIconSemantic.Remove, AppIconSemantic.Delete -> CanonicalIcons.TRASH_2
        AppIconSemantic.Edit -> CanonicalIcons.PENCIL
        AppIconSemantic.Copy -> CanonicalIcons.COPY
        AppIconSemantic.Refresh -> CanonicalIcons.REFRESH_CW
        AppIconSemantic.Start, AppIconSemantic.Zap -> CanonicalIcons.ZAP
        AppIconSemantic.Stop, AppIconSemantic.Square, AppIconSemantic.Check -> CanonicalIcons.SQUARE
        AppIconSemantic.ChevronRight -> CanonicalIcons.CHEVRON_RIGHT
        AppIconSemantic.ChevronLeft -> CanonicalIcons.CHEVRON_LEFT
        AppIconSemantic.ChevronUp -> CanonicalIcons.CHEVRON_UP
        AppIconSemantic.ChevronDown, AppIconSemantic.Expand -> CanonicalIcons.CHEVRON_DOWN
        AppIconSemantic.Close, AppIconSemantic.X -> CanonicalIcons.X
        AppIconSemantic.Shield -> CanonicalIcons.SHIELD
        AppIconSemantic.Server -> CanonicalIcons.SERVER
        AppIconSemantic.Target -> CanonicalIcons.TARGET
        AppIconSemantic.Minus -> CanonicalIcons.MINUS
        AppIconSemantic.ArrowUp -> CanonicalIcons.ARROW_UP
        AppIconSemantic.ArrowDown -> CanonicalIcons.ARROW_DOWN
        AppIconSemantic.Code2 -> CanonicalIcons.CODE_2
        AppIconSemantic.Wifi -> CanonicalIcons.WIFI
        AppIconSemantic.Plug -> CanonicalIcons.PLUG
        AppIconSemantic.Pin -> CanonicalIcons.PIN
    }
}

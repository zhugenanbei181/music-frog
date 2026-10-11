# Lumen adaptations to Slint Material Components 1.1.0

> 层级：L3

The library is vendored from the official 1.1.0 release and retains its MIT license.

`ui/styling/material_palette.slint` exposes the existing platform-selected
`Palette.color_scheme` as a read-only `MaterialPalette.color-scheme` output so
the MusicFrog root can follow desktop/Android system light and dark changes when
the shared theme preference is `system`. The original palette selection and
color values remain unchanged.

`ui/components/navigation_bar.slint` adds opt-in `compact` and `show_layout_bounds` inputs alongside `border_radius`. The compact phone bar is 56 px high, with 22 px icons, 12 px labels, and a 48 px high selected pill. After 4 px padding on each side, every item gets exactly one equal share of the available width. It retains BaseNavigation accessibility and ExtendedTouchArea keyboard activation. The regular navigation/rail branch remains available. The compact branch bypasses the regular icon row's 20 px padding on each side: four 26+40 px items, 24 px of gaps, and 16 px outer padding previously required at least 304 px inside a roughly 280 px bar. That minimum-size conflict clipped the fourth item; clip alone hid the error. Debug frames now show the actual slot, icon, and label bounds.

`ui/components/app_bar.slint` makes `show_background: false` transparent, so large page titles share the page canvas instead of forming a separate white band.

`LargeAppBar` adds an opt-in `compact` input that slightly reduces only its bottom padding while keeping the title baseline fixed. The Android Settings page uses it because its first section begins closer to the title in the supplied reference; Home and Config keep the original Material spacing.

`ui/components/bottom_sheet.slint` exposes `scrim_color`, `surface_color`, `corner_radius`, `side_inset`, and `sheet_elevation_level` so the same official Material bottom sheet can match the active light/dark theme and different reference widths. All four surface corners use the configured radius; the shadow uses the same radius. Its default phone width remains the available window minus 48 px total (24 px each side); all Config sheets use 12 px inset where specified, level 1 elevation for the reference's subtle edge shadow, and their page-specific 28/32 px corner radii. `bottom_safe_inset` keeps the complete surface and its elevation shadow above Android's bottom gesture area; the default is zero for consumers that do not use a full-window PopupWindow. The sheet retains its drag handle and dismissal behavior.

The bottom sheet inserts its children through the Material `Horizontal` layout, which reserves 8 px on every side. Sheet content containers must therefore use the allocated inner width (sheet width minus 16 px horizontally and vertically), and nested fields must size from their immediate content parent. The PopupWindow covers the full app window, while the visible Sheet surface ends at `Window.safe-area-insets.bottom`. `ModalBottomSheet.bottom_safe_inset` is passed from the app and applied to initial placement, content-height changes, and the drag-to-dismiss threshold. Position the sheet using its actual measured height, which includes its 36 px handle and the Material wrapper's vertical padding; do not align the inner content rectangle to the screen edge. A content-height change reanchors the surface unless the user is actively dragging it, so changing Profile Settings tabs cannot push the bottom edge below the safe area. All Config sheets use elevation level 1 to keep their reference shadow subtle. With `show_layout_bounds`, a cyan frame outlines the PopupWindow and a magenta frame follows the sheet surface during drag. Lumen applies these rules to Add Profile, Share, Delete, Edit and Profile Settings.

`ui/components/icon.slint` exposes an explicit `size` input for the regular navigation branch. Compact navigation uses the existing selected/inactive static SVG assets directly at 22 px, with each frame positioned from its allocated slot.

`ui/components/modal.slint` forwards a configurable `scrim_color` to its overlay. This lets day/night sheets preserve their underlying page theme while matching the opacity in the references.

`ui/styling/material_palette.slint` raises the modal scrim opacity to 94% to match the supplied add-profile sheet screenshot, where the page behind the sheet is almost fully dimmed.

`ui/components/menu.slint` exposes row heights, width limits, and layout-debug bounds. The Proxy sort menu uses a 56 px locate row, a 1 px divider, and three 48 px sorting rows. Its PopupWindow width tracks about 54% of the safe-area width, clamped to 200–220 px; its height is computed from row tokens and padding because PopupWindow has independent geometry. Rows use 20 px left and 24 px right insets. Text takes the row's remaining width after the fixed trailing check slot, and both frames share the row centerline. Debug bounds show the popup, rows, text frames, and check frames.

The Add Profile type selector is rendered as an in-Sheet Slint overlay with 56 px rows. A separate `PopupWindow` caused taps on a type option to close its parent Sheet on Android; keeping the menu inside the Sheet preserves selection state and lets Android Back dismiss the menu before the Sheet.

The Proxy sort popup keeps the reference menu hierarchy: a “locate current node” command, a divider, then default/name/latency sorting with a trailing check icon on the active sort. Refresh and Providers shortcuts are omitted from this popup; Providers remain reachable from Settings > Meta. `PopupMenu.divider_before_index`, its selected check, explicit row geometry, and per-element debug outlines are local additions to the vendored menu component.

The software renderer does not support `transform-rotation`, so the Proxy-node Ping clock uses a short sequence of static SVG rotation frames driven by a Slint Timer that runs only during a measurement. `Image.colorize` applies the pending/result semantic color. `flags/*.svg` remain the canonical Twemoji source; `tools/render-flags.ps1` uses the project-locked resvg 0.48.1 renderer to produce 144×144 PNGs with a true circular alpha edge. Slint loads those PNGs so the Android software renderer cannot fall back to the source's rounded-rectangle outline. The rounded-square flag badge remains a separate background element, and the original flag Emoji remains in accessible node names.

UI-16 reuses the existing four-corner ModalBottomSheet for User-Agent and icon previews. No library Switch change: only AppPreferenceSwitch in ui/app.slint uses a borderless 54×31 capsule with equal 22px thumbs to match the supplied application settings crops; it retains keyboard and accessibility semantics. AppPreferenceGroup dimensions and colors come from LumenTheme.

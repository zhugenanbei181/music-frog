# Asset catalog

> 层级：L3

`assets/manifest.json` is the single inventory for visual assets bundled with Lumen. Update it in the same change that adds, replaces, renames, or removes an asset. Use an existing asset path when refreshing the same symbol; delete obsolete files instead of leaving a second implementation beside the new one.

## Directories and formats

| Location / format | Use | Current owner |
|---|---|---|
| `icons/*.svg` | Functional and navigation icons. SVGs are static exports from the Lucide set distributed through Iconify; source SVGs are checked into the repository, so the app never fetches icons at runtime. | `ui/app.slint` |
| `flags/*.svg` | Canonical full-color Twemoji source artwork, keyed by lowercase ISO region code. Source SVG clipping is not used at runtime because the Android Slint software renderer displayed these source flags as rounded rectangles. | `tools/render-flags.ps1` |
| `flags/circle-png/*.png` | Checked-in 144×144 runtime renders of the source SVGs, with a true circular alpha mask. The image itself is circular; the node badge behind it remains a separate rounded square. | `ui/flag-icons.slint` maps the PNGs to Slint images; `src/flag_assets.rs` maps codes to the same stable indices. Both lists must remain identical and ordered. |
| UTF-8 text / emoji | Text labels, user-provided node/group names, punctuation, and the original region-flag Emoji kept in accessible names. Unicode symbols are not used as substitutes for functional UI icons. | Slint text properties and Rust UI state |
| PNG / raster | Runtime flag PNGs are the only current raster visuals. They are derived from licensed Twemoji SVG sources solely to guarantee the circular alpha crop in Slint's software renderer; functional icons remain SVG. Genuine photos or illustrations, if added, belong under `raster/` with source, license, and logical display size recorded in the manifest. | `ui/flag-icons.slint` |
| `third_party_licenses.json` | Generated license-catalog data displayed by the About page; it is data, not a visual asset collection. | Rust build output and the About/License pages |

## Change rules

1. Choose the format by the kind of content: functional symbol → SVG; user text/required Emoji semantics → UTF-8 text; photographic/raster content → PNG or another documented raster format.
2. Keep one canonical asset for a symbol and intentional variants only (for example, inactive/active navigation icons or inverse white icons). Reuse that path in every page that needs the same artwork; do not copy it under a page-specific filename. Pair every variant in `manifest.json` so the relationship is explicit.
3. Record the exact repository-relative path, symbol/source, license, intended role, and code consumer. Preserve the license text and any required attribution alongside the assets.
4. Check all Slint/Rust references when replacing a path, then remove the obsolete file in the same change. Do not add alternate directories or name-based fallbacks for the same UI icon.
5. Keep region flags as an explicit source-to-runtime asset pair: Twemoji SVG source under `flags/`, circular alpha-masked 144×144 PNG runtime render under `flags/circle-png/`. Regenerate with `pwsh -NoProfile -File tools/render-flags.ps1`; do not rely on SVG `clipPath` or a rectangular Slint clip to create a circle. Keep the original flag Emoji in node names/accessibility text. Do not add ordinary feature Emoji icons.

Before editing, read [manifest.json](manifest.json). It lists the exact files used by the current UI and the paired variants. Run `pwsh -NoProfile -File tools/check-assets.ps1` after changing resources or references; it rejects missing, unlisted, unused, byte-identical duplicate icon files, and duplicate manifest entries, and checks the shared flag order and runtime raster set.

UI-16 icon selection currently reuses nine registered SVGs as identity previews. Future user-supplied high-resolution app-identity PNGs must be recorded separately in manifest.json raster_assets before use; they do not replace functional SVGs. The palette action uses the canonical icons/palette.svg (Lucide/ISC).

When replacing UI-16 SVG identity placeholders with user PNGs, update the icon grid's registered image sources and disable colorize for PNG slots to preserve their original colors; retain the shared logical image frame and contain fit. Add each PNG to manifest.json raster_assets with its source/license metadata.

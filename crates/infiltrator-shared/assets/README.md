# Shared visual assets

> 层级：L3

`asset-catalog.json` is the single inventory for resources shared across the
Iced, Bevy, Slint, Android, and packaged desktop products. Every record has a
stable semantic ID, output format, intentional variant, relative asset path,
source description, and license pointer.

`infiltrator_shared::assets::AssetCatalog` is the renderer-neutral lookup.
Iced SVGs, Bevy plate PNGs, and product launcher art also have embedded-byte
adapters. Slint compiles the same Lumen SVG and flag files directly from this
directory; platform packagers use the PNG and ICO catalog paths.

| Directory | Content |
| --- | --- |
| `product/` | Shared product PNG and Windows ICO |
| `fonts/` | Embedded Inter, JetBrains Mono, and CJK faces with license texts |
| `ui/iced/svg/` | Canonical vector icon source for the Iced surface |
| `ui/bevy/png/` | White-ink raster variants generated from matching SVG symbols |
| `ui/lumen/svg/` | Imported Lumen vector UI icons and their license text |
| `flags/` | Imported full-color SVG flag sources and circular PNG runtime variants |

`fonts/HarmonyOS-Sans-LICENSE.txt` is the full agreement for `CJK-Regular.ttf`.
That face is embedded only by Iced and Bevy; both surfaces show the required
Huawei attribution in their shell/settings UI. Slint uses platform fonts.

Add or replace a resource in one change with its catalog record, source/license
metadata, and all consumers. Use a new variant only when a renderer needs a
different representation of the same semantic asset. Do not duplicate files
under a surface crate or add runtime network fetches.

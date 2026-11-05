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
| `fonts/` | Embedded UI runtime faces, design font collection, and font registry |
| `ui/iced/svg/` | Canonical vector icon source for the Iced surface |
| `ui/bevy/png/` | White-ink raster variants generated from matching SVG symbols |
| `ui/lumen/svg/` | Platform-native Slint ping animation frames and their license text |
| `flags/` | Imported full-color SVG flag sources and circular PNG runtime variants |

## Typography and Font Asset Library

1. **Shipped UI Runtime Faces**:
   `fonts/Inter-Regular.ttf`, `fonts/Inter-Medium.ttf`, `fonts/Inter-SemiBold.ttf`,
   `fonts/JetBrainsMono-Regular.ttf`, and `fonts/CJK-Regular.ttf` (HarmonyOS Sans SC)
   serve as the physical faces bound to `infiltrator_contract::design_resources::FontFaceToken`.
   Their license agreements are preserved in `fonts/*-OFL.txt` and
   `fonts/HarmonyOS-Sans-LICENSE.txt`.

2. **Design Collection Fonts (`fonts/collection/`)**:
   Acquired from design research in `dongxinanbei`, covering 22 OFL design font families
   (29 complete TTF/OTF originals and their individual `OFL.txt` files), plus 5 base
   standalone fonts (`NotoSerifCJKsc-Regular.otf`, `NotoSansCJKsc-Regular.otf`,
   `LongCang-Regular.ttf`, `MaShanZheng-Regular.ttf`, `ZCOOLXiaoWei-Regular.ttf`) and
   3 local commercial candidate families in `fonts/base-local/`.

3. **Font Registry & Research**:
   - `fonts/registry/FONT-ASSETS.json`: The machine-readable registry recording SHA256
     checksums, byte lengths, mapped codepoint counts, upstream repository sources, and
     exact license paths for all 37 complete font originals.
   - `docs/FONT_DESIGN.md`: Detailed research on aesthetic choices, pairing recommendations,
     character coverage limitations, Han dynasty atmospheric pairings, and performance budgeting.
   - `scripts/font-assets.py`: Verification utility to validate font integrity and licensing
     with `python3 scripts/font-assets.py --check-collection`.

Add or replace a resource in one change with its catalog record, source/license
metadata, and all consumers. Use a new variant only when a renderer needs a
different representation of the same semantic asset. Do not duplicate files
under a surface crate or add runtime network fetches.

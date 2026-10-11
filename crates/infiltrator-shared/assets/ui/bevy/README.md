# Bevy icon plates

> 层级：L3

The 64px RGBA PNG plates in `png/` are the white-ink renderer variants for
`infiltrator-bevy-widgets`. They are embedded through `infiltrator-shared::assets`
and decoded into Bevy's image store; there is no runtime asset-root lookup.

Each plate is rasterized from the matching canonical Iced Lucide SVG. The
stable semantic name, format variant, and ISC license record live in the
adjacent shared asset catalog. Preserve the checked-in `LUCIDE-LICENSE.txt`
when changing this set.

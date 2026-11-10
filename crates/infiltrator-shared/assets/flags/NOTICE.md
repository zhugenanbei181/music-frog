# Country flag artwork

> 层级：L3

This directory is the single national-flag inventory. Every SVG shares one
geometry: a `0 0 640 480` viewBox whose plate is a full-bleed 4:3 rectangle
with square corners. A flag is rectangular artwork, and a rounded or circular
flag container reads as a medallion at UI scale.

## Primary inventory: flag-icons

255 of the 258 SVGs come from [flag-icons](https://github.com/lipis/flag-icons)
(`flags/4x3/`), renamed to the ISO 3166-1 alpha-2 codes Lumen looks up. The
set is MIT-licensed; the license text is kept beside this notice as
`LICENSE-FLAG-ICONS`.

Attribution: “Flag Icons by Panayiotis Lipiridis, licensed under MIT.”

## Three squared derivatives

`ac.svg`, `ea.svg`, and `ta.svg` are not published by flag-icons. They are
derivatives of the former Twemoji artwork (CC BY 4.0, license kept as
`LICENSE-GRAPHICS`): the rounded 36x26 plate is replaced by the same
full-bleed 640x480 rectangle, and the emblem group is scaled onto that plate.

Attribution for those three files: “Flag Emoji artwork by Twemoji
contributors, licensed under CC BY 4.0.”

#!/usr/bin/env python3
"""SLINT-021: the light/dark fact has one owner and one channel into Material.

Measured on the published `proxies-dark` receipt before this slice: the page
header action ink was rgb(65, 73, 75) on a rgb(10, 11, 14) page - about 2.14:1 -
while the surface's own dark `onSurfaceVariant` token is `#A5B0BC` at about
7.4:1. Every `@material` component (9 `IconButton` sites, `Switch`, `PopupMenu`,
`ModalBottomSheet`, ...) painted its LIGHT scheme inside a DARK page.

The cause was not a colour value: it was a second owner for one fact. The
application resolves light/dark once (the user's theme preference; the platform
is consulted only when that preference is `system`) and publishes it as
`LumenTheme.dark-mode`. The vendored library re-derived the same fact from the
platform `Palette.color_scheme`, so the two disagreed whenever the preference
disagreed with the platform.

The rules this guard holds:

1. `MaterialPalette` exposes `application-dark-mode` as an input, and its
   `scheme` selector reads it - never `Palette.color_scheme`.
2. `Elevation.dark` reads the same channel, so a shadow never dims for a light
   page it is not on.
3. The root window feeds that channel everywhere it feeds `LumenTheme.dark-mode`
   - one owner, and every consumer written in the same breath.
4. `MaterialPalette.color-scheme` stays the platform observation, because the
   root seeds `platform-dark-mode` from it; that is the one legitimate read.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
PALETTE = ROOT / "crates/infiltrator-slint/lumen-source/material-1.1.0/ui/styling/material_palette.slint"
ELEVATION = ROOT / "crates/infiltrator-slint/lumen-source/material-1.1.0/ui/components/elevation.slint"
APP = ROOT / "crates/infiltrator-slint/ui/lumen-app.slint"

DARK_MODE_WRITE = re.compile(r"LumenTheme\.dark-mode\s*=\s*([^;]+);")
CHANNEL_WRITE = re.compile(r"MaterialPalette\.application-dark-mode\s*=\s*([^;]+);")
SCHEME_SELECTOR = re.compile(r"property <MaterialScheme> scheme:\s*([^;]+);")
ELEVATION_DARK = re.compile(r"property <bool> dark:\s*([^;]+);")


def violations(palette: str, elevation: str, app: str) -> list[str]:
    found: list[str] = []

    if "in property <bool> application-dark-mode" not in palette:
        found.append(
            "MaterialPalette declares no `application-dark-mode` input, so the "
            "application cannot publish its light/dark fact to Material"
        )

    selector = SCHEME_SELECTOR.search(palette)
    if selector is None:
        found.append("MaterialPalette has no `scheme` selector to inspect")
    elif "Palette.color_scheme" in selector.group(1):
        found.append(
            "MaterialPalette.scheme re-derives light/dark from the platform "
            f"`Palette.color_scheme` instead of `application-dark-mode`: {selector.group(1).strip()}"
        )
    elif "application-dark-mode" not in selector.group(1):
        found.append(
            f"MaterialPalette.scheme does not read the single channel: {selector.group(1).strip()}"
        )

    dark = ELEVATION_DARK.search(elevation)
    if dark is None:
        found.append("Elevation has no `dark` property to inspect")
    elif "Palette.color_scheme" in dark.group(1):
        found.append(
            "Elevation.dark re-derives light/dark from the platform instead of "
            f"reading MaterialPalette: {dark.group(1).strip()}"
        )

    owners = DARK_MODE_WRITE.findall(app)
    channels = CHANNEL_WRITE.findall(app)
    if not owners:
        found.append("the root window no longer publishes LumenTheme.dark-mode at all")
    if len(channels) != len(owners):
        found.append(
            f"the root writes LumenTheme.dark-mode {len(owners)} time(s) but "
            f"MaterialPalette.application-dark-mode {len(channels)} time(s); every "
            "writer of the fact must feed Material in the same breath"
        )
    missing = sorted({value.strip() for value in owners} - {value.strip() for value in channels})
    if missing:
        found.append(
            "expression(s) published as the app dark mode never reach Material: " + ", ".join(missing)
        )
    return found


def read(path: pathlib.Path) -> str:
    return path.read_text(encoding="utf-8")


def live_violations() -> list[str]:
    return violations(read(PALETTE), read(ELEVATION), read(APP))


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        self.palette = read(PALETTE)
        self.elevation = read(ELEVATION)
        self.app = read(APP)

    def findings(self, palette=None, elevation=None, app=None) -> list[str]:
        return violations(
            self.palette if palette is None else palette,
            self.elevation if elevation is None else elevation,
            self.app if app is None else app,
        )

    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(live_violations(), [])

    def test_the_platform_selector_coming_back_is_caught(self) -> None:
        planted = self.palette.replace(
            "property <MaterialScheme> scheme: root.application-dark-mode",
            "property <MaterialScheme> scheme: Palette.color_scheme == ColorScheme.dark",
            1,
        )
        self.assertNotEqual(planted, self.palette)
        self.assertTrue(
            any("re-derives light/dark from the platform" in item for item in self.findings(palette=planted)),
            self.findings(palette=planted),
        )

    def test_a_channel_that_selects_nothing_is_caught(self) -> None:
        planted = SCHEME_SELECTOR.sub("property <MaterialScheme> scheme: root.schemes.dark;", self.palette, count=1)
        self.assertTrue(
            any("does not read the single channel" in item for item in self.findings(palette=planted)),
            self.findings(palette=planted),
        )

    def test_elevation_reading_the_platform_again_is_caught(self) -> None:
        planted = self.elevation.replace(
            "property <bool> dark: MaterialPalette.application-dark-mode;",
            "property <bool> dark: Palette.color_scheme == ColorScheme.dark;",
            1,
        )
        self.assertNotEqual(planted, self.elevation)
        self.assertTrue(
            any("Elevation.dark re-derives" in item for item in self.findings(elevation=planted)),
            self.findings(elevation=planted),
        )

    def test_a_missing_channel_input_is_caught(self) -> None:
        planted = self.palette.replace("in property <bool> application-dark-mode", "in property <bool> unused-flag", 1)
        self.assertTrue(
            any("declares no `application-dark-mode` input" in item for item in self.findings(palette=planted)),
            self.findings(palette=planted),
        )

    def test_an_unfed_dark_mode_writer_is_caught(self) -> None:
        planted = self.app.replace(
            "function update-color-scheme() { LumenTheme.dark-mode = root.application-dark-mode;",
            "function update-color-scheme() { LumenTheme.dark-mode = root.application-dark-mode; // dropped",
            1,
        ).replace(
            "MaterialPalette.application-dark-mode = root.application-dark-mode; root.color-scheme-changed",
            "root.color-scheme-changed",
            1,
        )
        self.assertNotEqual(planted, self.app)
        self.assertTrue(
            any("must feed Material in the same breath" in item for item in self.findings(app=planted)),
            self.findings(app=planted),
        )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("enforce", "report"), default="enforce")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    found = live_violations()
    for item in found:
        print(f"SLINT-021 [{args.mode}]: {item}")
    print(f"slint theme channel guard: findings={len(found)}")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())

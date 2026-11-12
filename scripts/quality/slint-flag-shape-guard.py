#!/usr/bin/env python3
"""SLINT-020: national flags are rectangular artwork, painted from one inventory.

User feedback 2026-11-10: "一般国旗都是方形 最好不要做这种圆形旗帜图案容器" -
the receipt showed the Hong Kong flag as a red medallion. Three things were
wrong at once, and they were not cosmetic:

* the surface drew the flag inside `border-radius: diameter / 2` + `clip: true`
  on top of an asset that was ALREADY a circle, so the artwork was clipped
  twice and the name of the size input (`diameter`) described the clip rather
  than the plate;
* the inventory it drew from was the 144px circular raster derivative, a second
  copy of artwork the shared catalog already carries as `flag.<code>` /
  variant `source` (258 square Twemoji vectors). Two inventories for one fact
  is exactly what the one-source-of-truth rule forbids;
* the chip the two home rows then consumed was a *square ground* whose corner
  was the reference tile's 31% of the side. The shared assets are 36x36
  canvases whose plate is a 36x26 rectangle in rows 5..31, so that ground
  showed as two bands of chip paint above and below the plate and the whole
  slot still read as a medallion. The receipt of the second pass showed the
  same complaint, which is why the ground is now the fallback's alone and the
  corner is held at or below an eighth of the chip side.

The same feedback held a third time (2026-11-11, the receipt of that second
pass): keeping the artwork's own 4-of-36 corner still read as a container at
UI scale, because a rounded rectangle around a flag IS the container. The
inventory was therefore replaced by the square-corner flag-icons 4:3 set, the
chip declares no corner at all, and the corner token is retired so no surface
can reintroduce one. FlClash paints no circular flag either: its rows carry an
emoji or a two-letter text badge, so a rounded plate was never a parity
decision.

The rules this guard holds:

1. `FlagImages.flags` lists exactly the catalog's `flag.*` / `source` codes, in
   catalog order, as `.svg` - and `flag_assets::index_for_code` agrees, because
   the index is the only bridge between the owner's ISO code and this list.
2. The shared circular raster derivative is gone: no `circle-144` record in the
   catalog and no `flags/circle-png/` directory. The verbatim Lumen import under
   `lumen-source/` keeps its own copy - that is the vendor snapshot, not ours.
3. `FlagIcon` draws a square plate: no clip, no radius, `image-fit: contain`,
   and its size input is `size`, whose height is the artwork's own 3/4 ratio.
   `diameter` is a retired entry, so a call site that feeds it again is caught
   rather than silently ignored.
4. A call site never wraps a plate in a container of its own: the two home
   rows each declared a private `Rectangle` with a `background`, which is how
   they drifted from the shared chip and kept a circle nobody else drew. The
   shared `CountryChip` is the only element that may paint behind a plate.
5. No container that holds a plate may declare a corner at all. A rectangle
   inside a rounded container reads as a medallion whatever the radius is, so
   the eighth-of-the-side allowance is gone with the rounded artwork.
6. `LumenTheme.country-chip-radius` is retired: a corner token that exists is a
   corner waiting to be spent, and the chip must not name one.
7. The chip declares its ground (`background`) conditionally on the flag
   index: a resolved flag is the whole visual, and only the code fallback
   paints a tile.
8. The inventory is one geometry: every `flags/*.svg` is a `0 0 640 480`
   viewBox carrying a full-bleed plate, and no file reintroduces the Twemoji
   rounded-plate curve that started this.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
import unittest
import xml.etree.ElementTree as ET

ROOT = pathlib.Path(__file__).resolve().parents[2]
SLINT = ROOT / "crates/infiltrator-slint/ui/lumen-flags.slint"
RUST = ROOT / "crates/infiltrator-slint/src/flag_assets.rs"
CATALOG = ROOT / "crates/infiltrator-shared/assets/asset-catalog.json"
SHARED_CIRCLE_DIR = ROOT / "crates/infiltrator-shared/assets/flags/circle-png"
FLAGS_DIR = ROOT / "crates/infiltrator-shared/assets/flags"
UI_ROOT = ROOT / "crates/infiltrator-slint/ui"
PILL = ROOT / "crates/infiltrator-slint/ui/components/proxy_picker.slint"
CHIP = ROOT / "crates/infiltrator-slint/ui/components/country_chip.slint"
THEME = ROOT / "crates/infiltrator-slint/ui/components/theme.slint"

SVG_ENTRY = re.compile(r'@image-url\("\.\./\.\./infiltrator-shared/assets/flags/([a-z]{2})\.svg"\)')
CIRCLE_ENTRY = re.compile(r"flags/circle-png/([a-z]{2})\.png")
RUST_INDEX = re.compile(r'"([a-z]{2})" => Some\((\d+)\)')
COMPONENT_RE = re.compile(r"export component FlagIcon inherits Rectangle \{")
CHIP_COMPONENT_RE = re.compile(r"export component CountryChip inherits Rectangle \{")
RETIRED_SIZE_ENTRY = re.compile(r"\bdiameter\s*:")
FLAG_CHILD_RE = re.compile(r"(?:^|:)\s*FlagIcon$")
FLAG_SLOT_RE = re.compile(r"\b(?:flag-index|current-flag-index)\b")
CORNER_DECL_RE = re.compile(r"\bborder-radius\s*:\s*([^;]+);")
FLAG_VIEWBOX = "0 0 640 480"
ROUNDED_PLATE_MARKER = "2.209-1.791 4-4 4"


def mask_comments(source: str) -> str:
    out: list[str] = []
    in_block = False
    index = 0
    while index < len(source):
        if not in_block and source[index : index + 2] == "//":
            end = source.find("\n", index)
            index = len(source) if end == -1 else end
            continue
        if source[index : index + 2] == "/*":
            in_block = True
            index += 2
            continue
        if in_block:
            if source[index : index + 2] == "*/":
                in_block = False
                index += 2
            else:
                index += 1
            continue
        out.append(source[index])
        index += 1
    return "".join(out)


def catalog_square_codes() -> list[str]:
    records = json.loads(CATALOG.read_text(encoding="utf-8"))["assets"]
    return [
        record["id"].removeprefix("flag.")
        for record in records
        if record["id"].startswith("flag.")
    ]


def component_body(code: str, pattern: re.Pattern[str] = COMPONENT_RE) -> str:
    start = pattern.search(code)
    if start is None:
        return ""
    depth = 0
    index = start.end() - 1
    while index < len(code):
        if code[index] == "{":
            depth += 1
        elif code[index] == "}":
            depth -= 1
            if depth == 0:
                return code[start.start() : index + 1]
        index += 1
    return code[start.start() :]


def flag_icon_blocks(code: str) -> list[str]:
    """Every `FlagIcon { ... }` element body written by a call site."""
    blocks: list[str] = []
    for match in re.finditer(r"FlagIcon\s*\{", code):
        depth = 0
        index = match.end() - 1
        while index < len(code):
            if code[index] == "{":
                depth += 1
            elif code[index] == "}":
                depth -= 1
                if depth == 0:
                    blocks.append(code[match.start() : index + 1])
                    break
            index += 1
    return blocks


def call_site_findings(files: dict[pathlib.Path, str]) -> list[str]:
    """Call sites may only feed `size`; `diameter` is a retired entry."""
    found: list[str] = []
    for path, text in sorted(files.items()):
        for block in flag_icon_blocks(mask_comments(text)):
            if RETIRED_SIZE_ENTRY.search(block):
                found.append(
                    f"{path.relative_to(ROOT)}: a FlagIcon call site feeds the retired "
                    "`diameter` entry; the plate side is `size`, and a second size name "
                    "would be a silently-ignored input"
                )
    return found


def mask_strings(source: str) -> str:
    """Blank string literal contents, so a brace or `;` inside text is not syntax."""
    out: list[str] = []
    in_string = False
    for char in source:
        if char == '"':
            in_string = not in_string
            out.append(" ")
            continue
        out.append(" " if in_string else char)
    return "".join(out)


def brace_blocks(text: str) -> list[dict]:
    """Every `Header { body }` block, carrying the header that opened it.

    A plate's container is the block that holds the plate as a *direct* child,
    so these checks need the nesting rather than a flat search: the row that
    holds the chip in `cards.slint` paints a background too, and it is not what
    wraps a flag.
    """
    roots: list[dict] = []
    stack: list[dict] = []
    header_start = 0
    for index, char in enumerate(text):
        if char == "{":
            block: dict = {"header": text[header_start:index].strip(), "open": index, "children": []}
            (stack[-1]["children"] if stack else roots).append(block)
            stack.append(block)
            header_start = index + 1
        elif char == "}":
            if stack:
                stack.pop()["close"] = index
            header_start = index + 1
        elif char == ";":
            header_start = index + 1
    return roots


def all_blocks(text: str) -> list[dict]:
    found: list[dict] = []
    pending = brace_blocks(text)
    while pending:
        block = pending.pop()
        if "close" in block:
            found.append(block)
        pending.extend(block["children"])
    return found


def own_declarations(block: dict, text: str) -> str:
    """A block's own statements, with every nested child block cut out."""
    pieces: list[str] = []
    cursor = block["open"] + 1
    for child in block["children"]:
        if "close" not in child:
            continue
        pieces.append(text[cursor : child["open"]])
        cursor = child["close"] + 1
    pieces.append(text[cursor : block["close"]])
    return "".join(pieces)


def container_corner(own: str) -> str | None:
    """The radius literal of a container that rounds itself, whatever it is.

    The old rule allowed a corner up to an eighth of the side, because the
    artwork carried its own 4-of-36 corner. The artwork no longer carries one,
    so any corner a container declares is a corner nobody asked for.
    """
    match = CORNER_DECL_RE.search(own)
    return " ".join(match.group(1).split()) if match else None


def plate_container_findings(files: dict[pathlib.Path, str], chip: pathlib.Path) -> list[str]:
    """No call site paints behind a plate, and no container is a disc."""
    found: list[str] = []
    for path, raw in sorted(files.items()):
        if path == chip:
            continue
        code = mask_strings(mask_comments(raw))
        relative = path.relative_to(ROOT)
        for block in all_blocks(code):
            children = " ".join(child["header"] for child in block["children"])
            header = block["header"]
            owns_flag_icon = any(
                FLAG_CHILD_RE.search(child["header"]) for child in block["children"]
            )
            owns_flag_slot = bool(FLAG_SLOT_RE.search(header) or FLAG_SLOT_RE.search(children))
            if not owns_flag_icon and not owns_flag_slot:
                continue
            own = own_declarations(block, code)
            if owns_flag_icon and re.search(r"\bbackground\s*:", own):
                found.append(
                    f"{relative}: a hand-written container paints behind a flag plate; "
                    "`CountryChip` is the one owner of a plate's container"
                )
            corner = container_corner(own)
            if owns_flag_icon and corner is not None:
                found.append(
                    f"{relative}: a plate's container declares `border-radius: {corner}`; a "
                    "flag is a rectangle, and any rounded container around it reads as a "
                    "medallion"
                )
            if owns_flag_slot and not owns_flag_icon and corner is not None:
                found.append(
                    f"{relative}: a flag-slot fallback declares `border-radius: {corner}`; "
                    "country identity is a rectangle, not a medallion"
                )
    return found


def chip_findings(chip_text: str) -> list[str]:
    """The shared chip exists, declares no corner, takes the plate's own 4:3
    rectangle, never clips, and paints its ground only for the code fallback."""
    code = mask_comments(chip_text)
    body = component_body(code, CHIP_COMPONENT_RE)
    if not body:
        return ["the shared country chip is gone; a flag slot would own a private container again"]
    found: list[str] = []
    corner = CORNER_DECL_RE.search(body)
    if corner is not None:
        found.append(
            f"CountryChip declares `border-radius: {' '.join(corner.group(1).split())}`; a flag "
            "chip is the plate's own rectangle and has no corner to round"
        )
    if "plate-height: root.side * 3 / 4;" not in body:
        found.append(
            "CountryChip no longer takes the plate's own 4:3 geometry (`side * 3 / 4`); the "
            "inventory is one full-bleed 640x480 rectangle"
        )
    if re.search(r"\bclip:\s*true", body):
        found.append("CountryChip clips its plate; a square plate needs no clip")
    ground = re.search(r"\bbackground\s*:\s*([^;]+);", body)
    if ground is None:
        found.append("CountryChip declares no ground for the code fallback")
    elif "flag-index" not in ground.group(1):
        found.append(
            "CountryChip paints one ground for both states: a resolved flag must be the "
            "whole visual, and only the code fallback may sit on chip paint"
        )
    return found


def theme_token_findings(theme_text: str) -> list[str]:
    """The retired corner token must not come back: a token is a corner waiting
    to be spent on the medallion the receipts already rejected twice."""
    if re.search(r"country-chip-radius", mask_comments(theme_text)):
        return [
            "`LumenTheme.country-chip-radius` is back in the theme; the chip has no corner, so "
            "the token is a retired entry"
        ]
    return []


def inventory_findings(entries: dict[str, str]) -> list[str]:
    """Every flag asset is one geometry: a full-bleed 4:3 rectangle.

    The proof that the plate has square corners is a pixel receipt; this is the
    cheap half - the family that started this (36x36 canvases with a rounded
    plate) cannot come back unnoticed, and neither can the corner curve itself.
    """
    found: list[str] = []
    for name, text in sorted(entries.items()):
        try:
            root = ET.fromstring(text)
        except ET.ParseError as exc:
            found.append(f"{name}: not parseable SVG ({exc})")
            continue
        viewbox = root.get("viewBox")
        if viewbox != FLAG_VIEWBOX:
            found.append(
                f"{name}: viewBox {viewbox!r} is not the shared {FLAG_VIEWBOX} rectangle"
            )
        if ROUNDED_PLATE_MARKER in text:
            found.append(
                f"{name}: the rounded Twemoji plate curve is back; the plate is a full-bleed "
                "rectangle with square corners"
            )
    return found


def flag_inventory() -> dict[str, str]:
    return {
        path.name: path.read_text(encoding="utf-8")
        for path in sorted(FLAGS_DIR.glob("*.svg"))
    }


def violations(
    slint_text: str,
    rust_text: str,
    codes: list[str],
    call_sites: dict[pathlib.Path, str],
    chip_text: str,
    theme_text: str,
    flags: dict[str, str],
) -> list[str]:
    found: list[str] = []
    code = mask_comments(slint_text)

    circles = CIRCLE_ENTRY.findall(code)
    if circles:
        found.append(
            f"FlagImages still lists {len(circles)} circular raster(s): the square vector "
            "inventory is the one source, and a circle on top of a circle is a medallion"
        )
    listed = SVG_ENTRY.findall(code)
    if listed != codes:
        found.append(
            f"FlagImages order/count ({len(listed)}) drifts from the catalog flag.* source "
            f"inventory ({len(codes)}); the index is the only bridge to the owner ISO code"
        )
    if len(set(codes)) != len(codes):
        found.append("the catalog carries a duplicate flag code")

    rust_pairs = RUST_INDEX.findall(rust_text)
    rust_by_index = [c for c, _ in sorted(rust_pairs, key=lambda p: int(p[1]))]
    if rust_by_index != codes:
        found.append(
            "flag_assets::index_for_code disagrees with the catalog inventory, so an owner "
            "country code would paint another country's flag"
        )

    body = component_body(code)
    if not body:
        found.append("FlagIcon is missing; the surface has no single flag plate owner")
    else:
        if re.search(r"\bclip:\s*true", body):
            found.append("FlagIcon clips its plate: a flag is a rectangle, not a circle")
        if re.search(r"border-radius:", body):
            found.append("FlagIcon rounds its plate: the artwork owns its own corners")
        if "image-fit: contain" not in body:
            found.append(
                "FlagIcon must draw the plate with image-fit: contain; cover on a 4:3 asset "
                "crops the artwork itself"
            )
        if "in property <length> size:" not in body:
            found.append("FlagIcon no longer names its plate side `size`")
        if "height: root.size * 3 / 4;" not in body:
            found.append(
                "FlagIcon no longer takes the artwork's own 3/4 ratio; a square box would "
                "letterbox every flag it draws"
            )

    return (
        found
        + call_site_findings(call_sites)
        + plate_container_findings(call_sites, CHIP)
        + chip_findings(chip_text)
        + theme_token_findings(theme_text)
        + inventory_findings(flags)
    )


def ui_call_site_texts() -> dict[pathlib.Path, str]:
    """Every hand-written `.slint` except the flag inventory itself."""
    files: dict[pathlib.Path, str] = {}
    for path in sorted(UI_ROOT.rglob("*.slint")):
        if path in (SLINT, CHIP) or "lumen-source" in path.parts:
            continue
        files[path] = path.read_text(encoding="utf-8")
    return files


def live_violations() -> list[str]:
    return violations(
        SLINT.read_text(encoding="utf-8"),
        RUST.read_text(encoding="utf-8"),
        catalog_square_codes(),
        ui_call_site_texts(),
        CHIP.read_text(encoding="utf-8"),
        THEME.read_text(encoding="utf-8"),
        flag_inventory(),
    )


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        self.slint = SLINT.read_text(encoding="utf-8")
        self.rust = RUST.read_text(encoding="utf-8")
        self.codes = catalog_square_codes()
        self.call_sites = {PILL: PILL.read_text(encoding="utf-8")}
        self.chip = CHIP.read_text(encoding="utf-8")
        self.theme = THEME.read_text(encoding="utf-8")
        self.flags = flag_inventory()

    def findings(
        self,
        slint: str | None = None,
        rust: str | None = None,
        codes: list[str] | None = None,
        call_sites: dict[pathlib.Path, str] | None = None,
        chip: str | None = None,
        theme: str | None = None,
        flags: dict[str, str] | None = None,
    ) -> list[str]:
        return violations(
            self.slint if slint is None else slint,
            self.rust if rust is None else rust,
            self.codes if codes is None else codes,
            self.call_sites if call_sites is None else call_sites,
            self.chip if chip is None else chip,
            self.theme if theme is None else theme,
            self.flags if flags is None else flags,
        )

    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(live_violations(), [])

    def test_the_inventory_is_the_square_vector_set(self) -> None:
        self.assertEqual(len(self.codes), 258)
        self.assertTrue(all(len(code) == 2 and code.isalpha() for code in self.codes))

    def test_a_circular_raster_coming_back_is_caught(self) -> None:
        planted = self.slint.replace(
            '@image-url("../../infiltrator-shared/assets/flags/ac.svg")',
            '@image-url("../../infiltrator-shared/assets/flags/circle-png/ac.png")',
            1,
        )
        self.assertTrue(any("circular raster" in item for item in self.findings(slint=planted)), 
                        self.findings(slint=planted))

    def test_a_dropped_entry_is_caught(self) -> None:
        first = SVG_ENTRY.search(self.slint)
        assert first is not None
        planted = self.slint.replace(first.group(0), "@image-url()", 1)
        self.assertTrue(
            any("drifts from the catalog" in item for item in self.findings(slint=planted)),
            self.findings(slint=planted),
        )

    def test_a_reordered_catalog_is_caught(self) -> None:
        rotated = self.codes[1:] + self.codes[:1]
        self.assertTrue(
            any("drifts from the catalog" in item for item in self.findings(codes=rotated)),
            self.findings(codes=rotated),
        )

    def test_a_clipped_plate_is_caught(self) -> None:
        anchor = "    background: transparent;\n    border-width: 0 * LumenUiScale.unit;"
        planted = self.slint.replace(anchor, anchor + "\n    clip: true;", 1)
        self.assertNotEqual(planted, self.slint)
        self.assertTrue(
            any("clips its plate" in item for item in self.findings(slint=planted)),
            self.findings(slint=planted),
        )

    def test_a_rounded_plate_is_caught(self) -> None:
        anchor = "    background: transparent;\n    border-width: 0 * LumenUiScale.unit;"
        planted = self.slint.replace(anchor, anchor + "\n    border-radius: root.size / 2;", 1)
        self.assertTrue(
            any("rounds its plate" in item for item in self.findings(slint=planted)),
            self.findings(slint=planted),
        )

    def test_cover_fit_is_caught(self) -> None:
        planted = self.slint.replace("image-fit: contain", "image-fit: cover", 1)
        self.assertTrue(
            any("image-fit: contain" in item for item in self.findings(slint=planted)),
            self.findings(slint=planted),
        )

    def test_a_renamed_size_entry_is_caught(self) -> None:
        planted = self.slint.replace("in property <length> size:", "in property <length> side:", 1)
        self.assertTrue(
            any("plate side `size`" in item for item in self.findings(slint=planted)),
            self.findings(slint=planted),
        )

    def test_a_square_plate_box_is_caught(self) -> None:
        planted = self.slint.replace("height: root.size * 3 / 4;", "height: root.size;", 1)
        self.assertNotEqual(planted, self.slint)
        self.assertTrue(
            any("3/4 ratio" in item for item in self.findings(slint=planted)),
            self.findings(slint=planted),
        )

    def test_an_index_that_disagrees_with_the_inventory_is_caught(self) -> None:
        # Moving one code to the end of the ladder is the real failure mode: the
        # owner resolves a country, and the surface paints another country.
        planted = self.rust.replace('"ac" => Some(0)', '"ac" => Some(258)', 1)
        self.assertTrue(
            any("index_for_code disagrees" in item for item in self.findings(rust=planted)),
            self.findings(rust=planted),
        )

    def test_a_call_site_feeding_the_retired_entry_is_caught(self) -> None:
        text = self.call_sites[PILL]
        planted = text.replace(
            "            size: ProxyPillMetrics.pill-identity-size;",
            "            diameter: ProxyPillMetrics.pill-identity-size;",
            1,
        )
        self.assertNotEqual(planted, text)
        found = self.findings(call_sites={PILL: planted})
        self.assertTrue(any("retired `diameter`" in item for item in found), found)

    def test_a_comment_naming_the_retired_entry_does_not_count(self) -> None:
        text = self.call_sites[PILL]
        planted = text.replace(
            "        if root.group.current-flag-index >= 0: FlagIcon {",
            "        // diameter: was the old entry\n        if root.group.current-flag-index >= 0: FlagIcon {",
            1,
        )
        self.assertEqual(self.findings(call_sites={PILL: planted}), [])

    def test_a_private_container_behind_a_plate_is_caught(self) -> None:
        """The two home rows each declared their own chip; that is the drift."""
        planted = {
            ROOT / "planted.slint": (
                "HorizontalLayout {\n"
                "    Rectangle {\n"
                "        width: 38 * LumenUiScale.unit;\n"
                "        height: 38 * LumenUiScale.unit;\n"
                "        border-radius: LumenTheme.country-chip-radius;\n"
                "        background: LumenTheme.zinc-4;\n"
                "        if index >= 0: FlagIcon { index: index; }\n"
                "    }\n"
                "}\n"
            )
        }
        found = self.findings(call_sites=planted)
        self.assertTrue(any("hand-written container" in item for item in found), found)

    def test_a_disc_corner_around_a_plate_is_caught(self) -> None:
        """`border-radius: side / 2` on a square is the medallion this guard bans."""
        planted = {
            ROOT / "planted.slint": (
                "Rectangle {\n"
                "    width: 38 * LumenUiScale.unit;\n"
                "    height: 38 * LumenUiScale.unit;\n"
                "    border-radius: 19 * LumenUiScale.unit;\n"
                "    if index >= 0: FlagIcon { index: index; }\n"
                "}\n"
            )
        }
        found = self.findings(call_sites=planted)
        self.assertTrue(any("reads as a medallion" in item for item in found), found)

    def test_a_circular_flag_slot_fallback_is_caught(self) -> None:
        planted = {
            ROOT / "planted.slint": (
                "HorizontalLayout {\n"
                "    if root.current-flag-index < 0: Rectangle {\n"
                "        width: 20 * LumenUiScale.unit;\n"
                "        height: 20 * LumenUiScale.unit;\n"
                "        border-radius: 20 * LumenUiScale.unit / 2;\n"
                "    }\n"
                "}\n"
            )
        }
        found = self.findings(call_sites=planted)
        self.assertTrue(any("flag-slot fallback" in item for item in found), found)

    def test_the_reference_tile_corner_is_caught(self) -> None:
        """The reference's 31% tile is the medallion the receipt showed."""
        planted = {
            ROOT / "planted.slint": (
                "Rectangle {\n"
                "    width: 32 * LumenUiScale.unit;\n"
                "    height: 32 * LumenUiScale.unit;\n"
                "    border-radius: 10 * LumenUiScale.unit;\n"
                "    if index >= 0: FlagIcon { index: index; }\n"
                "}\n"
            )
        }
        found = self.findings(call_sites=planted)
        self.assertTrue(any("reads as a medallion" in item for item in found), found)

    def test_the_artworks_own_old_corner_is_caught_too(self) -> None:
        """The 4-of-36 corner the third receipt rejected is now a finding as well."""
        planted = {
            ROOT / "planted.slint": (
                "Rectangle {\n"
                "    width: 32 * LumenUiScale.unit;\n"
                "    height: 32 * LumenUiScale.unit;\n"
                "    border-radius: 3.5 * LumenUiScale.unit;\n"
                "    if index >= 0: FlagIcon { index: index; }\n"
                "}\n"
            )
        }
        found = self.findings(call_sites=planted)
        self.assertTrue(any("reads as a medallion" in item for item in found), found)

    def test_the_retired_corner_token_coming_back_is_caught(self) -> None:
        planted_theme = self.theme + (
            "    out property <length> country-chip-radius: 4 * LumenUiScale.unit;\n"
        )
        found = self.findings(theme=planted_theme)
        self.assertTrue(any("retired entry" in item for item in found), found)

    def test_a_chip_ground_behind_a_resolved_flag_is_caught(self) -> None:
        """The chip's ground is the fallback's alone; a flag is the whole visual."""
        planted = self.chip.replace(
            "    background: root.flag-index >= 0 ? transparent : LumenTheme.zinc-4;",
            "    background: LumenTheme.zinc-4;",
            1,
        )
        self.assertNotEqual(planted, self.chip)
        found = self.findings(chip=planted)
        self.assertTrue(any("one ground for both states" in item for item in found), found)

    def test_a_chip_that_rounds_itself_is_caught(self) -> None:
        planted = self.chip.replace(
            "    background: root.flag-index >= 0 ? transparent : LumenTheme.zinc-4;",
            "border-radius: 12 * LumenUiScale.unit;",
            1,
        )
        self.assertNotEqual(planted, self.chip)
        found = self.findings(chip=planted)
        self.assertTrue(any("no corner to round" in item for item in found), found)

    def test_a_rounded_inventory_file_is_caught(self) -> None:
        """The family that started this: a 36x36 canvas with the rounded plate."""
        planted = dict(self.flags)
        planted["zz.svg"] = (
            '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 36 36">'
            '<path fill="#DE2910" d="M36 27c0 2.209-1.791 4-4 4H4c-2.209 0-4-1.791-4-4V9'
            'c0-2.209 1.791-4 4-4h28c2.209 0 4 1.791 4 4v18z"/></svg>'
        )
        found = self.findings(flags=planted)
        self.assertTrue(any("zz.svg" in item and "rectangle" in item for item in found), found)

    def test_a_plate_whose_ratio_drifted_is_caught(self) -> None:
        planted = self.chip.replace(
            "plate-height: root.side * 3 / 4;", "plate-height: root.side * 26 / 36;", 1
        )
        self.assertNotEqual(planted, self.chip)
        found = self.findings(chip=planted)
        self.assertTrue(any("4:3 geometry" in item for item in found), found)

    def test_the_shared_circle_derivative_stays_retired(self) -> None:
        self.assertFalse(
            SHARED_CIRCLE_DIR.exists(),
            "the shared circular flag derivative came back",
        )
        records = json.loads(CATALOG.read_text(encoding="utf-8"))["assets"]
        self.assertFalse(
            [record for record in records if record.get("variant") == "circle-144"],
            "the catalog still registers a circular flag variant",
        )

    def test_the_vendor_snapshot_is_untouched(self) -> None:
        """The verbatim Lumen import keeps its own circle; that is not our inventory."""
        vendor = ROOT / "crates/infiltrator-slint/lumen-source/ui/flag-icons.slint"
        self.assertIn("circle-png/ac.png", vendor.read_text(encoding="utf-8"))


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
        print(f"SLINT-020 [{args.mode}]: {item}")
    print(f"slint flag shape guard: findings={len(found)}")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""SLINT-020: national flags are square artwork, painted from one inventory.

User feedback 2026-11-10: "一般国旗都是方形 最好不要做这种圆形旗帜图案容器" -
the receipt showed the Hong Kong flag as a red medallion. Two things were wrong
at once, and they were not cosmetic:

* the surface drew the flag inside `border-radius: diameter / 2` + `clip: true`
  on top of an asset that was ALREADY a circle, so the artwork was clipped
  twice and the name of the size input (`diameter`) described the clip rather
  than the plate;
* the inventory it drew from was the 144px circular raster derivative, a second
  copy of artwork the shared catalog already carries as `flag.<code>` /
  variant `source` (258 square Twemoji vectors). Two inventories for one fact
  is exactly what the one-source-of-truth rule forbids.

FlClash paints no circular flag at all: its rows carry an emoji or a two-letter
text badge, so the circle was never a parity decision either.

The rules this guard holds:

1. `FlagImages.flags` lists exactly the catalog's `flag.*` / `source` codes, in
   catalog order, as `.svg` - and `flag_assets::index_for_code` agrees, because
   the index is the only bridge between the owner's ISO code and this list.
2. The shared circular raster derivative is gone: no `circle-144` record in the
   catalog and no `flags/circle-png/` directory. The verbatim Lumen import under
   `lumen-source/` keeps its own copy - that is the vendor snapshot, not ours.
3. `FlagIcon` draws a square plate: no clip, no radius, `image-fit: contain`,
   and its size input is `size`. `diameter` is a retired entry, so a call site
   that feeds it again is caught rather than silently ignored.
4. A call site never wraps a plate in a container of its own: the two home
   rows each declared a private `Rectangle` with a `background`, which is how
   they drifted from the shared chip and kept a circle nobody else drew. The
   shared `CountryChip` is the only element that may paint behind a plate.
5. No container that holds a plate may declare a corner that reaches half its
   side. `border-radius: side / 2` on a square is a disc, and a disc crops a
   rectangular flag.
6. That corner is `LumenTheme.country-chip-radius`, and both of its values
   stay under half of the smallest chip side any call site feeds.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
SLINT = ROOT / "crates/infiltrator-slint/ui/lumen-flags.slint"
RUST = ROOT / "crates/infiltrator-slint/src/flag_assets.rs"
CATALOG = ROOT / "crates/infiltrator-shared/assets/asset-catalog.json"
SHARED_CIRCLE_DIR = ROOT / "crates/infiltrator-shared/assets/flags/circle-png"
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
LENGTH_DECL_RE = re.compile(
    r"\b(width|height|border-radius)\s*:\s*(\d+(?:\.\d+)?)\s*\*\s*LumenUiScale\.unit\s*;"
)
CHIP_RADIUS_RE = re.compile(r"country-chip-radius\s*:\s*([^;]+);")
CHIP_SIDE_RE = re.compile(r"CountryChip\s*\{[^}]*?\bside:\s*(\d+(?:\.\d+)?)\s*\*", re.S)


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


def disc_corner(own: str) -> tuple[float, float] | None:
    """The (radius, side) of a container whose corner reaches half its side."""
    declarations = {name: float(value) for name, value in LENGTH_DECL_RE.findall(own)}
    radius = declarations.get("border-radius")
    if radius is None or "width" not in declarations or "height" not in declarations:
        return None
    side = min(declarations["width"], declarations["height"])
    return (radius, side) if radius * 2 >= side else None


def plate_container_findings(files: dict[pathlib.Path, str], chip: pathlib.Path) -> list[str]:
    """No call site paints behind a plate, and no container is a disc."""
    found: list[str] = []
    for path, raw in sorted(files.items()):
        if path == chip:
            continue
        code = mask_strings(mask_comments(raw))
        relative = path.relative_to(ROOT)
        for block in all_blocks(code):
            if not any(FLAG_CHILD_RE.search(child["header"]) for child in block["children"]):
                continue
            own = own_declarations(block, code)
            if re.search(r"\bbackground\s*:", own):
                found.append(
                    f"{relative}: a hand-written container paints behind a flag plate; "
                    "`CountryChip` is the one owner of a plate's container"
                )
            corner = disc_corner(own)
            if corner is not None:
                found.append(
                    f"{relative}: a plate's container declares a {corner[0]}u corner on a "
                    f"{corner[1]}u side, which is a disc: a rectangle inside a circle is "
                    "cropped into a medallion"
                )
    return found


def chip_findings(chip_text: str) -> list[str]:
    """The shared chip exists, takes its corner from the theme and never clips."""
    code = mask_comments(chip_text)
    body = component_body(code, CHIP_COMPONENT_RE)
    if not body:
        return ["the shared country chip is gone; a flag slot would own a private container again"]
    found: list[str] = []
    if "border-radius: LumenTheme.country-chip-radius;" not in body:
        found.append("CountryChip no longer takes its corner from `LumenTheme.country-chip-radius`")
    if re.search(r"\bclip:\s*true", body):
        found.append("CountryChip clips its plate; a square plate needs no clip")
    return found


def chip_corner_findings(theme_text: str, call_sites: dict[pathlib.Path, str]) -> list[str]:
    """One corner token, and no chip side small enough to make it a disc."""
    match = CHIP_RADIUS_RE.search(mask_comments(theme_text))
    if match is None:
        return ["`LumenTheme.country-chip-radius` is gone; the chip corner would be a literal"]
    radii = [float(value) for value in re.findall(r"(\d+(?:\.\d+)?)\s*\*", match.group(1))]
    if not radii:
        return ["`LumenTheme.country-chip-radius` carries no length literal to check"]
    sides = [
        float(value)
        for text in call_sites.values()
        for value in CHIP_SIDE_RE.findall(mask_comments(text))
    ]
    if sides and max(radii) * 2 >= min(sides):
        return [
            f"`LumenTheme.country-chip-radius` reaches {max(radii)}u against the smallest "
            f"chip side {min(sides)}u, which is half the side: that is the disc again"
        ]
    return []


def violations(
    slint_text: str,
    rust_text: str,
    codes: list[str],
    call_sites: dict[pathlib.Path, str],
    chip_text: str,
    theme_text: str,
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
            found.append("FlagIcon clips its plate: a flag is a square, not a circle")
        if re.search(r"border-radius:", body):
            found.append("FlagIcon rounds its plate: the artwork owns its own corners")
        if "image-fit: contain" not in body:
            found.append(
                "FlagIcon must draw the square with image-fit: contain; cover on a square "
                "asset crops the artwork itself"
            )
        if "in property <length> size:" not in body:
            found.append("FlagIcon no longer names its plate side `size`")

    return (
        found
        + call_site_findings(call_sites)
        + plate_container_findings(call_sites, CHIP)
        + chip_findings(chip_text)
        + chip_corner_findings(theme_text, call_sites)
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
    )


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        self.slint = SLINT.read_text(encoding="utf-8")
        self.rust = RUST.read_text(encoding="utf-8")
        self.codes = catalog_square_codes()
        self.call_sites = {PILL: PILL.read_text(encoding="utf-8")}
        self.chip = CHIP.read_text(encoding="utf-8")
        self.theme = THEME.read_text(encoding="utf-8")

    def findings(
        self,
        slint: str | None = None,
        rust: str | None = None,
        codes: list[str] | None = None,
        call_sites: dict[pathlib.Path, str] | None = None,
        chip: str | None = None,
        theme: str | None = None,
    ) -> list[str]:
        return violations(
            self.slint if slint is None else slint,
            self.rust if rust is None else rust,
            self.codes if codes is None else codes,
            self.call_sites if call_sites is None else call_sites,
            self.chip if chip is None else chip,
            self.theme if theme is None else theme,
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
        self.assertTrue(any("which is a disc" in item for item in found), found)

    def test_a_rounded_container_under_half_the_side_is_clean(self) -> None:
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
        found = [item for item in self.findings(call_sites=planted) if "planted.slint" in item]
        self.assertEqual(found, [])

    def test_a_chip_corner_that_reaches_half_the_side_is_caught(self) -> None:
        planted_theme = self.theme.replace(
            "country-chip-radius: root.compact-layout ? 10 * LumenUiScale.unit : 12 * LumenUiScale.unit;",
            "country-chip-radius: root.compact-layout ? 16 * LumenUiScale.unit : 16 * LumenUiScale.unit;",
            1,
        )
        self.assertNotEqual(planted_theme, self.theme)
        planted_sites = {
            ROOT / "planted.slint": "CountryChip {\n    side: 32 * LumenUiScale.unit;\n}\n"
        }
        found = self.findings(call_sites=planted_sites, theme=planted_theme)
        self.assertTrue(any("that is the disc again" in item for item in found), found)

    def test_a_chip_corner_that_is_not_the_token_is_caught(self) -> None:
        planted = self.chip.replace(
            "border-radius: LumenTheme.country-chip-radius;",
            "border-radius: 12 * LumenUiScale.unit;",
            1,
        )
        self.assertNotEqual(planted, self.chip)
        found = self.findings(chip=planted)
        self.assertTrue(any("no longer takes its corner" in item for item in found), found)

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

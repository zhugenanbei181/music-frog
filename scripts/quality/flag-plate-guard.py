#!/usr/bin/env python3
"""FLAGART-001 / SLINT-020: national flags are one rectangular inventory, drawn as plates.

User feedback (five times, latest 2026-11-12): "一般国旗都是方形 最好不要做这种圆形旗帜
图案容器". The Slint surface was fixed in b177/b183/b188/b193, but the published
Iced and Bevy receipts still showed the complaint: `proxies-dark` drew the Hong
Kong node as a **wavy emoji glyph** and the unmatched node as a **literal blue
circle** (🌐). The reason was architectural, not cosmetic:

* Iced and Bevy painted `country_flags::node_flag_emoji`, so the flag's shape,
  colour and even its existence depended on the host's emoji font;
* three independent region alias tables existed (contract `GeoRegion`, shared
  `country_flags::Region`, domain `COUNTRY_DEFS`) and they disagreed on 41 of
  146 measured node names - nine catalogued countries (CH/SE/NO/FI/IT/ES/IE/
  PL/UA) were unreachable from the contract table at all;
* the catalog's 258 rectangular SVGs had one consumer (Slint), so the surfaces
  that could have drawn square artwork did not.

The rules this guard holds:

1. `asset-catalog.json` is the only flag inventory: 258 `flag.<iso>` / `source`
   SVG records, and the generated Iced table has exactly those codes.
2. The generated table is fresh (`scripts/generate_flag_assets.py --check`).
3. No shipped surface source draws a Unicode regional-indicator flag sequence;
   emoji fallbacks such as 🌐 / ⚡ / 🚫 in a flag slot are also rejected.
4. The retired glyph entry points stay retired: `node_flag_emoji`,
   `GeoRegion::emoji_flag`, `GeoRegion::flag_icon_token`,
   `proxy_flag_icon_token`.
5. Iced's plate owner is the only element that may paint a flag, and it declares
   a zero corner radius (a rounded container around a flag *is* the container).
6. Region resolution has one owner: `infiltrator_contract::geo_region` must
   resolve every catalogued ISO code from a node name. The contract's generated
   code table must equal the asset catalog, its tests must cover the whole
   table, and domain filtering must consume `NodeRegion` rather than a second
   alias matcher.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
CATALOG = ROOT / "crates/infiltrator-shared/assets/asset-catalog.json"
GENERATED = ROOT / "crates/infiltrator-iced/src/flag_assets_generated.rs"
CONTRACT_GENERATED = ROOT / "crates/infiltrator-contract/src/flag_codes_generated.rs"
CONTRACT_GEO_REGION = ROOT / "crates/infiltrator-contract/src/geo_region.rs"
CONTRACT_GEO_TESTS = ROOT / "crates/infiltrator-contract/src/geo_region_tests.rs"
DOMAIN_FILTER = ROOT / "crates/infiltrator-domain/src/filter.rs"
GENERATOR = ROOT / "scripts/generate_flag_assets.py"
PLATE = ROOT / "crates/infiltrator-iced/src/view/flag_plate.rs"
GATE = ROOT / "scripts/quality/check-structure.sh"
SHARED_SRC = ROOT / "crates/infiltrator-shared/src"
# Only the layers that *produce* a node's identity are scanned. Alias tables
# that match a flag emoji inside a user's node name (contract `geo_region`,
# domain `COUNTRY_DEFS`) are input parsing, not artwork, and stay out of scope.
IDENTITY_DIRS = (
    "crates/infiltrator-iced/src/view",
    "crates/infiltrator-bevy-ui/src/pages",
    "crates/infiltrator-slint/ui",
    "crates/infiltrator-slint/src/lumen_projection",
    "crates/infiltrator-presentation/src",
)
IDENTITY_FILES = (SHARED_SRC / "country_flags.rs",)
REGIONAL = re.compile("[\U0001F1E6-\U0001F1FF]{2}")
# The circular globe that was the complaint's second half: it may never be a
# flag slot's fallback. Other emoji elsewhere are icon questions (DSGSTD-001),
# not flag questions, so this rule is scoped to the files that own a flag slot.
GLOBE_FALLBACK = re.compile(r"🌐")
FLAG_SLOT_FILES = (
    ROOT / "crates/infiltrator-iced/src/view/flag_plate.rs",
    ROOT / "crates/infiltrator-iced/src/view/proxies/node_cards.rs",
    ROOT / "crates/infiltrator-iced/src/view/runtime/delay.rs",
    ROOT / "crates/infiltrator-iced/src/view/active_exit.rs",
    ROOT / "crates/infiltrator-bevy-ui/src/pages/proxies_refresh.rs",
    ROOT / "crates/infiltrator-bevy-ui/src/pages/proxies_card/node_card.rs",
    ROOT / "crates/infiltrator-shared/src/country_flags.rs",
)
TEST_MODULE = re.compile(r"^#\[cfg\(test\)\]", re.M)
RETIRED = (
    "node_flag_emoji",
    "emoji_flag",
    "flag_icon_token",
    "proxy_flag_icon_token",
)


def catalog_codes() -> list[str]:
    document = json.loads(CATALOG.read_text(encoding="utf-8"))
    return sorted(
        record["id"][len("flag."):].upper()
        for record in document["assets"]
        if record["id"].startswith("flag.")
        and record["format"] == "svg"
        and record["variant"] == "source"
    )


def generated_codes(text: str) -> list[str]:
    return sorted(set(re.findall(r'"([A-Z]{2})" => include_bytes!', text)))


def identity_sources() -> list[pathlib.Path]:
    files: list[pathlib.Path] = []
    for relative in IDENTITY_DIRS:
        for path in sorted((ROOT / relative).rglob("*")):
            if not path.is_file() or path.suffix not in {".rs", ".slint"}:
                continue
            if path.name.endswith("_tests.rs"):
                continue
            files.append(path)
    files.extend(path for path in IDENTITY_FILES if path.is_file())
    return files


def shipped_text(path: pathlib.Path) -> str:
    """The file's code before its test module: fixtures may name real nodes."""
    text = path.read_text(encoding="utf-8", errors="replace")
    if path.suffix == ".rs":
        match = TEST_MODULE.search(text)
        if match:
            text = text[: match.start()]
    return text


def findings() -> list[str]:
    problems: list[str] = []
    expected = catalog_codes()
    if not GENERATED.is_file():
        problems.append("the Iced flag byte table is missing")
        return problems
    actual = generated_codes(GENERATED.read_text(encoding="utf-8"))
    if actual != expected:
        missing = sorted(set(expected) - set(actual))
        extra = sorted(set(actual) - set(expected))
        problems.append(
            f"flag byte table does not match the catalog: missing={missing} extra={extra}"
        )
    check = subprocess.run(
        [sys.executable, str(GENERATOR), "--check"],
        capture_output=True,
        text=True,
        cwd=ROOT,
        check=False,
    )
    if check.returncode != 0:
        problems.append(f"flag byte table is stale: {check.stdout.strip() or check.stderr.strip()}")
    if not CONTRACT_GENERATED.is_file():
        problems.append("the contract flag-code vocabulary is missing")
    else:
        contract_codes = re.findall(
            r'"([A-Z]{2})",', CONTRACT_GENERATED.read_text(encoding="utf-8"))
        if contract_codes != expected:
            problems.append("the contract flag-code vocabulary does not match the catalog")
    geo_region = "\n".join(
        path.read_text(encoding="utf-8")
        for path in (CONTRACT_GEO_REGION, CONTRACT_GEO_TESTS)
        if path.is_file()
    )
    if "fn every_catalogued_flag_is_resolvable_from_a_node_name" not in geo_region:
        problems.append("the contract no longer tests every catalogued flag code")
    if "flag_code_from_catalog" not in geo_region:
        problems.append("geo_region no longer resolves the generated catalog vocabulary")
    domain_filter = DOMAIN_FILTER.read_text(encoding="utf-8")
    if "fn matches_alias(" in domain_filter:
        problems.append("domain filter reintroduced a private country alias matcher")
    if "NodeRegion::classify(name).flag_code()" not in domain_filter:
        problems.append("domain filter no longer consumes the contract region owner")
    for path in identity_sources():
        text = shipped_text(path)
        allow_globe = path not in FLAG_SLOT_FILES
        for line_number, line in enumerate(text.splitlines(), start=1):
            if REGIONAL.search(line) or (not allow_globe and GLOBE_FALLBACK.search(line)):
                problems.append(
                    f"{path.relative_to(ROOT)}:{line_number} emits a flag glyph "
                    "instead of the rectangular plate"
                )
        for symbol in RETIRED:
            if re.search(rf"\b{symbol}\b", text):
                problems.append(
                    f"{path.relative_to(ROOT)} reintroduces retired entry point {symbol}"
                )
    plate_text = PLATE.read_text(encoding="utf-8") if PLATE.is_file() else ""
    if "radius: border::Radius::from(0.0)" not in plate_text:
        problems.append("the Iced plate must declare a zero corner radius")
    if "svg::Handle::from_memory" not in plate_text:
        problems.append("the Iced plate must draw the shared vector artwork")
    for leg in ("flag-plate-guard.py --mode enforce", "flag-plate-guard.py --self-test"):
        if leg not in GATE.read_text(encoding="utf-8"):
            problems.append(f"structure gate is missing leg: {leg}")
    return problems


class GuardSelfTest(unittest.TestCase):
    def test_the_live_tree_passes(self) -> None:
        self.assertEqual(findings(), [])

    def test_dropping_a_catalogued_flag_is_rejected(self) -> None:
        global CATALOG
        original = CATALOG
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "asset-catalog.json"
            document = json.loads(original.read_text(encoding="utf-8"))
            document["assets"] = [
                record
                for record in document["assets"]
                if record["id"] != "flag.hk"
            ]
            planted.write_text(json.dumps(document), encoding="utf-8")
            CATALOG = planted
            try:
                self.assertTrue(any("extra=['HK']" in item for item in findings()))
            finally:
                CATALOG = original

    def test_a_flag_glyph_in_a_surface_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "proxy.rs"
            planted.write_text('const FLAG: &str = "\U0001F1ED\U0001F1F0";\n', encoding="utf-8")
            self.assertTrue(
                any("emits a flag glyph" in item for item in glyph_findings([planted]))
            )

    def test_a_emoji_globe_fallback_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "delay.rs"
            planted.write_text('let fallback = "🌐";\n', encoding="utf-8")
            self.assertTrue(
                any("emits a flag glyph" in item for item in glyph_findings([planted]))
            )

    def test_a_node_name_fixture_is_not_mistaken_for_artwork(self) -> None:
        """Input parsing may contain a user's flag emoji; shipped output may not."""
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "hero.rs"
            planted.write_text(
                'pub fn label(name: &str) -> String {\n    name.to_owned()\n}\n\n'
                '#[cfg(test)]\nmod tests {\n    #[test]\n'
                '    fn fixture() {\n        assert_eq!(super::label("\U0001F1ED\U0001F1F0 01"), "x");\n    }\n}\n',
                encoding="utf-8",
            )
            self.assertEqual(glyph_findings([planted]), [])

    def test_reintroducing_a_retired_entry_point_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "node_cards.rs"
            planted.write_text("use crate::country_flags::node_flag_emoji;\n", encoding="utf-8")
            self.assertTrue(
                any("retired entry point" in item for item in glyph_findings([planted]))
            )

    def test_a_rounded_plate_is_rejected(self) -> None:
        global PLATE
        original = PLATE
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "flag_plate.rs"
            planted.write_text(
                original.read_text(encoding="utf-8").replace(
                    "radius: border::Radius::from(0.0)",
                    "radius: border::Radius::from(9.0)",
                ),
                encoding="utf-8",
            )
            PLATE = planted
            try:
                self.assertTrue(any("zero corner radius" in item for item in findings()))
            finally:
                PLATE = original

    def test_a_private_domain_country_matcher_is_rejected(self) -> None:
        global DOMAIN_FILTER
        original = DOMAIN_FILTER
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "filter.rs"
            planted.write_text(
                original.read_text(encoding="utf-8")
                + "\nfn matches_alias(text: &str, upper: &str, alias: &str) -> bool { true }\n",
                encoding="utf-8",
            )
            DOMAIN_FILTER = planted
            try:
                self.assertTrue(
                    any("private country alias matcher" in item for item in findings()))
            finally:
                DOMAIN_FILTER = original

    def test_a_missing_gate_leg_is_rejected(self) -> None:
        global GATE
        original = GATE
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "check-structure.sh"
            planted.write_text(
                original.read_text(encoding="utf-8").replace(
                    "flag-plate-guard.py --mode enforce\n", "", 1
                ),
                encoding="utf-8",
            )
            GATE = planted
            try:
                self.assertTrue(any("structure gate is missing leg" in item for item in findings()))
            finally:
                GATE = original


def glyph_findings(files: list[pathlib.Path]) -> list[str]:
    """Run the glyph and retired-symbol rules over an explicit file list."""
    problems: list[str] = []
    for path in files:
        text = shipped_text(path)
        for line_number, line in enumerate(text.splitlines(), start=1):
            if REGIONAL.search(line) or GLOBE_FALLBACK.search(line):
                problems.append(f"{path}:{line_number} emits a flag glyph")
        for symbol in RETIRED:
            if re.search(rf"\b{symbol}\b", text):
                problems.append(f"{path}:{symbol} reintroduces a retired entry point")
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="enforce")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        result = unittest.TextTestRunner(verbosity=1).run(suite)
        return 0 if result.wasSuccessful() else 1
    problems = findings()
    for problem in problems:
        print(f"flag plate guard: {problem}", file=sys.stderr)
    if problems and args.mode == "enforce":
        return 1
    print(
        f"flag plate guard: catalog {len(catalog_codes())} flags, "
        f"findings={len(problems)}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())

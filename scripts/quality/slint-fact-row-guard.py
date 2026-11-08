#!/usr/bin/env python3
"""SLINT-011: one owner for a labelled fact row.

A card that shows a label above a value used to be spelled per page. The hero
card's bottom row existed twice - once on the phone, once on the desktop - and
the copies had already drifted: one printed a fabricated ``00:00:00`` for a
duration no owner had published, the other printed a dash, and the two values
used different type roles, so the row read as two unrelated fragments and the
values never shared a baseline.

The row now has one owner component, the value state is typed in the contract
(observed / unobserved with one shared marker), and every fact value uses one
type role so two values in a row sit on one baseline.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import unittest
import unittest.mock

ROOT = pathlib.Path(__file__).resolve().parents[2]
UI = ROOT / "crates/infiltrator-slint/ui"
CONTRACT = ROOT / "crates/infiltrator-contract/src/ui_fact.rs"
PRESENTATION = ROOT / "crates/infiltrator-presentation/src/dashboard_hero_projection.rs"
ROW_OWNER = UI / "pages/home_fact_row.slint"
CONSUMERS = (
    UI / "pages/home_desktop_grid.slint",
    UI / "pages/home_compact_flow.slint",
)

FABRICATED_FALLBACK = re.compile(r'"00:00:00"')
RETIRED_FACT_FIELDS = re.compile(
    r"dashboard-facts\.(uptime|organization)\b"
)
FACT_FALLBACK = re.compile(
    r"(uptime|organization)\s*!=\s*\"\"\s*\?"
)
MARKER = "—"
VALUE_ROLE = re.compile(r"DesignType\.(?:size|weight|family)\(([^()]*)\)")
VALUE_TEXT = re.compile(
    r"Text\s*\{[^{}]*?text:\s*root\.cell\.value;.*?\}",
    re.S,
)
UNOBSERVED_PROPERTY = re.compile(r"pub const UNOBSERVED_FACT_MARKER: &str = \"(.*?)\";")
TYPED_ROW = re.compile(
    r"pub struct DashboardHeroFactRow\s*\{[^{}]*?pub uptime: FactCell[,;][^{}]*?"
    r"pub organization: FactCell[,;][^{}]*?\}",
    re.S,
)
RETIRED_PROJECTION_FIELDS = re.compile(
    r"pub (uptime_formatted|organization): String,"
)
# A literal clock face in the projection would be a duration the surface
# wrote itself instead of the one the Core owner measured.
DURATION_LITERAL = re.compile(r'"\d{2}:\d{2}:\d{2}"')


def mask_slint_comments(source: str) -> str:
    result: list[str] = []
    index = 0
    in_block = False
    while index < len(source):
        if not in_block and source[index : index + 2] == "//":
            end = source.find("\n", index)
            if end == -1:
                break
            result.append("\n")
            index = end
        elif not in_block and source[index : index + 2] == "/*":
            in_block = True
            index += 2
        elif in_block and source[index : index + 2] == "*/":
            in_block = False
            index += 2
        else:
            result.append("" if in_block else source[index])
            index += 1
    return "".join(result)


def contract_violations(contract: pathlib.Path) -> list[str]:
    found: list[str] = []
    text = contract.read_text(encoding="utf-8")
    if "pub enum FactValue" not in text or "Unobserved" not in text:
        found.append("the contract no longer owns the observed/unobserved value")
    if "pub struct FactCell" not in text:
        found.append("the contract no longer owns the typed fact cell")
    marker = UNOBSERVED_PROPERTY.search(text)
    if marker is None:
        found.append("the contract lost the shared unobserved marker")
    elif marker.group(1) != MARKER:
        found.append(f"the shared unobserved marker drifted to {marker.group(1)!r}")
    if TYPED_ROW.search(text) is None:
        found.append("the hero fact row is no longer a typed two-cell row")
    return found


def production_source(source: str) -> str:
    """Drop the test module: a test may name the value it expects."""
    marker = source.find("#[cfg(test)]")
    return source if marker == -1 else source[:marker]


def presentation_violations(presentation: pathlib.Path) -> list[str]:
    found: list[str] = []
    text = presentation.read_text(encoding="utf-8")
    production = production_source(text)
    if RETIRED_PROJECTION_FIELDS.search(text):
        found.append("the projection still publishes retired fact strings")
    if "pub fact_row: DashboardHeroFactRow" not in text:
        found.append("the projection no longer publishes the typed fact row")
    if DURATION_LITERAL.search(production):
        found.append("the projection writes a session length instead of formatting one")
    return found


def source_violations(
    ui: pathlib.Path, contract: pathlib.Path, presentation: pathlib.Path
) -> list[str]:
    found: list[str] = []
    found.extend(contract_violations(contract))
    found.extend(presentation_violations(presentation))
    if not ROW_OWNER.is_file():
        found.append("the shared fact row component is missing")
        return found
    owner = mask_slint_comments(ROW_OWNER.read_text(encoding="utf-8"))
    if "export component HomeHeroFactRow" not in owner:
        found.append("the shared fact row lost its owner component")
    value_texts = VALUE_TEXT.findall(owner)
    roles = {
        frozenset(VALUE_ROLE.findall(block)) for block in value_texts
    }
    roles.discard(frozenset())
    if roles != {frozenset({"TextRole.factNamed"})}:
        found.append(
            "fact values must resolve one type role (TextRole.factNamed); "
            f"the owner resolves {sorted(sorted(r) for r in roles)}"
        )
    for path in CONSUMERS:
        text = mask_slint_comments(path.read_text(encoding="utf-8"))
        if not re.search(r"\bHomeHeroFactRow\s*\{", text):
            found.append(
                f"{path.relative_to(ROOT)}: renders its own hero fact row instead of the owner"
            )
        for pattern, message in (
            (re.compile(r"@tr\(\"运行时长\"\)|@tr\(\"组织\"\)"), "keeps its own fact labels"),
            (re.compile(r"hero-fact-column-\d"), "keeps its own fact column fractions"),
        ):
            if pattern.search(text):
                found.append(
                    f"{path.relative_to(ROOT)}: {message}"
                )
    for path in sorted(ui.rglob("*.slint")):
        text = mask_slint_comments(path.read_text(encoding="utf-8"))
        if FABRICATED_FALLBACK.search(text):
            found.append(
                f"{path.relative_to(ROOT)}: a fabricated duration came back for an unobserved fact"
            )
        if FACT_FALLBACK.search(text):
            found.append(
                f"{path.relative_to(ROOT)}: a fact value carries an empty-string fallback"
            )
        if RETIRED_FACT_FIELDS.search(text):
            found.append(
                f"{path.relative_to(ROOT)}: consumes retired flat fact fields"
            )
    return found


def violations(
    ui: pathlib.Path = UI,
    contract: pathlib.Path = CONTRACT,
    presentation: pathlib.Path = PRESENTATION,
) -> list[str]:
    return source_violations(ui, contract, presentation)


ORIGINAL_READ_TEXT = pathlib.Path.read_text


class GuardSelfTest(unittest.TestCase):
    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(violations(), [])

    def test_fabricated_duration_is_caught(self) -> None:
        planted = (ROW_OWNER.read_text(encoding="utf-8") + "\nText { text: \"00:00:00\"; }")
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(path: pathlib.Path, *args, **kwargs):
                if path == ROW_OWNER:
                    return planted
                return ORIGINAL_READ_TEXT(path, *args, **kwargs)

            read_text.side_effect = fake_read_text
            found = violations()
        self.assertTrue(any("fabricated duration" in item for item in found))

    def test_second_fact_row_copy_is_caught(self) -> None:
        planted = CONSUMERS[0].read_text(encoding="utf-8").replace(
            "HomeHeroFactRow {", "HomeHeroFactRowLocal {", 1
        )
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(path: pathlib.Path, *args, **kwargs):
                if path == CONSUMERS[0]:
                    return planted
                return ORIGINAL_READ_TEXT(path, *args, **kwargs)

            read_text.side_effect = fake_read_text
            found = violations()
        self.assertTrue(
            any("renders its own hero fact row" in item for item in found)
        )

    def test_drifted_marker_is_caught(self) -> None:
        planted = CONTRACT.read_text(encoding="utf-8").replace(
            'pub const UNOBSERVED_FACT_MARKER: &str = "—";',
            'pub const UNOBSERVED_FACT_MARKER: &str = "n/a";',
            1,
        )
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(path: pathlib.Path, *args, **kwargs):
                if path == CONTRACT:
                    return planted
                return ORIGINAL_READ_TEXT(path, *args, **kwargs)

            read_text.side_effect = fake_read_text
            found = violations()
        self.assertTrue(any("marker drifted" in item for item in found))

    def test_a_literal_duration_is_caught(self) -> None:
        planted = PRESENTATION.read_text(encoding="utf-8").replace(
            "pub quality_tag: String,",
            'pub quality_tag: String,\n    const FIXED: &str = "03:04:49";',
            1,
        )
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(path: pathlib.Path, *args, **kwargs):
                if path == PRESENTATION:
                    return planted
                return ORIGINAL_READ_TEXT(path, *args, **kwargs)

            read_text.side_effect = fake_read_text
            found = violations()
        self.assertTrue(any("writes a session length" in item for item in found))

    def test_retired_fact_strings_are_caught(self) -> None:
        planted = PRESENTATION.read_text(encoding="utf-8").replace(
            "pub quality_tag: String,",
            "pub quality_tag: String,\n    pub uptime_formatted: String,",
            1,
        )
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(path: pathlib.Path, *args, **kwargs):
                if path == PRESENTATION:
                    return planted
                return ORIGINAL_READ_TEXT(path, *args, **kwargs)

            read_text.side_effect = fake_read_text
            found = violations()
        self.assertTrue(any("retired fact strings" in item for item in found))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=["report", "enforce"], default="report")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--ui-root", type=pathlib.Path, default=UI)
    parser.add_argument("--contract", type=pathlib.Path, default=CONTRACT)
    parser.add_argument("--presentation", type=pathlib.Path, default=PRESENTATION)
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        return 0 if unittest.TextTestRunner(verbosity=0).run(suite).wasSuccessful() else 1
    found = violations(args.ui_root, args.contract, args.presentation)
    for item in found:
        print(f"SLINT-011 [{args.mode}]: {item}")
    print(f"slint fact row guard: findings={len(found)}")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())

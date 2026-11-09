#!/usr/bin/env python3
"""SLINT-019: the desktop sidebar keeps one FlClash-shaped destination list.

The reference desktop scene and `prism_ui/lib/sidebar.dart` paint no group
heading: a heading spends a text line plus 16u of top air and splits the nav
stack into two detached stacks. The shared contract now owns this as a typed
`NavigationSeparator`; the generated ladder exports one decision per form and
the Slint sidebar consumes it. This guard keeps all three legs tied together.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
EXPORT = ROOT / "crates/infiltrator-contract/assets/page-structure.json"
UI = ROOT / "crates/infiltrator-slint/ui"
LADDER = UI / "components/viewport-ladder.slint"
SIDEBAR = UI / "components/sidebar.slint"
BODY = UI / "components/app_body.slint"
TIER = UI / "components/viewport_tier.slint"

SEPARATOR_ENUM = re.compile(
    r"export enum NavigationSeparator \{(.*?)\}", re.S
)
HEADING_PADDING = re.compile(r"padding-top:\s*LumenTheme\.rail-section-gap;")


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


def findings(export: pathlib.Path, ui: pathlib.Path) -> list[str]:
    found: list[str] = []
    document = json.loads(export.read_text(encoding="utf-8"))
    separators = document.get("navigation_separators", {})
    forms = {entry["id"]: entry["separator"] for entry in separators.get("forms", [])}
    expected_forms = {"bottom_nav", "rail", "standard", "wide"}
    if set(forms) != expected_forms or any(value != "none" for value in forms.values()):
        found.append(
            "page-structure.json: every navigation form must export the "
            "FlClash `none` separator"
        )
    variants = {entry["id"]: entry["paints_heading"] for entry in separators.get("variants", [])}
    if variants != {"none": False, "heading": True}:
        found.append("page-structure.json: the separator vocabulary no longer matches the contract")

    enum_match = SEPARATOR_ENUM.search(mask_comments((ui / "components/viewport_tier.slint").read_text(encoding="utf-8")))
    if not enum_match or not re.findall(r"^\s*(\w[-\w]*)\s*,", enum_match.group(1), re.M):
        found.append("viewport_tier.slint: the separator enum is missing")
    else:
        declared = re.findall(r"^\s*(\w[-\w]*)\s*,", enum_match.group(1), re.M)
        if declared != [entry.replace("_", "-") for entry in variants]:
            found.append("viewport_tier.slint: the separator enum no longer spells the exported ids")

    ladder = mask_comments((ui / "components/viewport-ladder.slint").read_text(encoding="utf-8"))
    if "public pure function navigation_separator(" not in ladder:
        found.append("viewport-ladder.slint: the exported separator lookup is missing")
    for form, separator in forms.items():
        expected = f"if (form == NavigationForm.{form.replace('_', '-')}) {{ return NavigationSeparator.{separator.replace('_', '-')}; }}"
        if expected not in ladder:
            found.append(f"viewport-ladder.slint: {form} does not export {separator}")

    sidebar = mask_comments((ui / "components/sidebar.slint").read_text(encoding="utf-8"))
    if "ViewportLadder.navigation_separator(root.navigation-form)" not in sidebar:
        found.append("sidebar.slint: the sidebar no longer reads the exported separator")
    heading_guard = (
        "if root.separator == NavigationSeparator.heading"
    )
    if heading_guard not in sidebar:
        found.append("sidebar.slint: a heading no longer checks the typed separator")
    if sidebar.count("padding-top: LumenTheme.rail-section-gap;") != 1:
        found.append("sidebar.slint: the only section-air token is no longer owned by the heading branch")
    if HEADING_PADDING.search(sidebar) and heading_guard not in sidebar:
        found.append("sidebar.slint: section air escaped the heading branch")
    body = mask_comments((ui / "components/app_body.slint").read_text(encoding="utf-8"))
    if "navigation-form: root.navigation-form;" not in body:
        found.append("app_body.slint: the sidebar is not handed the shell's typed form")
    return found


def live_findings() -> list[str]:
    return findings(EXPORT, UI)


class GuardSelfTest(unittest.TestCase):
    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(live_findings(), [])

    def test_a_heading_form_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            export = root / "page-structure.json"
            document = json.loads(EXPORT.read_text(encoding="utf-8"))
            document["navigation_separators"]["forms"][2]["separator"] = "heading"
            export.write_text(json.dumps(document), encoding="utf-8")
            ui = root / "ui"
            ui.mkdir()
            for name in ("viewport_tier.slint", "viewport-ladder.slint", "sidebar.slint", "app_body.slint"):
                source = UI / "components" / name
                (ui / "components").mkdir(exist_ok=True)
                (ui / "components" / name).write_text(source.read_text(encoding="utf-8"), encoding="utf-8")
            self.assertTrue(any("FlClash `none` separator" in item for item in findings(export, ui)))

    def test_a_surface_that_stops_reading_the_policy_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            ui = pathlib.Path(directory)
            (ui / "components").mkdir()
            for name in ("viewport_tier.slint", "viewport-ladder.slint", "sidebar.slint", "app_body.slint"):
                source = UI / "components" / name
                text = source.read_text(encoding="utf-8")
                if name == "sidebar.slint":
                    text = text.replace("ViewportLadder.navigation_separator(root.navigation-form)", "NavigationSeparator.heading")
                (ui / "components" / name).write_text(text, encoding="utf-8")
            self.assertTrue(any("no longer reads" in item for item in findings(EXPORT, ui)))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("enforce", "report"), default="enforce")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    found = live_findings()
    for item in found:
        print(f"SLINT-019 [{args.mode}]: {item}")
    print(f"slint sidebar separator guard: findings={len(found)}")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())

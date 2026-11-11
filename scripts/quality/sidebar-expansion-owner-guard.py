#!/usr/bin/env python3
"""Guard the owner chain for FlClash's desktop sidebar-expansion preference.

The preference is persisted by the settings owner, changed through one typed
command, and read back in the shared settings page snapshot. This guard rejects
a UI-local boolean, an untyped key path, or a command that disappears from one
of the exhaustive dispatch/name tables. It also pins the exported closed
preference x viewport x surface-kind table and its generated Slint adapter, so
the surface and the space judge cannot invent a second resolution of the same
preference.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
COMMAND = ROOT / "crates/infiltrator-contract/src/command.rs"
SNAPSHOT = ROOT / "crates/infiltrator-contract/src/surface_snapshot.rs"
SETTINGS = ROOT / "crates/infiltrator-domain/src/settings.rs"
DISPATCH = ROOT / "crates/infiltrator-application/src/command_application/dispatch.rs"
APPLICATION = ROOT / "crates/infiltrator-application/src/command_application.rs"
COMMAND_NAME = ROOT / "crates/infiltrator-application/src/core_application/command_name.rs"
BUILDER = ROOT / "crates/infiltrator-composition/src/surface_reader/page_builders.rs"
CONTRACT_TESTS = ROOT / "crates/infiltrator-contract/src/lib.rs"
APPLICATION_TESTS = ROOT / (
    "crates/infiltrator-application/src/command_application_settings_tests.rs"
)
COMPOSITION_TESTS = ROOT / (
    "crates/infiltrator-composition/src/surface_reader/settings_observation_tests.rs"
)
GATE = ROOT / "scripts/quality/check-structure.sh"
PAGE_STRUCTURE = ROOT / "crates/infiltrator-contract/src/page_structure.rs"
PAGE_STRUCTURE_EXPORT = ROOT / "crates/infiltrator-contract/assets/page-structure.json"
SHELL_RECEIPT = ROOT / "crates/infiltrator-contract/src/shell_receipt.rs"
SHELL_RECEIPT_HOST = ROOT / "crates/infiltrator-slint/src/shell_receipt_host.rs"
PROJECTION = ROOT / "crates/infiltrator-slint/src/lumen_projection.rs"
NAVIGATION_CALLBACKS = ROOT / "crates/infiltrator-slint/src/callbacks/navigation.rs"
CAPTURE_MATRIX = ROOT / "scripts/capture_slint_scenarios.tsv"
GENERATOR = ROOT / "scripts/generate_viewport_ladder.py"
SLINT_TIER = ROOT / "crates/infiltrator-slint/ui/components/viewport_tier.slint"
SLINT_LADDER = ROOT / "crates/infiltrator-slint/ui/components/viewport-ladder.slint"
SLINT_THEME = ROOT / "crates/infiltrator-slint/ui/components/theme.slint"
SLINT_SHELL = ROOT / "crates/infiltrator-slint/ui/lumen-app.slint"
VIEWPORT_GUARD = ROOT / "scripts/quality/slint-viewport-tier-guard.py"

MATRIX_HEADER = (
    "name", "page", "skin", "window_size", "scroll", "mascot_frame",
    "mascot_millis", "sidebar_expansion", "interaction",
)
MATRIX_ROW_COUNT = 199

REQUIRED = {
    "command": (
        "use crate::shell_sidebar_preference::SidebarExpansionIntent;",
        "ApplySidebarExpansion {",
        "intent: SidebarExpansionIntent,",
        "| Self::ApplySidebarExpansion { .. }",
    ),
    "snapshot": (
        "use crate::shell_sidebar_preference::SidebarExpansionPreference;",
        "pub sidebar_expansion: SidebarExpansionPreference,",
    ),
    "settings": (
        "use infiltrator_contract::shell_sidebar_preference::SidebarExpansionPreference;",
        "pub sidebar_expansion: SidebarExpansionPreference,",
        "sidebar_expansion: SidebarExpansionPreference::default(),",
    ),
    "dispatch": (
        "CommandIntent::ApplySidebarExpansion { intent } => {",
        "self.apply_sidebar_expansion(intent).await",
    ),
    "application": (
        "async fn apply_sidebar_expansion(",
        "apply_sidebar_expansion_intent(current, intent)",
        "settings.sidebar_expansion = next",
    ),
    "name": ('CommandIntent::ApplySidebarExpansion { .. } => "apply_sidebar_expansion",',),
    "builder": ("sidebar_expansion: settings.sidebar_expansion,",),
    "contract_tests": (
        "use super::shell_sidebar_preference::SidebarExpansionIntent;",
        "CommandIntent::ApplySidebarExpansion {",
    ),
    "application_tests": (
        "async fn sidebar_expansion_intents_persist_one_typed_preference()",
        "SidebarExpansionPreference::Collapsed",
    ),
    "composition_tests": (
        "facts.sidebar_expansion",
        "SidebarExpansionPreference::Collapsed",
    ),
    "shell_receipt": (
        '"sidebar_expansion",',
        '"touch_surface",',
        '"sidebar_toggle_px",',
        "pub sidebar_expansion: SidebarExpansionPreference,",
        "pub touch_surface: bool,",
        "pub sidebar_toggle_px: u16,",
    ),
    "shell_receipt_host": (
        "chrome.sidebar_expansion",
        "chrome.touch_surface",
        "chrome.sidebar_toggle",
    ),
    "projection": (
        "window.set_sidebar_expansion(",
        "settings.sidebar_expansion",
    ),
    "navigation_callbacks": (
        "on_toggle_sidebar_expansion",
        "SidebarExpansionIntent::Toggle",
    ),
    "capture_matrix": (
        "\t".join(MATRIX_HEADER),
        "sidebar-collapsed-standard",
        "sidebar-collapsed-compact",
    ),
    "page_structure": (
        "pub struct SidebarVariantFact",
        "pub fn sidebar_variant_facts()",
        "resolve_sidebar_expansion(preference, capability)",
        "sidebar_variants",
    ),
    "page_structure_export": (
        "\"version\":11",
        "\"sidebar_variants\"",
        "\"toggle_available\":true",
    ),
    "generator": (
        "document[\"sidebar_variants\"]",
        "def sidebar_variant_function(",
        "\"sidebar_form\",",
        "\"sidebar_nav_column\",",
        "\"sidebar_labelled\",",
        "\"sidebar_toggle_available\",",
    ),
    "slint_tier": (
        "export enum SidebarExpansionPreference",
        "    expanded,",
        "    collapsed,",
    ),
    "slint_ladder": (
        "public pure function sidebar_form(",
        "public pure function sidebar_nav_column(",
        "public pure function sidebar_labelled(",
        "public pure function sidebar_toggle_available(",
    ),
    "slint_theme": (
        "ViewportLadder.sidebar_form(",
        "ViewportLadder.sidebar_nav_column(",
        "ViewportLadder.sidebar_labelled(",
        "ViewportLadder.sidebar_toggle_available(",
    ),
    "slint_shell": (
        "in-out property <SidebarExpansionPreference> sidebar-expansion:",
        "sidebar-expansion: root.sidebar-expansion",
        "touch-surface: root.android-platform",
        "LumenTheme.sidebar-expansion = root.sidebar-expansion;",
        "LumenTheme.touch-surface = root.android-platform;",
    ),
    "viewport_guard": (
        'document.get("version") != 11',
        "sidebar-variant export",
        "enum_variants(\"SidebarExpansionPreference\")",
    ),
}

FILES = {
    "command": COMMAND,
    "snapshot": SNAPSHOT,
    "settings": SETTINGS,
    "dispatch": DISPATCH,
    "application": APPLICATION,
    "name": COMMAND_NAME,
    "builder": BUILDER,
    "contract_tests": CONTRACT_TESTS,
    "application_tests": APPLICATION_TESTS,
    "composition_tests": COMPOSITION_TESTS,
    "shell_receipt": SHELL_RECEIPT,
    "shell_receipt_host": SHELL_RECEIPT_HOST,
    "projection": PROJECTION,
    "navigation_callbacks": NAVIGATION_CALLBACKS,
    "capture_matrix": CAPTURE_MATRIX,
    "page_structure": PAGE_STRUCTURE,
    "page_structure_export": PAGE_STRUCTURE_EXPORT,
    "generator": GENERATOR,
    "slint_tier": SLINT_TIER,
    "slint_ladder": SLINT_LADDER,
    "slint_theme": SLINT_THEME,
    "slint_shell": SLINT_SHELL,
    "viewport_guard": VIEWPORT_GUARD,
}


def capture_matrix_findings(source: str) -> list[str]:
    """The matrix owns shell fixtures and business interactions as separate facts."""
    lines = source.splitlines()
    found: list[str] = []
    if not lines or tuple(lines[0].split("\t")) != MATRIX_HEADER:
        return ["capture_matrix does not publish the nine-column typed header"]
    rows = [line.split("\t") for line in lines[1:] if line]
    if len(rows) != MATRIX_ROW_COUNT:
        found.append(f"capture_matrix has {len(rows)} rows, expected {MATRIX_ROW_COUNT}")
    for number, row in enumerate(rows, start=2):
        if len(row) != len(MATRIX_HEADER):
            found.append(f"capture_matrix line {number} has {len(row)} columns")
            continue
        name, *_rest, sidebar_expansion, interaction = row
        is_viewport_row = name.endswith(("-standard", "-compact"))
        is_shell_form = name in {"sidebar-collapsed-standard", "sidebar-collapsed-compact"}
        if is_shell_form:
            if sidebar_expansion != "collapsed":
                found.append(f"capture_matrix shell form {name} is not collapsed")
            if interaction:
                found.append(f"capture_matrix shell form {name} must not derive a business interaction")
        elif is_viewport_row:
            expected_interaction = name.rsplit("-", 1)[0]
            if sidebar_expansion != "expanded" or interaction != expected_interaction:
                found.append(f"capture_matrix business row {name} has the wrong owner or interaction")
        elif interaction:
            found.append(f"capture_matrix inspection row {name} must not carry a business interaction")
    return found


def findings(texts: dict[str, str], gate: str, export: dict | None = None) -> list[str]:
    found: list[str] = []
    for key, needles in REQUIRED.items():
        source = texts[key]
        for needle in needles:
            if needle not in source:
                found.append(f"{key} is missing {needle!r}")
    found.extend(capture_matrix_findings(texts["capture_matrix"]))
    if "sidebar_expansion" in texts["application"] and "ApplySidebarExpansion" not in texts["dispatch"]:
        found.append("the typed preference command is no longer dispatched")
    if "sidebar-expansion-owner-guard.py --self-test" not in gate:
        found.append("the guard self-test is not part of the structure gate")
    if "sidebar-expansion-owner-guard.py --mode enforce" not in gate:
        found.append("the guard enforce leg is not part of the structure gate")
    document = export if export is not None else {}
    variants = document.get("sidebar_variants", [])
    if len(variants) != 16:
        found.append(f"the exported sidebar variant table has {len(variants)} rows, expected 16")
    collapsed_desktop = [
        entry for entry in variants
        if entry.get("tier") == "expanded"
        and entry.get("preference") == "collapsed"
        and entry.get("touch_surface") is False
    ]
    if len(collapsed_desktop) != 1 or collapsed_desktop[0].get("nav_form") != "rail" \
            or collapsed_desktop[0].get("nav_column_px") != 64 \
            or collapsed_desktop[0].get("toggle_available") is not True:
        found.append("the collapsed desktop variant is not the exported rail")
    touch_desktop = [
        entry for entry in variants
        if entry.get("tier") == "expanded"
        and entry.get("preference") == "expanded"
        and entry.get("touch_surface") is True
    ]
    if len(touch_desktop) != 1 or touch_desktop[0].get("nav_form") != "bottom_nav" \
            or touch_desktop[0].get("toggle_available") is not False:
        found.append("the touch desktop variant still offers a sidebar toggle")
    return found


def check() -> list[str]:
    paths = list(FILES.values()) + [GATE]
    missing = [str(path.relative_to(ROOT)) for path in paths if not path.is_file()]
    if missing:
        return [f"missing owner-chain file: {path}" for path in missing]
    texts = {key: path.read_text(encoding="utf-8") for key, path in FILES.items()}
    export = json.loads(PAGE_STRUCTURE_EXPORT.read_text(encoding="utf-8"))
    return findings(texts, GATE.read_text(encoding="utf-8"), export)


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        self.texts = {key: path.read_text(encoding="utf-8") for key, path in FILES.items()}
        self.gate = GATE.read_text(encoding="utf-8")
        self.export = json.loads(PAGE_STRUCTURE_EXPORT.read_text(encoding="utf-8"))

    def found(self, **changes) -> list[str]:
        texts = dict(self.texts)
        export = self.export
        gate = changes.pop("gate", self.gate)
        export = changes.pop("export", export)
        texts.update(changes)
        return findings(texts, gate, export)

    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(self.found(), [])

    def test_a_missing_typed_command_is_caught(self) -> None:
        planted = "\n".join(
            line for line in self.texts["command"].splitlines()
            if "ApplySidebarExpansion {" not in line
        )
        self.assertTrue(any("missing 'ApplySidebarExpansion {'" in item for item in self.found(command=planted)))

    def test_a_missing_readback_is_caught(self) -> None:
        planted = self.texts["builder"].replace(
            "sidebar_expansion: settings.sidebar_expansion,\n", "", 1
        )
        self.assertTrue(any("builder is missing" in item for item in self.found(builder=planted)))

    def test_a_missing_gate_leg_is_caught(self) -> None:
        planted = self.gate.replace(
            "sidebar-expansion-owner-guard.py --mode enforce\n", "", 1
        )
        self.assertTrue(any("enforce leg" in item for item in self.found(gate=planted)))

    def test_a_missing_variant_table_is_caught(self) -> None:
        planted = dict(self.export)
        planted["sidebar_variants"] = []
        self.assertTrue(any("variant table has 0 rows" in item for item in self.found(export=planted)))

    def test_a_touch_surface_with_a_toggle_is_caught(self) -> None:
        planted = json.loads(json.dumps(self.export))
        for entry in planted["sidebar_variants"]:
            if entry["tier"] == "expanded" and entry["touch_surface"]:
                entry["toggle_available"] = True
        self.assertTrue(any("touch desktop variant" in item for item in self.found(export=planted)))

    def test_a_surface_that_stops_reading_the_variant_table_is_caught(self) -> None:
        planted = self.texts["slint_theme"].replace(
            "ViewportLadder.sidebar_nav_column(", "ViewportLadder.nav_column(", 1
        )
        self.assertTrue(any("slint_theme is missing" in item for item in self.found(slint_theme=planted)))

    def test_a_matrix_that_drops_the_explicit_interaction_column_is_caught(self) -> None:
        planted = self.texts["capture_matrix"].replace("\tinteraction", "", 1)
        self.assertTrue(any("nine-column typed header" in item for item in self.found(capture_matrix=planted)))

    def test_a_shell_form_that_derives_a_business_interaction_is_caught(self) -> None:
        planted = self.texts["capture_matrix"].replace(
            "sidebar-collapsed-standard\toverview\tdark\t1180x780\t\t\t\tcollapsed\t",
            "sidebar-collapsed-standard\toverview\tdark\t1180x780\t\t\t\tcollapsed\tsidebar-collapsed",
            1,
        )
        self.assertNotEqual(planted, self.texts["capture_matrix"])
        self.assertTrue(any("must not derive a business interaction" in item for item in self.found(capture_matrix=planted)))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="enforce")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        result = unittest.TextTestRunner(verbosity=1).run(suite)
        return 0 if result.wasSuccessful() else 1
    problems = check()
    for problem in problems:
        print(f"sidebar expansion owner guard: {problem}", file=sys.stderr)
    if problems and args.mode == "enforce":
        return 1
    print(
        f"sidebar expansion owner guard: {len(REQUIRED)} owner-chain files and "
        "the closed variant table adjudicated, "
        f"findings={len(problems)}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())

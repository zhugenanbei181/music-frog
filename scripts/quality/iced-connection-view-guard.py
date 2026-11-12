#!/usr/bin/env python3
"""CPPARITY-006: Iced Connections view controls stay passive and Core-owned.

The pinned ClashParty source persists ``connectionViewMode`` and an ordered
column set. Any Iced-local ``bool``/``Set`` or another 22-column literal would
publish a second preference truth. This guard reads the slice's source files
lexically and proves that:

* List and Table are both real segments and both emit the typed preference
  message;
* the 22 choices come from the shared projection, not ``ConnectionColumn::ALL``
  in the view;
* Reset emits ``ConnectionViewIntent::ResetColumns``;
* all user-visible copy goes through ``Lang(...).tr(...)``;
* the slice carries no hard-coded Chinese copy or ``connection.column.``
  string table;
* ``check-structure.sh`` executes both the self-test and enforce legs.

Usage:
    python3 scripts/quality/iced-connection-view-guard.py --self-test
    python3 scripts/quality/iced-connection-view-guard.py [--mode report|enforce]
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
QUALITY_ROOT = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(QUALITY_ROOT))
from rust_syntax import mask_comments, mask_noncode, mask_test_items  # noqa: E402

CONTROLS = ROOT / "crates/infiltrator-iced/src/view/runtime/connection_view_controls.rs"
CONTROLS_TESTS = ROOT / "crates/infiltrator-iced/tests/gui/connection_view_controls_tests.rs"
CONNECTIONS = ROOT / "crates/infiltrator-iced/src/view/runtime/connections.rs"
CONTROLS_SEAM = ROOT / "crates/infiltrator-iced/src/view/runtime/connections_controls.rs"
REGIONS = ROOT / "crates/infiltrator-iced/src/view_root/interaction_regions.rs"
INTEGRATION_TESTS = ROOT / "crates/infiltrator-iced/tests/gui/view_runtime_connections_tests.rs"
GATE = ROOT / "scripts/quality/check-structure.sh"
GUARD = "iced-connection-view-guard.py"

SLICE_FILES = (
    CONTROLS,
    CONTROLS_TESTS,
    CONNECTIONS,
    CONTROLS_SEAM,
    REGIONS,
    INTEGRATION_TESTS,
)
CHINESE = re.compile(r"[\u3400-\u9fff]")
COLUMN_LITERAL = re.compile(r"connection\.column\.")
VIEW_ARRAY = re.compile(
    r"ConnectionColumn::ALL|\bconst\s+[A-Z_]*(?:COLUMNS|CHOICES)[A-Z_]*\s*:"
)
MODE_LOOP = re.compile(r"ConnectionViewMode::ALL")
APPLY = re.compile(r"Message::ApplyConnectionViewPreference\s*\{")
TABLE = re.compile(r"ConnectionViewMode::Table|CONNECTION_VIEW_MODE_TABLE_KEY")
LIST = re.compile(r"ConnectionViewMode::List|CONNECTION_VIEW_MODE_LIST_KEY")
RESET = re.compile(r"ConnectionViewIntent::ResetColumns")
NOOP = re.compile(r"Message::Noop")
CHOICES = re.compile(r"project_connection_column_choices\s*\(")
LANG_TR = re.compile(r"\bLang\s*\([^)]*\)\s*|\b\w+\s*\.\s*tr\s*\(")
CONTROL_MOUNT = re.compile(r"view_mode_controls\s*\(")


def read(path: pathlib.Path) -> str:
    return path.read_text(encoding="utf-8") if path.is_file() else ""


def production(text: str) -> str:
    """Comments and test items cannot satisfy a production rule.

    String literals stay visible: these rules intentionally read stable copy
    keys, semantic IDs and typed wire vocabulary from the source.
    """
    return mask_test_items(mask_comments(text))


def findings(
    controls_text: str | None = None,
    controls_tests_text: str | None = None,
    connections_text: str | None = None,
    seam_text: str | None = None,
    regions_text: str | None = None,
    gate_text: str | None = None,
) -> list[str]:
    controls = production(
        read(CONTROLS) if controls_text is None else controls_text
    )
    controls_tests = production(
        read(CONTROLS_TESTS) if controls_tests_text is None else controls_tests_text
    )
    connections = production(
        read(CONNECTIONS) if connections_text is None else connections_text
    )
    seam = production(read(CONTROLS_SEAM) if seam_text is None else seam_text)
    regions = production(read(REGIONS) if regions_text is None else regions_text)
    gate = read(GATE) if gate_text is None else gate_text
    problems: list[str] = []

    if not LIST.search(controls) or not TABLE.search(controls):
        problems.append("the view is missing one of the typed List/Table segments")
    if len(APPLY.findall(controls)) < 3:
        problems.append("mode, column and reset controls must all publish the typed preference message")
    set_view_mode_windows = [
        controls[match.start() : match.start() + 240]
        for match in APPLY.finditer(controls)
    ]
    if not MODE_LOOP.search(controls) or not any(
        "SetViewMode" in window for window in set_view_mode_windows
    ):
        problems.append("both mode segments must publish ApplyConnectionViewPreference")
    if not CHOICES.search(controls):
        problems.append("the column choices no longer come from the shared projection")
    if VIEW_ARRAY.search(controls):
        problems.append("the view has rebuilt a second 22-column registry")
    if not RESET.search(controls) or NOOP.search(controls):
        problems.append("Reset must publish ResetColumns rather than a dead input")
    if not LANG_TR.search(controls):
        problems.append("the controls do not read user-visible copy through Lang")
    for path, text in (
        (CONTROLS, controls),
        (CONTROLS_TESTS, controls_tests),
        (CONNECTIONS, connections),
        (CONTROLS_SEAM, seam),
        (REGIONS, regions),
    ):
        if CHINESE.search(text):
            problems.append(f"{path.relative_to(ROOT)} hard-codes Chinese business copy")
        if COLUMN_LITERAL.search(text) and path != CONTROLS_TESTS:
            problems.append(f"{path.relative_to(ROOT)} owns a connection.column string table")

    if not CONTROL_MOUNT.search(connections) and not CONTROL_MOUNT.search(seam):
        problems.append("the Connections page no longer mounts the view preference controls")
    for region in (
        "ConnectionViewControls",
        "ConnectionViewModeList",
        "ConnectionViewModeTable",
        "ConnectionViewResetColumns",
    ):
        if region not in regions:
            problems.append(f"stable interaction region {region} is missing")
    for leg in (f"{GUARD} --self-test", f"{GUARD} --mode enforce"):
        if leg not in gate:
            problems.append(f"structure gate is missing leg: {leg}")
    return problems


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        self.live = {
            "controls_text": read(CONTROLS),
            "controls_tests_text": read(CONTROLS_TESTS),
            "connections_text": read(CONNECTIONS),
            "seam_text": read(CONTROLS_SEAM),
            "regions_text": read(REGIONS),
            "gate_text": read(GATE),
        }

    def planted(self, **changes: str) -> list[str]:
        values = dict(self.live)
        values.update(changes)
        return findings(**values)

    def test_the_live_tree_passes(self) -> None:
        self.assertEqual(findings(), [])

    def test_deleting_the_table_button_is_rejected(self) -> None:
        planted = self.live["controls_text"].replace(
            "ConnectionViewMode::Table => CONNECTION_VIEW_MODE_TABLE_KEY,",
            "ConnectionViewMode::Table => CONNECTION_VIEW_MODE_LIST_KEY,",
        )
        planted = planted.replace(
            "ConnectionViewMode::ALL;",
            "[ConnectionViewMode::List];",
        )
        planted = planted.replace("ConnectionViewMode::Table", "ConnectionViewMode::List")
        planted = planted.replace("CONNECTION_VIEW_MODE_TABLE_KEY", "CONNECTION_VIEW_MODE_LIST_KEY")
        planted = planted.replace(
            "let modes = ConnectionViewMode::ALL;",
            "let modes = [ConnectionViewMode::List];",
        )
        self.assertTrue(any("List/Table segments" in item for item in self.planted(controls_text=planted)))

    def test_a_view_owned_twenty_two_column_array_is_rejected(self) -> None:
        planted = self.live["controls_text"].replace(
            "let modes = ConnectionViewMode::ALL;",
            "let modes = ConnectionViewMode::ALL;\n    let shadow_registry = ConnectionColumn::ALL;",
        )
        self.assertTrue(any("second 22-column registry" in item for item in self.planted(controls_text=planted)))

    def test_reset_changed_to_noop_is_rejected(self) -> None:
        planted = self.live["controls_text"].replace(
            "intent: ConnectionViewIntent::ResetColumns,",
            "intent: ConnectionViewIntent::SetViewMode { mode: ConnectionViewMode::List },",
        )
        self.assertTrue(any("Reset must publish ResetColumns" in item for item in self.planted(controls_text=planted)))

    def test_hard_coded_chinese_copy_is_rejected(self) -> None:
        planted = self.live["controls_text"] + '\nconst SHADOW_COPY: &str = "表格视图";\n'
        self.assertTrue(any("hard-codes Chinese business copy" in item for item in self.planted(controls_text=planted)))

    def test_a_missing_structure_gate_leg_is_rejected(self) -> None:
        planted = self.live["gate_text"].replace(
            f"gate python3 scripts/quality/{GUARD} --mode enforce",
            "gate python3 scripts/quality/flag-plate-guard.py --mode enforce",
        )
        self.assertTrue(any("structure gate is missing leg" in item for item in self.planted(gate_text=planted)))

    def test_a_second_column_string_table_is_rejected(self) -> None:
        planted = self.live["controls_text"] + '\nconst SHADOW_KEY: &str = "connection.column.host";\n'
        self.assertTrue(any("connection.column string table" in item for item in self.planted(controls_text=planted)))

    def test_a_comment_does_not_satisfy_the_column_choices_rule(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "connection_view_controls.rs"
            path.write_text(
                "// project_connection_column_choices()\n"
                + self.live["controls_text"].replace(
                    "project_connection_column_choices()",
                    "project_connection_view(&registry, &preference)",
                ),
                encoding="utf-8",
            )
            self.assertTrue(
                any("shared projection" in item for item in self.planted(controls_text=path.read_text(encoding="utf-8")))
            )

    def test_dead_column_control_is_rejected(self) -> None:
        planted = self.live["controls_text"].replace(
            "move |_| Message::ApplyConnectionViewPreference {\n                    intent: intent.clone(),\n                },",
            "move |_| Message::Noop,",
        )
        self.assertTrue(any("dead input" in item for item in self.planted(controls_text=planted)))

    def test_shared_projection_is_used_in_tests_too(self) -> None:
        planted = self.live["controls_tests_text"].replace(
            "project_connection_column_choices()",
            "project_connection_view(&ConnectionColumnSet::all(), &ConnectionViewPreference::default())",
        )
        self.assertEqual(self.planted(controls_tests_text=planted), [])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        result = unittest.TextTestRunner(stream=sys.stdout, verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    problems = findings()
    if problems:
        for problem in problems:
            print(f"iced connection view guard: {problem}", file=sys.stderr)
        print(f"iced connection view guard: findings={len(problems)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("iced connection view guard: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())

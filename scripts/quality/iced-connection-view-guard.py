#!/usr/bin/env python3
"""CPPARITY-006: Iced Connections view controls stay passive and Core-owned.

The pinned ClashParty source persists ``connectionViewMode`` and an ordered
column set. Any Iced-local ``bool``/``Set`` or another 22-column literal would
publish a second preference truth. This guard reads the slice's source files
lexically and proves that:

* List and Table are both real segments and both emit the typed preference
  message;
* the 23 choices come from the shared projection, not ``ConnectionColumn::ALL``
  in the view;
* Reset emits ``ConnectionViewIntent::ResetColumns``;
* all user-visible copy goes through ``Lang(...).tr(...)``;
* the slice carries no hard-coded Chinese copy or ``connection.column.``
  string table;
* the page body *consumes* the persisted morphology - the readback decides
  whether the page renders the list of cards or the table of columns, and the
  table's cells come from the shared projection rather than from a second
  column vocabulary in the view;
* the column chooser and the grouping control are mounted by the shared
  morphology rule, so neither can appear as a control that changes nothing;
* the rule that mounts them lives in the shared owner rather than in the
  surface, and the page's control bands - and the rows they share at a given
  viewport tier - are published by that owner too, so a narrow window cannot
  spend its vertical budget on a stack the surface arranged on its own;
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
TABLE_VIEW = ROOT / "crates/infiltrator-iced/src/view/runtime/connections_table.rs"
TABLE_TESTS = ROOT / "crates/infiltrator-iced/tests/gui/connections_table_tests.rs"
CONTROLS_SEAM = ROOT / "crates/infiltrator-iced/src/view/runtime/connections_controls.rs"
REGIONS = ROOT / "crates/infiltrator-iced/src/view_root/interaction_regions.rs"
INTEGRATION_TESTS = ROOT / "crates/infiltrator-iced/tests/gui/view_runtime_connections_tests.rs"
OWNER = ROOT / "crates/infiltrator-presentation/src/connection_page_chrome.rs"
GATE = ROOT / "scripts/quality/check-structure.sh"
GUARD = "iced-connection-view-guard.py"

SLICE_FILES = (
    CONTROLS,
    CONTROLS_TESTS,
    CONNECTIONS,
    TABLE_VIEW,
    TABLE_TESTS,
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
MODE_READBACK = re.compile(r"connection_view\.mode")
TABLE_PROJECTION = re.compile(r"project_connection_view_table\s*\(")
CHOOSER_MOUNT = re.compile(r"column_chooser_mounted\s*\(")
GROUPING_MOUNT = re.compile(r"grouping_control_mounted\s*\(")
TABLE_READBACK = re.compile(r"diag\.connection_view")
OUTBOUND_FILTER = re.compile(r"Message::UpdateRuntimeConnectionFilter")
WALL_CLOCK = re.compile(r"Utc::now\(\)|SystemTime::now\(\)|current_unix_secs\(\)")
CONTROL_STACK = re.compile(r"connection_control_stack\s*\(")
PAGE_CHROME = re.compile(r"connection_page_chrome\s*\(")
CHOOSER_PLACEMENT = re.compile(r"chrome\.chooser")
TIER_BRANCH = re.compile(r"is_narrow\s*\(")
LOCAL_MOUNT_DEF = re.compile(
    r"fn\s+(?:grouping_control_mounted|column_chooser_mounted)\s*\("
)
OWNER_STACK = re.compile(r"pub fn connection_control_stack\s*\(")
OWNER_GROUPING = re.compile(r"pub const fn grouping_control_mounted\s*\(")
OWNER_CHOOSER = re.compile(r"pub const fn column_chooser_mounted\s*\(")
OWNER_TIER = re.compile(r"ViewportTier")
OWNER_TRAFFIC_PLACEMENT = re.compile(r"pub enum ConnectionTrafficPlacement")
OWNER_CHOOSER_PLACEMENT = re.compile(r"pub enum ConnectionChooserPlacement")


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
    table_text: str | None = None,
    seam_text: str | None = None,
    regions_text: str | None = None,
    gate_text: str | None = None,
    owner_text: str | None = None,
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
    table = production(read(TABLE_VIEW) if table_text is None else table_text)
    seam = production(read(CONTROLS_SEAM) if seam_text is None else seam_text)
    regions = production(read(REGIONS) if regions_text is None else regions_text)
    gate = read(GATE) if gate_text is None else gate_text
    owner = production(read(OWNER) if owner_text is None else owner_text)
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
        problems.append("the view has rebuilt a second 23-column registry")
    if not RESET.search(controls) or NOOP.search(controls):
        problems.append("Reset must publish ResetColumns rather than a dead input")
    if not LANG_TR.search(controls):
        problems.append("the controls do not read user-visible copy through Lang")
    for path, text in (
        (CONTROLS, controls),
        (CONTROLS_TESTS, controls_tests),
        (CONNECTIONS, connections),
        (TABLE_VIEW, table),
        (CONTROLS_SEAM, seam),
        (REGIONS, regions),
    ):
        if CHINESE.search(text):
            problems.append(f"{path.relative_to(ROOT)} hard-codes Chinese business copy")
        if COLUMN_LITERAL.search(text) and path != CONTROLS_TESTS:
            problems.append(f"{path.relative_to(ROOT)} owns a connection.column string table")

    if not CONTROL_MOUNT.search(connections) and not CONTROL_MOUNT.search(seam):
        problems.append("the Connections page no longer mounts the view preference controls")
    if not MODE_READBACK.search(connections):
        problems.append(
            "the Connections page does not read the persisted morphology readback"
        )
    if not TABLE_PROJECTION.search(connections):
        problems.append(
            "the Connections page does not render the shared table projection"
        )
    if not CHOOSER_MOUNT.search(controls):
        problems.append(
            "the column chooser is not mounted by the shared morphology rule"
        )
    if not GROUPING_MOUNT.search(connections):
        problems.append(
            "the grouping control is not mounted by the shared morphology rule"
        )
    if not CONTROL_STACK.search(connections):
        problems.append(
            "the Connections page does not consume the shared control stack"
        )
    if TIER_BRANCH.search(connections):
        problems.append(
            "the Connections page decides the control-stack arrangement itself"
        )
    if not PAGE_CHROME.search(connections):
        problems.append(
            "the Connections page does not consume the shared page chrome"
        )
    if not CHOOSER_PLACEMENT.search(connections):
        problems.append(
            "the column chooser ignores the shared placement rule"
        )
    for path, text in (
        (CONTROLS, controls),
        (CONNECTIONS, connections),
    ):
        if LOCAL_MOUNT_DEF.search(text):
            problems.append(
                f"{path.relative_to(ROOT)} owns a morphology mount rule instead of the shared owner"
            )
    if not OWNER_STACK.search(owner) or not OWNER_TIER.search(owner):
        problems.append(
            "the shared owner no longer publishes the connections control stack"
        )
    if not OWNER_GROUPING.search(owner) or not OWNER_CHOOSER.search(owner):
        problems.append(
            "the shared owner no longer publishes the morphology mount rules"
        )
    if not OWNER_TRAFFIC_PLACEMENT.search(owner) or not OWNER_CHOOSER_PLACEMENT.search(owner):
        problems.append(
            "the shared owner no longer publishes the page chrome placements"
        )
    if VIEW_ARRAY.search(table):
        problems.append("the table module has rebuilt a second column registry")
    if TABLE_READBACK.search(table):
        problems.append(
            "the table module reads the preference readback instead of the projection"
        )
    if not OUTBOUND_FILTER.search(table):
        problems.append(
            "the table's outbound chip does not publish the typed filter intent"
        )
    for path, text in (
        (CONNECTIONS, connections),
        (TABLE_VIEW, table),
    ):
        if WALL_CLOCK.search(text):
            problems.append(
                f"{path.relative_to(ROOT)} reads a wall clock instead of the owner's instant"
            )
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
            "table_text": read(TABLE_VIEW),
            "seam_text": read(CONTROLS_SEAM),
            "regions_text": read(REGIONS),
            "gate_text": read(GATE),
            "owner_text": read(OWNER),
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
        self.assertTrue(any("second 23-column registry" in item for item in self.planted(controls_text=planted)))

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

    def test_a_page_that_ignores_the_morphology_readback_is_rejected(self) -> None:
        planted = self.live["connections_text"].replace(
            "state.diag.connection_view.mode",
            "ConnectionViewMode::List",
        )
        self.assertTrue(
            any(
                "does not read the persisted morphology readback" in item
                for item in self.planted(connections_text=planted)
            )
        )

    def test_a_page_that_drops_the_table_projection_is_rejected(self) -> None:
        planted = self.live["connections_text"].replace(
            "project_connection_view_table(",
            "own_table(",
        )
        self.assertTrue(
            any(
                "does not render the shared table projection" in item
                for item in self.planted(connections_text=planted)
            )
        )

    def test_a_table_owned_column_registry_is_rejected(self) -> None:
        planted = self.live["table_text"] + "\nconst SHADOW: [ConnectionColumn; 23] = ConnectionColumn::ALL;\n"
        self.assertTrue(
            any(
                "second column registry" in item
                for item in self.planted(table_text=planted)
            )
        )

    def test_a_table_that_reads_the_preference_readback_is_rejected(self) -> None:
        planted = self.live["table_text"] + "\nlet _ = state.diag.connection_view.columns;\n"
        self.assertTrue(
            any(
                "reads the preference readback instead of the projection" in item
                for item in self.planted(table_text=planted)
            )
        )

    def test_a_dead_outbound_chip_is_rejected(self) -> None:
        planted = self.live["table_text"].replace(
            "Message::UpdateRuntimeConnectionFilter(filter)",
            "Message::Noop",
        )
        self.assertTrue(
            any(
                "outbound chip does not publish the typed filter intent" in item
                for item in self.planted(table_text=planted)
            )
        )

    def test_a_view_that_reads_its_own_clock_is_rejected(self) -> None:
        planted = self.live["table_text"].replace(
            "    let headers = &projection.headers;",
            "    let _now = chrono::Utc::now();\n    let headers = &projection.headers;",
        )
        self.assertTrue(
            any(
                "reads a wall clock instead of the owner's instant" in item
                for item in self.planted(table_text=planted)
            )
        )

    def test_a_chooser_without_the_shared_morphology_rule_is_rejected(self) -> None:
        planted = self.live["controls_text"].replace(
            "if !connection_page_chrome::column_chooser_mounted(projection.mode) {",
            "if projection.mode != ConnectionViewMode::Table {",
        ).replace("column_chooser_mounted", "unused_chooser_gate")
        self.assertTrue(
            any(
                "column chooser is not mounted by the shared morphology rule" in item
                for item in self.planted(controls_text=planted)
            )
        )

    def test_a_page_that_arranges_its_own_stack_is_rejected(self) -> None:
        planted = self.live["connections_text"].replace(
            "connection_page_chrome::connection_control_stack(state.shell.viewport.tier, view_mode)",
            "if state.shell.viewport.tier.is_narrow() { vec![] } else { vec![] }",
        )
        self.assertTrue(
            any(
                "decides the control-stack arrangement itself" in item
                for item in self.planted(connections_text=planted)
            )
        )

    def test_a_page_that_drops_the_shared_stack_is_rejected(self) -> None:
        planted = self.live["connections_text"].replace(
            "connection_page_chrome::connection_control_stack(state.shell.viewport.tier, view_mode)",
            "connection_control_stack_local(state.shell.viewport.tier, view_mode)",
        )
        self.assertTrue(
            any(
                "does not consume the shared control stack" in item
                for item in self.planted(connections_text=planted)
            )
        )

    def test_a_surface_that_owns_a_mount_rule_is_rejected(self) -> None:
        planted = self.live["controls_text"].replace(
            "use infiltrator_presentation::connection_page_chrome::{self, ConnectionChooserPlacement};",
            "use infiltrator_presentation::connection_page_chrome::{self, ConnectionChooserPlacement};\n"
            "const fn column_chooser_mounted(mode: ConnectionViewMode) -> bool {\n"
            "    matches!(mode, ConnectionViewMode::Table)\n"
            "}",
        )
        self.assertTrue(
            any(
                "owns a morphology mount rule instead of the shared owner" in item
                for item in self.planted(controls_text=planted)
            )
        )

    def test_an_owner_that_drops_a_mount_rule_is_rejected(self) -> None:
        planted = self.live["owner_text"].replace(
            "pub const fn column_chooser_mounted(mode: ConnectionViewMode) -> bool {",
            "pub const fn chooser_gate(mode: ConnectionViewMode) -> bool {",
        )
        self.assertTrue(
            any(
                "no longer publishes the morphology mount rules" in item
                for item in self.planted(owner_text=planted)
            )
        )

    def test_an_owner_that_drops_the_stack_is_rejected(self) -> None:
        planted = self.live["owner_text"].replace(
            "pub fn connection_control_stack(",
            "pub fn control_stack(",
        )
        self.assertTrue(
            any(
                "no longer publishes the connections control stack" in item
                for item in self.planted(owner_text=planted)
            )
        )

    def test_a_page_that_drops_the_shared_chrome_is_rejected(self) -> None:
        planted = self.live["connections_text"].replace(
            "connection_page_chrome::connection_page_chrome(state.shell.viewport.tier)",
            "connection_page_chrome::ConnectionPageChrome::wide()",
        )
        self.assertTrue(
            any(
                "does not consume the shared page chrome" in item
                for item in self.planted(connections_text=planted)
            )
        )

    def test_a_chooser_that_ignores_the_placement_is_rejected(self) -> None:
        planted = self.live["connections_text"].replace(
            "column_view_controls::column_chooser_controls(state, chrome.chooser)",
            "column_view_controls::column_chooser_controls(state)",
        ).replace(
            "connection_view_controls::column_chooser_controls(state, chrome.chooser)",
            "connection_view_controls::column_chooser_controls(state)",
        )
        self.assertTrue(
            any(
                "column chooser ignores the shared placement rule" in item
                for item in self.planted(connections_text=planted)
            )
        )

    def test_an_owner_that_drops_a_placement_is_rejected(self) -> None:
        planted = self.live["owner_text"].replace(
            "pub enum ConnectionTrafficPlacement {",
            "pub enum TrafficPlacement {",
        )
        self.assertTrue(
            any(
                "no longer publishes the page chrome placements" in item
                for item in self.planted(owner_text=planted)
            )
        )


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

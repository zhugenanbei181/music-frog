#!/usr/bin/env python3
"""Keep Iced observation pages and their receipts on one typed route truth.

ClashParty ships connections and logs as independent destinations. Iced used
to fold both into `Route::Runtime`, which made three sidebar rows share one
selected state and made business captures name a section of an unrelated page.
This bounded lexical reader holds the two halves of the correction together:
the typed route mapping and the generated capture carrier.
"""

from __future__ import annotations

import argparse
import pathlib
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
APP_TYPES = ROOT / "crates/infiltrator-iced/src/types/app.rs"
DEMO = ROOT / "crates/infiltrator-iced/src/demo.rs"
CAPTURE_SCRIPT = ROOT / "scripts/capture-iced-matrix.sh"
MATRIX = ROOT / "scripts/capture_iced_scenarios.tsv"
SIDEBAR = ROOT / "crates/infiltrator-iced/src/view/sidebar.rs"
CAPTURE_FRAME = ROOT / "crates/infiltrator-iced/src/capture_frame.rs"
DEMO_STATE = ROOT / "crates/infiltrator-iced/src/demo/state.rs"
CAPTURE_GEOMETRY = ROOT / "crates/infiltrator-iced/src/capture_geometry.rs"
OBSERVATION_PAGES = ROOT / "crates/infiltrator-iced/src/view/observation_pages.rs"
OBSERVATION_LAYOUT = ROOT / "crates/infiltrator-iced/src/view/observation_layout.rs"
CONNECTIONS = ROOT / "crates/infiltrator-iced/src/view/runtime/connections.rs"
LOGS = ROOT / "crates/infiltrator-iced/src/view/runtime/logs.rs"
CONNECTION_SEARCH = ROOT / "crates/infiltrator-iced/src/view/runtime/connection_search.rs"

BUSINESS_PREFIXES = (
    "connections-close-all-confirm",
    "connections-details-drawer",
    "connections-grouping",
    "connections-search-highlight",
    "logs-redacted-export",
    "logs-scroll-lock",
    "logs-search-highlight",
)


def masked(text: str) -> str:
    return "\n".join(line.split("//", 1)[0] for line in text.splitlines())


def findings(
    app: str,
    demo: str,
    capture: str,
    matrix: str,
    sidebar: str,
    capture_frame: str,
    demo_state: str,
    capture_geometry: str,
    observation_pages: str,
    observation_layout: str,
    connections: str,
    logs: str,
    connection_search: str,
) -> list[str]:
    app, demo, capture, sidebar, capture_frame, demo_state, capture_geometry, observation_pages, observation_layout, connections, logs, connection_search = map(
        masked,
        (
            app,
            demo,
            capture,
            sidebar,
            capture_frame,
            demo_state,
            capture_geometry,
            observation_pages,
            observation_layout,
            connections,
            logs,
            connection_search,
        ),
    )
    found: list[str] = []
    if "ShellPage::Connections | ShellPage::Logs => Self::Runtime" in app:
        found.append("app.rs: connections/logs are folded back into the runtime route")
    for page in ("Connections", "Logs"):
        if f"ShellPage::{page} => Self::{page}" not in app:
            found.append(f"app.rs: ShellPage::{page} lost its own typed route")
        if f'"{page.lower()}" => Some((Route::{page}, CaptureSection::RouteTop))' not in demo:
            found.append(f"demo.rs: the {page.lower()} page is not its own route top")
        if f'Route::{page} => "{page.lower()}"' not in demo:
            found.append(f"demo.rs: Route::{page} lost its inverse env name")
    if '[ "$marker_page" = "connections" ] && marker_page="runtime"' in capture:
        found.append("capture-iced-matrix.sh: connections receipts are aliased back to runtime")
    if '[ "$marker_page" = "logs" ] && marker_page="runtime"' in capture:
        found.append("capture-iced-matrix.sh: logs receipts are aliased back to runtime")
    if "selected_navigation_destination_count" not in sidebar:
        found.append("sidebar.rs: selected destinations are not counted from the mounted registry")
    if (
        "selected_navigation_destination_count(self.shell.current_route) == 1" not in capture_frame
        or "self.shell.current_route.observes_runtime()" not in capture_frame
    ):
        found.append("capture_frame.rs: stable runtime readbacks do not require one selected navigation row")
    if "demo_outbound_surface(CoreLifecycle::Running)" not in demo_state:
        found.append("demo/state.rs: all routes do not share the published running lifecycle fixture")
    if "if env.page.observes_runtime()" in demo_state:
        found.append("demo/state.rs: Core lifecycle is still derived from the UI route")
    if "self.shell.current_route == Route::Connections" not in capture_geometry:
        found.append("capture_geometry.rs: standalone connection captures still reveal the runtime page scroll")
    if "CONNECTIONS_PAGE_SCROLL_ID" not in capture_geometry:
        found.append("capture_geometry.rs: the standalone connection scroll has no typed identity")
    if 'FeatureId::ConnectionsSearchHighlight => highlight_id("group-0")' not in capture_geometry:
        found.append("capture_geometry.rs: connection search captures do not measure the visible highlight")
    if 'probe.revealing(Id::new("runtime-scroll"))' in capture_geometry and (
        "FeatureId::LogsScrollLock" in capture_geometry or "FeatureId::LogsSearchHighlight" in capture_geometry
    ):
        found.append("capture_geometry.rs: standalone log captures still reveal the runtime page scroll")
    if "logs_page_section" not in observation_pages:
        found.append("observation_pages.rs: the logs page does not own a page-body section")
    if "ObservationListOwner::PageBody" not in logs:
        found.append("logs.rs: the standalone logs list does not fill the page body")
    if "background: Some(tokens(t).control_bg.into())" in logs:
        found.append("logs.rs: log results still paint a near-ground surface that creates a false dead band")
    if (
        "connection_rows_use_stacked_headline(state)" not in connections
        or "pub fn connection_rows_use_single_line_text(state: &AppState)"
        not in observation_layout
    ):
        found.append("connections.rs: narrow connection rows have no typed headline owner")
    if "ConnectionListScrollOwner::ConnectionsPage => {\n                        grouped_scroll.id(CONNECTIONS_PAGE_SCROLL_ID)" not in connections:
        found.append("connections.rs: grouped standalone lists do not publish the page scroll owner")
    if "connection_rows_use_single_line_text(state)" not in connection_search:
        found.append("connection_search.rs: narrow connection text has no typed wrapping owner")
    if "ObservationListOwner" not in observation_layout:
        found.append("observation_layout.rs: list height has no typed owner")

    rows = [line.split("\t") for line in matrix.splitlines()[1:] if line.strip()]
    by_name = {cells[0]: cells for cells in rows}
    for name, cells in by_name.items():
        if not name.startswith(BUSINESS_PREFIXES):
            continue
        expected = "connections" if name.startswith("connections-") else "logs"
        if cells[1] != expected:
            found.append(f"matrix: {name} names page {cells[1]!r}, expected {expected!r}")
    for name, expected in (("connections-page-compact", "connections"), ("logs-page-compact", "logs")):
        cells = by_name.get(name)
        if cells is None or cells[1] != expected or cells[4] != "720x480" or cells[2] != expected:
            found.append(f"matrix: missing compact page-only receipt {name}")
    return found


def load() -> tuple[str, str, str, str, str, str, str, str, str, str, str, str, str]:
    return (
        APP_TYPES.read_text(encoding="utf-8"),
        DEMO.read_text(encoding="utf-8"),
        CAPTURE_SCRIPT.read_text(encoding="utf-8"),
        MATRIX.read_text(encoding="utf-8"),
        SIDEBAR.read_text(encoding="utf-8"),
        CAPTURE_FRAME.read_text(encoding="utf-8"),
        DEMO_STATE.read_text(encoding="utf-8"),
        CAPTURE_GEOMETRY.read_text(encoding="utf-8"),
        OBSERVATION_PAGES.read_text(encoding="utf-8"),
        OBSERVATION_LAYOUT.read_text(encoding="utf-8"),
        CONNECTIONS.read_text(encoding="utf-8"),
        LOGS.read_text(encoding="utf-8"),
        CONNECTION_SEARCH.read_text(encoding="utf-8"),
    )


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        (
            self.app,
            self.demo,
            self.capture,
            self.matrix,
            self.sidebar,
            self.capture_frame,
            self.demo_state,
            self.capture_geometry,
            self.observation_pages,
            self.observation_layout,
            self.connections,
            self.logs,
            self.connection_search,
        ) = load()

    def test_current_sources_have_no_findings(self) -> None:
        self.assertEqual(
            findings(
                self.app,
                self.demo,
                self.capture,
                self.matrix,
                self.sidebar,
                self.capture_frame,
                self.demo_state,
                self.capture_geometry,
                self.observation_pages,
                self.observation_layout,
                self.connections,
                self.logs,
                self.connection_search,
            ),
            [],
        )

    def test_folded_mapping_is_rejected(self) -> None:
        app = self.app.replace(
            "ShellPage::Connections => Self::Connections,\n            ShellPage::Logs => Self::Logs,",
            "ShellPage::Connections | ShellPage::Logs => Self::Runtime,",
        )
        self.assertTrue(any("folded back" in problem for problem in self.findings(app=app)))

    def test_old_capture_aliases_are_rejected(self) -> None:
        capture = self.capture.replace(
            '  local marker_page="$page"\n',
            '  local marker_page="$page"\n  [ "$marker_page" = "connections" ] && marker_page="runtime"\n',
        )
        self.assertTrue(any("aliased back" in problem for problem in self.findings(capture=capture)))

    def test_old_matrix_carrier_is_rejected(self) -> None:
        matrix = self.matrix.replace(
            "connections-search-highlight-standard\tconnections\t",
            "connections-search-highlight-standard\truntime\t",
        )
        self.assertTrue(any("matrix:" in problem for problem in self.findings(matrix=matrix)))

    def test_missing_page_only_compact_receipt_is_rejected(self) -> None:
        matrix = "\n".join(
            line for line in self.matrix.splitlines()
            if not line.startswith("connections-page-compact\t")
        )
        self.assertTrue(any("page-only receipt" in problem for problem in self.findings(matrix=matrix)))

    def test_missing_repeated_readback_selection_gate_is_rejected(self) -> None:
        capture_frame = self.capture_frame.replace(
            "|| selected_navigation_destination_count(self.shell.current_route) == 1",
            "|| selected_navigation_destination_count(self.shell.current_route) == 0",
        )
        self.assertTrue(
            any("one selected navigation row" in problem for problem in self.findings(capture_frame=capture_frame))
        )

    def test_route_derived_demo_lifecycle_is_rejected(self) -> None:
        demo_state = self.demo_state.replace(
            "demo_outbound_surface(CoreLifecycle::Running)",
            "demo_outbound_surface(if env.page.observes_runtime() { CoreLifecycle::Running } else { CoreLifecycle::Stopped })",
        )
        self.assertTrue(any("derived from the UI route" in problem for problem in self.findings(demo_state=demo_state)))

    def test_runtime_scroll_probe_for_standalone_pages_is_rejected(self) -> None:
        capture_geometry = self.capture_geometry.replace(
            "self.shell.current_route == Route::Connections",
            "self.shell.current_route == Route::Runtime",
        )
        self.assertTrue(any("standalone connection captures" in problem for problem in self.findings(capture_geometry=capture_geometry)))

    def findings(self, **overrides):
        sources = dict(
            app=self.app,
            demo=self.demo,
            capture=self.capture,
            matrix=self.matrix,
            sidebar=self.sidebar,
            capture_frame=self.capture_frame,
            demo_state=self.demo_state,
            capture_geometry=self.capture_geometry,
            observation_pages=self.observation_pages,
            observation_layout=self.observation_layout,
            connections=self.connections,
            logs=self.logs,
            connection_search=self.connection_search,
        )
        sources.update(overrides)
        return findings(**sources)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--mode", choices=("report", "enforce"), default="enforce")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    found = findings(*load())
    for problem in found:
        print(problem, file=sys.stderr)
    print(f"iced-observation-routes: {len(found)} finding(s)")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    raise SystemExit(main())

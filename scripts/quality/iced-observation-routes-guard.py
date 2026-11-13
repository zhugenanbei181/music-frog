#!/usr/bin/env python3
"""Keep Iced observation pages and their receipts on one typed route truth.

ClashParty ships connections and logs as independent destinations. Iced used
to fold both into `Route::Runtime`, which made three sidebar rows share one
selected state and made business captures name a section of an unrelated page.
This bounded lexical reader holds route ownership, selected-row projection,
capture readiness, and the generated page carrier together.
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
NAVIGATION = ROOT / "crates/infiltrator-contract/src/shell_navigation.rs"

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
    navigation: str,
) -> list[str]:
    app, demo, capture, sidebar, capture_frame, demo_state, capture_geometry, observation_pages, observation_layout, connections, logs, connection_search, navigation = map(
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
            navigation,
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
    if "    Traffic," not in app or "Self::Traffic => \"Traffic\"" not in app:
        found.append("app.rs: traffic has no standalone typed route identity")
    if (
        "Self::Runtime => \"Runtime\"" not in app
        or "destination_for_route(route)" not in app
    ):
        found.append("app.rs: runtime route ownership is not resolved through the contract")
    if (
        '"Settings" | "Network" | "Runtime" | "AppSettings"' not in navigation
        or '"Traffic" | "RuntimeWorkbench" => NavigationTarget::SurfaceLocal("traffic")'
        not in navigation
    ):
        found.append("shell_navigation.rs: Runtime and Traffic do not have distinct owners")
    if '"traffic" => Some((Route::Traffic, CaptureSection::RouteTop))' not in demo:
        found.append("demo.rs: traffic is not the standalone route top")
    if (
        '"traffic-audit" | "traffic_audit" => Some((Route::Traffic, CaptureSection::TrafficAudit))'
        not in demo
    ):
        found.append("demo.rs: traffic audit captures do not anchor their named section")
    if 'Route::Traffic => "traffic"' not in demo:
        found.append("demo.rs: Route::Traffic lost its inverse capture name")
    if '[ "$marker_page" = "connections" ] && marker_page="runtime"' in capture:
        found.append("capture-iced-matrix.sh: connections receipts are aliased back to runtime")
    if '[ "$marker_page" = "logs" ] && marker_page="runtime"' in capture:
        found.append("capture-iced-matrix.sh: logs receipts are aliased back to runtime")
    if (
        "PUBLISH_CORPUS=0" not in capture
        or 'if [ "$MATRIX" != "$MATRIX_SRC" ]; then' not in capture
        or 'if [ "$PUBLISH_CORPUS" -eq 0 ]; then' not in capture
        or 'sweep_unindexed_pixels "$OUT_DIR" "$MANIFEST"' not in capture
    ):
        found.append(
            "capture-iced-matrix.sh: a filtered capture can overwrite the published corpus"
        )
    if "selected_navigation_destination_count" not in sidebar:
        found.append("sidebar.rs: selected destinations are not counted from the mounted registry")
    if (
        "route_for_destination(route.navigation_target())" not in sidebar
        or "route_for_destination(item.target) == selected_route" not in sidebar
    ):
        found.append("sidebar.rs: rendered selection does not use the contract-owned route target")
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
    for name in ("traffic-standard", "traffic-compact"):
        cells = by_name.get(name)
        if cells is None or cells[1] != "traffic" or cells[2] != "traffic":
            found.append(f"matrix: {name} must publish the standalone traffic page top")
    for name in ("traffic-audit-standard", "traffic-audit-compact"):
        cells = by_name.get(name)
        if cells is None or cells[1] != "traffic-audit" or cells[2] != "traffic":
            found.append(f"matrix: {name} must anchor the traffic audit under the traffic shape")
    if '[ "$marker_page" = "traffic-audit" ] && marker_page="traffic"' not in capture:
        found.append("capture-iced-matrix.sh: traffic audit section captures do not publish the traffic route")
    if '[ "$marker_page" = "traffic-audit" ] && marker_page="runtime"' in capture:
        found.append("capture-iced-matrix.sh: traffic receipts are aliased back to runtime")
    return found


def load() -> tuple[str, str, str, str, str, str, str, str, str, str, str, str, str, str]:
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
        NAVIGATION.read_text(encoding="utf-8"),
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
            self.navigation,
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
                self.navigation,
            ),
            [],
        )

    def test_folded_mapping_is_rejected(self) -> None:
        app = self.app.replace(
            "ShellPage::Connections => Self::Connections,\n            ShellPage::Logs => Self::Logs,",
            "ShellPage::Connections | ShellPage::Logs => Self::Runtime,",
        )
        self.assertTrue(any("folded back" in problem for problem in self.findings(app=app)))

    def test_traffic_route_must_remain_standalone(self) -> None:
        app = self.app.replace("    Traffic,\n", "", 1)
        self.assertTrue(any("standalone typed route" in problem for problem in self.findings(app=app)))

    def test_runtime_and_traffic_contract_owners_must_remain_distinct(self) -> None:
        navigation = self.navigation.replace(
            '"Settings" | "Network" | "Runtime" | "AppSettings"',
            '"Settings" | "Network" | "AppSettings"',
        ).replace(
            '"Traffic" | "RuntimeWorkbench" => NavigationTarget::SurfaceLocal("traffic")',
            '"Traffic" | "Runtime" | "RuntimeWorkbench" => NavigationTarget::SurfaceLocal("traffic")',
        )
        self.assertTrue(
            any("distinct owners" in problem for problem in self.findings(navigation=navigation))
        )

    def test_selected_row_must_be_the_contract_resolved_route(self) -> None:
        sidebar = self.sidebar.replace(
            "route_for_destination(route.navigation_target())",
            "route",
        )
        self.assertTrue(
            any("contract-owned route target" in problem for problem in self.findings(sidebar=sidebar))
        )

    def test_traffic_receipts_must_use_the_traffic_shape(self) -> None:
        matrix = self.matrix.replace(
            "traffic-audit-standard\ttraffic-audit\ttraffic",
            "traffic-audit-standard\ttraffic-audit\truntime",
        )
        self.assertTrue(any("traffic shape" in problem for problem in self.findings(matrix=matrix)))

    def test_standalone_traffic_page_requires_top_and_audit_receipts(self) -> None:
        matrix = "\n".join(
            line for line in self.matrix.splitlines()
            if not line.startswith("traffic-compact\t")
        )
        self.assertTrue(any("standalone traffic page top" in problem for problem in self.findings(matrix=matrix)))

    def test_old_capture_aliases_are_rejected(self) -> None:
        capture = self.capture.replace(
            '  local marker_page="$page"\n',
            '  local marker_page="$page"\n  [ "$marker_page" = "connections" ] && marker_page="runtime"\n',
        )
        self.assertTrue(any("aliased back" in problem for problem in self.findings(capture=capture)))

    def test_a_filtered_capture_must_not_publish(self) -> None:
        capture = self.capture.replace("PUBLISH_CORPUS=0", "")
        self.assertTrue(
            any("overwrite the published corpus" in problem
                for problem in self.findings(capture=capture))
        )

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
            navigation=self.navigation,
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

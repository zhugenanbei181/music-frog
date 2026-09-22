#!/usr/bin/env python3
"""Fail-closed guard for the group 13 connections ledger (DUAL-13-01..15).

Group 13's closure standard is the same as group 06: one shared reduction,
both surfaces, dual headless tests, and an honest ledger. This guard asserts
the per-item ledger rows exist, that the shared domain reductions and both
surface wirings are present, and — critically — that neither surface has
re-introduced the fabricated DNS/TCP/TLS/TTFB timing waterfall (DUAL-13-04 is
typed unsupported because the core exposes no such stages).

DUAL-13-10/12 add the second hard rule: the per-connection instantaneous rate
is the shared `connection_rate` derivation over successive cumulative-counter
snapshots, published into the connections read model by the application layer.
The guard forbids the previous hardcoded `upload_bps: 0.0` in the surface
reader so a fabricated (or absent) rate cannot silently return.

DUAL-13-14 adds the drawer parity rule: the contract carries every host-backed
field both drawers render (endpoints, transport, rule payload) and the Bevy
drawer owns the same teardown action the Iced drawer submits.
"""

from __future__ import annotations

import argparse
import pathlib
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]

LEDGER = "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md"


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def require(violations: list[str], path: str, *markers: str) -> None:
    text = read(path)
    compact_text = " ".join(text.split())
    for marker in markers:
        compact_marker = " ".join(marker.split())
        rustfmt_marker = compact_marker.replace(" }", ", }")
        if (
            marker not in text
            and compact_marker not in compact_text
            and rustfmt_marker not in compact_text
        ):
            violations.append(f"{path} missing {marker!r}")


def forbid(violations: list[str], path: str, *markers: str) -> None:
    text = read(path)
    for marker in markers:
        if marker in text:
            violations.append(f"{path} still contains forbidden marker {marker!r}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    violations: list[str] = []

    # The full per-item ledger must exist for all fifteen items.
    require(
        violations,
        LEDGER,
        "DUAL-13-01",
        "DUAL-13-02",
        "DUAL-13-03",
        "DUAL-13-04",
        "DUAL-13-05",
        "DUAL-13-06",
        "DUAL-13-07",
        "DUAL-13-08",
        "DUAL-13-09",
        "DUAL-13-10",
        "DUAL-13-11",
        "DUAL-13-12",
        "DUAL-13-13",
        "DUAL-13-14",
        "DUAL-13-15",
        "parity-ready",
    )
    # Closed items carry their distinctive evidence tokens in the ledger.
    require(
        violations,
        LEDGER,
        "aggregate_connections",
        "CloseFilteredConnections",
        "test_connections_close_all_requires_confirmation",
        "test_connections_search_hides_non_matching_rows",
        "test_shared_connection_view_reductions_are_delegated",
        "typed unsupported",
    )

    # Shared reduction lives in the domain crate.
    require(
        violations,
        "crates/infiltrator-domain/src/lib.rs",
        "pub mod connection_view;",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/connection_view.rs",
        "pub enum ConnectionGroupingMode",
        "pub enum ConnectionSortKey",
        "pub struct ConnectionAggregate",
        "pub trait ConnectionView",
        "pub fn matches_search",
        "pub fn sort_connections",
        "pub fn aggregate_connections",
        "pub fn quick_rule_spec",
        "impl ConnectionView for Connection",
    )

    # Iced consumes the shared reductions and wires range teardown.
    require(
        violations,
        "crates/infiltrator-iced/src/view/runtime/connections.rs",
        "connection_view::aggregate_connections",
        "connection_view::matches_search",
        "ConnectionSortKey::from_identifier",
        "Message::CloseFilteredConnections",
        "conn_close_filtered_btn",
        "conn_aggregate_count",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/monitoring.rs",
        "Message::CloseFilteredConnections",
        "connection_view::matches_search",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view_root/connection_drawer.rs",
        "connection_view::quick_rule_spec",
        "conn_drawer_timing_unsupported",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_ext.rs",
        "conn_aggregate_count",
        "conn_close_filtered_btn",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en_ext.rs",
        "conn_aggregate_count",
        "conn_close_filtered_btn",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table.rs",
        "conn_drawer_timing_unsupported",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en.rs",
        "conn_drawer_timing_unsupported",
    )
    # DUAL-13-04: the fabricated waterfall must not come back.
    forbid(
        violations,
        "crates/infiltrator-iced/src/view_root/connection_drawer.rs",
        "waterfall_row",
        "dns_ms",
    )

    # Bevy consumes the same reductions and owns the two-step close-all.
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/connections_view.rs",
        "impl ConnectionView for ConnectionItem",
        "fn sync_connections_search",
        "fn aggregation_summary",
        "fn search_field_text",
        "pub struct ConnSearchField",
        "pub struct CloseFilteredConnectionsButton",
        "pub struct ConnectionRow",
        "pub struct ConnectionsViewState",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/connections.rs",
        "ConnAggregationPill(pub ConnectionGroupingMode)",
        "CloseAllConnectionsLabel",
        "on_connections_view_activated",
        "state.armed = true",
        "connection_view::matches_search",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages.rs",
        "pub mod connections_view;",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/connections_drawer.rs",
        "耗时明细",
    )
    forbid(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/connections_drawer.rs",
        "waterfall_bars",
        "18 ms",
    )

    require(
        violations,
        "crates/infiltrator-contract/src/lib.rs",
        "pub mod connection;",
    )

    # DUAL-13-01: one shared stream-phase vocabulary, mapped by both surfaces.
    require(
        violations,
        "crates/infiltrator-contract/src/connection.rs",
        "pub enum ConnectionStreamPhase",
        "pub fn from_page_status",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/runtime.rs",
        "fn shared_phase",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "ConnectionStreamPhase::from_page_status",
        "stream_phase,",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/connections.rs",
        "ConnectionsLineKind::Stream",
        "stream_phase_label",
    )

    # DUAL-13-06: shared parsed route chain rendered hop-by-hop on both ends.
    require(
        violations,
        "crates/infiltrator-domain/src/connection_view.rs",
        "pub struct RouteChain",
        "pub fn route_chain",
        "fn view_chain",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub chains: Vec<String>",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/runtime/connections.rs",
        "connection_view::route_chain",
        "route_chain.hops()",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/connections.rs",
        "ConnChainHopText",
        "connection_view::route_chain",
        "connection_chain_scenes",
    )

    # DUAL-13-03: Bevy detail is a right-docked drawer from the shared widget.
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/connections_drawer.rs",
        "infiltrator_bevy_widgets::drawer::",
        "DrawerPlacement::Right",
        "drawer_scene",
        "ConnInspectButton",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/connections.rs",
        "ConnInspectButton",
        "on_connections_drawer_activated",
    )

    # DUAL-13-09: one shared reverse-rule draft seam; Bevy is wired.
    require(
        violations,
        "crates/infiltrator-domain/src/connection_view.rs",
        "pub fn draft_rule_entry",
        "pub fn append_draft_rule",
        "pub fn bare_host",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/ui.rs",
        "append_draft_rule",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/connections_drawer.rs",
        "quick_rule_spec",
        "append_draft_rule",
        "ConnectionsRuleDraft",
        "DrawerAddRuleButton",
    )

    # DUAL-13-11: shared activity tracker + application sweep + dual controls.
    require(
        violations,
        "crates/infiltrator-domain/src/lib.rs",
        "pub mod connection_activity;",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/connection_activity.rs",
        "pub struct ConnectionActivityTracker",
        "pub fn observe",
        "pub fn idle_ids",
        "DEFAULT_IDLE_TIMEOUT_SECS",
        "IDLE_TIMEOUT_CHOICES",
    )
    require(
        violations,
        "crates/infiltrator-application/src/connection_application.rs",
        "pub struct IdleSweepReport",
        "pub async fn sweep_idle",
        "idle_connections",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/runtime/connections.rs",
        "connection_activity::IDLE_TIMEOUT_CHOICES",
        "Message::SweepIdleConnections",
        "conn_idle_sweep_btn",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/connections_idle.rs",
        "pub struct ConnectionsIdleState",
        "on_connections_idle_activated",
        "ConnIdleSweepButton",
        "ConnIdleTimeoutPill",
        "idle_status_label",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages.rs",
        "pub mod connections_idle;",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_ext.rs",
        "conn_idle_sweep_btn",
        "conn_idle_timeout_label",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en_ext.rs",
        "conn_idle_sweep_btn",
        "conn_idle_timeout_label",
    )

    # DUAL-13-10/12: one shared instantaneous-rate derivation, published in the
    # read model and consumed by both surfaces.
    require(
        violations,
        "crates/infiltrator-domain/src/lib.rs",
        "pub mod connection_rate;",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/connection_rate.rs",
        "pub const HIGH_THROUGHPUT_THRESHOLD_BPS",
        "pub const PULSE_MIN_INTENSITY",
        "pub const PULSE_BREATH_HZ",
        "pub struct ConnectionRate",
        "pub struct ConnectionRates",
        "pub struct ConnectionRateDiffer",
        "pub fn instantaneous_rate",
        "pub fn is_high_throughput",
        "pub fn pulse_intensity",
        "pub fn observe",
        "pub fn has_high_throughput",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/connection_view.rs",
        "pub struct RatedConnection",
        "ConnectionSortKey::DownloadRateDesc",
        "ConnectionSortKey::UploadRateDesc",
        "fn view_upload_rate_bps",
        "fn view_download_rate_bps",
    )
    require(
        violations,
        "crates/infiltrator-application/src/connection_rate_application.rs",
        "pub struct ConnectionRateApplication",
        "pub fn observe_at",
        "pub fn connections_page_snapshot",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "connection_rate_application::ConnectionRateApplication",
        "connection_rates",
        "connections_page_snapshot",
    )
    # The hardcoded zero rate this group removed must not come back.
    forbid(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "upload_bps: 0.0",
        "download_bps: 0.0",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/monitoring.rs",
        "pub(crate) fn apply_connections_snapshot",
        "observe_at",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/runtime/connections.rs",
        "pub fn sort_rated_connections",
        "pub fn connection_pulse_intensity",
        "connection_rate::pulse_intensity",
        "RatedConnection",
        "conn_pulse_high_throughput",
        "runtime_conn_sort_download_rate",
        "runtime_conn_sort_upload_rate",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view_root/connection_drawer.rs",
        "conn_drawer_upload_speed",
        "conn_drawer_download_speed",
        "conn_drawer_rate_pending",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/subscription.rs",
        "connection_pulse_active",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/ui.rs",
        "connection_rate::PULSE_BREATH_HZ",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table.rs",
        "runtime_conn_sort_download_rate",
        "runtime_conn_sort_upload_rate",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en.rs",
        "runtime_conn_sort_download_rate",
        "runtime_conn_sort_upload_rate",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_ext.rs",
        "conn_pulse_high_throughput",
        "conn_drawer_rate_pending",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en_ext.rs",
        "conn_pulse_high_throughput",
        "conn_drawer_rate_pending",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/connections_view.rs",
        "pub struct ConnSortPill",
        "pub fn sort_pills_scene",
        "pub(crate) fn on_connections_sort_activated",
        "pub(crate) fn apply_connection_row_order",
        "fn view_upload_rate_bps",
        "fn view_download_rate_bps",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/connections_pulse.rs",
        "pub struct ConnHighThroughputPulse",
        "pub struct ConnectionsPulseState",
        "pub fn connection_pulse_scene",
        "pub(crate) fn animate_connection_pulses",
        "connection_rate::PULSE_BREATH_HZ",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages.rs",
        "pub mod connections_demo;",
        "pub mod connections_pulse;",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/connections.rs",
        "on_connections_sort_activated",
        "connection_pulse_scene",
        "sort_pills_scene",
        "ConnSortPill",
        "apply_connection_row_order",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/route.rs",
        "ConnectionsPulseState",
        "animate_connection_pulses",
    )
    # DUAL-13-14: the drawer bodies carry the same host-backed data and the
    # same teardown action on both surfaces.
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/connections_drawer.rs",
        "DrawerCloseConnectionButton",
        "ConnDrawerFieldKind::Endpoints",
        "ConnDrawerFieldKind::Network",
        "ConnDrawerFieldKind::Rate",
        "ConnDrawerFieldKind::RulePayload",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub rule_payload: String",
        "pub network: String",
        "pub source_ip: String",
        "pub destination_ip: String",
    )

    # Dual headless evidence.
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/view_runtime_connections_tests.rs",
        "test_shared_connection_view_reductions_are_delegated",
        "test_stream_state_maps_to_shared_phase",
        "test_route_chain_hops_parse_through_shared_model",
        "test_high_throughput_pulse_uses_the_shared_threshold",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/app_state_tests.rs",
        "connection_idle_timeout_and_activity_tracking",
        "connection_instantaneous_rates_derive_from_successive_snapshots",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_connections_aggregation_pill_switches_shared_mode",
        "test_connections_search_hides_non_matching_rows",
        "test_connections_close_all_requires_confirmation",
        "test_connections_close_filtered_submits_matching_only",
        "test_connections_stream_badge_reflects_shared_phase",
        "test_connections_route_chain_renders_each_hop",
        "test_connections_inspect_opens_shared_drawer",
        "test_connections_add_rule_draft_uses_shared_seam",
        "test_connections_idle_sweep_submits_and_reports",
        "test_connections_idle_timeout_pill_switches_shared_choice",
        "test_connections_high_throughput_pulse_follows_shared_threshold",
        "test_connections_sort_pills_reorder_rows_by_instantaneous_rate",
        "test_connections_drawer_parity_exposes_shared_fields_and_close_action",
    )
    require(
        violations,
        "crates/infiltrator-application/src/connection_rate_application_test.rs",
        "successive_observations_publish_real_bytes_per_second",
        "the_shared_read_model_carries_the_derived_rates",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/connection_rate.rs",
        "second_observation_divides_the_delta_by_the_measured_interval",
        "first_observation_never_fabricates_a_rate",
        "threshold_and_pulse_only_react_to_real_high_throughput",
    )

    # The guard itself is registered on both suites.
    require(
        violations,
        "scripts/test.sh",
        "connections-audit-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        'connections-audit-guard.py" --mode enforce',
    )

    if violations:
        for violation in violations:
            print(f"connections-audit-guard: {violation}", file=sys.stderr)
        print(
            f"connections-audit-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("connections-audit-guard: DUAL-13 ledger and dual-surface markers=complete")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

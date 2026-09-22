#!/usr/bin/env python3
"""Fail-closed guard for the DUAL-08 multi-source aggregator ledger.

Group 08 (multi-subscription aggregation, node cleaning, automatic topology
generation) must keep an honest per-item ledger (`DUAL-08-01..15`) and the
shared aggregation path must stay single-sourced:

* one domain implementation (`ProfileAggregator::plan`) behind both the
  structured report and the legacy string aggregate; region clustering and
  group synthesis may not be duplicated in `profile_converter.rs`;
* one shared application (`ProfileAggregationApplication`) turns a draft into
  the report, publishes it process-wide, and is the only writer of a new
  aggregated profile;
* the contract owns the serializable draft/report/cluster/group read model and
  the two command intents; the surface reader projects the report;
* both surfaces (Iced + Bevy) render only the shared report -- the Bevy card
  may not keep the hard-coded regional demo list, and the Iced modal may not
  keep the fabricated `Merged N profiles` summary string;
* the guard is registered in both test entrypoints.

Modeled on `subscription-lifecycle-guard.py`.
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

    # 1. The per-item ledger covers all 15 rows.
    plan = read(LEDGER)
    for index in range(1, 16):
        row = f"DUAL-08-{index:02d}"
        if row not in plan:
            violations.append(f"{LEDGER} missing ledger row {row}")
    if "### 组 08 逐项账目" not in plan:
        violations.append(f"{LEDGER} missing the group 08 per-item ledger section")
    if "| 组 08 多源聚合器与自动拓扑 | 15 | `planned`" in plan:
        violations.append(
            f"{LEDGER} group 08 row is still an unexpanded `planned` group row"
        )
    require(
        violations,
        LEDGER,
        "`in-progress (8/15)`",
        "aggregator-guard.py",
        "聚合生成结果可视化预览",
    )

    # 2. The domain aggregator is the single implementation.
    require(
        violations,
        "crates/infiltrator-domain/src/profile_aggregator.rs",
        "pub struct ProfileAggregator",
        "pub fn plan(",
        "pub fn pipeline(",
        "pub fn cluster_regions(",
        "pub fn synthesize_groups(",
        "pub const MASTER_SELECT_GROUP",
        "pub fn region_label(",
        "pub struct AggregationPlan",
        "pub struct RegionalCluster",
        "pub struct GeneratedGroup",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/profile_converter.rs",
        "ProfileAggregator::plan(sources, options)?.yaml",
    )
    forbid(
        violations,
        "crates/infiltrator-domain/src/profile_converter.rs",
        "fn generate_profile_with_groups",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/lib.rs",
        "pub mod profile_aggregator;",
    )

    # 3. The shared contract + application own the report.
    require(
        violations,
        "crates/infiltrator-contract/src/aggregator.rs",
        "pub struct AggregationDraft",
        "pub struct AggregationReport",
        "pub struct RegionalClusterSnapshot",
        "pub struct GeneratedGroupSnapshot",
        "pub fn master_group(",
        "pub fn yaml_preview(",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "PreviewProfileAggregation {",
        "CreateAggregatedProfile {",
    )
    require(
        violations,
        "crates/infiltrator-application/src/profile_aggregation_application.rs",
        "pub struct ProfileAggregationApplication",
        "pub async fn preview(",
        "pub fn plan_documents(",
        "pub async fn create_profile(",
        "pub fn aggregation_options(",
        "pub fn last_aggregation_report(",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "CommandIntent::PreviewProfileAggregation { draft }",
        "CommandIntent::CreateAggregatedProfile { draft }",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "aggregation: crate::profile_aggregation_application::last_aggregation_report()",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub aggregation: Option<crate::aggregator::AggregationReport>",
    )

    # 4. Iced consumes the shared report and drops stale previews.
    require(
        violations,
        "crates/infiltrator-iced/src/update/aggregator.rs",
        "pub(crate) fn aggregator_draft(",
        "pub(super) fn update_aggregator(",
        "Message::PreviewProfileAggregation",
        "Message::AggregationPreviewFinished",
        "Message::CreateAggregatedProfile",
        "ProfileAggregationApplication::new",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "pub aggregator_report: Option<infiltrator_contract::aggregator::AggregationReport>",
        "pub aggregator_deduplicate: bool",
        "pub aggregator_geo_cluster: bool",
        "pub aggregator_generate_groups: bool",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view_root/aggregator_modal.rs",
        "Message::PreviewProfileAggregation",
        "Message::CreateAggregatedProfile",
        "aggregator_preview_nodes",
        "aggregator_preview_regions",
        "aggregator_preview_groups",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/update/aggregator.rs",
        "Merged {count} profiles into",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/update/ui.rs",
        "Merged {count} profiles into",
        "aggregator_result_summary",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/view_root/aggregator_modal.rs",
        "aggregator_result_summary",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/update/ui.rs",
        "Message::ExecuteProfileAggregation",
    )

    # 5. Bevy renders the shared report instead of the hard-coded demo list.
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/profiles_aggregator.rs",
        "pub struct AggregatorSourceToggle",
        "pub struct PreviewAggregationButton",
        "pub struct SaveAggregatedProfileButton",
        "pub fn profiles_aggregator_scene(",
        "pub fn aggregation_counters(",
        "pub fn aggregation_regions(",
        "pub fn aggregation_groups(",
        "pub(super) fn sync_aggregation_preview(",
        "pub(super) fn on_preview_aggregation(",
        "pub(super) fn on_save_aggregated_profile(",
        "UiCommand::PreviewProfileAggregation",
        "UiCommand::CreateAggregatedProfile",
    )
    forbid(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/profiles_aggregator.rs",
        "香港自动测速 (12 节点)",
        "日本自动测速 (18 节点)",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/profiles.rs",
        "crate::pages::profiles_aggregator::profiles_aggregator_scene(projection, palette)",
        "crate::pages::profiles_aggregator::sync_aggregation_preview",
        "crate::pages::profiles_aggregator::on_preview_aggregation",
        "crate::pages::profiles_aggregator::on_save_aggregated_profile",
        "pub aggregation: Option<infiltrator_contract::aggregator::AggregationReport>",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "PreviewProfileAggregation {",
        "CreateAggregatedProfile {",
        "CommandIntent::PreviewProfileAggregation",
        "CommandIntent::CreateAggregatedProfile",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "aggregation: value.aggregation.clone()",
    )

    # 6. Dual headless evidence.
    require(
        violations,
        "crates/infiltrator-application/src/profile_aggregation_application_test.rs",
        "aggregation_pipeline_regression_matrix",
        "create_profile_saves_a_new_independent_profile",
        "preview_reports_real_dedup_clusters_and_master_cascade",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/profile_aggregator_test.rs",
        "plan_reports_real_dedup_and_region_counts",
        "plan_generates_master_cascade_before_concrete_nodes",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/iced_six_advancements_wave2_tests.rs",
        "test_advancement_w2_3_multi_profile_aggregator_workflow",
        "test_advancement_w2_3b_aggregator_preview_lifecycle_is_shared",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_profiles_aggregator_previews_and_saves_through_shared_command",
    )

    # 7. Both test entrypoints register this guard.
    require(
        violations,
        "scripts/test.sh",
        "aggregator-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        'aggregator-guard.py" --mode enforce',
    )

    if violations:
        for violation in violations:
            print(f"aggregator-guard: {violation}", file=sys.stderr)
        print(f"aggregator-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("aggregator-guard: DUAL-08 ledger_rows=15 parity_items=8 violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
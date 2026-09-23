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
  aggregated profile; rename rules, the required-field precheck, custom groups
  and the template library all live in the shared layers;
* the contract owns the serializable draft/report/cluster/group read model,
  the template library and the command intents; the surface reader projects
  the report and the template library;
* both surfaces (Iced + Bevy) render only the shared report -- the Bevy card
  may not keep the hard-coded regional demo list, the Iced modal may not keep
  the fabricated `Merged N profiles` summary string, and neither surface may
  call the domain aggregator directly;
* every closed item carries dual-surface + dual-test evidence;
* the guard is registered in both test entrypoints.

Modeled on `subscription-lifecycle-guard.py`.
"""

from __future__ import annotations

import argparse
import pathlib
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]

LEDGER = "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md"

# Items closed to `parity-ready` and the tests that pin them down.
PARITY_ITEMS = 15
ITEM_STATUS = {
    "DUAL-08-01": "parity-ready",
    "DUAL-08-02": "parity-ready",
    "DUAL-08-03": "parity-ready",
    "DUAL-08-04": "parity-ready",
    "DUAL-08-05": "parity-ready",
    "DUAL-08-06": "parity-ready",
    "DUAL-08-07": "parity-ready",
    "DUAL-08-08": "parity-ready",
    "DUAL-08-09": "parity-ready",
    "DUAL-08-10": "parity-ready",
    "DUAL-08-11": "parity-ready",
    "DUAL-08-12": "parity-ready",
    "DUAL-08-13": "parity-ready",
    "DUAL-08-14": "parity-ready",
    "DUAL-08-15": "parity-ready",
}


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


def check_ledger_rows(violations: list[str]) -> None:
    """Every `DUAL-08-XX` ledger row must carry its recorded status."""
    plan = read(LEDGER)
    for item, status in ITEM_STATUS.items():
        row = None
        for line in plan.splitlines():
            if line.startswith(f"| `{item}`"):
                row = line
                break
        if row is None:
            violations.append(f"{LEDGER} missing ledger row {item}")
            continue
        if f"`{status}`" not in row:
            violations.append(f"{LEDGER} row {item} is not `{status}`")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    violations: list[str] = []

    # 1. The per-item ledger covers all 15 rows with the recorded status.
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
        "`parity-ready (15/15)`",
        "aggregator-guard.py",
        "聚合生成结果可视化预览",
        "自定义节点重命名规则",
        "节点可用性预检与过滤",
        "一键保持源订阅联动更新",
        "历史聚合模板保存与复用",
    )
    check_ledger_rows(violations)

    # 2. The domain aggregator is the single implementation.
    require(
        violations,
        "crates/infiltrator-domain/src/profile_aggregator.rs",
        "pub struct ProfileAggregator",
        "pub fn plan(",
        "pub fn pipeline(",
        "pub fn rename_pipeline(",
        "fn precheck_nodes(",
        "fn validate_custom_groups(",
        "pub fn cluster_regions(",
        "pub fn synthesize_groups(",
        "pub const MASTER_SELECT_GROUP",
        "pub const MAX_PRECHECK_SAMPLES",
        "pub const CUSTOM_GROUP_TYPES",
        "pub fn region_label(",
        "fn custom_members(",
        "pub struct AggregationPlan",
        "pub struct RegionalCluster",
        "pub struct GeneratedGroup",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/proxy_nodes/validate.rs",
        "pub fn validate_item(",
        "uuid is required",
        "password is required",
        "port must be positive",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/profile_converter.rs",
        "ProfileAggregator::plan(sources, options)?.yaml",
        "pub rename_rules: Vec<infiltrator_contract::aggregator::AggregationRenameRule>",
        "pub custom_groups: Vec<infiltrator_contract::aggregator::AggregationCustomGroup>",
        "pub availability_precheck: bool",
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

    # 3. The shared contract + application own the report and the templates.
    require(
        violations,
        "crates/infiltrator-contract/src/aggregator.rs",
        "pub struct AggregationDraft",
        "pub struct AggregationReport",
        "pub struct AggregationRenameRule",
        "pub struct AggregationCustomGroup",
        "pub struct AggregationTemplate",
        "pub struct AggregatedProfileOutcome",
        "pub struct RegionalClusterSnapshot",
        "pub struct GeneratedGroupSnapshot",
        "pub fn parse_list(",
        "pub fn to_text(",
        "pub fn master_group(",
        "pub fn yaml_preview(",
        "pub rule_renamed_nodes: usize",
        "pub invalid_nodes_removed: usize",
        "pub invalid_node_samples: Vec<String>",
        "pub is_custom: bool",
        "pub availability_precheck: bool",
        "pub activate_after_create: bool",
        "pub rename_rules: Vec<AggregationRenameRule>",
        "pub custom_groups: Vec<AggregationCustomGroup>",
        "pub template_name: Option<String>",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "PreviewProfileAggregation {",
        "CreateAggregatedProfile {",
        "SaveAggregationTemplate {",
        "DeleteAggregationTemplate {",
        "ReAggregateProfile {",
    )
    require(
        violations,
        "crates/infiltrator-application/src/profile_aggregation_application.rs",
        "pub struct ProfileAggregationApplication",
        "pub async fn preview(",
        "pub fn plan_documents(",
        "pub async fn create_profile(",
        "pub async fn create_profile_with_runtime<",
        "pub async fn list_templates(",
        "pub async fn save_template(",
        "pub async fn delete_template(",
        "pub async fn reaggregate<",
        "pub fn aggregation_options(",
        "pub fn last_aggregation_report(",
        "rename_rules: draft.rename_rules.clone()",
        "custom_groups: draft.custom_groups.clone()",
        "availability_precheck: draft.availability_precheck",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application/dispatch.rs",
        "CommandIntent::PreviewProfileAggregation { draft }",
        "CommandIntent::CreateAggregatedProfile { draft }",
        "CommandIntent::SaveAggregationTemplate { name, draft }",
        "CommandIntent::DeleteAggregationTemplate { name }",
        "CommandIntent::ReAggregateProfile { template_name }",
        "create_profile_with_runtime(self.managed_runtime.clone(), &draft)",
        "reaggregate(self.managed_runtime.clone(), &template_name)",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "aggregation: crate::profile_aggregation_application::last_aggregation_report()",
        "load_aggregation_templates()",
        "aggregation_templates,",
        "aggregation_templates_available,",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub aggregation: Option<crate::aggregator::AggregationReport>",
        "pub aggregation_templates: Vec<crate::aggregator::AggregationTemplate>",
        "pub aggregation_templates_available: bool",
    )

    # 3b. The template library persists through the profile store port.
    require(
        violations,
        "crates/infiltrator-ports/src/profile_store.rs",
        "async fn load_aggregation_templates(",
        "async fn save_aggregation_templates(",
    )
    require(
        violations,
        "crates/mihomo-config/src/profile_store.rs",
        "fn aggregation_templates_path(",
        ".aggregation-templates.yaml",
    )
    require(
        violations,
        "crates/infiltrator-application/src/profile_application.rs",
        "pub async fn load_aggregation_templates(",
        "pub async fn save_aggregation_templates(",
    )

    # 4. Iced consumes the shared report, drafts and template library.
    require(
        violations,
        "crates/infiltrator-iced/src/update/aggregator.rs",
        "pub(crate) fn aggregator_draft(",
        "pub(super) fn update_aggregator(",
        "Message::PreviewProfileAggregation",
        "Message::AggregationPreviewFinished",
        "Message::CreateAggregatedProfile",
        "Message::AddAggregatorCustomGroup",
        "Message::RemoveAggregatorCustomGroup(",
        "Message::SaveAggregatorTemplate",
        "Message::ApplyAggregatorTemplate(",
        "Message::ReAggregateProfile(",
        "Message::DeleteAggregatorTemplate(",
        "aggregator_availability_precheck",
        "aggregator_activate_after_create",
        "AggregationRenameRule::parse_list(",
        "create_profile_with_runtime(runtime, &draft)",
        ".reaggregate(runtime, &template_name)",
        "ProfileAggregationApplication::new",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "pub aggregator_report: Option<infiltrator_contract::aggregator::AggregationReport>",
        "pub aggregator_deduplicate: bool",
        "pub aggregator_geo_cluster: bool",
        "pub aggregator_generate_groups: bool",
        "pub aggregator_availability_precheck: bool",
        "pub aggregator_activate_after_create: bool",
        "pub aggregator_renames: String",
        "pub aggregator_custom_groups: Vec<infiltrator_contract::aggregator::AggregationCustomGroup>",
        "pub aggregator_templates: Vec<infiltrator_contract::aggregator::AggregationTemplate>",
        "pub aggregator_template_name: String",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view_root/aggregator_modal.rs",
        "Message::PreviewProfileAggregation",
        "Message::CreateAggregatedProfile",
        "Message::AddAggregatorCustomGroup",
        "Message::RemoveAggregatorCustomGroup(index)",
        "Message::SaveAggregatorTemplate",
        "Message::ApplyAggregatorTemplate(",
        "Message::ReAggregateProfile(",
        "Message::DeleteAggregatorTemplate(",
        "Message::UpdateAggregatorRenames",
        "aggregator_preview_nodes",
        "aggregator_preview_regions",
        "aggregator_preview_groups",
        "aggregator_preview_yaml",
        "aggregator_preview_cleaning",
        "aggregator_activate_after_create",
        "aggregator_availability_precheck",
        "aggregator_custom_group_label",
        "aggregator_template_title",
        "yaml_preview(",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/update/aggregator.rs",
        "Merged {count} profiles into",
        "infiltrator_domain::profile_aggregator::ProfileAggregator",
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

    # 5. Bevy renders the shared report and submits shared commands.
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/profiles_aggregator.rs",
        "pub struct AggregatorSourceToggle",
        "pub struct PreviewAggregationButton",
        "pub struct SaveAggregatedProfileButton",
        "pub struct AggregatorSwitch(",
        "pub enum AggregatorSwitchKind",
        "pub struct AggregatorPreviewText(",
        "pub enum AggregatorPreviewKind",
        "pub struct AggregatorTemplateNameField",
        "pub struct SaveAggregationTemplateButton",
        "pub struct UseAggregationTemplateButton",
        "pub struct ReAggregateTemplateButton",
        "pub struct DeleteAggregationTemplateButton",
        "pub struct AggregatorCustomGroupsText",
        "pub struct AggregatorStatusText",
        "pub fn profiles_aggregator_scene(",
        "pub fn aggregation_counters(",
        "pub fn aggregation_regions(",
        "pub fn aggregation_groups(",
        "pub fn aggregation_yaml_preview(",
        "pub fn aggregation_custom_groups(",
        "pub fn aggregation_templates(",
        "pub(super) fn sync_aggregation_preview(",
        "AggregatorPreviewKind::Yaml => aggregation_yaml_preview(report)",
        "AggregatorPreviewKind::Templates => aggregation_templates(projection)",
    )
    forbid(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/profiles_aggregator.rs",
        "香港自动测速 (12 节点)",
        "日本自动测速 (18 节点)",
        "infiltrator_domain::profile_aggregator::ProfileAggregator",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/profiles_aggregator_wizard.rs",
        "pub struct AggregatorComposerState",
        "fn draft_from_widgets(",
        "AggregationRenameRule::parse_list(",
        "AggregatorSwitchKind::AvailabilityPrecheck",
        "AggregatorSwitchKind::ActivateAfterCreate",
        "pub(super) fn on_preview_aggregation(",
        "pub(super) fn on_save_aggregated_profile(",
        "pub(super) fn on_add_aggregator_custom_group(",
        "pub(super) fn on_clear_aggregator_custom_groups(",
        "pub(super) fn on_save_aggregation_template(",
        "pub(super) fn on_use_aggregation_template(",
        "pub(super) fn on_reaggregate_template(",
        "pub(super) fn on_delete_aggregation_template(",
        "UiCommand::PreviewProfileAggregation",
        "UiCommand::CreateAggregatedProfile",
        "UiCommand::SaveAggregationTemplate",
        "UiCommand::DeleteAggregationTemplate",
        "UiCommand::ReAggregateProfile",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/profiles.rs",
        "crate::pages::profiles_aggregator::profiles_aggregator_scene(projection, palette)",
        "crate::pages::profiles_aggregator::sync_aggregation_preview",
        "profiles_aggregator_wizard::on_preview_aggregation",
        "profiles_aggregator_wizard::on_save_aggregated_profile",
        "profiles_aggregator_wizard::on_add_aggregator_custom_group",
        "profiles_aggregator_wizard::on_use_aggregation_template",
        "profiles_aggregator_wizard::on_reaggregate_template",
        "profiles_aggregator_wizard::on_delete_aggregation_template",
        "profiles_aggregator_wizard::AggregatorComposerState",
        "pub aggregation: Option<infiltrator_contract::aggregator::AggregationReport>",
        "pub aggregation_templates: Vec<infiltrator_contract::aggregator::AggregationTemplate>",
        "pub aggregation_templates_available: bool",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "PreviewProfileAggregation {",
        "CreateAggregatedProfile {",
        "SaveAggregationTemplate {",
        "DeleteAggregationTemplate {",
        "ReAggregateProfile {",
        "CommandIntent::PreviewProfileAggregation",
        "CommandIntent::CreateAggregatedProfile",
        "CommandIntent::SaveAggregationTemplate",
        "CommandIntent::DeleteAggregationTemplate",
        "CommandIntent::ReAggregateProfile",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "aggregation: value.aggregation.clone()",
        "aggregation_templates: value.aggregation_templates.clone()",
        "aggregation_templates_available: value.aggregation_templates_available",
    )

    # 6. Dual headless evidence, one marker per closed batch-B item.
    require(
        violations,
        "crates/infiltrator-application/src/profile_aggregation_application_test.rs",
        "aggregation_pipeline_regression_matrix",
        "create_profile_saves_a_new_independent_profile",
        "create_profile_refuses_to_overwrite_an_existing_profile",
        "preview_reports_real_dedup_clusters_and_master_cascade",
        # 08-07 / 08-12 / 08-13
        "reaggregate_refreshes_a_generated_profile_from_live_sources",
        "create_profile_activates_through_the_shared_profile_path",
        "create_profile_reports_missing_template_sidecar_without_failing",
        "templates_upsert_list_and_delete",
        # 08-08 / 08-09 / 08-10
        "preview_applies_precheck_and_rename_switches",
        "preview_synthesizes_custom_groups_from_the_draft",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/profile_aggregator_test.rs",
        "plan_reports_real_dedup_and_region_counts",
        "plan_generates_master_cascade_before_concrete_nodes",
        "plan_precheck_drops_invalid_nodes_and_reports_samples",
        "plan_precheck_failure_names_the_dropped_nodes",
        "plan_applies_regex_rename_rules_before_clustering",
        "plan_synthesizes_custom_groups_with_keyword_members",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/proxy_nodes/proxy_nodes_test.rs",
        "validate_item_reports_missing_credentials_and_port",
    )
    require(
        violations,
        "crates/mihomo-config/src/profile_store_test.rs",
        "aggregation_templates_persist_beside_the_profile_options",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/iced_six_advancements_wave2_tests.rs",
        "test_advancement_w2_3_multi_profile_aggregator_workflow",
        "test_advancement_w2_3b_aggregator_preview_lifecycle_is_shared",
        "Message::AddAggregatorCustomGroup",
        "Message::ApplyAggregatorTemplate(",
        "aggregator_draft()",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_profiles_aggregator_previews_and_saves_through_shared_command",
        "聚合 YAML 结构（共 4 行）",
        "预检剔除 2",
        "已保存模板 → Template-Target",
        "UiCommand::ReAggregateProfile",
        "UiCommand::SaveAggregationTemplate",
        "UiCommand::DeleteAggregationTemplate",
        "重命名规则格式错误",
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
    print(
        f"aggregator-guard: DUAL-08 ledger_rows=15 parity_items={PARITY_ITEMS} violations=0"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

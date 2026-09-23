#!/usr/bin/env python3
"""Fail-closed guard for the group 11 rules-engine ledger (DUAL-11-01..15).

Group 11's closure standard is the same as groups 06/07/13: one shared
reduction, both surfaces, dual headless tests, and an honest ledger. This guard
asserts the per-item ledger rows exist, that the shared rules-view reductions
and both surface wirings are present, and — critically — that no surface has
re-introduced a fabricated provider payload: the Bevy MRS card renders the
shared `MrsAccelerationSnapshot`, and 11-06/11-07 resolve real provider rules
and real cache removals through the shared application/port seam instead of the
old `apple.com`/`icloud.com` samples and the no-op purge toast.
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
        "DUAL-11-01",
        "DUAL-11-02",
        "DUAL-11-03",
        "DUAL-11-04",
        "DUAL-11-05",
        "DUAL-11-06",
        "DUAL-11-07",
        "DUAL-11-08",
        "DUAL-11-09",
        "DUAL-11-10",
        "DUAL-11-11",
        "DUAL-11-12",
        "DUAL-11-13",
        "DUAL-11-14",
        "DUAL-11-15",
        "parity-ready",
        "shared-ready",
        "planned",
    )
    # Closed items carry their distinctive evidence tokens in the ledger.
    require(
        violations,
        LEDGER,
        "MrsAccelerationSnapshot",
        "mrs_acceleration_card",
        "provider_lifecycle_line",
        "RuleProviderSnapshot.source_url",
        "filter_rule_indices",
        "sync_rules_view",
        "test_rules_mrs_renders_shared_snapshot_not_fabricated",
        "test_rules_provider_lifecycle_renders_shared_source_url",
        "test_rules_search_hides_non_matching_rows",
        "test_rules_pagination_hides_rows_outside_page",
        "test_mrs_acceleration_card_renders_shared_status_and_items",
        "test_rules_filter_and_pagination_delegate_to_shared_reduction",
    )
    # Batch B: the shared edit reductions, the command bus and both surfaces.
    require(
        violations,
        LEDGER,
        "RuleMoveDirection",
        "build_custom_rule",
        "inject_game_presets",
        "test_rules_toggle_and_reorder_submit_shared_intents",
        "test_rules_add_wizard_submits_shared_draft_and_type_selection",
        "test_rules_game_presets_submit_shared_target",
        "test_rules_matrix_covers_closed_items",
        "test_rules_type_matrix_and_logical_builder_delegate_to_shared",
        "test_rules_edit_operations_delegate_to_shared_module",
        "toggle_and_move_apply_shared_reductions_and_persist",
        "add_custom_rule_prepends_and_rejects_invalid_logical_form",
        "game_presets_prepend_the_shared_default_list",
        "matrix_11_01_rule_type_vocabulary_parses",
        "matrix_11_09_to_12_rule_edit_reductions",
        "rules_matrix_test.rs",
    )
    # Batch C (DUAL-11-01/02/05/08): the shared type catalogue, the shared
    # logical sub-rule draft reduction, the declared refresh interval and the
    # honest publish-cap facts, all wired on both surfaces.
    require(
        violations,
        LEDGER,
        "RULE_TYPE_MATRIX",
        "RuleTypeFamily",
        "matrix_label",
        "RuleTypeBadge",
        "rule_type_chip_fill",
        "test_rules_type_matrix_renders_every_shared_label",
        "LogicalDraft",
        "LOGICAL_OPERATOR_CHOICES",
        "SUB_RULE_CONDITION_PRESETS",
        "build_logical_rule",
        "draft_expression",
        "draft_issue",
        "RulesSubRuleState",
        "rules_subrules.rs",
        "on_rules_subrules_activated",
        "SubRuleConditionList",
        "test_rules_subrules_builder_submits_shared_logical_intent",
        "test_advancement_w3_2_subrules_logical_builder_workflow",
        "matrix_11_02_logical_draft_builds_recursive_expression",
        "refresh_interval_secs",
        "format_refresh_interval",
        "kernel-scheduled",
        "内核调度",
        "rule_publish_limit",
        "truncation_label",
        "publish_truncation_line",
        "published_rule_count",
        "omitted_rule_count",
        "is_truncated_rule_list",
        "test_rules_truncation_note_reports_publish_cap",
        "test_rules_provider_interval_and_publish_truncation_project_from_snapshot",
        "matrix_11_05_provider_refresh_intent_and_declared_interval",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/rules.rs",
        "pub mod logical;",
        "pub mod matrix;",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/rules/matrix.rs",
        "pub const RULE_TYPE_MATRIX",
        "pub enum RuleTypeFamily",
        "pub struct RuleTypeSpec",
        "pub fn matrix_label",
        "pub fn matrix_family",
        "pub fn spec_for_name",
        "pub fn spec(&self)",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/rules/logical.rs",
        "pub const LOGICAL_OPERATOR_CHOICES",
        "pub const SUB_RULE_CONDITION_PRESETS",
        "pub fn default_logical_draft",
        "pub fn select_operator",
        "pub fn add_condition",
        "pub fn remove_condition",
        "pub fn set_target",
        "pub fn draft_payload",
        "pub fn draft_operator",
        "pub fn draft_expression",
        "pub fn draft_issue",
        "pub fn build_logical_rule",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/rules/view.rs",
        "pub const RULE_PUBLISH_LIMIT",
        "pub fn published_rule_count",
        "pub fn omitted_rule_count",
        "pub fn is_truncated_rule_list",
        "pub fn format_refresh_interval",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/rule_edit.rs",
        "pub struct LogicalDraft",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub refresh_interval_secs: Option<u64>",
        "pub rule_publish_limit: usize",
        "pub fn omitted_rule_count",
        "pub fn is_truncated",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "refresh_interval_secs",
        "published_rule_count",
        "RULE_PUBLISH_LIMIT",
    )
    # Iced consumes the shared catalogue and the shared logical builder.
    require(
        violations,
        "crates/infiltrator-iced/src/view/rules.rs",
        "matrix::matrix_label",
        "RuleTypeFamily::Host",
        "RuleTypeFamily::Address",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/rules/providers.rs",
        "format_refresh_interval",
        "kernel-scheduled",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/rules/rules_list.rs",
        "pub fn publish_truncation_line",
        "rules_publish_truncated",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/subrules_builder.rs",
        "logical::LOGICAL_OPERATOR_CHOICES",
        "logical::SUB_RULE_CONDITION_PRESETS",
        "logical::draft_expression",
        "logical::draft_issue",
        "Message::UpdateSubRuleTarget",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/ui_wave3.rs",
        "logical::select_operator",
        "logical::add_condition",
        "logical::remove_condition",
        "logical::set_target",
        "logical::build_logical_rule",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "rule_provider_intervals",
        "rule_publish_omitted",
        "rule_publish_limit",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table.rs",
        "rules_publish_truncated",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en.rs",
        "rules_publish_truncated",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_ext.rs",
        "subrules_validate_ok",
        "subrules_no_conditions",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en_ext.rs",
        "subrules_validate_ok",
        "subrules_no_conditions",
    )
    # Bevy renders the shared catalogue labels/chips and the visual logical
    # sub-rule builder, and publishes the truncation/schedule facts.
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages.rs",
        "pub mod rules_projection;",
        "pub mod rules_subrules;",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules_subrules.rs",
        "pub struct RulesSubRuleState",
        "pub struct SubRuleOperatorChip",
        "pub struct SubRuleConditionRow",
        "pub struct SubRuleConditionList",
        "pub struct SubRulePresetButton",
        "pub struct SubRuleTargetField",
        "pub struct SubRuleInsertButton",
        "pub fn rules_subrules_scene",
        "pub(crate) fn on_rules_subrules_activated",
        "logical::build_logical_rule",
        "logical::select_operator",
        "logical::draft_expression",
        "UiCommand::AddCustomRule",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules_projection.rs",
        "pub struct RuleTypeBadge",
        "pub(crate) fn rule_type_chip_fill",
        "pub(crate) fn truncation_label",
        "matrix_label",
        "RuleTypeFamily::Host",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules.rs",
        "RuleTypeBadge",
        "rule_type_chip_scene",
        "truncated_rule_count",
        "自动刷新",
        "内核调度",
        "rules_subrules_scene",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "value.is_truncated()",
        "refresh_interval_secs: provider.refresh_interval_secs",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_rules_type_matrix_renders_every_shared_label",
        "test_rules_subrules_builder_submits_shared_logical_intent",
        "test_rules_truncation_note_reports_publish_cap",
    )
    require(
        violations,
        "crates/infiltrator-domain/tests/rules_matrix_test.rs",
        "matrix_11_02_logical_draft_builds_recursive_expression",
        "matrix_11_05_provider_refresh_intent_and_declared_interval",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/rules_dns_tests.rs",
        "test_rules_provider_interval_and_publish_truncation_project_from_snapshot",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/view_rules_tests.rs",
        "Auto: 1d (kernel-scheduled)",
    )
    # Honest-boundary markers: the fabricated/corrupted forms must not return.
    forbid(
        violations,
        "crates/infiltrator-iced/src/view/rules.rs",
        '"DOMAIN-SUFFIX" | "DOMAINSUFFIX" => "DomainSuffix"',
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/update/ui_wave3.rs",
        'subrule_draft.conditions.join(", ")',
    )
    require(
        violations,
        "crates/infiltrator-contract/src/rule_edit.rs",
        "pub enum RuleMoveDirection",
        "pub struct RuleDraft",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "ToggleRuleEnabled",
        "MoveRule",
        "AddCustomRule",
        "ApplyGameRoutingPresets",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/rules.rs",
        "pub mod edit;",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/rules/edit.rs",
        "pub const CUSTOM_RULE_TYPE_CHOICES",
        "pub const DEFAULT_RULE_TARGET",
        "pub fn build_custom_rule",
        "pub fn toggle_rule_enabled",
        "pub fn move_rule",
        "pub fn prepend_rules",
        "pub fn inject_game_presets",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "async fn edit_rules",
        "edit::toggle_rule_enabled",
        "edit::move_rule",
        "edit::build_custom_rule",
        "edit::inject_game_presets",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application_tests.rs",
        "toggle_and_move_apply_shared_reductions_and_persist",
        "add_custom_rule_prepends_and_rejects_invalid_logical_form",
        "game_presets_prepend_the_shared_default_list",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/rules.rs",
        "rules::edit::toggle_rule_enabled",
        "rules::edit::move_rule",
        "rules::edit::build_custom_rule",
        "rules::edit::inject_game_presets",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/rules/rules_list.rs",
        "CUSTOM_RULE_TYPE_CHOICES",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules_edit.rs",
        "pub struct RuleToggleButton",
        "pub struct RuleMoveUpButton",
        "pub struct RuleMoveDownButton",
        "on_rules_row_edit_activated",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules_builder.rs",
        "pub struct RuleTypeChip",
        "pub struct RulePayloadField",
        "pub struct RuleTargetField",
        "pub struct RulesBuilderState",
        "on_rules_builder_activated",
        "CUSTOM_RULE_TYPE_CHOICES",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules.rs",
        "pub is_enabled:",
        "rule_row_controls_scene",
        "RuleToggleButton",
        "RuleMoveUpButton",
        "RuleMoveDownButton",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "is_enabled: rule.is_enabled",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "ToggleRuleEnabled",
        "MoveRuleUp",
        "MoveRuleDown",
        "AddCustomRule",
        "ApplyGameRoutingPresets",
    )
    require(
        violations,
        "crates/infiltrator-domain/tests/rules_matrix_test.rs",
        "matrix_11_01_rule_type_vocabulary_parses",
        "matrix_11_02_logical_sub_rules_evaluate",
        "matrix_11_03_mrs_binary_pipeline",
        "matrix_11_09_to_12_rule_edit_reductions",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_rules_toggle_and_reorder_submit_shared_intents",
        "test_rules_add_wizard_submits_shared_draft_and_type_selection",
        "test_rules_game_presets_submit_shared_target",
        "test_rules_matrix_covers_closed_items",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/rules_dns_tests.rs",
        "test_rules_edit_operations_delegate_to_shared_module",
        "test_rules_type_matrix_and_logical_builder_delegate_to_shared",
    )

    # Shared reduction lives in the domain crate.
    require(
        violations,
        "crates/infiltrator-domain/src/rules.rs",
        "pub mod view;",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/rules/view.rs",
        "pub trait RuleView",
        "pub const DEFAULT_RULE_PAGE_SIZE",
        "pub fn matches_rule_search",
        "pub fn filter_rule_indices",
        "pub fn effective_page_size",
        "pub fn page_count",
        "pub fn clamp_page",
        "pub fn page_bounds",
        "impl RuleView for RuleEntry",
    )

    # Shared contract + reader publish the provider source URL and MRS model.
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub source_url: Option<String>",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "load_rule_providers",
        "source_url",
        "MrsAccelerationApplication::new()",
    )

    # Iced consumes the shared reductions and renders the shared MRS model.
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/rules.rs",
        "infiltrator_domain::rules::view::filter_rule_indices",
        "infiltrator_domain::rules::view::clamp_page",
        "infiltrator_domain::rules::view::page_count",
        "rules_window::page_scroll_offset",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/rules_window.rs",
        "view::page_bounds",
        "view::clamp_page",
        "view::page_for_rule_index",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/rules/providers.rs",
        "provider_lifecycle_line",
        "rule_provider_source_urls",
        "mrs_acceleration_card",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/mrs_panel.rs",
        "mrs_acceleration_card",
        "mrs_acceleration_status_line",
        "mrs_acceleration_item_label",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "mrs_acceleration",
        "rule_provider_source_urls",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_ext.rs",
        "mrs_accel_title",
        "mrs_accel_ready",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en_ext.rs",
        "mrs_accel_title",
        "mrs_accel_ready",
    )

    # Bevy consumes the same reductions and shared MRS read model.
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules_view.rs",
        "impl RuleView for RuleItem",
        "pub struct RuleRow",
        "pub struct RuleSearchField",
        "pub struct RulesViewState",
        "pub struct RulesPageIndicator",
        "pub(crate) fn sync_rules_view",
        "pub(crate) fn on_rules_paging_activated",
        "view::filter_rule_indices",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules_mrs.rs",
        "MrsAccelerationSnapshot",
        "pub fn rules_mrs_scene",
        "pub fn apply_mrs_projection",
        "pub(crate) fn mrs_status_line",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules.rs",
        "pub mrs_acceleration:",
        "rules_mrs_scene(palette, mrs, provider_cache)",
        "pub(crate) fn provider_updated_label",
        "RuleSearchField",
        "RulesPagePrevButton",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages.rs",
        "pub mod rules_view;",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/route.rs",
        "sync_rules_view",
    )
    # DUAL-11-03: the fabricated MRS list must not come back.
    forbid(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules_mrs.rs",
        "14,200 条目",
        "28,500 条目",
    )

    # Batch D (DUAL-11-06/07): the real provider unpack and cache purge seam.
    require(
        violations,
        LEDGER,
        "matrix_11_06_provider_declaration_and_payload_deconstruct",
        "matrix_11_07_provider_cache_purge_fact",
        "test_rules_provider_unpack_and_cache_purge_submit_shared_intents",
        "test_rules_provider_cache_line_reports_observed_facts",
        "test_rules_page_renders_observed_provider_cache_fact",
        "test_rules_rule_provider_unpack_reads_shared_application",
        "unpack_rule_provider_imports_real_payload_and_rejects_unknown",
        "purge_rule_provider_cache_requires_and_reports_the_host_location",
        "unpack_without_any_source_is_unsupported",
        "ProviderContentOrigin",
        "ProviderCachePurge",
        "RuleProviderCacheSnapshot",
        "RuleProviderDeclaration",
        "provider_cache_file_name",
        "deconstruct_provider_payload",
        "provider_source_candidates",
        "RuleProviderCachePort",
        "RuleProviderApplication",
        "DesktopRuleProviderCache",
        "rule_provider_payload",
        "PurgeRuleProviderCacheButton",
        "RulesMrsState",
        "provider_cache_line",
        "on_rules_mrs_action_activated",
        "with_rule_provider_cache",
        "KernelCacheFile",
        "inline-payload",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/provider_cache.rs",
        "pub enum ProviderContentOrigin",
        "pub struct ProviderCachePurge",
        "pub struct RuleProviderCacheSnapshot",
        "pub enum RuleProviderCacheState",
        "pub fn is_noop",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/rules/provider_store.rs",
        "pub const PROVIDER_CACHE_DIR_NAME",
        "pub enum ProviderBehavior",
        "pub enum ProviderFormat",
        "pub struct RuleProviderDeclaration",
        "pub fn parse_rule_provider_declarations",
        "pub fn provider_cache_file_name",
        "pub fn provider_source_candidates",
        "pub fn unpack_provider_rules_with_behavior",
        "pub fn deconstruct_provider_payload",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/rules.rs",
        "pub mod provider_store;",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/rule_provider_cache.rs",
        "pub trait RuleProviderCachePort",
        "async fn read_provider",
        "async fn purge",
        "async fn snapshot",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/host_runtime.rs",
        "fn rule_provider_cache_port",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/runtime_gateway.rs",
        "async fn rule_provider_payload",
    )
    require(
        violations,
        "crates/infiltrator-application/src/rule_provider_application.rs",
        "pub struct RuleProviderApplication",
        "pub struct ProviderUnpackPlan",
        "pub async fn deconstruct",
        "pub async fn purge",
        "ProviderContentOrigin::InlinePayload",
        "ProviderContentOrigin::ControllerPayload",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "async fn unpack_rule_provider",
        "async fn purge_rule_provider_cache",
        "parse_rule_provider_declarations",
        "CommandIntent::PurgeRuleProviderCache",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "fn with_rule_provider_cache",
        "RuleProviderApplication",
        "provider_cache",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application_tests.rs",
        "unpack_rule_provider_imports_real_payload_and_rejects_unknown",
        "purge_rule_provider_cache_requires_and_reports_the_host_location",
        "unpack_without_any_source_is_unsupported",
    )
    require(
        violations,
        "crates/infiltrator-domain/tests/rules_matrix_test.rs",
        "matrix_11_06_provider_declaration_and_payload_deconstruct",
        "matrix_11_07_provider_cache_purge_fact",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/rule_provider_cache.rs",
        "pub struct DesktopRuleProviderCache",
        "async fn read_provider",
        "async fn purge",
        "async fn snapshot",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/runtime/ports.rs",
        "fn rule_provider_cache_port",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/rules_provider.rs",
        "pub(crate) fn update_rule_provider",
        "RuleProviderApplication::new",
        "parse_rule_provider_declarations",
        "purge()",
        "provider_cache",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core.rs",
        "mod rules_provider;",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "RuleProviderUnpacked",
        "RuleProviderCachePurged",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/provider_unpack_card.rs",
        "provider_btn_unpack_idle",
        "rule_providers",
        "provider_cache",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_ext.rs",
        "provider_btn_unpack_idle",
        "provider_cache_ready",
        "provider_unpack_total",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en_ext.rs",
        "provider_btn_unpack_idle",
        "provider_cache_ready",
        "provider_unpack_total",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "UnpackRuleProvider(String)",
        "PurgeRuleProviderCache",
        "CommandIntent::UnpackRuleProvider",
        "CommandIntent::PurgeRuleProviderCache",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules_mrs.rs",
        "pub struct PurgeRuleProviderCacheButton",
        "pub struct ProviderCacheText",
        "pub struct RulesMrsState",
        "pub(crate) fn provider_cache_line",
        "pub fn apply_provider_cache_projection",
        "pub(crate) fn on_rules_mrs_action_activated",
        "UiCommand::UnpackRuleProvider",
        "UiCommand::PurgeRuleProviderCache",
        "宿主未声明缓存目录",
        "内核规则集缓存",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "provider_cache: value.provider_cache.clone()",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_rules_provider_unpack_and_cache_purge_submit_shared_intents",
        "test_rules_provider_cache_line_reports_observed_facts",
        "test_rules_page_renders_observed_provider_cache_fact",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/rules_dns_tests.rs",
        "test_rules_rule_provider_unpack_reads_shared_application",
    )
    # Reverse assertions: the fabricated samples and the no-op purge path must
    # never come back on either surface.
    forbid(
        violations,
        "crates/infiltrator-iced/src/update/core/rules.rs",
        "apple.com",
        "icloud.com",
        "sample_rules",
        "DOMAIN-SUFFIX,google.com",
        "DOMAIN-KEYWORD,google",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/update/ui_wave5.rs",
        "apple.com",
        "icloud.com",
        "Provider cache purged successfully",
        "Unpacked {count} rules to custom rules",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/update/core/rules_provider.rs",
        "apple.com",
        "icloud.com",
        "google.com",
        "Provider cache purged successfully",
        "unpacked_rules_count += count",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/view/provider_unpack_card.rs",
        "Apple-Provider",
    )
    forbid(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "CommandIntent::UnpackRuleProvider { .. }",
        "CommandIntent::PurgeRuleProviderCache => Err(unsupported())",
    )
    forbid(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules_mrs.rs",
        "解构 2 条",
        "Unpacked",
    )

    # Dual headless evidence.
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_rules_mrs_renders_shared_snapshot_not_fabricated",
        "test_rules_provider_lifecycle_renders_shared_source_url",
        "test_rules_search_hides_non_matching_rows",
        "test_rules_pagination_hides_rows_outside_page",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/rules_dns_tests.rs",
        "test_rules_filter_and_pagination_delegate_to_shared_reduction",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/view_mrs_panel_tests.rs",
        "test_mrs_acceleration_card_renders_shared_status_and_items",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/view_rules_tests.rs",
        "test_provider_lifecycle_line_reports_shared_source_url",
    )

    # Batch E (DUAL-11-08): the real virtual window. One shared O(1) window
    # reduction, an Iced spacer/scroll-driven render, a Bevy despawn+respawn
    # window, and a bounded-render test on each surface.
    require(
        violations,
        LEDGER,
        "RuleWindow",
        "rendered_row_bound",
        "visible_rule_items",
        "test_rules_virtual_window_renders_bounded_rows_for_50k_list",
        "test_rules_window_mounts_bounded_rows_for_50k_projection",
        "visible_projection_rows",
        "sync_rules_window",
        "RulesListScrollArea",
        "RulesWindowRows",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/rules/view.rs",
        "pub const RULE_ROW_HEIGHT_PX",
        "pub const RULE_WINDOW_OVERSCAN",
        "pub const RULE_DEFAULT_VIEWPORT_PX",
        "pub struct RuleWindow",
        "pub fn rule_window",
        "pub fn rendered_row_bound",
        "pub fn visible_rule_rows",
        "pub fn rule_scroll_offset_for_index",
        "pub fn rule_index_at_scroll_offset",
        "pub fn page_for_rule_index",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/rules_window.rs",
        "pub const RULES_LIST_SCROLL_ID",
        "pub fn rules_window",
        "pub fn visible_rule_items",
        "pub fn rendered_rule_rows",
        "pub fn rules_window_page",
        "pub fn page_scroll_offset",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "RulesListScrolled",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/rules/rules_list.rs",
        "rules_window::visible_rule_items",
        "rules_window::rules_window_spacers",
        "RULES_LIST_SCROLL_ID",
        "on_scroll",
        "RULE_ROW_HEIGHT_PX",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules_view.rs",
        "pub struct RulesListScrollArea",
        "pub struct RulesWindowRows",
        "pub rendered_rows: usize",
        "pub fn visible_projection_rows",
        "pub(crate) fn sync_rules_window",
        "pub(crate) fn rebuild_rules_window",
        "view::rule_window",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules.rs",
        "RulesListScrollArea",
        "RulesWindowRows",
        "rule_window(",
    )
    require(
        violations,
        "crates/infiltrator-domain/tests/rules_matrix_test.rs",
        "matrix_11_08_search_and_pagination_reduce_in_shared_view",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_rules_window_mounts_bounded_rows_for_50k_projection",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/rules_dns_tests.rs",
        "test_rules_virtual_window_renders_bounded_rows_for_50k_list",
    )
    # The window is real now: the old "not a virtual scroll" disclaimer (and
    # the paging-only visible-row fact it described) must not come back.
    forbid(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules_projection.rs",
        "非 O(1) 虚拟滚动",
    )
    forbid(
        violations,
        "crates/infiltrator-shared/src/locales_table.rs",
        "非 O(1) 虚拟滚动",
    )
    forbid(
        violations,
        "crates/infiltrator-shared/src/locales_table_en.rs",
        "no O(1) virtual scroll",
    )

    # Batch E (DUAL-11-14): the shared workspace partition vocabulary, the
    # shared Geo upgrade intent, the published JSON documents and the Bevy
    # JSON partition that edits them through the shared application.
    require(
        violations,
        LEDGER,
        "RulesJsonSection",
        "RulesJsonDocumentSnapshot",
        "UpgradeGeoDatabases",
        "ApplyRulesJsonDocument",
        "json_documents",
        "rules_tabs_scene",
        "rules_json_scene",
        "test_rules_workspace_partitions_delegate_to_shared_vocabulary",
        "test_rules_tab_partition_matches_the_shared_capability_set",
        "test_rules_geo_databases_button_submits_shared_intent",
        "test_rules_json_partition_edits_and_submits_shared_intent",
        "apply_rules_json_document_validates_and_persists_each_section",
        "upgrade_geo_databases_requires_the_runtime_gateway",
        "rules_json_documents_cover_the_shared_sections_and_omit_unreadable_ones",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/rules_workspace.rs",
        "pub enum RulesTab",
        "pub enum RulesJsonSection",
        "pub struct RulesJsonDocumentSnapshot",
        "pub const ALL: [Self; 4]",
        "pub const ALL: [Self; 3]",
        "pub const fn index(self)",
        "pub const fn from_index(index: usize)",
        "pub const fn i18n_key(self)",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "UpgradeGeoDatabases",
        "ApplyRulesJsonDocument",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub json_documents: Vec<crate::rules_workspace::RulesJsonDocumentSnapshot>",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "fn rules_json_documents(",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "async fn apply_rules_json_document",
        "RulesJsonSection::RuleProviders",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application_tests.rs",
        "apply_rules_json_document_validates_and_persists_each_section",
        "upgrade_geo_databases_requires_the_runtime_gateway",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader_test.rs",
        "rules_json_documents_cover_the_shared_sections_and_omit_unreadable_ones",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages.rs",
        "pub mod rules_json;",
        "pub mod rules_tabs;",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules_tabs.rs",
        "pub struct RulesTabState",
        "pub struct RulesTabChip",
        "pub struct RulesTabBody",
        "pub fn rules_tabs_scene",
        "pub fn tab_body_scene",
        "pub fn sync_rules_tabs",
        "pub fn on_rules_tab_activated",
        "pub const fn tab_label_zh",
        "RulesTab::ALL",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules_json.rs",
        "pub struct RulesJsonState",
        "pub struct RulesJsonSectionChip",
        "pub struct RulesJsonEditorBody",
        "pub struct RulesJsonEditButton",
        "pub struct RulesJsonSaveButton",
        "pub fn rules_json_scene",
        "pub fn sync_rules_json",
        "pub fn restamp_rules_json",
        "pub fn refresh_rules_json_body",
        "pub fn on_rules_json_action_activated",
        "pub fn rules_json_keyboard_input",
        "UiCommand::ApplyRulesJsonDocument",
        "code_editor_scene",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules_mrs.rs",
        "pub struct UpgradeGeoDatabasesButton",
        "UiCommand::UpgradeGeoDatabases",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "UpgradeGeoDatabases",
        "ApplyRulesJsonDocument {",
        "CommandIntent::ApplyRulesJsonDocument",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "json_documents: value.json_documents.clone()",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "infiltrator_contract::rules_workspace::{RulesJsonSection, RulesTab}",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/rules.rs",
        "RulesTab::ALL",
        "RulesTab::from_index",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/rules/providers.rs",
        "RulesJsonSection::ALL",
        "RulesJsonSection::from_index",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table.rs",
        "rules_proxy_providers_json",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en.rs",
        "rules_proxy_providers_json",
    )
    # The partition vocabulary is shared: neither surface may re-declare its
    # own tab or JSON-section enums.
    forbid(
        violations,
        "crates/infiltrator-iced/src/types/rules.rs",
        "pub enum RulesTab",
        "pub enum RulesJsonTab",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_rules_tab_partition_matches_the_shared_capability_set",
        "test_rules_geo_databases_button_submits_shared_intent",
        "test_rules_json_partition_edits_and_submits_shared_intent",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/rules_dns_tests.rs",
        "test_rules_workspace_partitions_delegate_to_shared_vocabulary",
    )

    # DUAL-11-05 (2026-09-23): the client-visible local fact is the provider
    # cache file's content fingerprint. The contract/reduction, the host port,
    # the previous-observation comparison and both surfaces must all be wired;
    # the fingerprint must stay explicitly non-HTTP.
    require(
        violations,
        LEDGER,
        "ProviderFileFingerprint",
        "ProviderFingerprintChange",
        "observe_fingerprint",
        "rules_provider_fingerprint_label",
        "本地缓存内容指纹（非 HTTP ETag）",
        "test_provider_fingerprint_line_reports_local_file_facts",
        "test_rules_provider_local_cache_fingerprint_renders_non_etag_label",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/provider_cache.rs",
        "pub struct ProviderFileFingerprint",
        "pub fn same_content",
        "pub enum ProviderFingerprintChange",
        "pub struct ProviderCacheFingerprint",
        "pub fn compare",
        'Self::FirstSeen => "first-seen"',
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub cache_fingerprint: Option<crate::provider_cache::ProviderCacheFingerprint>",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/rule_provider_cache.rs",
        "pub struct ProviderFileFact",
        "async fn fingerprint(",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/rule_provider_cache.rs",
        "async fn fingerprint(",
        "digest_for",
        "content_hash",
        "provider_cache_file_name",
    )
    require(
        violations,
        "crates/infiltrator-application/src/rule_provider_application.rs",
        "observed_fingerprints",
        "pub async fn observe_fingerprint(",
        "ProviderCacheFingerprint::compare",
        "fingerprint_observation_compares_against_the_previous_local_read",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "observe_fingerprint",
        "cache_fingerprint",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/rules/view.rs",
        "pub fn format_content_fingerprint",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "rule_provider_fingerprints",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/rules/providers.rs",
        "pub fn provider_fingerprint_line",
        "rules_provider_fingerprint_label",
        "rules_provider_fingerprint_changed",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_ext.rs",
        "rules_provider_fingerprint_label",
        "rules_provider_fingerprint_first_seen",
        "rules_provider_fingerprint_unchanged",
        "rules_provider_fingerprint_changed",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en_ext.rs",
        "rules_provider_fingerprint_label",
        "rules_provider_fingerprint_first_seen",
        "rules_provider_fingerprint_unchanged",
        "rules_provider_fingerprint_changed",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules.rs",
        "pub cache_fingerprint: Option<infiltrator_contract::provider_cache::ProviderCacheFingerprint>",
        "本地缓存内容指纹（非 HTTP ETag）",
        "较上次观测未变化",
        "较上次观测已变化",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "cache_fingerprint: provider.cache_fingerprint.clone()",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/rule_provider_cache.rs",
        "fingerprint_reports_real_size_digest_and_mtime_for_the_cache_file",
        "digest_memo_only_reuses_a_digest_for_identical_size_and_mtime",
    )
    # The local fingerprint must never be rendered as an HTTP validator claim.
    forbid(
        violations,
        "crates/infiltrator-iced/src/view/rules/providers.rs",
        "ETag hit",
        "304 Not Modified",
    )
    forbid(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules.rs",
        "ETag 命中",
        "304 未修改",
    )

    # DUAL-11-05 (2026-09-23, ETag capability): the kernel's REAL top-level
    # `etag-support` declaration (mihomo `General.ETagSupport`, default true) is
    # read from the active profile and surfaced on both surfaces. This is a
    # declaration fact: the per-request 304 outcome stays inside the kernel and
    # is never invented.
    require(
        violations,
        LEDGER,
        "KernelEtagSupportSnapshot",
        "KernelEtagSupportState",
        "etag-support",
        "load_etag_support",
        "etag_support_line",
        "etag_support_label",
        "rules_etag_support_label",
        "内核已启用",
        "内核未启用",
        "test_etag_support_line_reports_the_declared_kernel_capability",
        "test_rules_provider_etag_support_renders_declared_kernel_capability",
        "load_etag_support_reads_the_top_level_declaration_honestly",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/provider_cache.rs",
        "pub enum KernelEtagSupportState",
        "pub struct KernelEtagSupportSnapshot",
        "pub fn from_declared",
        'Self::NotDeclared => "not-declared"',
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub etag_support: crate::provider_cache::KernelEtagSupportSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-application/src/configuration_application.rs",
        "pub async fn load_etag_support",
        "etag-support",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "load_etag_support",
        "etag_support",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application_tests.rs",
        "load_etag_support_reads_the_top_level_declaration_honestly",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "rule_etag_support",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/rules/providers.rs",
        "pub fn etag_support_line",
        "rules_etag_support_label",
        "rules_etag_support_enabled",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_ext.rs",
        "rules_etag_support_label",
        "rules_etag_support_enabled",
        "rules_etag_support_disabled",
        "rules_etag_support_not_declared",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en_ext.rs",
        "rules_etag_support_label",
        "rules_etag_support_enabled",
        "rules_etag_support_disabled",
        "rules_etag_support_not_declared",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules.rs",
        "pub etag_support:",
        "pub(crate) fn etag_support_label",
        "ETag 缓存",
        "内核已启用",
        "内核未启用",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "etag_support: value.etag_support",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_rules_provider_etag_support_renders_declared_kernel_capability",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/view_rules_tests.rs",
        "test_etag_support_line_reports_the_declared_kernel_capability",
    )
    require(
        violations,
        "crates/infiltrator-domain/tests/rules_matrix_test.rs",
        "KernelEtagSupportSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_tests.rs",
        "rules_etag_support_not_declared",
    )

    # The guard itself is registered on both suites.
    require(
        violations,
        "scripts/test.sh",
        "rules-engine-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        'rules-engine-guard.py" --mode enforce',
    )

    if violations:
        for violation in violations:
            print(f"rules-engine-guard: {violation}", file=sys.stderr)
        print(f"rules-engine-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("rules-engine-guard: DUAL-11 ledger and dual-surface markers=complete")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

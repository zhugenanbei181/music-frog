#!/usr/bin/env python3
"""Fail-closed guard for the group 11 rules-engine ledger (DUAL-11-01..15).

Group 11's closure standard is the same as groups 06/07/13: one shared
reduction, both surfaces, dual headless tests, and an honest ledger. This guard
asserts the per-item ledger rows exist, that the shared rules-view reductions
and both surface wirings are present, and — critically — that the Bevy MRS card
has not re-introduced its fabricated rule-set list (11-03 is now rendered from
the shared `MrsAccelerationSnapshot`).
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
        "crates/infiltrator-iced/src/view/rules.rs",
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
        "infiltrator_domain::rules::view::page_bounds",
        "infiltrator_domain::rules::view::page_count",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/rules.rs",
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
        "rules_mrs_scene(palette, &projection.mrs_acceleration)",
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

#!/usr/bin/env python3
"""Fail-closed guard for the DUAL-09 AST YAML / snapshot-diff ledger.

Group 09 (byte-faithful YAML AST engine, snapshot history and visual diff)
must keep an honest per-item ledger (`DUAL-09-01..15`) and the shared path
must stay single-sourced:

* one byte-faithful text engine (`SourceDoc`) plus the verified rule-list
  splice (`rules_fidelity::apply_rule_list`); the structural serde writer is a
  fallback, never the first choice for rule adds/removes or scalar mixins;
* one shared snapshot application that computes and publishes the real Myers
  diff (`SnapshotApplication::{diff_snapshot,diff_newest}`) and one surface
  reader projection (`last_snapshot_diff`);
* one shared write-protection classification (`ProfileWriteProtection`) that
  the application guard (`save_edited_profile_content`) enforces;
* both surfaces render the shared diff rows and perform a two-step confirmed
  rollback: the Iced modal and the Bevy card may not keep their fabricated
  `+ Added` / `- Removed` / demo rows;
* one AST-preserving formatter (`yaml_edit::format::format_yaml`) is the only
  formatter either surface may call — the lossy `serde_yaml_ng` re-serialize is
  forbidden on the format paths;
* one snapshot history read model + prune policy
  (`SnapshotHistorySnapshot` / `history::prune_snapshots` over
  `backup::prune_snapshots`) is surfaced on both ends (list, manual backup,
  retention presets, prune) and the Bevy editor is a real surface running the
  shared preflight, formatter and guarded save;
* the host core publishes the typed apply transaction outcome
  (`ApplyTransactionStage::RolledBack` / `RollbackFailed`) instead of letting a
  surface infer a rollback from an error string;
* the guard is registered in both test entrypoints.

Modeled on `aggregator-guard.py`.
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
        row = f"DUAL-09-{index:02d}"
        if row not in plan:
            violations.append(f"{LEDGER} missing ledger row {row}")
    if "### 组 09 逐项账目" not in plan:
        violations.append(f"{LEDGER} missing the group 09 per-item ledger section")
    if "| 组 09 AST YAML 引擎与快照 Diff | 15 | `planned`" in plan:
        violations.append(
            f"{LEDGER} group 09 row is still an unexpanded `planned` group row"
        )
    require(
        violations,
        LEDGER,
        "yaml-diff-guard.py",
        "### 组 09 逐项账目",
        "parity-ready",
        "shared-ready",
        "planned",
        "可视化并排/行内 Diff",
        "只读保护与远程订阅防手滑覆写",
        "LEFT-05",
    )

    # 2. One byte-faithful text engine, structurally verified.
    require(
        violations,
        "crates/infiltrator-domain/src/yaml_edit.rs",
        "pub struct SourceDoc",
        "pub fn parse(",
        "pub fn render(",
        "pub fn append_rule(",
        "pub fn remove_rule(",
        "pub fn set_top_scalar(",
        "pub enum YamlEditError",
        "pub mod rules_fidelity;",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/yaml_edit/rules_fidelity.rs",
        "pub fn apply_rule_list(",
        "fn match_order_preserving(",
        "fn retarget_item_line(",
        "load_rules_from_yaml(&rendered)",
        "rule-list splice failed the structural verification",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/yaml_edit/mixin_fidelity.rs",
        "pub fn can_apply_mixin_via_fidelity(",
        "pub fn apply_mixin_to_doc(",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/yaml_edit/anchor.rs",
        "pub fn scan_anchors_and_aliases(",
        "pub fn rewrite_anchor_namespace(",
    )
    # The rule writer tries fidelity first and keeps serde as the fallback.
    require(
        violations,
        "crates/infiltrator-domain/src/rules.rs",
        "crate::yaml_edit::rules_fidelity::apply_rule_list(&mut doc, rules).is_ok()",
        "serde_yaml_ng::to_string(&doc).context(\"serialize profile yaml\")",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/mixin.rs",
        "pub fn merge_profile_with_config_fidelity(",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/profile_options.rs",
        "crate::mixin::merge_profile_with_config_fidelity(&current, &options.mixin)?",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/profile/options.rs",
        "infiltrator_domain::mixin::merge_profile_with_config_fidelity(",
    )

    # 3. The shared diff read model and its process-wide publisher.
    require(
        violations,
        "crates/infiltrator-contract/src/yaml_ast_diff.rs",
        "pub struct YamlAstDiffSnapshot",
        "pub source_path: Option<String>",
        "pub fn with_source_path(",
        "pub fn change_summary(",
        "pub struct SplitDiffRow",
        "pub enum FidelityGrade",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/myers_diff.rs",
        "pub fn compute_diff(",
    )
    require(
        violations,
        "crates/infiltrator-application/src/snapshot_application.rs",
        "pub fn last_snapshot_diff()",
        "pub fn publish_snapshot_diff(",
        "pub fn clear_snapshot_diff()",
        "pub async fn diff_snapshot(",
        "pub async fn diff_newest(",
        ".with_source_path(snapshot_path.to_string_lossy().to_string())",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "yaml_ast_diff: crate::snapshot_application::last_snapshot_diff()",
        "ProfileWriteProtection::from_subscription_url(",
    )

    # 4. The shared protection model, its contract intent and its guard.
    require(
        violations,
        "crates/infiltrator-contract/src/profile_protection.rs",
        "pub enum ProfileWriteProtection",
        "pub fn from_subscription_url(",
        "pub const fn label_zh(",
        "pub const fn hint_zh(",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub write_protection: crate::profile_protection::ProfileWriteProtection",
        "pub yaml_ast_diff: Option<crate::yaml_ast_diff::YamlAstDiffSnapshot>",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "LoadSnapshotDiff {",
    )
    require(
        violations,
        "crates/infiltrator-application/src/profile_application.rs",
        "pub async fn write_protection(",
        "pub async fn save_edited_profile_content<",
        "allow_protected: bool",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "CommandIntent::LoadSnapshotDiff { snapshot_id }",
        "diff_newest(&profile).await",
    )
    require(
        violations,
        "crates/infiltrator-application/src/core_application.rs",
        "CommandIntent::LoadSnapshotDiff { .. } => \"load_snapshot_diff\"",
    )

    # 5. Iced renders the shared diff and gates protected direct edits.
    require(
        violations,
        "crates/infiltrator-iced/src/view_root/snapshot_diff_modal.rs",
        "use infiltrator_contract::yaml_ast_diff::{DiffKind, DiffLine, SplitDiffRow, YamlAstDiffSnapshot}",
        "diff.change_summary()",
        "Message::SetSnapshotDiffMode(",
        "Message::ArmSnapshotRollback",
        "fn unified_row(",
        "fn split_row(",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/view_root/snapshot_diff_modal.rs",
        "mock_diff_rows",
        "+ Added",
        "- Removed",
        "~ Modified",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/snapshot_diff.rs",
        "pub(super) fn update_snapshot_diff(",
        "Message::SnapshotDiffLoaded",
        "Message::RollbackToSnapshot",
        "diff_snapshot(&profile, std::path::Path::new(&id))",
        "self.editor.pending_restore_snapshot = Some(id.clone().into())",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/profile/editor.rs",
        "Message::ArmRestoreProfileSnapshot(path)",
        "Message::CancelRestoreProfileSnapshot",
        "pending_restore_snapshot.as_deref() != Some(path.as_path())",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/view_snapshot_diff_tests.rs",
        "history_panel_restore_is_armed_before_it_executes",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/editor.rs",
        "editor_protection_use_mixin",
        "Message::SetProfileProtectionOverride(",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/editor_history.rs",
        "Message::OpenSnapshotDiff(",
        "Message::ArmRestoreProfileSnapshot(",
        "Message::RestoreProfileSnapshot(",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "pub snapshot_diff: Option<infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot>",
        "pub snapshot_diff_rollback_armed: bool",
        "pub profile_protection_override: bool",
        "pub fn edited_profile_write_protection(",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/profile_apply.rs",
        "pub(crate) async fn save_edited_profile_content(",
        "save_edited_profile_content(runtime, profile, content, strategy, allow_protected)",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/profile/editor.rs",
        "save_edited_profile_content(",
        "allow_protected",
    )

    # 6. Bevy renders the shared diff, confirms rollback and shows protection.
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/profiles_diff.rs",
        "pub struct SnapshotDiffBody",
        "pub struct SnapshotDiffModeButton",
        "pub struct RefreshSnapshotDiffButton",
        "pub struct RollbackSnapshotLabel",
        "pub struct SnapshotDiffViewState",
        "pub fn snapshot_diff_scene(",
        "pub fn diff_summary(",
        "pub fn rollback_target(",
        "pub(super) fn sync_snapshot_diff(",
        "pub(super) fn on_snapshot_diff_mode_activated(",
        "pub(super) fn on_refresh_snapshot_diff(",
        "pub(super) fn on_rollback_snapshot_activated(",
        "UiCommand::LoadSnapshotDiff { snapshot_id: None }",
        "UiCommand::RestoreSnapshot { id: path }",
        "projection.yaml_ast_diff.as_ref()",
    )
    forbid(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/profiles_diff.rs",
        "dns.fallback-filter.geoip-code: CN -> US",
        "let diff_items = vec![",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/profiles.rs",
        "pub struct ProfileProtectionText",
        "pub yaml_ast_diff: Option<infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot>",
        "pub write_protection: infiltrator_contract::profile_protection::ProfileWriteProtection",
        "crate::pages::profiles_diff::snapshot_diff_scene(projection, palette)",
        "crate::pages::profiles_diff::sync_snapshot_diff",
        "crate::pages::profiles_diff::on_rollback_snapshot_activated",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "LoadSnapshotDiff { snapshot_id: Option<String> }",
        "CommandIntent::LoadSnapshotDiff",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "yaml_ast_diff: snapshot.yaml_ast_diff.clone()",
        "write_protection: profile.write_protection",
    )

    # 7. Dual headless evidence.
    require(
        violations,
        "crates/infiltrator-domain/src/yaml_edit_test.rs",
        "rule_list_edit_preserves_comments_and_inline_notes",
        "rule_list_edit_removes_and_reorders_without_touching_other_lines",
        "apply_rules_to_yaml_keeps_comments_for_add_and_remove",
    )
    require(
        violations,
        "crates/infiltrator-application/src/snapshot_application_test.rs",
        "diff_newest_publishes_the_real_diff_and_its_snapshot_path",
        "restore_clears_the_cached_diff",
    )
    require(
        violations,
        "crates/infiltrator-application/src/profile_application_tests.rs",
        "edited_writes_refuse_protected_subscriptions_until_unlocked",
        "write_protection_is_derived_from_the_subscription_source",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application_tests.rs",
        "rule_commands_keep_handwritten_comments_end_to_end",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/view_snapshot_diff_tests.rs",
        "modal_renders_the_shared_diff_in_inline_and_split_modes",
        "rollback_never_executes_before_the_second_confirmation",
        "editor_protection_follows_the_shared_subscription_metadata",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/rules_dns_tests.rs",
        "test_rule_save_path_preserves_comments_through_the_shared_fidelity_writer",
        "test_mixin_save_path_preserves_comments_through_the_shared_fidelity_writer",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_profiles_snapshot_diff_renders_shared_rows_and_confirms_rollback",
        "test_profiles_snapshot_diff_states_are_honest_without_a_diff",
    )

    # 8. One AST-preserving formatter, shared by both surfaces.
    require(
        violations,
        "crates/infiltrator-domain/src/yaml_edit/format.rs",
        "pub fn format_yaml(",
        "pub enum FormatSkipReason",
        "pub const CLASH_TOP_LEVEL_ORDER",
        "fn verify_structure_preserved(",
        "fn structure_signature(",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/yaml_edit.rs",
        "pub mod format;",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/ui.rs",
        "infiltrator_domain::yaml_edit::format::format_yaml(",
        "FormatSkipReason::AnchorsPresent",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/update/ui.rs",
        "serde_yaml_ng::to_string(&val)",
        "serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&text)",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/profiles_editor.rs",
        "infiltrator_bevy_widgets::editor::{CodeEditorState, SyntaxTokenKind, tokenize_yaml_line}",
        "pub fn profile_editor_scene(",
        "pub fn profile_editor_keyboard_input(",
        "UiCommand::SaveProfileDocument",
        "UiCommand::LoadProfileDocument",
        "pub struct ProfilesEditorPlugin",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/profiles_editor_state.rs",
        "infiltrator_domain::yaml_edit::format::format_yaml",
        "infiltrator_domain::config::preflight_yaml_syntax",
        "pub const PROFILE_EDITOR_RENDER_LIMIT",
    )

    # 9. One snapshot history read model, one prune policy, both ends.
    require(
        violations,
        "crates/infiltrator-contract/src/snapshot_history.rs",
        "pub struct SnapshotEntry",
        "pub struct SnapshotHistorySnapshot",
        "pub pending_prune: usize",
        "pub duplicate_entries: usize",
        "pub enum SnapshotPruneSource",
        "pub const SNAPSHOT_DEFAULT_KEEP",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/backup.rs",
        "pub fn prune_snapshots(",
    )
    require(
        violations,
        "crates/infiltrator-core/src/history.rs",
        "infiltrator_domain::backup::prune_snapshots(&snapshots, keep)",
        "prune_deduplicates_identical_content_before_the_lru_cut",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/snapshot_store.rs",
        "async fn delete(&self, profile: &str, path: &Path)",
    )
    require(
        violations,
        "crates/infiltrator-application/src/snapshot_application.rs",
        "pub fn last_snapshot_history()",
        "pub fn publish_snapshot_history(",
        "pub async fn history(",
        "pub async fn prune(",
        "infiltrator_domain::backup::prune_snapshots(&snapshots, keep)",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "CommandIntent::LoadSnapshotHistory",
        "CommandIntent::PruneSnapshots { keep }",
        "SnapshotPruneSource::Manual",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "snapshot_history: crate::snapshot_application::last_snapshot_history()",
        "apply_transaction:",
        "profile_document:",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/profile/editor.rs",
        ".history(",
        "Message::BackupProfileSnapshot",
        "Message::PruneProfileSnapshots",
        "SnapshotPruneSource::Manual",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/editor_history.rs",
        "editor_backup_now",
        "editor_prune_now",
        "editor_prune_pending",
        "editor_apply_stage_rolled_back",
        "Message::SetSnapshotPruneKeep(",
        "Message::PruneProfileSnapshots",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/editor.rs",
        "super::editor_history::history_panel(",
        "super::editor_history::apply_banner(",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table.rs",
        "editor_backup_now",
        "editor_prune_now",
        "editor_prune_pending",
        "editor_prune_duplicates",
        "editor_prune_last",
        "editor_apply_stage_committed",
        "editor_apply_stage_rolled_back",
        "editor_apply_stage_rollback_failed",
        "yaml_format_skipped_anchors",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en.rs",
        "editor_backup_now",
        "editor_prune_now",
        "editor_prune_pending",
        "editor_prune_duplicates",
        "editor_prune_last",
        "editor_apply_stage_committed",
        "editor_apply_stage_rolled_back",
        "editor_apply_stage_rollback_failed",
        "yaml_format_skipped_anchors",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/profiles_diff.rs",
        "pub fn snapshot_history_summary(",
        "super::profiles_diff_history::history_rows_scene(",
        "super::profiles_diff_history::{SnapshotHistoryBody, SnapshotHistorySummaryText}",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/profiles_diff_history.rs",
        "pub struct BackupSnapshotButton",
        "pub struct RefreshSnapshotHistoryButton",
        "pub struct SnapshotPruneKeepButton",
        "pub struct PruneSnapshotsButton",
        "pub struct SnapshotHistoryEntryButton",
        "pub struct SnapshotHistoryBody",
        "pub struct SnapshotHistorySummaryText",
        "UiCommand::CreateBackupSnapshot",
        "UiCommand::LoadSnapshotHistory",
        "UiCommand::PruneSnapshots {",
        "snapshot_id: Some(button.id.clone())",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/profiles_editor_state.rs",
        "pub struct ProfileEditorState",
        "pub fn refresh_preflight(",
        "pub fn format(",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/profiles_editor.rs",
        "pub fn profile_editor_scene(",
        "pub struct ProfileEditorDiagnosticText",
        "pub struct ProfileEditorSaveButton",
        "pub struct ProfileEditorProtectionToggle",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/profiles.rs",
        "crate::pages::profiles_editor::profile_editor_scene(projection, palette)",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/app.rs",
        "crate::pages::profiles_editor::ProfilesEditorPlugin",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages.rs",
        "pub mod profiles_editor;",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "LoadSnapshotHistory,",
        "PruneSnapshots { keep: Option<usize> },",
        "LoadProfileDocument { profile: Option<String> },",
        "SaveProfileDocument {",
    )

    # 10. One typed apply-transaction outcome published by the core.
    require(
        violations,
        "crates/infiltrator-contract/src/apply_transaction.rs",
        "pub enum ApplyTransactionStage",
        "RolledBack",
        "RollbackFailed",
        "pub struct ApplyTransactionSnapshot",
        "pub fn record_apply_transaction(",
        "pub fn last_apply_transaction(",
    )
    require(
        violations,
        "crates/infiltrator-core/src/apply.rs",
        "ApplyTransactionSnapshot::committed(",
        "ApplyTransactionSnapshot::rolled_back(",
        "ApplyTransactionSnapshot::rollback_failed(",
        "infiltrator_contract::apply_transaction::record_apply_transaction(",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/profile_document.rs",
        "pub struct ProfileDocumentSnapshot",
        "pub struct SyntaxDiagnosticSnapshot",
        "pub fn publish_profile_document(",
        "pub fn last_profile_document(",
    )
    require(
        violations,
        "crates/infiltrator-application/src/profile_document_application.rs",
        "pub async fn load(",
        "pub async fn save<",
        "preflight_yaml_syntax(content)",
        "save_edited_profile_content(",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "LoadSnapshotHistory,",
        "PruneSnapshots {",
        "LoadProfileDocument {",
        "SaveProfileDocument {",
    )

    # 11. Dual headless evidence for the second batch.
    require(
        violations,
        "crates/infiltrator-domain/src/yaml_edit_test.rs",
        "format_keeps_every_comment_anchor_and_scalar_style",
        "format_orders_top_level_keys_and_keeps_blocks_together",
        "format_is_idempotent_and_reports_no_change_on_second_pass",
        "format_keeps_block_scalar_bodies_verbatim",
        "format_refuses_documents_that_do_not_parse",
    )
    require(
        violations,
        "crates/infiltrator-application/src/snapshot_application_test.rs",
        "history_reports_duplicates_and_the_shared_prune_view",
        "prune_executes_the_shared_policy_and_publishes_the_report",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application_tests.rs",
        "load_and_save_profile_document_round_trips_through_the_shared_guard",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/view_editor_tests.rs",
        "format_yaml_editor_keeps_comments_through_the_shared_engine",
        "format_yaml_editor_refuses_an_invalid_buffer_without_rewriting_it",
        "snapshot_history_state_follows_the_shared_prune_view_and_renders_it",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_profiles_snapshot_history_lists_prunes_and_backs_up_through_the_shared_app",
        "test_profiles_editor_runs_the_shared_preflight_and_formatter",
        "test_profiles_editor_formats_with_the_shared_engine_and_saves_through_the_guard",
    )

    # 8. Both test entrypoints register this guard.
    require(
        violations,
        "scripts/test.sh",
        "yaml-diff-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        'yaml-diff-guard.py" --mode enforce',
    )

    if violations:
        for violation in violations:
            print(f"yaml-diff-guard: {violation}", file=sys.stderr)
        print(f"yaml-diff-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("yaml-diff-guard: DUAL-09 ledger_rows=15 parity_items=11 violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

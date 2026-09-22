#!/usr/bin/env python3
"""Fail-closed guard for the group 15 multimodal-shell ledger (DUAL-15-01..15).

Group 15's closure standard matches groups 06/12/14: one shared
contract/application source, both surfaces, dual headless tests, and an honest
per-item ledger. This guard asserts the per-item ledger rows exist, that the
shared theme/shortcut/notification contracts and both surface wirings stay
present, that the widget-layer skin mirror cannot drift from the contract
vocabulary, and that the fixed defects do not regress (hard-coded two-way
theme flip, local hotkey list, index-based toast removal, zero-capacity toast
queue).
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]

LEDGER = "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md"
RESPONSIVE_LEDGER = "docs/RESPONSIVE_PARITY_LEDGER.md"


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


def skin_settings(path: str) -> set[str]:
    """Extract the quoted skin setting names from one `as_setting` arm."""
    text = read(path)
    match = re.search(
        r"pub const fn as_setting\(self\)[^{]*\{(?P<body>.*?)\n    \}",
        text,
        re.DOTALL,
    )
    if not match:
        return set()
    return set(re.findall(r'=>\s*"([a-z]+)"', match.group("body")))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    violations: list[str] = []

    # The full per-item ledger must exist for all fifteen items, with the
    # status vocabulary the rest of the master plan uses.
    require(
        violations,
        LEDGER,
        "DUAL-15-01",
        "DUAL-15-02",
        "DUAL-15-03",
        "DUAL-15-04",
        "DUAL-15-05",
        "DUAL-15-06",
        "DUAL-15-07",
        "DUAL-15-08",
        "DUAL-15-09",
        "DUAL-15-10",
        "DUAL-15-11",
        "DUAL-15-12",
        "DUAL-15-13",
        "DUAL-15-14",
        "DUAL-15-15",
        "parity-ready",
        "shared-ready",
        "planned",
        "组 15 逐项账目",
        "2026-09-22 组 15 批次 A",
        "组 15 多模态外壳与极客命令流 | 15 | `in progress`",
        # 15-01 stays authoritatively tracked in the responsive ledger.
        "RESPONSIVE_PARITY_LEDGER.md",
    )
    require(violations, RESPONSIVE_LEDGER, "DUAL-15-01", "parity-ready")

    # Closed items carry their distinctive evidence tokens in the ledger.
    require(
        violations,
        LEDGER,
        "ShortcutRegistry",
        "ShortcutChord",
        "ThemePreference",
        "ThemeSkin",
        "ToastGate",
        "ToastPolicy",
        "push_toast",
        "sync_system_appearance",
        "a_sensitive_toast_is_redacted_before_it_renders",
        "identical_toasts_are_coalesced_and_the_stack_is_capped",
        "widget_skin_mirror_matches_the_shared_contract",
        "the_keyboard_chord_dispatches_through_the_shared_registry",
        "the_open_palette_owns_the_arrow_keys",
        "capture_rebinds_through_the_shared_settings_command",
        "a_stored_custom_chord_blocks_later_captures",
    )

    # Shared contracts: appearance, shortcuts, notifications.
    require(
        violations,
        "crates/infiltrator-contract/src/theme.rs",
        "pub enum ThemeSkin",
        "pub enum ThemePreference",
        "pub const fn resolve",
        "pub fn parse_strict",
        "pub const fn next",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/shortcuts.rs",
        "pub enum ShortcutAction",
        "pub struct ShortcutChord",
        "pub struct ShortcutRegistry",
        "pub fn display_string",
        "pub fn find_conflict",
        "pub fn bind_or_replace",
        "pub fn detect_conflicts",
        "pub fn normalize",
        "pub fn reset_action",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/toast.rs",
        "pub enum ToastSeverity",
        "pub struct ToastPolicy",
        "pub struct ToastGate",
        "pub fn admit",
        "pub fn live_offers",
    )

    # Application layer: shared shortcut use-cases + validated settings writes.
    require(
        violations,
        "crates/infiltrator-application/src/shortcut_application.rs",
        "pub struct ShortcutApplication",
        "pub async fn registry",
        "pub async fn capture",
        "pub async fn set_enabled",
        "pub async fn reset_action",
        "settings.shortcuts",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/settings.rs",
        "pub shortcuts: Vec<infiltrator_contract::shortcuts::ShortcutBinding>",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        'strip_prefix("shortcut.")',
        "ShortcutChord::parse",
        "ThemePreference::parse_strict",
    )

    # Iced surface: appearance follow, registry-driven hotkeys, toast policy.
    require(
        violations,
        "crates/infiltrator-iced/src/update/shell.rs",
        "pub fn push_toast",
        "fn handle_keyboard_chord",
        "SystemThemeChanged",
        "toast_gate.admit",
        "Message::RemoveToast",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/shortcuts_store.rs",
        "pub async fn capture",
        "pub async fn set_enabled",
        "pub async fn reset_action",
        "ShortcutApplication::new",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/settings.rs",
        "hotkeys_card",
        "shortcut_registry.get(action)",
        "Message::BeginHotkeyCapture",
        "Message::ResetHotkey",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/theme.rs",
        "pub fn theme_for_skin",
        "ThemeSkin::from_setting",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/toast_state.rs",
        "infiltrator_contract::toast",
        "ToastGate",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/subscription.rs",
        "iced::system::theme_changes()",
        "Message::KeyboardChord",
        "Message::SystemThemeChanged",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update.rs",
        "| Message::SetTheme(_)",
        "| Message::KeyboardChord { .. }",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "pub shortcut_registry",
        "pub theme_preference",
        "pub toast_gate",
        "pub fn apply_theme_preference",
    )

    # Bevy surface: shared registry dispatch, OS appearance follow, toast overlay.
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/shortcuts.rs",
        "pub struct ShortcutBindings",
        "pub struct HotkeyCapture",
        "pub struct ChordPressed",
        "pub fn forward_pressed_chords",
        "pub fn dispatch_chords",
        "ShortcutRegistry",
        "UiCommand::UpdateSetting",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/toast.rs",
        "pub struct ToastPolicyGate",
        "pub struct ShellToast",
        "pub fn on_shell_toast",
        "pub fn sync_toast_stack",
        "redact::redact_line",
        "toast_stack_scene",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/appearance.rs",
        "pub struct ThemeMode",
        "pub struct SystemAppearance",
        "pub fn resolved_skin",
        "pub fn sync_system_appearance",
        "window_theme",
    )
    require(
        violations,
        "crates/infiltrator-bevy-widgets/src/theme.rs",
        "pub enum ThemeSkin",
        "pub fn forest",
        "pub fn amoled",
    )
    require(
        violations,
        "crates/infiltrator-bevy-widgets/src/toast.rs",
        "DEFAULT_TOAST_CAPACITY",
        "impl Default for ToastQueue",
    )

    # Dual headless tests: one evidence test per closed item on each surface.
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/multimodal_shell_tests.rs",
        "the_shell_follows_the_os_appearance_while_the_preference_is_system",
        "a_system_preference_survives_the_settings_round_trip",
        "the_keyboard_chord_dispatches_through_the_shared_registry",
        "the_open_palette_owns_the_arrow_keys",
        "identical_toasts_are_coalesced_and_the_stack_is_capped",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/theme_skin_tests.rs",
        "widget_skin_mirror_matches_the_shared_contract",
        "the_shell_follows_the_os_appearance_while_preference_is_system",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/shortcut_tests.rs",
        "capture_rebinds_through_the_shared_settings_command",
        "a_bound_chord_reaches_the_command_sink",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/toast_overlay_tests.rs",
        "a_sensitive_toast_is_redacted_before_it_renders",
        "the_overlay_mounts_a_single_stack_root",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application_settings_tests.rs",
        "shortcut_capture_persists_and_rejects_conflicts",
        "theme_writes_are_validated_and_canonicalised",
    )

    # ---- Batch B (DUAL-15-03/04/05): command palette catalogue and Mini HUD.
    # The ledger records the batch and its evidence tokens.
    require(
        violations,
        LEDGER,
        "2026-09-22 组 15 批次 B",
        "CommandCatalogue",
        "CommandTarget",
        "ShellPage",
        "MiniHudReadModel",
        "MiniHudPlacement",
        "MiniHudWindowPort",
        "MiniHudApplication",
        "the_palette_lists_the_shared_catalogue_and_wraps_like_bevy",
        "the_palette_executes_shared_targets",
        "test_palette_keyboard_navigation_typing_and_close",
        "test_palette_row_click_executes_that_row",
        "the_mini_hud_read_model_comes_from_live_projections",
        "the_mini_hud_drag_moves_the_persisted_placement",
        "the_mounted_scene_renders_the_shared_read_model",
        "the_pin_request_persists_through_the_shared_settings_command",
        "place_snaps_clamps_persists_and_reports_the_host_outcome",
        "a_host_without_the_window_adapter_reports_typed_unsupported",
        "mini_hud_placement_writes_are_validated_per_field",
    )

    # Shared contracts: the palette catalogue and the Mini HUD read model.
    require(
        violations,
        "crates/infiltrator-contract/src/command_catalogue.rs",
        "pub enum ShellPage",
        "pub enum CommandCategory",
        "pub enum CommandTarget",
        "pub struct CommandCatalogue",
        "pub fn with_profiles",
        "pub fn filtered_indices",
        "pub const fn label_zh",
        "pub const fn shortcut_action",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/mini_hud.rs",
        "pub struct MiniHudPlacement",
        "pub struct MiniHudDisplay",
        "pub struct MiniHudReadModel",
        "pub struct MiniHudSnapPlacement",
        "pub fn snap_to_edges",
        "pub fn status_line",
        "pub enum MiniHudHostOutcome",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub mini_hud: crate::mini_hud::MiniHudPlacement",
    )

    # Application + ports: placement use-cases and the host window capability.
    require(
        violations,
        "crates/infiltrator-application/src/mini_hud_application.rs",
        "pub struct MiniHudApplication",
        "pub async fn place_from",
        "pub async fn set_pinned_from",
        "pub struct MiniHudPlacementReport",
        "MINI_HUD_SNAP_THRESHOLD_PX",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/mini_hud_window.rs",
        "pub trait MiniHudWindowPort",
        "async fn apply_placement",
        "async fn set_visible",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/host_runtime.rs",
        "fn mini_hud_window_port",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/settings.rs",
        "pub mini_hud: infiltrator_contract::mini_hud::MiniHudPlacement",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        'strip_prefix("mini_hud.")',
        "placement.pinned = value.parse",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "mini_hud: settings.mini_hud",
    )

    # Iced surface: catalogue-driven palette and the real HUD window mode.
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "pub command_catalogue",
        "pub mini_hud_placement",
        "pub fn rebuild_command_catalogue",
        "pub fn filtered_command_indices",
        "pub fn mini_hud_read_model",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view_root/command_palette.rs",
        "infiltrator_contract::command_catalogue",
        "state.filtered_command_indices()",
        "shortcut_action()",
        "command_category_badge",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/ui.rs",
        "update_mini_hud",
        "self.filtered_command_indices()",
        "on_shell_shortcut(action)",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/mini_hud.rs",
        "update_mini_hud",
        "mini_hud_store::place",
        "mini_hud_store::set_pinned",
        "mini_hud_window::enter",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/mini_hud_window.rs",
        "iced::window::move_to",
        "iced::window::set_level",
        "Level::AlwaysOnTop",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/mini_hud_store.rs",
        "MiniHudApplication::new",
        "place_from",
        "set_pinned_from",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/mini_hud.rs",
        "mini_hud_read_model",
        "Message::MiniHudMoved",
        "Message::MiniHudDragReleased",
        "mouse_area",
    )

    # Bevy surface: mounted palette overlay + keyboard seam, mounted HUD model.
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command_palette_shell.rs",
        "pub struct CommandPalettePlugin",
        "pub fn palette_keyboard_input",
        "pub fn sync_palette_overlay",
        "pub fn sync_palette_catalogue",
        "pub fn on_execute_selected_palette_action",
        "Route::from_shell_page",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command_palette.rs",
        "pub catalogue: CommandCatalogue",
        "pub fn from_catalogue",
        "command_palette_modal_scene",
        "pinyin_fuzzy_match",
        "pub fn accelerator_for",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/mini_hud_shell.rs",
        "pub struct MiniHudPlugin",
        "pub fn sync_mini_hud_model",
        "pub fn sync_mini_hud_overlay",
        '"mini_hud.pinned"',
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/mini_hud.rs",
        "MiniHudReadModel",
        "pub fn mini_hud_scene",
        "status_line",
        "pub struct ToggleMiniHud",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/shortcuts.rs",
        "crate::mini_hud::ToggleMiniHud",
    )

    # Dual headless tests for this batch.
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/multimodal_shell_tests.rs",
        "the_palette_lists_the_shared_catalogue_and_wraps_like_bevy",
        "the_palette_executes_shared_targets",
        "the_mini_hud_read_model_comes_from_live_projections",
        "the_mini_hud_drag_moves_the_persisted_placement",
        "always_on_top_mirrors_the_pin_onto_the_shared_placement",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/command_palette_tests.rs",
        "test_palette_mounts_and_unmounts_from_the_shared_catalogue",
        "test_palette_keyboard_navigation_typing_and_close",
        "test_palette_row_click_executes_that_row",
        "test_command_palette_action_execution_dispatches_route_and_command",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/mini_hud_tests.rs",
        "the_toggle_event_mounts_and_unmounts_the_overlay",
        "the_read_model_comes_from_the_live_projections",
        "the_mounted_scene_renders_the_shared_read_model",
        "the_pin_request_persists_through_the_shared_settings_command",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application_settings_tests.rs",
        "mini_hud_placement_writes_are_validated_per_field",
    )
    require(
        violations,
        "crates/infiltrator-application/src/mini_hud_application.rs",
        "place_snaps_clamps_persists_and_reports_the_host_outcome",
        "a_host_without_the_window_adapter_reports_typed_unsupported",
    )

    # The mirrored skin vocabulary must match the contract numerically.
    contract_skins = skin_settings("crates/infiltrator-contract/src/theme.rs")
    widget_skins = skin_settings("crates/infiltrator-bevy-widgets/src/theme.rs")
    if contract_skins != widget_skins or not contract_skins:
        violations.append(
            "skin setting vocabulary drift: contract "
            f"{sorted(contract_skins)} != widget {sorted(widget_skins)}"
        )

    # Fixed defects must not regress.
    forbid(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "hotkeys_config",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "UpdateHotkeyCombo",
        "RemoveToast(usize)",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/toast_state.rs",
        "elapsed <= 2000",
        "max_visible: usize,\n    active_toasts",
    )
    forbid(
        violations,
        "crates/infiltrator-bevy-ui/src/appearance.rs",
        "ThemeSkin::Light => ThemeSkin::Dark",
    )
    forbid(
        violations,
        "crates/infiltrator-bevy-ui/src/command_palette.rs",
        "ThemeSkin::Light => ThemeSkin::Dark",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/view/theme.rs",
        'value.trim().to_ascii_lowercase().as_str() {\n        "forest"',
    )
    # Batch B regressions: the HUD must stay read-model driven, the palette
    # must stay catalogue driven, and the Iced command model must stay shared.
    forbid(
        violations,
        "crates/infiltrator-bevy-ui/src/mini_hud.rs",
        "系统代理: 开启",
        'Text({ "RULE".to_owned() })',
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/types/app.rs",
        "pub enum CommandAction",
        "pub struct CommandItem",
    )
    forbid(
        violations,
        "crates/infiltrator-bevy-ui/src/command_palette.rs",
        "Vec<PaletteAction>",
        "pub enum PaletteCategory",
        "pub struct PaletteAction",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "CommandAction",
    )

    if args.mode == "enforce" and violations:
        print("multimodal-shell guard: FAIL", file=sys.stdout)
        for violation in violations:
            print(f"  - {violation}")
        return 1
    print(
        "multimodal-shell guard: "
        f"ledger_rows=15 skins={sorted(contract_skins)} violations={len(violations)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

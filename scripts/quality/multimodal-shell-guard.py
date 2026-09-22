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

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
MATRIX = "docs/MULTIMODAL_SHELL_MATRIX.md"
DESIGN_TOKENS = "crates/infiltrator-contract/src/design_tokens.rs"
ICED_THEME = "crates/infiltrator-iced/src/view/theme.rs"
BEVY_THEME = "crates/infiltrator-bevy-widgets/src/theme.rs"

#: Contract core-palette field -> the Bevy widget mirror's field name.
BEVY_CORE_FIELDS = {
    "canvas": "window_bg",
    "sidebar": "sidebar",
    "card": "surface",
    "card_border": "border",
    "control_bg": "surface_elevated",
    "ink": "ink",
    "ink_dim": "ink_dim",
    "accent": "accent",
    "on_accent": "on_accent",
    "success": "success",
    "warning": "warning",
    "danger": "danger",
}

#: Contract core-palette field -> the Iced shell token field it resolves into.
ICED_CORE_FIELDS = {
    "canvas": "canvas",
    "sidebar": "sidebar",
    "card": "card_bg",
    "card_border": "card_border",
    "control_bg": "control_bg",
    "ink": "text_primary",
    "ink_dim": "text_secondary",
    "accent": "accent",
    "on_accent": "on_accent",
    "success": "success",
    "warning": "warning",
    "danger": "danger",
}

SKINS = ("Dark", "Light", "Forest", "Amoled")


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


def color_fields(body: str) -> dict[str, tuple[float, ...]]:
    """Extract `name: Ctor(0.1, 0.2, 0.3[, 0.4])` token tuples."""
    fields: dict[str, tuple[float, ...]] = {}
    for match in re.finditer(
        r"(\w+):\s*\w+::rgba?\(([^)]*)\)", body, re.DOTALL
    ):
        fields[match.group(1)] = tuple(
            float(value.strip()) for value in match.group(2).split(",")
        )
    return fields


def contract_cores() -> dict[str, dict[str, tuple[float, ...]]]:
    """The authoritative per-skin core palette from the shared contract."""
    text = read(DESIGN_TOKENS)
    cores: dict[str, dict[str, tuple[float, ...]]] = {}
    for skin in SKINS:
        match = re.search(
            rf"ThemeSkin::{skin} => SkinCorePalette \{{(?P<body>.*?)\n        \}},",
            text,
            re.DOTALL,
        )
        if match:
            cores[skin] = color_fields(match.group("body"))
    return cores


def bevy_cores() -> dict[str, dict[str, tuple[float, ...]]]:
    """The mirrored per-skin palettes from the Bevy widget layer."""
    text = read(BEVY_THEME)
    cores: dict[str, dict[str, tuple[float, ...]]] = {}
    for skin in SKINS:
        match = re.search(
            rf"pub fn {skin.lower()}\(\) -> Self \{{(?P<body>.*?)\n    \}}",
            text,
            re.DOTALL,
        )
        if match:
            cores[skin] = color_fields(match.group("body"))
    return cores


def number_const(text: str, name: str) -> float | None:
    match = re.search(
        rf"pub const {name}\s*:\s*f32\s*=\s*([0-9]+(?:\.[0-9]+)?)\s*;", text
    )
    return float(match.group(1)) if match else None


def check_matrix(violations: list[str]) -> None:
    """Verify the group 15 regression matrix against the real test sources.

    This is what makes `DUAL-15-15` a machine-checkable claim instead of a doc
    promise: every `path::test_name` evidence token must point at an existing
    file that really contains that test, closed items must carry evidence from
    both surfaces plus the shared/application source, planned items must not
    claim any, and every matrix status must equal the authoritative per-item
    status in the master plan.
    """
    text = read(MATRIX)
    evidence = re.compile(r"`(crates/[^`\s]+?\.rs)::([A-Za-z0-9_]+)`")
    rows: dict[str, str] = {}
    for line in text.splitlines():
        if not line.startswith("| `DUAL-15-"):
            continue
        item = re.match(r"\| `(DUAL-15-\d+)` \|", line)
        if not item:
            continue
        rows[item.group(1)] = line

    ledger = ledger_statuses()
    shared_roots = (
        "crates/infiltrator-contract/",
        "crates/infiltrator-application/",
        "crates/infiltrator-ports/",
        "crates/infiltrator-domain/",
        "crates/infiltrator-desktop/",
    )

    expected = [f"DUAL-15-{index:02d}" for index in range(1, 16)]
    for item in expected:
        if item not in rows:
            violations.append(f"{MATRIX} missing matrix row for {item}")
            continue
        line = rows[item]
        status_match = re.match(r"\| `DUAL-15-\d+` \| (?P<status>[^|]+)\|", line)
        status_cell = status_match.group("status").strip() if status_match else ""
        status = next(
            (
                word
                for word in ("parity-ready", "shared-ready", "planned")
                if word in status_cell
            ),
            None,
        )
        if status is None:
            violations.append(f"{MATRIX} {item} has no known status: {status_cell!r}")
        ledger_status = ledger.get(item)
        if ledger_status is None:
            violations.append(f"{LEDGER} has no group 15 item row for {item}")
        elif status is not None and ledger_status != status:
            violations.append(
                f"{MATRIX} {item} says {status!r} but {LEDGER} says {ledger_status!r}"
            )
        tokens = evidence.findall(line)
        for path, name in tokens:
            try:
                source = read(path)
            except FileNotFoundError:
                violations.append(f"{MATRIX} {item} cites missing file {path}")
                continue
            if name not in source:
                violations.append(
                    f"{MATRIX} {item} cites {path}::{name} but the test is absent"
                )
        if status == "planned":
            if tokens:
                violations.append(
                    f"{MATRIX} {item} is planned but claims evidence {tokens}"
                )
            continue
        if "外链" in status_cell:
            continue
        surfaces = {
            "iced": any("/infiltrator-iced/" in path for path, _ in tokens),
            "bevy": any("/infiltrator-bevy-ui/" in path for path, _ in tokens),
        }
        if not all(surfaces.values()):
            violations.append(
                f"{MATRIX} {item} is {status_cell} but lacks dual-surface evidence: {surfaces}"
            )
        shared_evidence = [
            (path, name)
            for path, name in tokens
            if path.startswith(shared_roots)
        ]
        if not shared_evidence:
            violations.append(
                f"{MATRIX} {item} claims {status!r} without shared/application evidence"
            )
        if len(tokens) < 3:
            violations.append(
                f"{MATRIX} {item} claims {status!r} with only {len(tokens)} evidence "
                "tokens; shared + Iced + Bevy are the minimum"
            )


def ledger_statuses() -> dict[str, str]:
    """The authoritative per-item group 15 statuses from the master plan.

    The first `DUAL-15-XX | 任务 | 状态 | 证据` row of an item wins, so the
    per-item ledger table stays authoritative over any later batch notes.
    """
    text = read(LEDGER)
    statuses: dict[str, str] = {}
    for line in text.splitlines():
        match = re.match(
            r"\| `(DUAL-15-\d+)` \| [^|]+ \| (?P<status>[^|]+) \|", line
        )
        if not match:
            continue
        item = match.group(1)
        if item in statuses:
            continue
        status = next(
            (
                word
                for word in ("parity-ready", "shared-ready", "planned")
                if word in match.group("status")
            ),
            None,
        )
        if status is not None:
            statuses[item] = status
    return statuses


def check_design_token_mirrors(violations: list[str]) -> None:
    """Numeric drift check across the shared contract, Iced and Bevy.

    This is the real 15-14 gate: the same numbers must appear in the contract
    (authoritative), be consumed by the Iced token module by name, and be
    mirrored channel-exactly by the business-agnostic Bevy widget layer.
    """
    cores = contract_cores()
    if set(cores) != set(SKINS):
        violations.append(
            f"{DESIGN_TOKENS} must define skin_core for every skin, found {sorted(cores)}"
        )
        return
    for skin, fields in cores.items():
        missing = set(BEVY_CORE_FIELDS) - set(fields)
        if missing:
            violations.append(
                f"{DESIGN_TOKENS} {skin} core palette missing {sorted(missing)}"
            )

    mirror = bevy_cores()
    if set(mirror) != set(SKINS):
        violations.append(
            f"{BEVY_THEME} must define all four skins, found {sorted(mirror)}"
        )
        return
    for skin in SKINS:
        for field, bevy_field in BEVY_CORE_FIELDS.items():
            expected = cores[skin].get(field)
            actual = mirror[skin].get(bevy_field)
            if expected is None:
                continue
            if actual is None:
                violations.append(
                    f"{BEVY_THEME} {skin} missing token {bevy_field!r}"
                )
                continue
            if len(expected) != len(actual) or any(
                abs(left - right) > 1e-6 for left, right in zip(expected, actual)
            ):
                violations.append(
                    f"{BEVY_THEME} {skin}.{bevy_field}={actual} must mirror "
                    f"contract {field}={expected}"
                )

    iced_text = read(ICED_THEME)
    for skin in SKINS:
        core_name = f"{skin.upper()}_CORE"
        if f"skin_core(ThemeSkin::{skin})" not in iced_text:
            violations.append(
                f"{ICED_THEME} must resolve {skin} from the shared skin_core"
            )
            continue
        block = re.search(
            rf"pub const {skin.upper()}: Tokens = Tokens \{{(?P<body>.*?)\n\}};",
            iced_text,
            re.DOTALL,
        )
        if not block:
            violations.append(f"{ICED_THEME} missing the {skin.upper()} token block")
            continue
        body = block.group("body")
        for field, iced_field in ICED_CORE_FIELDS.items():
            if f"{iced_field}: token_color({core_name}.{field})" not in body:
                violations.append(
                    f"{ICED_THEME} {skin.upper()}.{iced_field} must consume "
                    f"{core_name}.{field}"
                )

    # The hairline is part of the claimed shared scope: Iced must consume the
    # contract metric by name, and the whole Iced shell must have exactly one
    # spelling of a 1px border (the token).
    if (
        "pub const HAIRLINE: f32 = "
        "infiltrator_contract::design_tokens::metrics::HAIRLINE;" not in iced_text
    ):
        violations.append(
            f"{ICED_THEME} HAIRLINE must consume the shared contract metric"
        )
    check_no_raw_hairlines(violations)

    # Structural ladders: contract numbers, Iced by-name consumption, Bevy
    # numeric mirror.
    contract_text = read(DESIGN_TOKENS)
    expected_space = {
        "XS": 4.0,
        "SM": 8.0,
        "MD": 12.0,
        "LG": 16.0,
        "XL": 20.0,
        "XXL": 24.0,
    }
    for name, expected in expected_space.items():
        contract_value = number_const(contract_text, name)
        if contract_value != expected:
            violations.append(
                f"{DESIGN_TOKENS} space::{name}={contract_value!r}, expected {expected}"
            )
        if f"pub const SP_{name}: f32 = infiltrator_contract::design_tokens::space::{name};" not in iced_text:
            violations.append(
                f"{ICED_THEME} SP_{name} must consume the shared spacing ladder"
            )
    for name, expected in {"CARD": 16.0, "CONTROL": 10.0}.items():
        contract_value = number_const(contract_text, name)
        if contract_value != expected:
            violations.append(
                f"{DESIGN_TOKENS} radius::{name}={contract_value!r}, expected {expected}"
            )
        if f"pub const R_{name}: f32 = infiltrator_contract::design_tokens::radius::{name};" not in iced_text:
            violations.append(
                f"{ICED_THEME} R_{name} must consume the shared radius ladder"
            )

    bevy_text = read(BEVY_THEME)
    bevy_space = {
        "S4": "XS",
        "S8": "SM",
        "S12": "MD",
        "S16": "LG",
        "S20": "XL",
        "S24": "XXL",
    }
    for bevy_name, contract_name in bevy_space.items():
        bevy_value = number_const(bevy_text, bevy_name)
        expected = expected_space[contract_name]
        if bevy_value != expected:
            violations.append(
                f"{BEVY_THEME} space::{bevy_name}={bevy_value!r} must mirror "
                f"contract space::{contract_name}={expected}"
            )
    for name, expected in {"CARD": 16.0, "CONTROL": 10.0}.items():
        bevy_value = number_const(bevy_text, name)
        if bevy_value != expected:
            violations.append(
                f"{BEVY_THEME} radius::{name}={bevy_value!r} must mirror contract {expected}"
            )
    bevy_hairline = number_const(bevy_text, "HAIRLINE")
    contract_hairline = number_const(contract_text, "HAIRLINE")
    if bevy_hairline is None or bevy_hairline != contract_hairline:
        violations.append(
            f"{BEVY_THEME} metrics::HAIRLINE={bevy_hairline!r} must mirror "
            f"contract metrics::HAIRLINE={contract_hairline!r}"
        )


def check_no_raw_hairlines(violations: list[str]) -> None:
    """Fail closed if an Iced border goes back to a raw `width: 1.0`.

    DUAL-15-14 claims the hairline as a shared token; a page-local literal
    would silently break the claim (and the Bevy mirror would have nothing to
    mirror), so the whole Iced view layer must spell it `theme::HAIRLINE`.
    """
    roots = [ROOT / "crates/infiltrator-iced/src/view", ROOT / "crates/infiltrator-iced/src/view_root.rs"]
    roots.append(ROOT / "crates/infiltrator-iced/src/view_root")
    for root in roots:
        files = [root] if root.is_file() else sorted(root.rglob("*.rs"))
        for path in files:
            relative = path.relative_to(ROOT).as_posix()
            if relative.endswith("view/theme.rs"):
                continue
            for number, line in enumerate(
                path.read_text(encoding="utf-8").splitlines(), start=1
            ):
                if "width: 1.0" in line or "width: 1," in line:
                    violations.append(
                        f"{relative}:{number} hardcodes a 1px border width; "
                        "use theme::HAIRLINE (DUAL-15-14)"
                    )


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
        "组 15 多模态外壳与极客命令流 | 15 | `in progress (8/15)`",
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
        "MiniHudApplication::with_window_port",
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

    # ---- Batch C (DUAL-15-03/04/08/14/15): the desktop HUD window adapter,
    # the shared design tokens, the shared render cadence and the matrix.
    require(
        violations,
        LEDGER,
        "2026-09-22 组 15 批次 C",
        "DesktopMiniHudWindow",
        "MiniHudWindowHandle",
        "IcedMiniHudWindowHandle",
        "RenderCadence",
        "design_tokens",
        "MULTIMODAL_SHELL_MATRIX.md",
        "a_persisted_placement_reaches_the_iced_window_handle",
        "the_placement_update_drains_the_host_window_requests",
        "a_bound_handle_receives_the_placement_and_the_visibility_request",
        "an_unbound_or_stale_handle_reports_typed_unsupported",
        "the_hud_quick_switches_dispatch_the_shared_toggle_commands",
        "a_pending_hud_quick_switch_dispatches_nothing",
        "the_widget_palette_mirrors_the_shared_design_tokens",
        "the_widget_ladders_mirror_the_shared_contract_numbers",
        "the_iced_tokens_resolve_the_shared_design_contract",
        "the_shell_tracks_window_focus_for_the_shared_cadence",
        "the_winit_modes_follow_the_shared_cadence",
        "focus_and_occlusion_events_reselect_the_winit_cadence",
        "the_widget_frame_pacing_vocabulary_mirrors_the_shared_cadence",
    )

    # Ports: the window-owner half of the HUD window capability.
    require(
        violations,
        "crates/infiltrator-ports/src/mini_hud_window.rs",
        "pub trait MiniHudWindowHandle",
        "fn apply_placement(&self, placement: MiniHudPlacement) -> bool",
        "fn set_visible(&self, visible: bool) -> bool",
    )
    # Desktop: the real host adapter + runtime capability.
    require(
        violations,
        "crates/infiltrator-desktop/src/mini_hud_window.rs",
        "pub struct DesktopMiniHudWindow",
        "pub fn shared",
        "pub fn bind",
        "impl MiniHudWindowPort for DesktopMiniHudWindow",
        "MiniHudHostOutcome::Unsupported",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/runtime.rs",
        "fn mini_hud_window_port",
        "DesktopMiniHudWindow::shared()",
    )
    # Iced: the window handle, the update-path drain and the store port.
    require(
        violations,
        "crates/infiltrator-iced/src/mini_hud_window.rs",
        "pub struct IcedMiniHudWindowHandle",
        "pub fn install_host_handle",
        "pub fn host_requests_task",
        "pub fn mark_live",
        "pub fn take_pending",
        "impl MiniHudWindowHandle for IcedMiniHudWindowHandle",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/mini_hud.rs",
        "host_requests_task",
        "mark_live",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/mini_hud_store.rs",
        "crate::host::mini_hud::window_port",
    )
    # The concrete desktop dependency stays at the composition boundary.
    require(
        violations,
        "crates/infiltrator-iced/src/host.rs",
        "DesktopMiniHudWindow::shared",
        "pub fn window_port",
        "pub fn bind_window_handle",
    )
    # Iced: the shared render cadence.
    require(
        violations,
        "crates/infiltrator-iced/src/subscription.rs",
        "frame_cadence_subscription",
        "window::Event::Focused",
        "window::Event::Unfocused",
        "RenderCadence::from_focused",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "pub window_focused",
    )
    # Contract: the shared palette + cadence policy.
    require(
        violations,
        DESIGN_TOKENS,
        "pub struct RgbaToken",
        "pub struct SkinCorePalette",
        "pub const fn skin_core",
        "pub mod space",
        "pub mod radius",
        "pub mod metrics",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/cadence.rs",
        "pub enum RenderCadence",
        "pub const fn from_focused",
        "pub const fn from_visible_focused",
        "pub const fn frame_time_ms",
        "pub fn frame_interval",
        "BACKGROUND_FRAME_TIME_MS",
    )
    # Bevy: cadence projection onto winit + the HUD quick switches.
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/cadence.rs",
        "pub fn winit_settings_for",
        "pub fn sync_window_cadence",
        "pub struct CadencePlugin",
        "WinitSettings",
        "WindowFocused",
        "WindowOccluded",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/lib.rs",
        "pub mod cadence",
        "cadence::CadencePlugin",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/mini_hud.rs",
        "MiniHudSystemProxyToggle",
        "MiniHudTunToggle",
        "ButtonDisabled",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/mini_hud_shell.rs",
        "on_mini_hud_system_proxy_activated",
        "on_mini_hud_tun_activated",
        "submit_toggle",
        "UiCommand::ToggleTun",
    )
    require(
        violations,
        "crates/infiltrator-bevy-widgets/src/cadence.rs",
        "FramePacingMode::BackgroundThrottled => 500",
    )
    # The dual tests introduced by this batch.
    require(
        violations,
        "crates/infiltrator-desktop/src/mini_hud_window.rs",
        "fn a_bound_handle_receives_the_placement_and_the_visibility_request",
        "fn an_unbound_or_stale_handle_reports_typed_unsupported",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/mini_hud_window.rs",
        "fn a_handle_without_a_live_window_refuses_the_placement",
        "fn a_live_window_accepts_exactly_once_per_request",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/mini_hud_store.rs",
        "fn a_persisted_placement_reaches_the_iced_window_handle",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/multimodal_shell_tests.rs",
        "the_placement_update_drains_the_host_window_requests",
        "the_iced_tokens_resolve_the_shared_design_contract",
        "the_shell_tracks_window_focus_for_the_shared_cadence",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/mini_hud_tests.rs",
        "the_hud_quick_switches_dispatch_the_shared_toggle_commands",
        "a_pending_hud_quick_switch_dispatches_nothing",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/design_token_tests.rs",
        "the_widget_palette_mirrors_the_shared_design_tokens",
        "the_widget_ladders_mirror_the_shared_contract_numbers",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/cadence_tests.rs",
        "the_winit_modes_follow_the_shared_cadence",
        "focus_and_occlusion_events_reselect_the_winit_cadence",
        "the_widget_frame_pacing_vocabulary_mirrors_the_shared_cadence",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/cadence.rs",
        "fn every_host_fact_maps_to_one_cadence",
        "fn the_product_rates_are_sixty_and_two_fps",
    )
    require(
        violations,
        DESIGN_TOKENS,
        "fn every_skin_has_a_distinct_core_palette",
        "fn the_spacing_and_radius_ladders_are_positive_and_ordered",
    )

    # ---- Batch D (DUAL-15-03/14/15): the shared HUD waveform strip, the
    # consumed hairline token, and the hardened matrix.
    require(
        violations,
        LEDGER,
        "2026-09-22 组 15 批次 D",
        "MiniHudWaveformStrip",
        "the_waveform_strip_projects_the_newest_real_samples",
        "an_empty_or_non_finite_waveform_never_fabricates_bars",
        "the_waveform_strip_comes_from_the_shared_live_samples",
        "the_mounted_waveform_slots_rasterize_the_shared_strip",
        "the_mini_hud_view_renders_the_shared_strip",
        "demo_seeds_the_shared_live_waveform_slot",
        "sparkline_image_projects_normalized_bars_without_a_grid",
        "theme_hairline_consumes_the_shared_contract_metric",
        "check_no_raw_hairlines",
        "ledger_statuses",
        "sync_mini_hud_waveforms",
    )

    # Contract: the shared normalization both surfaces consume.
    require(
        violations,
        "crates/infiltrator-contract/src/mini_hud.rs",
        "pub const MINI_HUD_WAVEFORM_BARS",
        "pub struct MiniHudWaveformStrip",
        "pub fn from_snapshot(snapshot: &TrafficWaveformSnapshot) -> Self",
        "pub fn bar_fraction(bar: u16) -> f32",
        "pub fn with_waveform(mut self, snapshot: &TrafficWaveformSnapshot) -> Self",
        "pub waveform: MiniHudWaveformStrip",
        "pub const WIDTH_PX: u32 = 60",
    )

    # Iced: the strip canvas and the hairline token.
    require(
        violations,
        "crates/infiltrator-iced/src/view/waveform.rs",
        "pub enum StripInk",
        "pub struct MiniWaveformStrip",
        "pub fn hud_waveform<'a, Message: 'a>(bars: &[u16], ink: StripInk) -> Element<'a, Message>",
        "MiniHudWaveformStrip::WIDTH_PX",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/mini_hud.rs",
        "hud_waveform(&model.waveform.down, StripInk::Accent)",
        "hud_waveform(&model.waveform.up, StripInk::Success)",
    )
    require(
        violations,
        ICED_THEME,
        "pub const HAIRLINE: f32 = "
        "infiltrator_contract::design_tokens::metrics::HAIRLINE;",
    )

    # Bevy: the business-agnostic sparkline seam and the mounted HUD slots.
    require(
        violations,
        "crates/infiltrator-bevy-widgets/src/chart.rs",
        "pub fn sparkline_image(",
        "linear_polyline(samples, width as f32, height as f32, Some(1.0))",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/mini_hud.rs",
        "pub struct MiniHudDownWaveform",
        "pub struct MiniHudUpWaveform",
        "fn down_waveform_slot_scene()",
        "fn up_waveform_slot_scene()",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/mini_hud_shell.rs",
        ".with_waveform(&overview.traffic_waveform)",
        "pub fn sync_mini_hud_waveforms",
        "sparkline_image(",
        "MiniHudDownWaveform",
        "MiniHudUpWaveform",
    )
    require(
        violations,
        "crates/infiltrator-bevy-widgets/tests/headless/chart_tests.rs",
        "fn sparkline_image_projects_normalized_bars_without_a_grid",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/mini_hud_tests.rs",
        "fn the_waveform_strip_comes_from_the_shared_live_samples",
        "fn the_mounted_waveform_slots_rasterize_the_shared_strip",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/view_theme_tests.rs",
        "fn theme_hairline_consumes_the_shared_contract_metric",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/multimodal_shell_tests.rs",
        "fn the_mini_hud_view_renders_the_shared_strip",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/mini_hud.rs",
        "fn the_waveform_strip_projects_the_newest_real_samples",
        "fn an_empty_or_non_finite_waveform_never_fabricates_bars",
    )

    # The mirrored skin vocabulary must match the contract numerically.
    contract_skins = skin_settings("crates/infiltrator-contract/src/theme.rs")
    widget_skins = skin_settings("crates/infiltrator-bevy-widgets/src/theme.rs")
    if contract_skins != widget_skins or not contract_skins:
        violations.append(
            "skin setting vocabulary drift: contract "
            f"{sorted(contract_skins)} != widget {sorted(widget_skins)}"
        )

    # DUAL-15-14: the numeric design-token mirror (contract -> Iced -> Bevy).
    check_design_token_mirrors(violations)

    # DUAL-15-15: the machine-checkable multimodal regression matrix.
    check_matrix(violations)

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

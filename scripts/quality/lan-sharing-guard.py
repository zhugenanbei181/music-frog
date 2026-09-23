#!/usr/bin/env python3
"""Fail-closed guard for DUAL-02-07 live Allow-LAN listener settings."""

from __future__ import annotations

import argparse
import pathlib
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def require(violations: list[str], path: str, *markers: str) -> None:
    text = read(path)
    for marker in markers:
        if marker not in text:
            violations.append(f"{path} missing {marker!r}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    violations: list[str] = []

    require(
        violations,
        "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md",
        "`DUAL-02-07` Allow-LAN 混合端口与绑定地址",
        "allow-lan",
        "mixed-port",
        "bind-address",
        "PATCH /configs",
        "ACL/认证留给 DUAL-02-08",
        "`parity-ready`",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/lan.rs",
        "DEFAULT_BIND_ADDRESS",
        "LanSharingSnapshot",
        "pub enabled: bool",
        "pub mixed_port: u16",
        "pub bind_address: String",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "SetLanSharing {",
        "mixed_port: u16",
        "bind_address: String",
        "Self::SetLanSharing { .. }",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/runtime.rs",
        "pub bind_address: String",
        "default_bind_address",
    )
    require(
        violations,
        "crates/mihomo-api/src/types.rs",
        "bind-address",
        "pub bind_address: String",
        "bind_address: value.bind_address",
    )
    require(
        violations,
        "crates/infiltrator-application/src/runtime_query_lan.rs",
        "pub async fn set_lan_sharing",
        "canonical_bind_address",
        "bind_address_matches",
        '"allow-lan": enabled',
        '"mixed-port": mixed_port',
        '"bind-address": bind_address',
        "Allow-LAN readback mismatch",
    )
    require(
        violations,
        "crates/infiltrator-application/src/runtime_query_application.rs",
        "lan_sharing_patch_is_atomic_and_reads_back_bind_address",
        "lan_sharing_rejects_zero_port_and_invalid_bind_address_before_io",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application/dispatch.rs",
        "CommandIntent::SetLanSharing",
        ".set_lan_sharing(enabled, mixed_port, &bind_address)",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader/page_builders.rs",
        "lan_bind_address",
        "value.bind_address.clone()",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub lan_bind_address: String",
        "default_lan_bind_address",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/app.rs",
        "pub bind_address: String",
        "DEFAULT_BIND_ADDRESS",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/runtime.rs",
        "pub allow_lan: bool",
        "pub mixed_port: u16",
        "pub bind_address: String",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "UpdateLanBindAddress(String)",
        "ApplyLanSharing",
        "LanSharingSet(",
        "LanSharingSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/runtime_config.rs",
        "let allow_lan = config.allow_lan",
        "let mixed_port = config.mixed_port",
        "let bind_address = config.bind_address",
        "self.runtime.lan_sharing_committed",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "lan_sharing_committed",
        "lan_sharing_dirty",
        "snapshot.pages.settings",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/ui_wave5.rs",
        "fn apply_lan_sharing",
        "RuntimeQueryApplication::new(gateway)",
        ".set_lan_sharing(",
        "Message::LanSharingSet",
        "lan_sharing_committed",
        "lan_sharing_dirty = false",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/lan_sharing_card.rs",
        "UpdateLanBindAddress",
        "ApplyLanSharing",
        "lan_sharing_bind",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/iced_six_advancements_wave5_tests.rs",
        "test_lan_sharing_readback_updates_the_iced_state",
        "test_shared_surface_keeps_a_dirty_lan_draft_until_apply_result",
        "LanSharingSnapshot::new",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "SetLanSharing {",
        "CommandIntent::SetLanSharing",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_lan.rs",
        "LanSharingToggle",
        "LanMixedPortField",
        "LanBindAddressField",
        "LanSharingApplyButton",
        "TextField",
        "UiCommand::SetLanSharing",
        "on_toggle_changed",
        "on_apply_activated",
        "apply_projection",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings.rs",
        "settings_lan::scene",
        "settings_lan::on_apply_activated",
        "settings_lan::apply_projection",
        "SettingsLineKind::LanBindAddress",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "lan_bind_address: value.lan_bind_address.clone()",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "test_settings_lan_fields_submit_the_live_listener_intent",
        "UiCommand::SetLanSharing",
        "192.168.1.10",
    )

    if violations:
        for violation in violations:
            print(f"lan-sharing-guard: {violation}", file=sys.stderr)
        print(f"lan-sharing-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("lan-sharing-guard: DUAL-02-07 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

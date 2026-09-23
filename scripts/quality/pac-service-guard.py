#!/usr/bin/env python3
"""Fail-closed guard for DUAL-02-11 PAC generation and loopback service."""

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
        "`DUAL-02-11` PAC 动态代理脚本与本地服务",
        "PacSnapshot/PacRequest",
        "PacServicePort",
        "127.0.0.1",
        "live gateway 读取规则",
        "`parity-ready`",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/pac.rs",
        "PacServiceState",
        "PacSnapshot",
        "PacRequest",
        "not copied into the cross-surface snapshot",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "ApplyPac {",
        "bypass_domains: Vec<String>",
        "Self::ApplyPac { .. }",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/capability.rs",
        "PacService",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/pac.rs",
        "trait PacServicePort",
        "async fn start",
        "async fn stop",
        "async fn status",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/runtime_gateway.rs",
        "async fn get_rules",
        "RuleEntry",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/pac_policy.rs",
        "normalize_bypass_domains",
        "pac_bypass_patterns_reject_script_injection",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/pac_generator.rs",
        "compile_pac_script",
        "validate_pac_script",
    )
    require(
        violations,
        "crates/infiltrator-application/src/pac_application.rs",
        "pub async fn snapshot",
        "pub async fn apply",
        "get_rules().await",
        "validate_pac_script",
        "PAC service readback mismatch",
        "pac_application_compiles_live_rules_and_reads_back_service",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "with_pac",
        "CommandIntent::ApplyPac",
        ".apply(infiltrator_contract::pac::PacRequest",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "with_pac",
        "read_pac",
        "pac,",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub pac: crate::pac::PacSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/pac_service.rs",
        "DesktopPacServicePort",
        "127.0.0.1:0",
        "application/x-ns-proxy-autoconfig",
        "loopback_server_serves_generated_pac_and_stops",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/runtime/ports.rs",
        "fn pac_service_port",
        "async fn get_rules",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/runtime.rs",
        "DesktopPacServicePort",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/composition.rs",
        "PacApplication",
        ".with_pac",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/surface.rs",
        "PacApplication",
        ".with_pac",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/host_runtime.rs",
        "pac_service_port",
        "PacServicePort",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/app.rs",
        "pub snapshot: infiltrator_contract::pac::PacSnapshot",
        "pub dirty: bool",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "settings.pac",
        "pac.snapshot",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/ui_wave4.rs",
        "fn apply_pac",
        "PacApplication::new",
        "PacRequest",
        "Message::PacApplied",
        "pac_manager.dirty",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "PacApplied(Result<",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/pac_card.rs",
        "pac.pac_url.as_str()",
        "pac_title",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/iced_six_advancements_wave4_tests.rs",
        "test_pac_live_result_updates_url_and_committed_snapshot",
        "PacApplied(Ok(snapshot))",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "ApplyPac {",
        "CommandIntent::ApplyPac",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_core.rs",
        "pub pac: infiltrator_contract::pac::PacSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_pac.rs",
        "PacToggle",
        "PacBypassField",
        "PacApplyButton",
        "on_apply_activated",
        "UiCommand::ApplyPac",
        "format_status",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings.rs",
        "settings_pac::scene",
        "settings_pac::on_changed",
        "settings_pac::on_apply_activated",
        "settings_pac::apply_projection",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "test_settings_pac_projection_and_apply_submit_shared_request",
        "PacApplyButton",
        "UiCommand::ApplyPac",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/surface.rs",
        "Capability::PacService",
    )
    require(
        violations,
        "crates/infiltrator-android/src/runtime.rs",
        "Capability::PacService",
        "Android uses VpnService instead of desktop PAC injection",
    )
    require(
        violations,
        "crates/infiltrator-ios/src/lib.rs",
        "Capability::PacService",
        "iOS has no desktop PAC injection surface",
    )

    if violations:
        for violation in violations:
            print(f"pac-service-guard: {violation}", file=sys.stderr)
        print(f"pac-service-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("pac-service-guard: DUAL-02-11 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

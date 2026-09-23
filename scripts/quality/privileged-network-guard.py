#!/usr/bin/env python3
"""Fail-closed guard for DUAL-02-15 privileged network regressions."""

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
        "DUAL-02-15",
        "特权网络无头回归测试",
        "PrivilegedNetworkSnapshot/PrivilegedNetworkApplication",
        "parity-ready",
    )
    require(
        violations,
        "docs/CORE_030_REARCHITECTURE.md",
        "DUAL-02-15",
        "注入、回读、清理与失败回滚",
    )
    require(
        violations,
        "docs/DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md",
        "DUAL-02-15",
        "mock host adapter",
        "host-verified",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/privileged_network.rs",
        "PrivilegedNetworkOperation",
        "PrivilegedNetworkRequest",
        "TunService",
        "SystemProxy",
        "RouteRepair",
        "PrivilegedNetworkState",
        "RollingBack",
        "PrivilegedNetworkSnapshot",
        "is_clean",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/privileged_network_policy.rs",
        "validate_request",
        "duplicate operation",
        "standard_request_is_accepted",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/privileged_network.rs",
        "trait PrivilegedNetworkPort",
        "async fn inject",
        "async fn cleanup",
        "async fn snapshot",
    )
    require(
        violations,
        "crates/infiltrator-application/src/privileged_network_application.rs",
        "pub struct PrivilegedNetworkApplication",
        "pub async fn run",
        "rollback",
        "successful_injection_always_cleans_and_reads_back",
        "injection_failure_attempts_rollback_and_preserves_primary_error",
        "cleanup_failure_becomes_a_typed_failed_snapshot",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "with_privileged_network",
        "read_privileged_network",
        "privileged_network,",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader_services.rs",
        "read_privileged_network",
        "privileged network regression port is not composed",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub privileged_network: crate::privileged_network::PrivilegedNetworkSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/host_runtime.rs",
        "privileged_network_port",
        "No host may claim support without an injected adapter",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/runtime.rs",
        "privileged_network_port",
        "destructive privilege probes",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "RunPrivilegedNetworkRegression",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "RunPrivilegedNetworkRegression",
        "privileged_network()",
        "PrivilegedNetworkRequest::standard",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "pub privileged_network: PrivilegedNetworkSnapshot",
        "snapshot.privileged_network",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "RunPrivilegedNetworkRegression",
        "PrivilegedNetworkRegressionUpdated",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/ui_wave5.rs",
        "run_privileged_network_regression",
        "PrivilegedNetworkApplication",
        "PrivilegedNetworkRegressionUpdated",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/privileged_network_card.rs",
        "privileged_network_card",
        "RunPrivilegedNetworkRegression",
        "format_status",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/privileged_network_card_tests.rs",
        "card_consumes_cleaned_readback_without_fabricating_active_state",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "RunPrivilegedNetworkRegression",
        "CommandIntent::RunPrivilegedNetworkRegression",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_core.rs",
        "privileged_network",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_privileged_network.rs",
        "PrivilegedNetworkRunButton",
        "PrivilegedNetworkStatusLine",
        "UiCommand::RunPrivilegedNetworkRegression",
        "format_status",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings.rs",
        "settings_privileged_network::scene",
        "settings_privileged_network::on_action_activated",
        "settings_privileged_network::apply_projection",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "snapshot.privileged_network",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader_test.rs",
        "first.privileged_network.state",
        "PrivilegedNetworkState::Unsupported",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "test_privileged_network_regression_button_uses_shared_command",
        "PrivilegedNetworkRunButton",
        "UiCommand::RunPrivilegedNetworkRegression",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/settings.rs",
        "privileged_network_card::privileged_network_card",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_ext.rs",
        "privileged_network_title",
        "privileged_network_status_cleaned",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en_ext.rs",
        "privileged_network_title",
        "privileged_network_status_cleaned",
    )
    require(
        violations,
        "scripts/test.sh",
        "privileged-network-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        "privileged-network-guard.py\" --mode enforce",
    )

    if violations:
        for violation in violations:
            print(f"privileged-network-guard: {violation}", file=sys.stderr)
        print(f"privileged-network-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("privileged-network-guard: DUAL-02-15 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

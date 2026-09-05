#!/usr/bin/env python3
"""Fail-closed guard for DUAL-01-07 controller secret/header parity."""

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
        "`DUAL-01-07` 外部 Controller 免密拉起",
        "`parity-ready`",
    )
    require(
        violations,
        "crates/mihomo-config/src/manager/controller.rs",
        "SystemRandom",
        "ensure_controller_secret",
        "generated secret",
    )
    require(
        violations,
        "crates/mihomo-config/src/manager/manager_test.rs",
        "ensure_external_controller_generates_and_reuses_a_secret",
        "endpoint_source_injects_the_persisted_secret_without_exposing_it_in_contracts",
    )
    require(
        violations,
        "crates/mihomo-config/src/endpoint.rs",
        "current_profile_secret",
        "ControllerEndpoint { url, secret }",
    )
    require(
        violations,
        "crates/mihomo-api/src/client.rs",
        "fn add_auth",
        "req.bearer_auth(secret)",
        "controller secret cannot be encoded as an HTTP header",
    )
    require(
        violations,
        "crates/mihomo-api/src/client_auth_test.rs",
        "test_get_version_injects_controller_secret_as_bearer_auth",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/controller.rs",
        "ControllerAuthStatus",
        "secret itself is never part of this contract",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub controller_auth: ControllerAuthSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "with_endpoint_source",
        "read_controller_auth",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader_test.rs",
        "ControllerAuthStatus::Secured",
        "first.controller_auth.status",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/storage.rs",
        "pub async fn endpoint_source",
        "ProfileEndpointSource",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/surface.rs",
        "endpoint_source",
        ".with_endpoint_source(endpoint_source)",
    )
    require(
        violations,
        "crates/infiltrator-core/src/bootstrap_test.rs",
        "secret.len(), 64",
        "external_controller",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "controller_auth: ControllerAuthSnapshot",
        "runtime.controller_auth = snapshot.controller_auth",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/settings.rs",
        "format_controller_auth",
        "Controller auth",
        "secured · Bearer",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/surface_contract_tests.rs",
        "ControllerAuthStatus::Secured",
        "state.runtime.controller_auth.status",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings.rs",
        "SettingsLineKind::ControllerAuth",
        "controller_auth_row_scene",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_core.rs",
        "控制器认证 (Controller Auth)",
        "已保护 · Bearer",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "controller_auth: snapshot.controller_auth",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_tests.rs",
        "ControllerAuthStatus::Secured",
        "已保护 · Bearer",
    )

    if violations:
        for violation in violations:
            print(f"controller-auth-guard: {violation}", file=sys.stderr)
        print(f"controller-auth-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("controller-auth-guard: DUAL-01-07 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

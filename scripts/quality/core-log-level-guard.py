#!/usr/bin/env python3
"""Fail-closed guard for DUAL-01-08 live core log-level parity."""

from __future__ import annotations

import argparse
import pathlib
import re
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def require(violations: list[str], path: str, *markers: str) -> None:
    text = read(path)
    for marker in markers:
        if marker not in text:
            violations.append(f"{path} missing {marker!r}")


def require_regex(violations: list[str], path: str, pattern: str) -> None:
    text = read(path)
    if re.search(pattern, text, flags=re.DOTALL) is None:
        violations.append(f"{path} missing regex {pattern!r}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    violations: list[str] = []

    require(
        violations,
        "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md",
        "`DUAL-01-08` 内核日志等级即时下发",
        "`parity-ready`",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "CoreLogLevel",
        "pub fn parse",
    )
    require_regex(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        r"SetCoreLogLevel\s*\{\s*level:\s*CoreLogLevel\s*,?\s*\}",
    )
    require(
        violations,
        "crates/mihomo-api/src/client.rs",
        "pub async fn patch_config",
        "/configs",
        "error_for_status",
    )
    require(
        violations,
        "crates/mihomo-api/src/client_auth_test.rs",
        "test_get_version_injects_controller_secret_as_bearer_auth",
        "test_patch_config_changes_log_level_and_surfaces_rejection",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/runtime_gateway.rs",
        "async fn patch_config",
        "get_config",
    )
    require(
        violations,
        "crates/infiltrator-application/src/runtime_query_application.rs",
        "pub async fn set_core_log_level",
        "log-level",
        "readback mismatch",
        "core_log_level_patch_is_read_back_before_success",
        "core_log_level_readback_mismatch_is_not_reported_as_success",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application/dispatch.rs",
        "CommandIntent::SetCoreLogLevel",
        "set_core_log_level(level)",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "SetCoreLogLevel(String)",
        "CoreLogLevelFinished",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/monitoring.rs",
        "Message::SetCoreLogLevel",
        "Message::CoreLogLevelFinished",
        "unsupported core log level",
        "core is not running; log level was not changed",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/runtime/logs.rs",
        "Message::SetCoreLogLevel",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/settings/integration.rs",
        "CORE_LOG_LEVEL_OPTIONS",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/settings/kernel.rs",
        "Message::SetCoreLogLevel",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/business_flow/runtime_tray_kernels.rs",
        "core_log_level_rejects_invalid_or_stopped_updates_without_optimism",
        "Message::SetCoreLogLevel",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "SetCoreLogLevel(CoreLogLevel)",
        "CommandIntent::SetCoreLogLevel",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings.rs",
        "CoreLogLevelButton",
        "UiCommand::SetCoreLogLevel",
        "SettingsLineKind::LogLevel",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_core.rs",
        "core_log_level_row_scene",
        "core_log_level_button_scene",
        "CoreLogLevel::ALL",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "CoreLogLevel::Debug",
        "UiCommand::SetCoreLogLevel",
    )

    if violations:
        for violation in violations:
            print(f"core-log-level-guard: {violation}", file=sys.stderr)
        print(f"core-log-level-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("core-log-level-guard: DUAL-01-08 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

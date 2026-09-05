#!/usr/bin/env python3
"""Fail-closed guard for DUAL-01-02 hot-reload semantics."""

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
        "crates/mihomo-api/src/client.rs",
        "pub async fn reload_config",
        '"force", "true"',
        ".error_for_status()",
        "test_reload_config_uses_force_query_path_and_auth",
        "test_reload_config_surfaces_controller_rejection",
    )
    require(
        violations,
        "crates/infiltrator-core/src/apply.rs",
        "ApplyMethod::HotReload",
        "begin_reload",
        "wait_for_ready_session",
        "complete_reload",
        "fail_reload",
        "session_token",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/core_lifecycle.rs",
        "fn begin_reload",
        "fn complete_reload",
        "fn fail_reload",
        "wait_for_ready_session",
    )
    require(
        violations,
        "crates/infiltrator-application/src/core_application_tests.rs",
        "hot_reload_fences_to_the_current_session_without_bumping_generation",
    )
    require(
        violations,
        "crates/infiltrator-core/src/apply_test.rs",
        "hot_reload_success_keeps_generation_and_updates_file",
        "assert_eq!(outcome.session_token, session_token)",
        "assert_ne!(outcome.session_token, old_session)",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/surface_contract_tests.rs",
        "hot_reload_snapshot_keeps_the_session_identity_and_generation",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/surface_tests.rs",
        "hot_reload_snapshot_keeps_bevy_generation_and_session_identity",
    )
    require(
        violations,
        "crates/infiltrator-android/src/runtime.rs",
        "android_host_composition_preserves_hot_reload_session_identity",
    )
    require(
        violations,
        "crates/infiltrator-ios/src/lib.rs",
        "ios_host_composition_preserves_hot_reload_session_identity",
    )

    if violations:
        for violation in violations:
            print(f"hot-reload-guard: {violation}", file=sys.stderr)
        print(f"hot-reload-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("hot-reload-guard: DUAL-01-02 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

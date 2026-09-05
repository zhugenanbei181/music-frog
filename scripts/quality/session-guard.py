#!/usr/bin/env python3
"""Fail-closed guard for DUAL-01-01 core session semantics.

The compiler can verify that a token type exists, but it cannot ensure that
the token is threaded through the lifecycle reducer, application fence,
surface caches, and orphan cleanup evidence. This small source guard keeps
that first feature from regressing while later DUAL items are delivered.
"""

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
        "crates/infiltrator-contract/src/session.rs",
        "pub struct SessionToken",
        "pub const fn is_valid",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/snapshot.rs",
        "pub session_token: Option<SessionToken>",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub fn is_newer_than",
        "session_token",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/core_state.rs",
        "session_token: SessionToken",
        "stale_session_events_cannot_advance_the_state_machine",
        "active_state_requires_a_nonzero_session_token",
    )
    require(
        violations,
        "crates/infiltrator-application/src/core_application.rs",
        "fn allocate_session_token",
        "pub fn check_session",
        "cleanup_orphaned",
        "wait_for_ready_session",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/core_process.rs",
        "async fn cleanup_orphaned",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/core_lifecycle.rs",
        "fn session_token",
        "wait_for_ready_session",
    )
    require(
        violations,
        "crates/mihomo-platform/src/desktop.rs",
        "owned_pid",
        "process_matches_binary",
        "cleanup_orphaned_reclaims_a_live_expected_process",
        "parent_death_signal",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/surface.rs",
        "is_newer_than(current)",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/route.rs",
        "is_newer_than(&latest.0)",
    )
    require(
        violations,
        "crates/infiltrator-application/src/core_application_tests.rs",
        "stale_session_tokens_are_rejected_after_stop_and_restart",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/surface_contract_tests.rs",
        "stale_session_snapshot_is_rejected_even_with_a_larger_revision",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/surface_tests.rs",
        "stale_session_snapshot_cannot_replace_a_newer_bevy_projection",
    )

    if violations:
        for violation in violations:
            print(f"session-guard: {violation}", file=sys.stderr)
        print(f"session-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("session-guard: DUAL-01-01 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Fail-closed guard for DUAL-01-12 core resource soft-quota parity."""

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
        "`DUAL-01-12` 内核内存与 CPU 软限配额",
        "512MB",
        "`parity-ready`",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/resources.rs",
        "CORE_MEMORY_SOFT_LIMIT_BYTES",
        "512 * 1024 * 1024",
        "CoreGcStatus",
        "CoreResourceSnapshot",
        "over_memory_limit",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/runtime_gateway.rs",
        "get_cpu_percent",
        "trigger_gc",
    )
    require(
        violations,
        "crates/mihomo-api/src/client.rs",
        "pub async fn trigger_gc",
        "debug/gc",
        "error_for_status",
    )
    require(
        violations,
        "crates/mihomo-api/src/client_auth_test.rs",
        "test_trigger_gc_uses_authenticated_debug_endpoint",
    )
    require(
        violations,
        "crates/infiltrator-application/src/resource_application.rs",
        "pub async fn poll",
        "CORE_MEMORY_SOFT_LIMIT_BYTES",
        "GC_COOLDOWN",
        "trigger_gc",
        "after_bytes",
    )
    require(
        violations,
        "crates/infiltrator-application/src/runtime_query_application.rs",
        "resource_poll_triggers_gc_once_when_memory_exceeds_soft_limit",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "with_resources",
        "read_resources",
        "resources,",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader_services.rs",
        "read_resources",
        "application.poll()",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/runtime.rs",
        "get_cpu_percent",
        "cpu_usage",
        "trigger_gc",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/surface.rs",
        "ResourceApplication",
        "with_resources(resources)",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub resources: CoreResourceSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "core_resources: CoreResourceSnapshot",
        "runtime.core_resources = snapshot.resources.clone()",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/settings.rs",
        "format_core_resources",
        "Core resources",
        "limit=512 MiB",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/surface_contract_tests.rs",
        "CoreGcStatus::Triggered",
        "core_resources.cpu_percent",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings.rs",
        "CoreResources",
        "core_resources_row_scene",
        "format_core_resources",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_core.rs",
        "core_resources_row_scene",
        "CoreGcStatus::Triggered",
        "上限=512 MiB",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "core_resources: snapshot.resources.clone()",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_tests.rs",
        "CoreResourceSnapshot",
        "内存=400.0 MiB",
    )

    if violations:
        for violation in violations:
            print(f"core-resource-guard: {violation}", file=sys.stderr)
        print(f"core-resource-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("core-resource-guard: DUAL-01-12 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

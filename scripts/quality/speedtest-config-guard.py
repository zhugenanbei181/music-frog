#!/usr/bin/env python3
"""Fail-closed guard for DUAL-06-01 / DUAL-06-03 speedtest configuration.

DUAL-06-01 (runtime concurrency flow control) and DUAL-06-03 (dynamic test
target URL) must close over the one shared `SpeedtestApplication` fact source:
the port exposes `set_concurrency`, both surfaces read the live bound from
`snapshot.config.concurrency`, and the typed URL rides into the port/intent.
No UI may own an effective concurrency or target-URL fact of its own.
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
            violations.append(f"{path} still owns UI-local fact {marker!r}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    violations: list[str] = []

    require(
        violations,
        "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md",
        "DUAL-06-01",
        "DUAL-06-03",
        "parity-ready",
    )
    # Shared contract / application engine.
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "SetSpeedtestConcurrency",
    )
    require(
        violations,
        "crates/infiltrator-application/src/speedtest_application.rs",
        "concurrency_limit: Arc<AtomicUsize>",
        "pub fn set_concurrency",
        "buffer_unordered(concurrency_limit)",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application/dispatch.rs",
        "CommandIntent::SetSpeedtestConcurrency",
        "speedtest.set_concurrency",
    )
    require(
        violations,
        "crates/infiltrator-application/src/core_application/command_name.rs",
        '"set_speedtest_concurrency"',
    )
    # Port + desktop adapter.
    require(
        violations,
        "crates/infiltrator-ports/src/speedtest.rs",
        "fn set_concurrency",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/speedtest.rs",
        "fn set_concurrency",
        "application.set_concurrency",
    )
    # Iced surface: read the live bound, write through the port, carry the URL.
    require(
        violations,
        "crates/infiltrator-iced/src/view/speedtest_modal.rs",
        "snapshot.config.concurrency",
        "Message::AdjustSpeedtestConcurrency",
        "Message::UpdateSpeedtestTestUrl",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/proxies.rs",
        "set_concurrency",
        "speedtest_target_url",
        "AdjustSpeedtestConcurrency",
        "UpdateSpeedtestTestUrl",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/ui_wave3.rs",
        "speedtest_target_url",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "UpdateSpeedtestTestUrl",
        "AdjustSpeedtestConcurrency",
    )
    # Bevy surface: stepper + URL field submit the shared intents.
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "SetSpeedtestConcurrency",
        "TestAllProxyGroupsWithUrl",
        "CommandIntent::SetSpeedtestConcurrency",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/overview_speedtest.rs",
        "OverviewSpeedtestConcurrencyStep",
        "OverviewSpeedtestUrlField",
        "snapshot.config.concurrency",
        "UiCommand::SetSpeedtestConcurrency",
        "UiCommand::TestAllProxyGroupsWithUrl",
    )
    # Dual headless proof.
    require(
        violations,
        "crates/infiltrator-application/src/speedtest_application_tests.rs",
        "test_runtime_set_concurrency_updates_config_and_effective_bound",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/iced_six_advancements_wave3_tests.rs",
        "test_advancement_w3_3_speedtest_custom_url_flows_into_the_port_call",
        "test_advancement_w3_3_speedtest_concurrency_reads_shared_and_clamps",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/overview_tests.rs",
        "overview_speedtest_typed_url_reaches_the_shared_intent",
        "overview_speedtest_concurrency_stepper_submits_shared_intent",
    )
    # No surface may own the effective concurrency or target URL.
    for ui_path in (
        "crates/infiltrator-iced/src/update/core/proxies.rs",
        "crates/infiltrator-iced/src/view/speedtest_modal.rs",
        "crates/infiltrator-bevy-ui/src/pages/overview_speedtest.rs",
    ):
        forbid(violations, ui_path, "config.concurrency =", "concurrency: 30")
    # The speedtest card / Overview must never hardcode an effective target URL.
    for ui_path in (
        "crates/infiltrator-iced/src/view/speedtest_modal.rs",
        "crates/infiltrator-bevy-ui/src/pages/overview_speedtest.rs",
    ):
        forbid(violations, ui_path, "http://www.gstatic.com/generate_204")
    require(
        violations,
        "scripts/test.sh",
        "speedtest-config-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        'speedtest-config-guard.py" --mode enforce',
    )

    if violations:
        for violation in violations:
            print(f"speedtest-config-guard: {violation}", file=sys.stderr)
        print(
            f"speedtest-config-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("speedtest-config-guard: DUAL-06-01/06-03 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

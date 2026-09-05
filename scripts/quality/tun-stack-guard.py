#!/usr/bin/env python3
"""Fail-closed guard for DUAL-02-01 TUN stack parity."""

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
        "`DUAL-02-01` TUN 四堆栈安全调度",
        "gVisor / System / Mixed / LWIP",
        "ReferenceOnly",
        "`parity-ready`",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/tun.rs",
        "pub enum TunStack",
        "Self::Gvisor, Self::System, Self::Mixed, Self::Lwip",
        "is_live_supported",
        "TunStackAvailability::ReferenceOnly",
        "pub fn options",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "SetTunStack { stack: TunStack }",
        "Self::SetTunStack { .. } => CommandKind::Runtime",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/tun.rs",
        "TunStack::parse",
        "value.is_live_supported()",
        "test_validate_tun_accepts_all_current_mihomo_stacks",
    )
    require(
        violations,
        "crates/infiltrator-application/src/runtime_query_application.rs",
        "pub async fn set_tun_stack",
        "reference-only",
        "TUN stack readback mismatch",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "CommandIntent::SetTunStack",
        "set_tun_stack(stack)",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/composition.rs",
        "with_runtime(std::sync::Arc::new(client.clone()))",
    )
    require(
        violations,
        "crates/infiltrator-android/src/composition.rs",
        "Some(Arc::new(client)",
        "CommandApplication::new().with_runtime",
    )
    require(
        violations,
        "crates/infiltrator-composition/src/lib.rs",
        "CommandApplication::new().with_runtime",
        "ios_core_application",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/tun_stack_card.rs",
        "TunStack::Lwip",
        "Reference-only",
        "Message::SetTunStack",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/tun_config.rs",
        "TunStack::parse",
        "parsed.is_live_supported()",
        "reference-only",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/runtime_config.rs",
        "RuntimeQueryApplication",
        "TunStack::parse",
        "Message::SetTunStack",
        "set_tun_stack(parsed)",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/iced_six_advancements_wave5_tests.rs",
        "TunStack::options",
        "test_advancement_w5_3_tun_multi_stack_and_mtu_negotiation",
        "reference-only",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "SetTunStack(TunStack)",
        "CommandIntent::SetTunStack",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings.rs",
        "TunStackButton",
        "TunStackButtonAvailability",
        "UiCommand::SetTunStack",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_core.rs",
        "tun_stack_selector_scene",
        "TunStack::ALL",
        "Reference-only",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "test_settings_tun_stack_catalog_has_three_live_values_and_safe_lwip",
        "TunStack::Lwip",
        "reference-only LWIP is inert",
        "UiCommand::SetTunStack(TunStack::Mixed)",
    )

    if violations:
        for violation in violations:
            print(f"tun-stack-guard: {violation}", file=sys.stderr)
        print(f"tun-stack-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("tun-stack-guard: DUAL-02-01 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

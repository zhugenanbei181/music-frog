#!/usr/bin/env python3
"""Fail-closed guard for DUAL-12-10 rule-tracer source-IP sandbox parity.

The simulated sandbox environment (source IP / source port / inbound port)
must have exactly one shared fact source: the application-owned
`RuleTracerApplication` context merged onto `TrafficContext` for both the
`RuleTracerPort` path (Iced) and the surface-reader projection (Bevy).
Neither surface may fabricate a source IP or a UI-local inbound stage.
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
            violations.append(f"{path} still contains forbidden regression {marker!r}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    violations: list[str] = []

    require(
        violations,
        "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md",
        "DUAL-12-10",
        "parity-ready",
        "来源 IP",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/rule_tracer.rs",
        "pub src_ip: Option<String>",
        "pub src_port: Option<u16>",
        "pub in_port: Option<u16>",
        "simulated_context",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/rule_tracer.rs",
        "fn set_context",
        "TrafficContextSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-application/src/rule_tracer_application.rs",
        "pub fn set_context",
        "pub fn context",
        "fn merged_context",
        "simulated_context",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "SetRuleTracerContext",
        "set_context",
    )
    require(
        violations,
        "crates/infiltrator-application/src/core_application.rs",
        "set_rule_tracer_context",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "SetRuleTracerContext",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "UpdateTracerSourceIp",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message/debug/config.rs",
        "UpdateTracerSourceIp",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "rules_tracer_src_ip",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/rules.rs",
        "fn run_rules_tracer",
        "set_context",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/rules_tracer.rs",
        "UpdateTracerSourceIp",
        "tracer_src_ip_placeholder",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_ext.rs",
        "tracer_src_ip_label",
        "tracer_src_ip_placeholder",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en_ext.rs",
        "tracer_src_ip_label",
        "tracer_src_ip_placeholder",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules_tracer.rs",
        "TracerSourceIpField",
        "TracerQueryField",
        "on_tracer_action_activated",
        "SetRuleTracerContext",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "SetRuleTracerContext",
        "CommandIntent::SetRuleTracerContext",
        "SimulateRuleTrace",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/rules_dns_tests.rs",
        "test_rules_tracer_source_ip_sandbox_flow",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_rules_tracer_source_ip_sandbox_submits_shared_context",
        "TracerSourceIpField",
    )
    # The inert simulate intent must no longer fall through to unsupported.
    forbid(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "| CommandIntent::SimulateRuleTrace { .. }",
    )
    # DUAL-12-10 must have left the combined "planned" row.
    forbid(
        violations,
        "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md",
        "DUAL-12-08` 反向应用 / `DUAL-12-10`",
    )
    require(
        violations,
        "scripts/test.sh",
        "rule-tracer-sandbox-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        'rule-tracer-sandbox-guard.py" --mode enforce',
    )

    if violations:
        for violation in violations:
            print(f"rule-tracer-sandbox-guard: {violation}", file=sys.stderr)
        print(
            f"rule-tracer-sandbox-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("rule-tracer-sandbox-guard: DUAL-12-10 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

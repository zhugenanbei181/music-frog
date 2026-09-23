#!/usr/bin/env python3
"""Fail-closed guard for DUAL-12-08 tracer reverse-apply parity.

After a tracer simulation resolves an outbound, both surfaces can rewrite the
matched rule's target and commit it through the one existing apply
transaction. The rule-rewrite + apply capability lives once in
`RuleTracerApplication` over the `RuleOverridePort` seam; neither surface may
invent its own success, and a host without the capability must answer with a
typed unsupported result rather than a silent no-op.
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
        "DUAL-12-08",
        "parity-ready",
        "反向应用",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/rule_tracer.rs",
        "pub struct TracerRuleOverride",
        "pub struct TracerRuleOverrideResult",
        "pub enum TracerRuleOverrideStatus",
        "pub fn can_reverse_apply",
        "pub fn suggested_override_target",
        "pub fn into_failure",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/rules.rs",
        "pub fn rewrite_rule_target",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/rule_tracer.rs",
        "pub trait RuleOverridePort",
        "async fn load_rule_entries",
        "async fn apply_rule_entries",
        "async fn apply_override",
    )
    require(
        violations,
        "crates/infiltrator-application/src/rule_tracer_application.rs",
        "pub fn set_override_port",
        "pub async fn apply_override",
        "RuleOverridePort",
        "rewrite_rule_target",
        "TracerRuleOverrideStatus::Unsupported",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application/dispatch.rs",
        "CommandIntent::ApplyTracerRuleOverride",
        "apply_override",
        "into_failure",
    )
    require(
        violations,
        "crates/infiltrator-application/src/core_application/command_name.rs",
        "apply_tracer_rule_override",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "ApplyTracerRuleOverride",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/rule_override.rs",
        "pub struct DesktopRuleOverridePort",
        "impl RuleOverridePort for DesktopRuleOverridePort",
        "load_rule_entries",
        "apply_rule_entries",
        "apply_current_profile",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/runtime.rs",
        "set_override_port",
        "DesktopRuleOverridePort",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "ApplyTracerRuleOverride",
        "TracerRuleOverrideApplied",
        "UpdateTracerOverrideTarget",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message/debug/config.rs",
        "ApplyTracerRuleOverride",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "rules_tracer_can_reverse_apply",
        "rules_tracer_suggested_target",
        "rules_tracer_override_target",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/rules.rs",
        "fn sync_tracer_override_state",
        "apply_override",
        "TracerRuleOverrideApplied",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/rules_tracer.rs",
        "tracer_override_apply",
        "Message::ApplyTracerRuleOverride",
        "rules_tracer_can_reverse_apply",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_ext.rs",
        "tracer_override_apply",
        "tracer_override_label",
        "tracer_override_placeholder",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en_ext.rs",
        "tracer_override_apply",
        "tracer_override_label",
        "tracer_override_placeholder",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules_tracer.rs",
        "ApplyTracerRuleOverrideButton",
        "TracerOverrideTargetField",
        "on_tracer_override_activated",
        "can_reverse_apply",
        "suggested_override_target",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules.rs",
        "on_tracer_override_activated",
        "LastRulesProjection",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "ApplyTracerRuleOverride",
        "CommandIntent::ApplyTracerRuleOverride",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/rules_dns_tests.rs",
        "test_rules_tracer_reverse_apply_override_dual_surface_flow",
        "TracerRuleOverrideApplied",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_rules_tracer_override_submits_and_consumes_shared_result",
        "ApplyTracerRuleOverrideButton",
        "RuleOverridePort",
    )
    # The reverse-apply must never be routed as an inert unsupported fallthrough
    # nor remain absent from the command vocabulary.
    forbid(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "| CommandIntent::ApplyTracerRuleOverride { .. }",
    )
    # DUAL-12-08 must have left the "planned" ledger row.
    forbid(
        violations,
        "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md",
        "| `DUAL-12-08` | 分流结果一键反向应用（修改此规则出站） | `planned` |",
    )
    require(
        violations,
        "scripts/test.sh",
        "rule-tracer-override-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        'rule-tracer-override-guard.py" --mode enforce',
    )

    if violations:
        for violation in violations:
            print(f"rule-tracer-override-guard: {violation}", file=sys.stderr)
        print(
            f"rule-tracer-override-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("rule-tracer-override-guard: DUAL-12-08 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

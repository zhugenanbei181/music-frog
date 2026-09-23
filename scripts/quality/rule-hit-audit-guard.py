#!/usr/bin/env python3
"""Fail-closed guard for DUAL-12-04/05/06/07/11/12 rule hit-audit parity.

The hit-audit cluster must have exactly one shared fact source: the
application-owned `RuleHitAuditSnapshot` fed by `RuleHitCounter` and
`find_shadowed_rules`. Neither surface may fabricate counts, dead rules,
timestamps or CIDR conflicts.
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
            violations.append(f"{path} still contains forbidden fabrication {marker!r}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    violations: list[str] = []

    require(
        violations,
        "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md",
        "DUAL-12-04",
        "DUAL-12-05",
        "DUAL-12-06",
        "DUAL-12-07",
        "DUAL-12-11",
        "DUAL-12-12",
        "DUAL-12-09",
        "RuleHitAuditSnapshot",
        "parity-ready",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/rule_tracer.rs",
        "RuleHitAuditSnapshot",
        "RuleDeadEntry",
        "RuleDeadReason",
        "cidr_overlaps",
        "last_hit_rule",
        "can_clear",
        "avg_match_latency_us",
    )
    require(
        violations,
        "crates/infiltrator-application/src/rule_tracer_application.rs",
        "pub fn record_hits",
        "pub fn clear_hits",
        "pub fn audit",
        "hit_audit",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader/rules_page.rs",
        "hit_audit",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "with_rule_tracer",
    )

    require(
        violations,
        "crates/infiltrator-application/src/command_application/dispatch.rs",
        "ResetRuleHitCounters",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/rule_tracer.rs",
        "fn record_hits",
        "fn clear_hits",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/rule_hit_card.rs",
        "rule_hit_btn_clear",
        "rule_hit_cidr_conflicts",
        "audit.dead_rules",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "rule_hit_audit.audit = rules_page.tracer.hit_audit",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/rules.rs",
        "hit_audit",
        "RulesLineKind::HitAudit",
        "ClearRuleHitCountersButton",
        "rule_hit_label",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "ClearRuleHitCounters",
        "CommandIntent::ResetRuleHitCounters",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/iced_six_advancements_wave5_tests.rs",
        "test_advancement_w5_1_rule_hit_counter_and_stale_analyzer",
        "RuleHitAuditSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_rules_hit_audit_projection_and_clear_command",
        "ClearRuleHitCountersButton",
    )
    # The Iced audit path must never regress to the fabricated stub.
    forbid(violations, "crates/infiltrator-iced/src/update/ui_wave5.rs", "1250", "idx % 2 == 1")
    require(
        violations,
        "scripts/test.sh",
        "rule-hit-audit-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        'rule-hit-audit-guard.py" --mode enforce',
    )

    if violations:
        for violation in violations:
            print(f"rule-hit-audit-guard: {violation}", file=sys.stderr)
        print(
            f"rule-hit-audit-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("rule-hit-audit-guard: DUAL-12-04/05/06/07/11/12 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

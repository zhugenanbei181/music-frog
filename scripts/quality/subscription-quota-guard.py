#!/usr/bin/env python3
"""Fail-closed guard for DUAL-03-06 subscription quota parity."""

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
        "DUAL-03-06",
        "主活动出口节点高保真卡片",
        "订阅配额与临期动态仪表盘",
        "SubscriptionQuotaSnapshot",
        "parity-ready",
    )
    require(
        violations,
        "docs/CORE_030_REARCHITECTURE.md",
        "DUAL-03-06",
        "SubscriptionQuotaApplication",
        "active profile",
        "reset",
    )
    require(
        violations,
        "docs/DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md",
        "DUAL-03-06",
        "SubscriptionQuotaSnapshot/SubscriptionQuotaApplication",
        "host-verified",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/subscription_quota.rs",
        "SubscriptionQuotaStatus",
        "SubscriptionQuotaSnapshot",
        "usage_percent",
        "remaining_percent",
        "remaining_days",
        "reset_days",
        "demo_fixture",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/subscription_quota.rs",
        "pub fn derive",
        "SubscriptionUserInfo",
        "no_active_profile_is_empty",
        "derives_usage_remaining_expiry_and_warning_state",
        "exhausted_and_expired_are_distinct_typed_states",
        "no_active_profile_is_empty_not_a_fabricated_quota",
    )
    require(
        violations,
        "crates/infiltrator-application/src/subscription_quota_application.rs",
        "SubscriptionQuotaApplication",
        "pub fn project",
        "missing_profile_application_is_typed_unsupported",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "subscription_quota",
        "SubscriptionQuotaApplication",
        "let subscription_quota = self",
        ".subscription_quota",
        "profile_result",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub subscription_quota: crate::subscription_quota::SubscriptionQuotaSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/overview_cards.rs",
        "subscription_quota_scene_with_snapshot",
        "SubscriptionQuotaText",
        "SubscriptionQuotaProgress",
        "quota_status_color",
        "subscription_quota_text_value",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/overview_restamp.rs",
        "projection.subscription_quota",
        "subscription_quota_text_value",
        "quota_progress",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/overview_tests.rs",
        "test_subscription_quota_card_mounts_with_progress_bar",
        "SubscriptionQuotaCard",
        "reset not reported",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "pub subscription_quota",
        "snapshot.subscription_quota",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/subscription_quota.rs",
        "subscription_quota_card",
        "usage_bar",
        "status_label",
        "quota_status_label_has_no_fake_success_for_missing_metadata",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/overview.rs",
        "subscription_quota_card(state, &lang)",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/surface_contract_tests.rs",
        "shared_subscription_quota_reaches_the_iced_runtime_projection",
    )
    require(
        violations,
        "scripts/test.sh",
        "subscription-quota-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        "subscription-quota-guard.py\" --mode enforce",
    )

    if violations:
        for violation in violations:
            print(f"subscription-quota-guard: {violation}", file=sys.stderr)
        print(f"subscription-quota-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("subscription-quota-guard: DUAL-03-06 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

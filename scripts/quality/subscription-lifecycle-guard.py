#!/usr/bin/env python3
"""Fail-closed guard for the DUAL-07 subscription lifecycle ledger closure.

Group 07 (subscription lifecycle: multi-channel import, conditional requests,
retry/backoff, scheduling, backup) must keep a per-item ledger, and the shared
conditional-update path must stay single-sourced:

* one shared application builds ETag / If-Modified-Since / custom User-Agent /
  insecure-TLS headers from `ProfileMetadata` and returns a
  `SubscriptionUpdateReport`; a `304` must not rewrite content;
* insecure TLS is opt-in per profile and only `infiltrator-http` may call
  `danger_accept_invalid_certs` -- no surface may own a second HTTP path;
* both surfaces (Iced + Bevy) consume the shared settings/state.

Modeled on `speedtest-history-guard.py`.
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
            violations.append(f"{path} still contains forbidden path {marker!r}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    violations: list[str] = []

    # 1. The per-item ledger covers all 15 rows and records the closed trio.
    plan = read("docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md")
    for index in range(1, 16):
        row = f"DUAL-07-{index:02d}"
        if row not in plan:
            violations.append(f"docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md missing {row}")
    require(
        violations,
        "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md",
        "组 07 逐项账目",
        "DUAL-07-02",
        "DUAL-07-04",
        "DUAL-07-05",
        "DUAL-07-06",
        "DUAL-07-11",
        "DUAL-07-12",
        "DUAL-07-13",
        "parity-ready",
    )

    # 2. Shared contract / port seam for conditional update.
    require(
        violations,
        "crates/infiltrator-contract/src/subscription_import.rs",
        "SubscriptionImportChannel",
        "SubscriptionUpdateOutcome",
        "NotModified",
        "SubscriptionUpdateReport",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/subscription_source.rs",
        "struct ConditionalFetchHeaders",
        "custom_user_agent",
        "insecure_skip_verify",
        "async fn fetch_conditional",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "UpdateSubscriptionFetchSettings",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub user_agent: String",
        "pub insecure_skip_verify: bool",
        "pub etag: Option<String>",
        "pub last_modified: Option<String>",
    )

    # 3. One shared application path; no UI-local fetch logic.
    require(
        violations,
        "crates/infiltrator-application/src/profile_application.rs",
        "async fn update_subscription_conditional",
        "async fn update_subscription_fetch_settings",
        "fn report_from_metadata",
        "SubscriptionUpdateReport",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "UpdateSubscriptionFetchSettings",
        "update_subscription_fetch_settings",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "insecure_skip_verify",
    )
    # 4. Insecure TLS is built only by the HTTP crate and gated in core.
    require(
        violations,
        "crates/infiltrator-http/src/lib.rs",
        "pub fn build_insecure_http_client",
        "danger_accept_invalid_certs(true)",
    )
    require(
        violations,
        "crates/infiltrator-core/src/subscription_io.rs",
        "build_insecure_http_client",
        "headers.insecure_skip_verify",
    )
    for surface in (
        "crates/infiltrator-iced/src/update/profile/subscription.rs",
        "crates/infiltrator-bevy-ui/src/pages/profiles_import.rs",
    ):
        forbid(violations, surface, "danger_accept_invalid_certs", "reqwest::Client")
    forbid(
        violations,
        "crates/infiltrator-core/src/subscription_io.rs",
        "danger_accept_invalid_certs",
    )

    # 5. Domain policy source of truth for schedule/retry/quota/format.
    require(
        violations,
        "crates/infiltrator-domain/src/subscription_scheduler_policy.rs",
        "pub struct CronSchedule",
        "pub enum SubscriptionSchedule",
        "pub struct RetryBackoffPolicy",
        "pub struct QuotaWarningPolicy",
        "pub fn count_nodes",
    )

    # 6. Iced surface: real handlers, localized copy, honest 304 feedback.
    require(
        violations,
        "crates/infiltrator-iced/src/update/profile/subscription.rs",
        "Message::UpdateSubscriptionUserAgent",
        "Message::UpdateSubscriptionInsecureSkipVerify",
        "SubscriptionRefreshApplication::with_default_policy",
        "pub(crate) fn subscription_update_toast",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/profile.rs",
        "UpdateSubscriptionUserAgent",
        "UpdateSubscriptionInsecureSkipVerify",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/profiles.rs",
        "UpdateSubscriptionInsecureSkipVerify",
        "profiles_insecure_skip_verify",
        "profiles_conditional_request",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_ext.rs",
        "profiles_insecure_skip_verify",
        "sub_update_not_modified",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en_ext.rs",
        "profiles_insecure_skip_verify",
        "sub_update_not_modified",
    )

    # 7. Bevy surface: typed controls bound to the shared projection + command.
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/profiles_import.rs",
        "SubscriptionUserAgentField",
        "SubscriptionInsecureToggle",
        "SaveUserAgentButton",
        "sync_subscription_fetch_controls",
        "on_save_subscription_fetch_settings",
        "SaveSubscriptionFetchSettings",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "SaveSubscriptionFetchSettings",
        "UpdateSubscriptionFetchSettings",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "user_agent",
        "insecure_skip_verify",
    )

    # 8. Dual-surface headless tests for the closed items.
    require(
        violations,
        "crates/infiltrator-ports/src/subscription_source.rs",
        "default_conditional_fetch_rejects_unhonored_insecure_flag",
        "default_conditional_fetch_is_a_plain_fetch",
    )
    require(
        violations,
        "crates/infiltrator-application/src/profile_application_tests.rs",
        "conditional_update_sends_stored_validators_and_persists_new_etag",
        "conditional_update_not_modified_keeps_content_and_keeps_last_updated",
        "fetch_settings_are_persisted_and_blank_ua_cleared",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/business_flow/profile_lifecycle.rs",
        "subscription_fetch_options_load_and_not_modified_feedback",
        "subscription_update_toast",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_profiles_fetch_options_projection_restamps_ua_insecure_and_validators",
        "test_profiles_save_fetch_settings_submits_shared_command",
    )

    # 9. DUAL-07-11 "update all subscriptions": one shared bounded-concurrency
    #    batch that both surfaces drive through the same contract/application.
    require(
        violations,
        "crates/infiltrator-contract/src/subscription_import.rs",
        "pub struct SubscriptionBatchReport",
        "pub outcomes: Vec<SubscriptionUpdateReport>",
    )
    require(
        violations,
        "crates/infiltrator-application/src/profile_application.rs",
        "async fn update_all_subscriptions",
        "buffer_unordered",
        "SubscriptionBatchReport",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "UpdateAllSubscriptions",
        "BATCH_UPDATE_CONCURRENCY",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "UpdateAllSubscriptions",
        "RestoreSubscriptionBackup",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/profile/subscription.rs",
        "refresh_all",
        "BATCH_UPDATE_CONCURRENCY",
        "pub(crate) fn batch_update_toast",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/profiles.rs",
        "profiles_update_all",
        "Message::UpdateAllSubscriptionsNow",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_ext.rs",
        "profiles_update_all",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en_ext.rs",
        "profiles_update_all",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "UpdateAllSubscriptions",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/profiles.rs",
        "UpdateAllSubscriptionsButton",
        "on_update_all_subscriptions_activated",
    )

    # 10. DUAL-07-13 "safe config backup": the pre-save `.bak` presence rides the
    #     shared profile projection and both surfaces can restore it.
    require(
        violations,
        "crates/infiltrator-domain/src/profiles.rs",
        "pub has_backup: bool",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub has_backup: bool",
    )
    require(
        violations,
        "crates/infiltrator-application/src/profile_application.rs",
        "async fn restore_backup",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "RestoreSubscriptionBackup",
        "restore_backup",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "has_backup: item.has_backup",
    )
    require(
        violations,
        "crates/mihomo-config/src/manager/profiles.rs",
        "profile.has_backup",
    )
    require(
        violations,
        "crates/mihomo-config/src/profile_store.rs",
        "has_backup: profile.has_backup",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/profile/subscription.rs",
        "restore_backup",
        "profiles_backup_restored",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/profiles.rs",
        "profiles_restore_backup",
        "Message::RestoreSubscriptionBackup",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_ext.rs",
        "profiles_backup_available",
        "profiles_restore_backup",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en_ext.rs",
        "profiles_backup_available",
        "profiles_restore_backup",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "RestoreSubscriptionBackup",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/profiles_import.rs",
        "RestoreSubscriptionBackupButton",
        "SubscriptionBackupStatus",
        "on_restore_subscription_backup",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "has_backup: profile.has_backup",
    )

    # 11. Dual-surface headless tests for the second batch.
    require(
        violations,
        "crates/infiltrator-application/src/profile_application_tests.rs",
        "batch_update_skips_url_less_profiles_and_aggregates_counts",
        "restore_backup_reports_availability_and_is_one_shot",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/business_flow/profile_lifecycle.rs",
        "subscription_batch_update_and_backup_restore_are_shared_application_wired",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_profiles_update_all_toolbar_submits_shared_command",
        "test_profiles_restore_backup_submits_shared_command_and_restamps_status",
    )

    require(
        violations,
        "scripts/test.sh",
        "subscription-lifecycle-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        'subscription-lifecycle-guard.py" --mode enforce',
    )

    # 12. DUAL-07-05/06 "retry/backoff + single-flight": one shared refresh
    #     orchestration that both surfaces drive through the runtime sleep seam.
    require(
        violations,
        "crates/infiltrator-application/src/subscription_refresh_application.rs",
        "pub struct SubscriptionRefreshApplication",
        "pub struct SubscriptionRefreshGuard",
        "pub async fn refresh_profile",
        "pub async fn refresh_all",
        "pub fn begin_refresh",
        "fn inflight_refreshes",
        "ApplicationRuntime",
        "RetryBackoffPolicy",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "with_application_runtime",
        "SubscriptionRefreshApplication",
        "refresh_profile",
        "refresh_all",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/host.rs",
        "pub mod runtime",
        "tokio_application_runtime",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/profile/subscription.rs",
        "SubscriptionRefreshApplication",
        "refresh_profile",
        "refresh_all",
        "application_runtime",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/profiles.rs",
        "UpdateProfileButton",
        "on_update_profile_activated",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "UpdateProfile",
    )
    for composition in (
        "crates/infiltrator-composition/src/lib.rs",
        "crates/infiltrator-desktop/src/composition.rs",
        "crates/infiltrator-android/src/composition.rs",
    ):
        require(violations, composition, "with_application_runtime")
    # The desktop host composes the profile store + subscription source into
    # the shared command path so the Bevy UpdateProfile / batch / fetch-settings
    # commands reach the retry+single-flight application in the shipped app.
    require(
        violations,
        "crates/infiltrator-desktop/src/composition.rs",
        "with_profile(ProfileApplication::new(profile_store))",
        "with_subscription_source(subscription_source)",
    )

    # 13. Dual-surface headless tests for the retry/backoff + single-flight batch.
    require(
        violations,
        "crates/infiltrator-application/src/subscription_refresh_application_test.rs",
        "retries_transient_failures_then_succeeds",
        "backoff_delays_ride_the_injected_runtime_sleep_seam",
        "gives_up_when_the_policy_is_exhausted",
        "refresh_is_single_flight_per_store_and_profile",
        "refresh_all_aggregates_and_skips_url_less_profiles",
        "command_application_update_profile_uses_the_shared_retry_seam",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/business_flow/profile_lifecycle.rs",
        "subscription_refresh_retries_and_single_flights_through_shared_application",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_profiles_update_button_submits_shared_command",
    )

    if violations:
        for violation in violations:
            print(f"subscription-lifecycle-guard: {violation}", file=sys.stderr)
        print(
            f"subscription-lifecycle-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print(
        "subscription-lifecycle-guard: DUAL-07 ledger+shared+dual-surface markers=complete violations=0"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

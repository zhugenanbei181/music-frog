//! test-intent: behavior
use super::{backup_status, conditional_request, filter_status, schedule_status};
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_shared::locales::Lang;

#[test]
fn last_modified_only_cache_is_observed_and_values_are_not_interpolated_twice() {
    let copy = conditional_request(None, Some("Mon {etag}"));
    assert_eq!(
        copy.render(&Lang("en-US")),
        "Conditional request cached · ETag: — · Last-Modified: Mon {etag}"
    );
    assert_eq!(
        copy.render(&Lang("zh-CN")),
        "条件请求已缓存 · ETag: — · Last-Modified: Mon {etag}"
    );
    assert_eq!(
        conditional_request(None, None).render(&Lang("en-US")),
        "Conditional request: no ETag / Last-Modified cached yet"
    );
}

#[test]
fn missing_profile_is_distinct_from_observed_absent_backup_and_filter() {
    assert_eq!(
        backup_status(None).render(&Lang("en-US")),
        "No profile selected"
    );
    assert_eq!(
        backup_status(Some(false)).render(&Lang("en-US")),
        "Safe backup: none yet (created when a config is saved)"
    );
    assert_eq!(
        filter_status(None).render(&Lang("en-US")),
        "No profile selected"
    );
    assert_eq!(
        filter_status(Some(&SubscriptionFilterDraft::default())).render(&Lang("en-US")),
        "Filter pipeline: disabled"
    );
}

#[test]
fn advanced_only_filter_is_active_and_invalid_policy_is_not_reported_as_disabled() {
    let filter = SubscriptionFilterDraft {
        advanced_policy: Some("{drop-private-ip: true}".into()),
        ..Default::default()
    };
    assert!(
        filter_status(Some(&filter))
            .render(&Lang("en-US"))
            .contains("advanced policy: {drop-private-ip: true}")
    );
    assert!(
        filter_status(Some(&filter))
            .render(&Lang("zh-CN"))
            .contains("高级策略")
    );
    for policy in ["{}", "{drop-private-ip: false}", "{node-mutator: {}}"] {
        let filter = SubscriptionFilterDraft {
            advanced_policy: Some(policy.into()),
            ..Default::default()
        };
        assert_eq!(
            filter_status(Some(&filter)).render(&Lang("en-US")),
            "Filter pipeline: disabled"
        );
    }
    let invalid = SubscriptionFilterDraft {
        advanced_policy: Some("{unknown: true}".into()),
        ..Default::default()
    };
    assert!(
        filter_status(Some(&invalid))
            .render(&Lang("en-US"))
            .starts_with("Stored filter policy is invalid:")
    );
}

#[test]
fn filter_summary_preserves_raw_patterns_and_each_rename_dedup_branch() {
    for (renames, dedup, expected) in [
        ("", 0, "no renames · deduplication disabled"),
        ("", 1, "no renames · deduplication: keep first"),
        ("", 2, "no renames · deduplication: keep last"),
        ("", 3, "no renames · deduplication: append index"),
        (
            "pattern=>replacement",
            0,
            "renames configured · deduplication disabled",
        ),
        (
            "pattern=>replacement",
            1,
            "renames configured · deduplication: keep first",
        ),
        (
            "pattern=>replacement",
            2,
            "renames configured · deduplication: keep last",
        ),
        (
            "pattern=>replacement",
            3,
            "renames configured · deduplication: append index",
        ),
    ] {
        let filter = SubscriptionFilterDraft {
            include: r"\{exclude\}".into(),
            exclude: r"\{protocols\}".into(),
            exclude_types: "vmess".into(),
            renames: renames.into(),
            dedup_index: dedup,
            ..Default::default()
        };
        assert_eq!(
            filter_status(Some(&filter)).render(&Lang("en-US")),
            format!(
                r"Filter pipeline: include `\{{exclude\}}` · exclude `\{{protocols\}}` · excluded protocols `vmess` · {expected}"
            )
        );
        assert_eq!(filter.include, r"\{exclude\}");
    }
}

#[test]
fn schedule_reports_paused_configuration_and_does_not_fabricate_an_interval() {
    assert_eq!(
        schedule_status(None, None, None).render(&Lang("en-US")),
        "No profile selected"
    );
    assert_eq!(
        schedule_status(Some(true), None, None).render(&Lang("en-US")),
        "Scheduled updates enabled · schedule not observed"
    );
    assert_eq!(
        schedule_status(Some(false), None, None).render(&Lang("en-US")),
        "Update schedule: manual"
    );
    assert_eq!(
        schedule_status(Some(true), Some("0 */6 * * *"), Some(24)).render(&Lang("en-US")),
        "Update schedule: Cron `0 */6 * * *`"
    );
    assert_eq!(
        schedule_status(Some(false), Some("0 */6 * * *"), Some(24)).render(&Lang("en-US")),
        "Scheduled updates paused · Cron `0 */6 * * *`"
    );
    assert_eq!(
        schedule_status(Some(true), Some(" "), Some(3)).render(&Lang("en-US")),
        "Update schedule: every 3 hours"
    );
    assert_eq!(
        schedule_status(Some(false), None, Some(3)).render(&Lang("en-US")),
        "Scheduled updates paused · every 3 hours"
    );
    assert_eq!(
        schedule_status(Some(true), None, Some(0)).render(&Lang("en-US")),
        "Update schedule: every 0 hours"
    );
}

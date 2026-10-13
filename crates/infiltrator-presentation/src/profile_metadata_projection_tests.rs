use super::*;
#[test]
fn partial_quota_overflow_and_real_zero_never_become_fabricated_usage_or_unlimited() {
    let partial = traffic(None, Some(0), None);
    assert_eq!(
        (
            partial.upload.as_str(),
            partial.download.as_str(),
            partial.used.as_str(),
            partial.total.as_str()
        ),
        ("—", "0 B", "—", "—")
    );
    assert_eq!(partial.fraction, None);
    let zero = traffic(Some(0), Some(0), Some(0));
    assert_eq!((zero.used.as_str(), zero.total.as_str()), ("0 B", "0 B"));
    assert_eq!(zero.grade, UsageGrade::Unknown);
    assert_eq!(traffic(Some(u64::MAX), Some(1), Some(u64::MAX)).used, "—");
    assert_eq!(
        traffic(Some(0), Some(90), Some(100)).grade,
        UsageGrade::High
    );
    assert!(traffic_caption(&partial, "en-US").contains("Used: — / Total: —"));
}
#[test]
fn missing_schedule_is_not_cron_and_opaque_names_and_next_times_are_not_reinterpreted() {
    assert_eq!(
        schedule_overview([(true, None, None)], "en-US"),
        "Auto update: 1 enabled; 1 schedules not observed"
    );
    assert_eq!(
        schedule_overview([(true, Some("0 0 * * *"), None)], "en-US"),
        "Auto update: 1 enabled; Cron schedules"
    );
    assert_eq!(
        summary(1, Some("profile {count}/中文🙂"), "en-US"),
        "Profiles · 1 profiles (active: profile {count}/中文🙂)"
    );
    assert!(
        schedule(true, None, Some(2), Some("{cadence}/🙂"), "en-US")
            .ends_with("Next update {cadence}/🙂")
    );
}

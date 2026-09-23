use super::*;

#[test]
fn quota_status_label_has_no_fake_success_for_missing_metadata() {
    let lang = Lang("en-US");
    let snapshot = SubscriptionQuotaSnapshot::unsupported(1, 1, "provider unavailable");
    assert_eq!(status_label(&snapshot, &lang), "provider unavailable");
    assert_eq!(status_kind(snapshot.status), BadgeKind::Neutral);
}

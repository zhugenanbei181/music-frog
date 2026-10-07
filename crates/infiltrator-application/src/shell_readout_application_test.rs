//! test-intent: behavior
use super::*;
use crate::shell_readout_projection::{
    count_copy, profile_name, profile_percent, profile_usage, rate_copy,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{
    PageId, ProfileSnapshot, ProfilesPageSnapshot, RulesPageSnapshot,
};
use serde_json::{from_value, json};

fn snapshot() -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::unsupported("not composed"),
    );
    snapshot.generation = 1;
    snapshot.revision = 1;
    snapshot.core.generation = 1;
    snapshot.core.lifecycle = CoreLifecycle::Running;
    snapshot.core.failure = None;
    snapshot
}
fn profile(id: &str) -> ProfileSnapshot {
    from_value(json!({"id":id,"name":"Home {count}","url":"https://example.test/profile","updated_at":"", "upload_bytes":0,"download_bytes":0,"total_bytes":0,"is_active":true})).unwrap()
}
fn profiles(profile: ProfileSnapshot) -> PageData<ProfilesPageSnapshot> {
    PageData::ready(
        from_value(json!({"profiles":[profile],"auto_update_interval_hours":0,"updating":false}))
            .unwrap(),
    )
}
fn rules(total: usize) -> RulesPageSnapshot {
    from_value(json!({"total_rules":total,"default_action":"DIRECT","rules":[],"providers":[]}))
        .unwrap()
}

#[test]
fn complete_rule_count_zero_and_stale_observations_do_not_follow_the_render_window_or_defaults() {
    let owner = ShellReadoutApplication::default();
    let mut source = snapshot();
    let unknown = owner.project(&source);
    assert_eq!(count_copy(&unknown, PageId::Rules, "en-US"), "Not observed");
    source.pages.rules = PageData::ready(rules(50_000));
    source.revision += 1;
    let full = owner.project(&source);
    assert_eq!(count_copy(&full, PageId::Rules, "en-US"), "50000");
    source.pages.rules = PageData::failed(Failure::new(ErrorCode::Permission, "denied", true));
    source.revision += 1;
    let stale = owner.project(&source);
    assert_eq!(count_copy(&stale, PageId::Rules, "en-US"), "50000 (stale)");
    source.pages.rules = PageData::empty(rules(0));
    source.revision += 1;
    assert_eq!(
        count_copy(&owner.project(&source), PageId::Rules, "en-US"),
        "0"
    );
    source.revision -= 1;
    source.pages.rules = PageData::ready(rules(999));
    assert_eq!(owner.project(&source).rules.value, Some(0));
}

#[test]
fn quota_missing_fields_profile_switch_failed_read_and_locale_keep_only_the_matching_observation() {
    let owner = ShellReadoutApplication::default();
    let mut source = snapshot();
    source.pages.profiles = profiles(profile("first"));
    source.subscription_quota.profile_name = Some("Home {count}".into());
    source.subscription_quota.status = SubscriptionQuotaStatus::Empty;
    let missing = owner.project(&source);
    assert_eq!(profile_name(&missing, "en-US"), "Home {count}");
    assert_eq!(profile_usage(&missing, "en-US"), "Not observed");
    assert!(
        missing
            .profile
            .value
            .as_ref()
            .unwrap()
            .usage_fraction
            .is_none()
    );
    source.subscription_quota.status = SubscriptionQuotaStatus::Ready;
    source.subscription_quota.used_bytes = Some(512);
    source.subscription_quota.total_bytes = Some(1024);
    source.subscription_quota.usage_percent = Some(50.0);
    source.revision += 1;
    let observed = owner.project(&source);
    assert_eq!(profile_usage(&observed, "en-US"), "512 B / 1.00 KB");
    assert_eq!(profile_percent(&observed, "en-US"), "50%");
    source.pages.profiles = PageData::loading();
    source.revision += 1;
    let retained = owner.project(&source);
    assert_eq!(profile_usage(&retained, "en-US"), "512 B / 1.00 KB (stale)");
    assert_eq!(profile_name(&retained, "zh-CN"), "Home {count}（已失效）");
    source.pages.profiles = profiles(profile("second"));
    source.subscription_quota.status = SubscriptionQuotaStatus::Failed;
    source.revision += 1;
    let changed = owner.project(&source);
    assert_eq!(changed.profile.value.as_ref().unwrap().id, "second");
    assert_eq!(profile_usage(&changed, "en-US"), "Not observed");
    source.pages.profiles = PageData::empty(
        from_value(json!({"profiles":[],"auto_update_interval_hours":0,"updating":false})).unwrap(),
    );
    source.revision += 1;
    assert!(owner.project(&source).profile.value.is_none());
}

#[test]
fn observed_zero_rate_invalid_sample_and_new_generation_are_distinct() {
    let owner = ShellReadoutApplication::default();
    let mut source = snapshot();
    assert_eq!(
        rate_copy(&owner.project(&source).upload_bps, "en-US"),
        "Not observed"
    );
    source.core.sampled_at_epoch_ms = Some(10);
    source.revision += 1;
    assert_eq!(
        rate_copy(&owner.project(&source).upload_bps, "en-US"),
        "0 B/s"
    );
    source.core.upload_bps = f64::NAN;
    source.revision += 1;
    assert_eq!(
        rate_copy(&owner.project(&source).upload_bps, "en-US"),
        "0 B/s (stale)"
    );
    source.core.generation = 2;
    source.generation = 2;
    source.revision += 1;
    assert_eq!(
        rate_copy(&owner.project(&source).upload_bps, "en-US"),
        "Not observed"
    );
}

#[test]
fn failed_reads_preserve_zero_as_stale_and_same_generation_new_tokens_retire_old_rates() {
    let owner = ShellReadoutApplication::default();
    let mut source = snapshot();
    source.core.session_token = Some(SessionToken::new(12));
    source.core.sampled_at_epoch_ms = Some(10);
    assert_eq!(
        rate_copy(&owner.project(&source).upload_bps, "en-US"),
        "0 B/s"
    );
    source.shell_readout.rate_failure = Some(Failure::new(ErrorCode::Permission, "denied", false));
    source.revision += 1;
    let failed = owner.project(&source);
    assert_eq!(rate_copy(&failed.upload_bps, "en-US"), "0 B/s (stale)");
    assert_eq!(
        failed.rate_failure.as_ref().unwrap().code,
        ErrorCode::Permission
    );
    source.core.session_token = Some(SessionToken::new(13));
    source.revision += 1;
    let changed = owner.project(&source);
    assert_eq!(rate_copy(&changed.upload_bps, "en-US"), "Not observed");
    assert_eq!(changed.session_token, Some(SessionToken::new(13)));
    assert!(changed.upload_bps.value.is_none());
}

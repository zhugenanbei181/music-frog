//! Quota source changes, failed reads, real zero and actual request ordering.
//! test-intent: behavior
use super::*;
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::CoreLifecycle;
fn core(generation: u64, token: Option<SessionToken>) -> CoreSnapshot {
    CoreSnapshot {
        lifecycle: CoreLifecycle::Stopped,
        generation,
        session_token: token,
        revision: 1,
        proxy_mode: None,
        core_version: None,
        sampled_at_epoch_ms: None,
        failure: None,
        upload_bps: 0.0,
        download_bps: 0.0,
        active_connections: 0,
        memory_bytes: None,
        watchdog: Default::default(),
    }
}
fn profile(name: &str, used: u64) -> ProfileInfo {
    ProfileInfo {
        name: name.into(),
        path: format!("/isolated/{name}.yaml"),
        active: true,
        subscription_url: Some("https://provider.test/account".into()),
        traffic_upload: Some(used),
        traffic_download: Some(0),
        traffic_total: Some(100),
        ..Default::default()
    }
}
fn identity(name: &str) -> QuotaProfileIdentity {
    QuotaProfileIdentity {
        profile: name.into(),
        provider_hash: Some(hash_document_bytes("https://provider.test/account")),
    }
}
fn observe(
    owner: &SubscriptionQuotaApplication,
    core: &CoreSnapshot,
    name: &str,
    profiles: Result<Vec<ProfileInfo>, Failure>,
) -> SubscriptionQuotaSnapshot {
    owner.project_at(
        &owner.begin(),
        core,
        Some(&Ok(identity(name))),
        Some(&profiles),
        Some(1_700_000_000),
    )
}
#[test]
fn missing_profile_application_is_typed_unsupported() {
    let owner = SubscriptionQuotaApplication::default();
    let snapshot = owner.project_at(&owner.begin(), &core(1, None), None, None, Some(1));
    assert_eq!(snapshot.status, SubscriptionQuotaStatus::Unsupported);
    assert_eq!(snapshot.failure.unwrap().code, ErrorCode::Unsupported);
    assert!(snapshot.source.is_none());
}
#[test]
fn same_source_permission_failure_retains_actual_zero_and_recovery_replaces_it_without_a_sample_default()
 {
    let owner = SubscriptionQuotaApplication::default();
    let context = core(2, Some(SessionToken::new(22)));
    let observed = observe(&owner, &context, "alpha", Ok(vec![profile("alpha", 0)]));
    assert_eq!(observed.used_bytes, Some(0));
    let failure = Failure::new(ErrorCode::Permission, "denied {reason}", true);
    let failed = observe(&owner, &context, "alpha", Err(failure.clone()));
    assert_eq!(failed.source, observed.source);
    assert_eq!(failed.used_bytes, Some(0));
    assert_eq!(failed.total_bytes, Some(100));
    assert_eq!(failed.failure, Some(failure));
    assert!(failed.retained);
    assert_eq!(failed.status, SubscriptionQuotaStatus::Failed);
    let recovered = observe(&owner, &context, "alpha", Ok(vec![profile("alpha", 25)]));
    assert_eq!(recovered.used_bytes, Some(25));
    assert!(!recovered.retained);
    assert!(recovered.failure.is_none());
    assert_eq!(recovered.status, SubscriptionQuotaStatus::Ready);
}
#[test]
fn profile_switch_unknown_pointer_generation_and_session_changes_never_borrow_old_quota() {
    let failure = Failure::new(ErrorCode::Storage, "metadata read failed", true);
    for context in [
        core(3, Some(SessionToken::new(31))),
        core(4, Some(SessionToken::new(30))),
        core(3, None),
    ] {
        let owner = SubscriptionQuotaApplication::default();
        observe(
            &owner,
            &core(3, Some(SessionToken::new(30))),
            "alpha",
            Ok(vec![profile("alpha", 40)]),
        );
        let failed = observe(&owner, &context, "alpha", Err(failure.clone()));
        assert!(failed.used_bytes.is_none());
        assert!(failed.source.is_none());
        assert!(!failed.retained);
    }
    let owner = SubscriptionQuotaApplication::default();
    let context = core(3, None);
    observe(&owner, &context, "alpha", Ok(vec![profile("alpha", 40)]));
    let switched = observe(&owner, &context, "beta", Err(failure.clone()));
    assert!(switched.source.is_none());
    assert!(switched.used_bytes.is_none());
    observe(&owner, &context, "alpha", Ok(vec![profile("alpha", 40)]));
    let unknown = owner.project_at(
        &owner.begin(),
        &context,
        Some(&Err(failure.clone())),
        Some(&Err(failure.clone())),
        Some(1),
    );
    assert!(unknown.source.is_none());
    assert!(unknown.used_bytes.is_none());
    assert_eq!(unknown.failure, Some(failure));
}
#[test]
fn late_or_foreign_requests_cannot_replace_a_newer_observation_even_with_the_same_generation() {
    let owner = SubscriptionQuotaApplication::default();
    let peer = SubscriptionQuotaApplication::default();
    let context = core(5, None);
    let old = owner.begin();
    let newer = owner.begin();
    let current = owner.project_at(
        &newer,
        &context,
        Some(&Ok(identity("beta"))),
        Some(&Ok(vec![profile("beta", 25)])),
        Some(1),
    );
    let late = owner.project_at(
        &old,
        &context,
        Some(&Ok(identity("alpha"))),
        Some(&Ok(vec![profile("alpha", 99)])),
        Some(1),
    );
    assert_eq!(late, current);
    let foreign = owner.project_at(
        &peer.begin(),
        &context,
        Some(&Ok(identity("alpha"))),
        Some(&Ok(vec![profile("alpha", 99)])),
        Some(1),
    );
    assert_eq!(foreign, current);
    let isolated = observe(
        &peer,
        &context,
        "beta",
        Err(Failure::new(ErrorCode::Permission, "denied", true)),
    );
    assert!(isolated.used_bytes.is_none());
}
#[test]
fn successful_empty_metadata_retires_prior_values_but_overflow_is_a_failed_observation() {
    let owner = SubscriptionQuotaApplication::default();
    let context = core(6, None);
    observe(&owner, &context, "alpha", Ok(vec![profile("alpha", 30)]));
    let mut invalid = profile("alpha", u64::MAX);
    invalid.traffic_download = Some(1);
    let failed = observe(&owner, &context, "alpha", Ok(vec![invalid]));
    assert_eq!(failed.used_bytes, Some(30));
    assert!(failed.retained);
    assert_eq!(failed.failure.unwrap().code, ErrorCode::InvalidInput);
    let empty = observe(
        &owner,
        &context,
        "alpha",
        Ok(vec![ProfileInfo {
            name: "alpha".into(),
            path: "/isolated/alpha.yaml".into(),
            active: true,
            subscription_url: Some("https://provider.test/account".into()),
            ..Default::default()
        }]),
    );
    assert_eq!(empty.status, SubscriptionQuotaStatus::Empty);
    assert!(empty.used_bytes.is_none());
    assert!(!empty.retained);
}

#[test]
fn replacement_path_with_the_same_profile_name_cannot_inherit_failed_old_values() {
    let owner = SubscriptionQuotaApplication::default();
    let context = core(7, None);
    observe(&owner, &context, "alpha", Ok(vec![profile("alpha", 30)]));
    let mut replacement = profile("alpha", u64::MAX);
    replacement.path = "/replacement/alpha.yaml".into();
    replacement.traffic_download = Some(1);
    let failed = observe(&owner, &context, "alpha", Ok(vec![replacement]));
    assert!(failed.used_bytes.is_none());
    assert!(failed.total_bytes.is_none());
    assert!(!failed.retained);
}

#[test]
fn changed_or_unknown_provider_identity_cannot_reuse_another_accounts_observation() {
    let owner = SubscriptionQuotaApplication::default();
    let context = core(8, None);
    observe(&owner, &context, "alpha", Ok(vec![profile("alpha", 30)]));
    let failure = Failure::new(ErrorCode::Storage, "quota read denied", true);
    let replacement = QuotaProfileIdentity {
        profile: "alpha".into(),
        provider_hash: Some(hash_document_bytes("https://other-provider.test/account")),
    };
    let changed = owner.project_at(
        &owner.begin(),
        &context,
        Some(&Ok(replacement)),
        Some(&Err(failure.clone())),
        Some(1),
    );
    assert!(changed.used_bytes.is_none());
    assert!(!changed.retained);
    observe(&owner, &context, "alpha", Ok(vec![profile("alpha", 30)]));
    let unknown = QuotaProfileIdentity {
        profile: "alpha".into(),
        provider_hash: None,
    };
    let failed = owner.project_at(
        &owner.begin(),
        &context,
        Some(&Ok(unknown)),
        Some(&Err(failure)),
        Some(1),
    );
    assert!(failed.used_bytes.is_none());
    assert!(!failed.retained);
}

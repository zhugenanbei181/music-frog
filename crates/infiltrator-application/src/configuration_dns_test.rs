//! test-intent: behavior
use super::*;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceWriteAuthorization;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceCasOutcome, ActiveProfileReferenceTarget,
};
use infiltrator_contract::dns_hosts::DnsHostEntry;
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_test_support::dns_hosts_fixtures::{
    HOSTS_PROFILE, HostsCaptureStore, ORIGINAL_HOSTS,
};
use serde_yaml_ng::Value;
use std::sync::Arc;
use std::sync::atomic::Ordering;
async fn select_profile(store: &HostsCaptureStore, profile: &str) {
    let expected = ProfileStore::read_active_profile_reference(store)
        .await
        .expect("read selection before test CAS");
    let target = ActiveProfileReferenceTarget::selected(
        ProfileStore::engine_id(store),
        ProfileId::new(profile).expect("valid test profile ID"),
    );
    match ProfileStore::compare_and_set_active_profile_reference(
        store,
        expected,
        target,
        ActiveProfileReferenceWriteAuthorization::Unowned,
    )
    .await
    .expect("revisioned test selection CAS")
    {
        ActiveProfileReferenceCasOutcome::Committed { .. } => {}
        ActiveProfileReferenceCasOutcome::Stale { expected, observed } => {
            panic!("test selection unexpectedly stale: {expected:?} -> {observed:?}")
        }
    }
}
fn row(domain: &str, address: &str) -> DnsHostEntry {
    DnsHostEntry {
        domain: domain.into(),
        address: address.into(),
    }
}
#[tokio::test]
async fn root_write_legacy_preview_permission_retry_and_clear_use_actual_profile_bytes() {
    let store = Arc::new(HostsCaptureStore::default());
    let application = ConfigurationApplication::test_configuration_application(store.clone());
    let observed = application.load_hosts_profile().await.unwrap();
    assert_eq!(observed.profile, HOSTS_PROFILE);
    assert_eq!(observed.entries.len(), 2);
    assert_eq!(observed.legacy_entries, vec![row("legacy.test", "1.1.1.1")]);
    let mut rows = observed.entries.clone();
    rows.extend(observed.legacy_entries.clone());
    let patch = DnsSettingsPatch {
        expected_profile: Some(HOSTS_PROFILE.into()),
        expected_hosts: Some(observed.entries.clone()),
        hosts: Some(rows.clone()),
        remove_legacy_hosts: true,
        ..Default::default()
    };
    store.deny_save.store(true, Ordering::SeqCst);
    assert_eq!(
        application
            .apply_dns_settings(patch.clone())
            .await
            .unwrap_err()
            .code,
        ErrorCode::Permission
    );
    assert_eq!(store.content(), ORIGINAL_HOSTS);
    assert_eq!(
        application
            .load_hosts_profile()
            .await
            .unwrap()
            .entries
            .len(),
        2
    );
    store.deny_save.store(false, Ordering::SeqCst);
    application.apply_dns_settings(patch).await.unwrap();
    let doc: Value = serde_yaml_ng::from_str(&store.content()).unwrap();
    assert_eq!(doc["hosts"]["legacy.test"].as_str(), Some("1.1.1.1"));
    assert!(doc["dns"].get("hosts").is_none());
    assert_eq!(doc["dns"]["future-setting"].as_str(), Some("preserve-me"));
    assert_eq!(doc["mixed-port"].as_u64(), Some(7890));
    let observed = application.load_hosts_profile().await.unwrap();
    assert_eq!(observed.entries.len(), 3);
    assert!(observed.legacy_entries.is_empty());
    application
        .apply_dns_settings(DnsSettingsPatch {
            expected_profile: Some(HOSTS_PROFILE.into()),
            expected_hosts: Some(observed.entries),
            clear_hosts: true,
            ..Default::default()
        })
        .await
        .unwrap();
    let doc: Value = serde_yaml_ng::from_str(&store.content()).unwrap();
    assert!(doc.get("hosts").is_none());
    assert_eq!(doc["mixed-port"].as_u64(), Some(7890));
}
#[tokio::test]
async fn stale_profile_stale_root_invalid_rows_and_read_failure_cannot_write_or_hide_categories() {
    let store = Arc::new(HostsCaptureStore::default());
    let application = ConfigurationApplication::test_configuration_application(store.clone());
    let observed = application.load_hosts_profile().await.unwrap();
    let patch = DnsSettingsPatch {
        expected_profile: Some(HOSTS_PROFILE.into()),
        expected_hosts: Some(observed.entries),
        hosts: Some(vec![row("new.test", "1.2.3.4")]),
        ..Default::default()
    };
    select_profile(&store, "other").await;
    assert_eq!(
        application
            .apply_dns_settings(patch.clone())
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidState
    );
    select_profile(&store, HOSTS_PROFILE).await;
    store.replace("hosts:\n  changed.test: 7.7.7.7\n");
    assert_eq!(
        application
            .apply_dns_settings(patch)
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidState
    );
    assert_eq!(
        application
            .apply_dns_settings(DnsSettingsPatch {
                hosts: Some(vec![row(" ", "1.1.1.1")]),
                ..Default::default()
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    store.deny_read.store(true, Ordering::SeqCst);
    assert_eq!(
        application.load_hosts_profile().await.unwrap_err().code,
        ErrorCode::Permission
    );
    store.deny_read.store(false, Ordering::SeqCst);
    store.replace("hosts:\n  bad.test: true\n");
    assert_eq!(
        application.load_hosts_profile().await.unwrap_err().code,
        ErrorCode::Configuration
    );
}

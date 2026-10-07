//! Behavior cases for rules provider.
//! test-intent: behavior

use super::*;
use crate::view::rules::rules_list::publish_truncation_line;
use infiltrator_contract::provider_cache::{
    KernelEtagSupportSnapshot, KernelEtagSupportState, ProviderCacheFingerprint,
    ProviderFileFingerprint, ProviderFingerprintChange,
};
use infiltrator_domain::rules::view::RULE_PUBLISH_LIMIT;
use infiltrator_shared::locales::Lang;

/// DUAL-11-05/11-08: the provider's declared refresh interval and the shared
/// publish-truncation fact are projected from one surface snapshot.
#[test]
fn test_rules_provider_interval_and_publish_truncation_project_from_snapshot() {
    use infiltrator_contract::error::{ErrorCode, Failure};
    use infiltrator_contract::rule_provider_snapshot::RuleProviderSnapshot;
    use infiltrator_contract::surface::{HostKind, SurfaceKind};
    use infiltrator_contract::surface_snapshot::{PageData, RulesPageSnapshot, SurfaceSnapshot};

    let (mut state, _) = AppState::new();
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "test snapshot", true),
    );
    snapshot.revision = 7;
    snapshot.pages.rules = PageData::ready(RulesPageSnapshot {
        document: None,
        total_rules: 12_000,
        default_action: "DIRECT".to_owned(),
        providers: vec![RuleProviderSnapshot {
            name: "ads".to_owned(),
            rule_count: 120,
            behavior: "domain".to_owned(),
            updated_at: "2026-09-01".to_owned(),
            source_url: Some("https://example.com/ads.mrs".to_owned()),
            refresh_interval_secs: Some(86_400),
            cache_fingerprint: Some(ProviderCacheFingerprint {
                provider: "ads".to_owned(),
                path: "/home/u/.config/mihomo-rs/rules/1f0f1c3f0d0f0a0b".to_owned(),
                change: ProviderFingerprintChange::Changed,
                current: ProviderFileFingerprint {
                    size_bytes: 4_096,
                    sha256: "abcdef0123456789".to_owned(),
                    modified_unix_secs: Some(1_700_000_000),
                },
                previous: Some(ProviderFileFingerprint {
                    size_bytes: 2_048,
                    sha256: "0123456789abcdef".to_owned(),
                    modified_unix_secs: Some(1_699_000_000),
                }),
            }),
        }],
        rules: vec![Default::default(); 5_000],
        tracer: Default::default(),
        mrs_acceleration: Default::default(),
        hit_audit: None,
        rule_publish_limit: RULE_PUBLISH_LIMIT,
        provider_cache: Default::default(),
        etag_support: KernelEtagSupportSnapshot::from_declared(Some(true)),
        json_documents: Vec::new(),
    });
    assert!(state.apply_shared_surface_snapshot(snapshot));

    // DUAL-11-05: the declared interval reaches the surface, keyed by provider.
    assert_eq!(
        state.editor.rule_provider_intervals.get("ads").copied(),
        Some(86_400)
    );
    // DUAL-11-05: the local cache fingerprint observation (a real file read,
    // compared with the previous observation) reaches the surface too.
    let observed = state
        .editor
        .rule_provider_fingerprints
        .get("ads")
        .expect("fingerprint observation");
    assert_eq!(observed.change_token(), "changed");
    assert_eq!(observed.current.size_bytes, 4_096);
    assert_eq!(
        observed.previous.as_ref().map(|fact| fact.size_bytes),
        Some(2_048)
    );
    // DUAL-11-05: the kernel's real `etag-support` declaration is projected from
    // the same shared read model; the state and the raw declared value survive.
    assert_eq!(
        state.editor.rule_etag_support.state,
        KernelEtagSupportState::Enabled
    );
    assert_eq!(state.editor.rule_etag_support.declared, Some(true));
    // DUAL-11-08: the publish cap and the omitted count are honest facts.
    assert_eq!(state.editor.rule_publish_limit, RULE_PUBLISH_LIMIT);
    assert_eq!(state.editor.rule_publish_omitted, Some(7_000));
    let lang = Lang("en");
    let note = publish_truncation_line(&state, &lang)
        .expect("truncation note while the shared view is capped");
    assert!(note.contains("7000"), "{note}");
    assert!(note.contains("5000"), "{note}");

    // A complete published list renders no truncation note.
    let mut complete = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "test snapshot", true),
    );
    complete.revision = 8;
    complete.pages.rules = PageData::ready(RulesPageSnapshot {
        document: None,
        total_rules: 5,
        default_action: "DIRECT".to_owned(),
        providers: Vec::new(),
        rules: vec![Default::default(); 5],
        tracer: Default::default(),
        mrs_acceleration: Default::default(),
        hit_audit: None,
        rule_publish_limit: RULE_PUBLISH_LIMIT,
        provider_cache: Default::default(),
        etag_support: Default::default(),
        json_documents: Vec::new(),
    });
    assert!(state.apply_shared_surface_snapshot(complete));
    assert_eq!(state.editor.rule_publish_omitted, None);
    assert!(publish_truncation_line(&state, &lang).is_none());
}

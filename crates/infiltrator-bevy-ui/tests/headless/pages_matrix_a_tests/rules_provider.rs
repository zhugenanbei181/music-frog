//! Behavior cases for rules provider.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;

#[test]
fn test_rules_provider_lifecycle_renders_shared_source_url() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Rules);

    let mut projection = RulesProjection::demo();
    projection.providers[0].source_url = Some("https://example.com/geo.mrs".to_owned());
    projection.providers[0].refresh_interval_secs = Some(86_400);
    projection.providers[1].source_url = None;
    projection.providers[1].refresh_interval_secs = None;
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection));
    app.update();

    // DUAL-11-04: declared URL is shown; runtime-only providers stay honest.
    assert!(subtree_has_text(
        app.world(),
        root,
        "更新: 2026-09-02 06:00 · 来源: https://example.com/geo.mrs · 自动刷新: 1d (内核调度)"
    ));
    assert!(subtree_has_text(app.world(), root, "来源: 未声明"));
    // DUAL-11-05: the declared automatic-refresh interval is rendered as the
    // kernel-scheduled fact; no cache hit/miss state is invented.
    assert!(subtree_has_text(
        app.world(),
        root,
        "自动刷新: 1d (内核调度)"
    ));
    assert!(subtree_has_text(app.world(), root, "自动刷新: 未声明"));
}

#[test]
fn test_rules_provider_local_cache_fingerprint_renders_non_etag_label() {
    use infiltrator_contract::provider_cache::{
        ProviderCacheFingerprint, ProviderFileFingerprint, ProviderFingerprintChange,
    };

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Rules);

    let mut projection = RulesProjection::demo();
    projection.providers[0].cache_fingerprint = Some(ProviderCacheFingerprint {
        provider: "geosite-geolocation-!cn".to_owned(),
        path: "/home/u/.config/mihomo-rs/rules/8f14e45fceea167a5a36dedd4bea2543".to_owned(),
        change: ProviderFingerprintChange::FirstSeen,
        current: ProviderFileFingerprint {
            size_bytes: 4_096,
            sha256: "abcdef0123456789deadbeef".to_owned(),
            modified_unix_secs: Some(1_700_000_000),
        },
        previous: None,
    });
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection));
    app.update();

    // DUAL-11-05: the local file facts are rendered verbatim and the label
    // says what they are (a local fingerprint, not an HTTP ETag).
    assert!(subtree_has_text(
        app.world(),
        root,
        "本地缓存内容指纹（非 HTTP ETag）: sha256:abcdef012345… · 4096 B · mtime 2023-11-14 22:13:20 UTC · 首次观测"
    ));

    // The other two comparison tokens are rendered for their states too.
    let mut projection = RulesProjection::demo();
    projection.providers[0].cache_fingerprint = Some(ProviderCacheFingerprint {
        provider: "geoip-cn".to_owned(),
        path: "/home/u/.config/mihomo-rs/rules/deadbeef".to_owned(),
        change: ProviderFingerprintChange::Unchanged,
        current: ProviderFileFingerprint {
            size_bytes: 850,
            sha256: "0123456789abcdef".to_owned(),
            modified_unix_secs: Some(1_700_000_100),
        },
        previous: Some(ProviderFileFingerprint {
            size_bytes: 850,
            sha256: "0123456789abcdef".to_owned(),
            modified_unix_secs: Some(1_700_000_000),
        }),
    });
    projection.providers[1].cache_fingerprint = Some(ProviderCacheFingerprint {
        provider: "geoip-cn".to_owned(),
        path: "/home/u/.config/mihomo-rs/rules/deadbeef".to_owned(),
        change: ProviderFingerprintChange::Changed,
        current: ProviderFileFingerprint {
            size_bytes: 900,
            sha256: "fedcba9876543210".to_owned(),
            modified_unix_secs: None,
        },
        previous: Some(ProviderFileFingerprint {
            size_bytes: 850,
            sha256: "0123456789abcdef".to_owned(),
            modified_unix_secs: Some(1_700_000_000),
        }),
    });
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection));
    app.update();
    assert!(subtree_has_text(app.world(), root, "较上次观测未变化"));
    assert!(subtree_has_text(app.world(), root, "较上次观测已变化"));
    // A host without a modification time publishes two real facts, no date.
    assert!(subtree_has_text(
        app.world(),
        root,
        "sha256:fedcba987654… · 900 B · 较上次观测已变化"
    ));
}

#[test]
fn test_rules_provider_etag_support_renders_declared_kernel_capability() {
    use infiltrator_contract::provider_cache::KernelEtagSupportSnapshot;

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Rules);

    // DUAL-11-05: the kernel's real top-level `etag-support` declaration is
    // rendered on the providers card for each of the three honest states.
    let mut enabled = RulesProjection::demo();
    enabled.etag_support = KernelEtagSupportSnapshot::from_declared(Some(true));
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(enabled));
    app.update();
    assert!(subtree_has_text(app.world(), root, "ETag 缓存: 内核已启用"));

    let mut disabled = RulesProjection::demo();
    disabled.etag_support = KernelEtagSupportSnapshot::from_declared(Some(false));
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(disabled));
    app.update();
    assert!(subtree_has_text(app.world(), root, "ETag 缓存: 内核未启用"));

    let mut absent = RulesProjection::demo();
    absent.etag_support = KernelEtagSupportSnapshot::from_declared(None);
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(absent));
    app.update();
    assert!(subtree_has_text(app.world(), root, "ETag 缓存: 未声明"));
    // The declaration line never claims a per-request 304 outcome.
    assert!(!subtree_has_text(app.world(), root, "ETag 缓存: 304"));
}

/// DUAL-11-06/07: the MRS card's unpack and purge buttons submit the shared
/// command intents and never a UI-local fabricated payload.
#[test]
fn test_rules_provider_unpack_and_cache_purge_submit_shared_intents() {
    use infiltrator_bevy_ui::pages::rules_mrs::{
        PurgeRuleProviderCacheButton, RulesMrsState, UnpackRuleProviderButton,
    };

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Rules);

    let unpack = app
        .world_mut()
        .query_filtered::<Entity, With<UnpackRuleProviderButton>>()
        .single(app.world())
        .expect("unpack button");
    let purge = app
        .world_mut()
        .query_filtered::<Entity, With<PurgeRuleProviderCacheButton>>()
        .single(app.world())
        .expect("purge button");

    // The unpack target is the provider the shared MRS read model reports.
    let state = app.world().resource::<RulesMrsState>();
    let provider = state.provider_name.clone().expect("projected provider");
    assert_eq!(
        provider, "geoip-cn.mrs",
        "the first shared MRS item names the unpack target"
    );

    app.world_mut()
        .commands()
        .trigger(Activate { entity: unpack });
    app.update();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: purge });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![
            UiCommand::UnpackRuleProvider(provider),
            UiCommand::PurgeRuleProviderCache,
        ]
    );
}

/// DUAL-11-07: the cache fact line reports the observed directory/count/size
/// and says so when the host has no cache location.
#[test]
fn test_rules_provider_cache_line_reports_observed_facts() {
    use infiltrator_contract::provider_cache::RuleProviderCacheSnapshot;

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Rules);

    let mut projection = RulesProjection::demo();
    projection.provider_cache = RuleProviderCacheSnapshot::ready("/kernel/configs/rules", 4, 8192);
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection.clone()));
    app.update();
    assert!(subtree_has_text(
        app.world(),
        root,
        "/kernel/configs/rules · 4 个缓存文件 · 8192 字节"
    ));

    projection.provider_cache = RuleProviderCacheSnapshot::ready("/kernel/configs/rules", 0, 0);
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection.clone()));
    app.update();
    assert!(subtree_has_text(
        app.world(),
        root,
        "/kernel/configs/rules · 0 个缓存文件 · 0 字节"
    ));

    projection.provider_cache = RuleProviderCacheSnapshot::unsupported("no kernel home");
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection.clone()));
    app.update();
    assert!(subtree_has_text(
        app.world(),
        root,
        "宿主未声明规则集缓存目录: no kernel home"
    ));

    projection.provider_cache = RuleProviderCacheSnapshot::failed("permission denied");
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection));
    app.update();
    assert!(subtree_has_text(
        app.world(),
        root,
        "规则集缓存不可读: permission denied"
    ));
}

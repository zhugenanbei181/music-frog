//! test-intent: behavior
use super::{build_settings_page, build_sync_page};
use crate::runtime_control_projection::RuntimeControlApplication;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface_snapshot::PageStatus;
use infiltrator_contract::sync::SyncStatus;
use infiltrator_domain::runtime::{ConfigSnapshot, TunSnapshot};
use infiltrator_domain::settings::AppSettings;
use infiltrator_ports::error::PortError;

#[test]
fn preference_flags_are_observed_false_values_and_failed_reads_cannot_invent_enabled_defaults() {
    let settings = Ok(AppSettings {
        close_to_tray: false,
        notifications_enabled: false,
        ..AppSettings::default()
    });
    let page = build_settings_page(
        Some(&settings),
        &Default::default(),
        &Default::default(),
        Default::default(),
    );
    let facts = page.data.unwrap();
    assert_eq!(facts.close_to_tray, Some(false));
    assert_eq!(facts.notifications_enabled, Some(false));
    let failed = Err(Failure::new(ErrorCode::Storage, "preferences denied", true));
    let page = build_settings_page(
        Some(&failed),
        &Default::default(),
        &Default::default(),
        Default::default(),
    );
    assert!(page.data.is_none());
    assert!(
        matches!(page.status, PageStatus::Failed { failure } if failure.message == "preferences denied")
    );
}

#[test]
fn preference_success_preserves_missing_runtime_fields_and_failed_reads_keep_only_same_generation_facts()
 {
    let prefs = Ok(AppSettings::default());
    let owner = RuntimeControlApplication::default();
    let page = build_settings_page(
        Some(&prefs),
        &owner.observe(1, 1, None),
        &Default::default(),
        Default::default(),
    );
    assert_eq!(page.status, PageStatus::Ready);
    let unknown = page.data.unwrap();
    assert_eq!(unknown.mixed_port, None);
    assert_eq!(unknown.allow_lan, None);
    assert_eq!(unknown.lan_bind_address, None);
    assert_eq!(unknown.lan_security, None);
    assert_eq!(unknown.ipv6_routing, None);
    assert_eq!(unknown.tun_enabled, None);
    assert_eq!(unknown.log_level, None);
    let config = ConfigSnapshot {
        mode: "direct".into(),
        mixed_port: 0,
        allow_lan: false,
        ipv6: Some(false),
        bind_address: Some("*".into()),
        lan_allowed_ips: Some(Vec::new()),
        lan_disallowed_ips: Some(Vec::new()),
        skip_auth_prefixes: Some(Vec::new()),
        authentication_enabled: Some(false),
        authentication_user_count: Some(0),
        log_level: "warning".into(),
        tun: Some(TunSnapshot {
            enable: Some(false),
            strict_route: Some(false),
            ..Default::default()
        }),
        ..Default::default()
    };
    let current = owner.observe(1, 2, Some(&Ok(config)));
    let known = build_settings_page(
        Some(&prefs),
        &current,
        &Default::default(),
        Default::default(),
    )
    .data
    .unwrap();
    assert_eq!(known.mixed_port, Some(0));
    assert_eq!(known.allow_lan, Some(false));
    assert_eq!(known.tun_enabled, Some(false));
    assert_eq!(known.tun_stack, None);
    assert_eq!(known.tun_auto_route, None);
    assert_eq!(known.tun_strict_route, Some(false));
    assert_eq!(
        known
            .lan_security
            .as_ref()
            .unwrap()
            .authentication_user_count,
        0
    );
    assert!(!known.ipv6_routing.unwrap().enabled);
    let failure = Failure::new(ErrorCode::Authentication, "read denied", false);
    let failed = owner.observe(1, 3, Some(&Err(PortError::Rejected(failure))));
    let retained = build_settings_page(
        Some(&prefs),
        &failed,
        &Default::default(),
        Default::default(),
    )
    .data
    .unwrap();
    assert_eq!(retained, known);
    let changed = owner.observe(2, 1, None);
    let reset = build_settings_page(
        Some(&prefs),
        &changed,
        &Default::default(),
        Default::default(),
    )
    .data
    .unwrap();
    assert_eq!(reset, unknown);
}

#[tokio::test]
async fn webdav_configuration_does_not_prove_connection_or_snapshot_history_and_failures_stay_typed()
 {
    let mut settings = AppSettings::default();
    settings.webdav.enabled = true;
    settings.webdav.url = "https://dav.example/".into();
    let page = build_sync_page(Some(&Ok(settings)), None, None).await;
    let facts = page.data.unwrap();
    assert_eq!(facts.status, SyncStatus::Configured);
    assert!(facts.last_sync.is_none());
    assert!(facts.snapshots.is_empty());
    assert!(matches!(
        facts.history_status,
        PageStatus::Unavailable { .. }
    ));
    let failure = Failure::new(ErrorCode::Storage, "settings denied", true);
    let page = build_sync_page(Some(&Err(failure.clone())), None, None).await;
    assert!(page.data.is_none());
    assert_eq!(page.status, PageStatus::Failed { failure });
}

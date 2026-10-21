//! test-intent: behavior
use super::{build_settings_page, build_sync_page};
use infiltrator_application::runtime_configuration_application::RuntimeConfigurationReadout;
use infiltrator_application::runtime_control_projection::RuntimeControlApplication;
use infiltrator_contract::sync::SyncStatus;
use infiltrator_domain::settings::AppSettings;
use infiltrator_ports::runtime_configuration::{
    ConfigurationObservation, ConfigurationRevision, DnsRuntimeConfiguration, Ipv6Configuration,
    LanAuthenticationState, LanSecurityConfiguration, LanSharingConfiguration, ObservedValue,
    ProxyPortsConfiguration, SnifferConfiguration, TunConfiguration,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use lurker_core_contract::page::PageStatus;
use lurker_core_contract::session::SessionToken;

fn known_readout() -> RuntimeConfigurationReadout {
    let scope = CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("mihomo").expect("engine"),
            CoreInstanceId::new("settings-observation").expect("instance"),
            Some(CoreVersion::new("test").expect("version")),
        ),
        generation: CoreGeneration::new(1),
        session_token: Some(SessionToken::new(1)),
    };
    let revision = ConfigurationRevision::new(1);
    RuntimeConfigurationReadout {
        tun: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: TunConfiguration {
                enabled: ObservedValue::Reported(false),
                strict_route: ObservedValue::Reported(false),
                ..Default::default()
            },
        },
        lan_sharing: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: LanSharingConfiguration {
                enabled: ObservedValue::Reported(false),
                mixed_port: ObservedValue::Reported(0),
                bind_address: ObservedValue::Reported("*".to_owned()),
            },
        },
        lan_security: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: LanSecurityConfiguration {
                allowed_ips: ObservedValue::Reported(Vec::new()),
                disallowed_ips: ObservedValue::Reported(Vec::new()),
                skip_auth_prefixes: ObservedValue::Reported(Vec::new()),
                authentication: LanAuthenticationState::Disabled,
            },
        },
        ipv6: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: Ipv6Configuration {
                enabled: ObservedValue::Reported(false),
                tun_enabled: ObservedValue::Reported(false),
            },
        },
        sniffer: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: SnifferConfiguration {
                enabled: ObservedValue::Unreported,
            },
        },
        dns: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: DnsRuntimeConfiguration::default(),
        },
        core_log_level: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: ObservedValue::Unreported,
        },
        proxy_ports: ConfigurationObservation {
            scope,
            revision,
            value: ProxyPortsConfiguration::default(),
        },
    }
}

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
    assert_eq!(unknown.lan_sharing, None);
    assert_eq!(unknown.lan_security, None);
    assert_eq!(unknown.ipv6_routing, None);
    assert_eq!(unknown.tun_enabled, None);
    assert_eq!(unknown.log_level, None);
    let current = owner.observe(1, 2, Some(&Ok(known_readout())));
    let known = build_settings_page(
        Some(&prefs),
        &current,
        &Default::default(),
        Default::default(),
    )
    .data
    .unwrap();
    // The LAN sharing configuration is one owner readback: the page carries
    // the confirmed configuration whole, so a consumer can tell "sharing is
    // off" from "the runtime configuration was never observed".
    let sharing = known
        .lan_sharing
        .as_ref()
        .expect("the owner published a LAN sharing readback");
    assert!(!sharing.enabled);
    assert_eq!(sharing.mixed_port, 0);
    assert_eq!(sharing.bind_address, "*");
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
    let failed = owner.observe(1, 3, Some(&Err(failure)));
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

#[test]
fn the_settings_page_publishes_the_lan_sharing_readback_whole_or_not_at_all() {
    let prefs = Ok(AppSettings::default());
    let owner = RuntimeControlApplication::default();

    // The owner never published a runtime configuration: the page carries no
    // LAN sharing readback at all, so a consumer cannot read the absent
    // readback as "the sharing is off".
    let unobserved = build_settings_page(
        Some(&prefs),
        &owner.observe(1, 1, None),
        &Default::default(),
        Default::default(),
    )
    .data
    .unwrap();
    assert_eq!(unobserved.lan_sharing, None);

    // The owner published a confirmed configuration: the page carries that
    // readback whole, so the confirmed facts and the revision the
    // observation was taken at reach the surface unchanged.
    let observed = owner.observe(1, 2, Some(&Ok(known_readout())));
    let confirmed = build_settings_page(
        Some(&prefs),
        &observed,
        &Default::default(),
        Default::default(),
    )
    .data
    .unwrap();
    assert_eq!(confirmed.lan_sharing, observed.lan_sharing);
    let sharing = confirmed
        .lan_sharing
        .clone()
        .expect("the owner published a LAN sharing readback");
    assert!(!sharing.enabled);
    assert_eq!(sharing.mixed_port, 0);
    assert_eq!(sharing.bind_address, "*");

    // A read the owner refused keeps the last confirmed readback of the same
    // generation: the page never falls back to a guessed configuration.
    let denied = Failure::new(ErrorCode::Authentication, "read denied", false);
    let refused = owner.observe(1, 3, Some(&Err(denied)));
    let retained = build_settings_page(
        Some(&prefs),
        &refused,
        &Default::default(),
        Default::default(),
    )
    .data
    .unwrap();
    assert_eq!(retained.lan_sharing, confirmed.lan_sharing);
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

//! Behaviour of the isolated LAN configuration port fixture.

use infiltrator_contract::lan::LanCredentials;
use infiltrator_ports::runtime_configuration::{
    ConfigurationApply, ConfigurationRevision, LanAuthenticationUpdate, LanConfigurationPort,
    LanSecurityUpdate, LanSharingUpdate, ObservedValue,
};
use infiltrator_test_support::lan_configuration_fixtures::{
    CAPTURE_ALLOWED_IP, CAPTURE_BIND_ADDRESS, CAPTURE_MIXED_PORT, LanConfigurationCapturePort,
};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use lurker_core_ports::error::PortError;

fn capture_scope() -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("mihomo").expect("valid engine ID"),
            CoreInstanceId::new("capture-core").expect("valid instance ID"),
            Some(CoreVersion::new("1.19.30").expect("valid version")),
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    }
}

fn foreign_scope() -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("sing-box").expect("valid engine ID"),
            CoreInstanceId::new("capture-core").expect("valid instance ID"),
            None,
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    }
}

#[tokio::test]
async fn the_lan_snapshot_publishes_the_observed_configuration() {
    let scope = capture_scope();
    let port = LanConfigurationCapturePort::new(scope.clone());

    let sharing = port.observe_sharing(&scope).await.expect("LAN sharing");
    assert_eq!(
        sharing.value.mixed_port,
        ObservedValue::Reported(CAPTURE_MIXED_PORT)
    );
    assert_eq!(
        sharing.value.bind_address,
        ObservedValue::Reported(CAPTURE_BIND_ADDRESS.to_owned())
    );

    let security = port.observe_security(&scope).await.expect("LAN security");
    assert_eq!(
        security.value.allowed_ips,
        ObservedValue::Reported(vec![CAPTURE_ALLOWED_IP.to_owned()])
    );
}

#[tokio::test]
async fn a_write_is_fenced_by_the_observed_revision() {
    let scope = capture_scope();
    let port = LanConfigurationCapturePort::new(scope.clone());
    let observed = port.observe_sharing(&scope).await.expect("LAN sharing");

    let applied = port
        .apply_sharing(ConfigurationApply {
            scope: scope.clone(),
            expected_revision: observed.revision,
            update: LanSharingUpdate {
                enabled: false,
                mixed_port: 7891,
                bind_address: "127.0.0.1".to_owned(),
            },
        })
        .await
        .expect("LAN sharing write");

    assert_eq!(applied.value.mixed_port, ObservedValue::Reported(7891));
    assert_eq!(
        applied.revision,
        ConfigurationRevision::new(observed.revision.value() + 1)
    );
}

#[tokio::test]
async fn a_stale_revision_is_refused_instead_of_overwriting() {
    let scope = capture_scope();
    let port = LanConfigurationCapturePort::new(scope.clone());
    let observed = port.observe_security(&scope).await.expect("LAN security");
    let stale = observed.revision;

    port.apply_security(ConfigurationApply {
        scope: scope.clone(),
        expected_revision: stale,
        update: LanSecurityUpdate {
            allowed_ips: vec!["10.0.0.0/8".to_owned()],
            disallowed_ips: Vec::new(),
            skip_auth_prefixes: Vec::new(),
            authentication: LanAuthenticationUpdate::Enabled(LanCredentials {
                username: "second-user".to_owned(),
                password: "secret".to_owned(),
            }),
        },
    })
    .await
    .expect("first write");

    let error = port
        .apply_security(ConfigurationApply {
            scope: scope.clone(),
            expected_revision: stale,
            update: LanSecurityUpdate {
                allowed_ips: Vec::new(),
                disallowed_ips: Vec::new(),
                skip_auth_prefixes: Vec::new(),
                authentication: LanAuthenticationUpdate::Disabled,
            },
        })
        .await
        .expect_err("the stale write must fail");

    assert!(matches!(error, PortError::Rejected(_)));
    let current = port.observe_security(&scope).await.expect("LAN security");
    assert_eq!(
        current.value.allowed_ips,
        ObservedValue::Reported(vec!["10.0.0.0/8".to_owned()]),
        "the stale write must not have replaced the newer configuration"
    );
}

#[tokio::test]
async fn a_foreign_engine_never_reaches_the_configuration() {
    let port = LanConfigurationCapturePort::new(capture_scope());

    let error = port
        .observe_sharing(&foreign_scope())
        .await
        .expect_err("a foreign engine must be refused");

    assert!(matches!(error, PortError::Rejected(_)));
}

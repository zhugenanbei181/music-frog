//! test-intent: behavior
use super::*;
use crate::proxy_inspection_fixtures::{INSPECTION_NODE, observed_proxy};
use crate::proxy_preferences_application::ProxyPreferencesApplication;
use crate::settings_application::SettingsApplication;
use crate::speedtest_application::SpeedtestApplication;
use async_trait::async_trait;
use infiltrator_contract::proxy_probe_options::ProxyProbeOptions;
use infiltrator_domain::proxy::ProxyGroup;
use infiltrator_domain::settings::AppSettings;
use infiltrator_ports::settings_store::SettingsStore;

#[derive(Default)]
struct ProbeSettings(Mutex<AppSettings>);
#[async_trait]
impl SettingsStore for ProbeSettings {
    async fn load(&self) -> Result<AppSettings, PortError> {
        Ok(self.0.lock().unwrap().clone())
    }
    async fn load_hydrated(&self) -> Result<AppSettings, PortError> {
        self.load().await
    }
    async fn save(&self, settings: &AppSettings) -> Result<(), PortError> {
        *self.0.lock().unwrap() = settings.clone();
        Ok(())
    }
}

#[tokio::test]
async fn fallback_group_probe_reports_partial_failure_preserves_permission_and_waits_for_every_leaf()
 {
    let gateway = Arc::new(RecordingGeoGateway::default());
    *gateway.proxy_facts.lock().unwrap() = HashMap::from([(
        "group".into(),
        Proxy::Selector(ProxyGroup {
            name: "group".into(),
            all: vec!["allowed".into(), "denied".into(), "denied".into()],
            ..Default::default()
        }),
    )]);
    gateway
        .denied_probe_names
        .lock()
        .unwrap()
        .push("denied".into());
    let app = CommandApplication::new().with_runtime(gateway.clone());
    let intent = CommandIntent::TestDelay {
        group: Some("group".into()),
        url: None,
        timeout_ms: None,
    };
    let failure = app.execute(intent.clone()).await.unwrap_err();
    assert_eq!(failure.code, ErrorCode::Permission);
    assert!(failure.message.contains("1/2 proxy probes failed; denied:"));
    assert!(failure.message.contains("grant proxy probe permission"));
    let mut names: Vec<_> = gateway
        .probe_calls
        .lock()
        .unwrap()
        .iter()
        .map(|call| call.0.clone())
        .collect();
    names.sort();
    assert_eq!(names, ["allowed", "denied"]);
    assert_eq!(gateway.switches.load(Ordering::SeqCst), 0);
    gateway.denied_probe_names.lock().unwrap().clear();
    app.execute(intent).await.unwrap();
    assert_eq!(gateway.probe_calls.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn leaf_delay_probe_executes_exact_identity_and_options_without_selecting_a_proxy() {
    let gateway = Arc::new(RecordingGeoGateway::default());
    gateway
        .proxy_facts
        .lock()
        .unwrap()
        .insert(INSPECTION_NODE.into(), observed_proxy());
    let app = CommandApplication::new().with_runtime(gateway.clone());
    let intent = CommandIntent::TestNodeDelay {
        node: INSPECTION_NODE.into(),
        url: Some("http://probe.example.test/204".into()),
        timeout_ms: Some(1234),
    };
    app.execute(intent.clone()).await.unwrap();
    assert_eq!(
        *gateway.probe_calls.lock().unwrap(),
        vec![(
            INSPECTION_NODE.into(),
            "http://probe.example.test/204".into(),
            1234
        )]
    );
    assert_eq!(gateway.switches.load(Ordering::SeqCst), 0);
    gateway.deny_probe.store(true, Ordering::SeqCst);
    let failure = app.execute(intent).await.unwrap_err();
    assert_eq!(
        failure,
        Failure::from(PortError::PermissionDenied(
            "grant proxy probe permission".into()
        ))
    );
    assert_eq!(gateway.probe_calls.lock().unwrap().len(), 2);
    assert_eq!(gateway.switches.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn removed_unknown_or_group_probe_targets_fail_before_any_gateway_operation() {
    let gateway = Arc::new(RecordingGeoGateway::default());
    *gateway.proxy_facts.lock().unwrap() = HashMap::from([
        ("unknown".into(), Proxy::Unknown),
        ("group".into(), Proxy::Selector(ProxyGroup::default())),
    ]);
    let app = CommandApplication::new().with_runtime(gateway.clone());
    for node in ["deleted", "unknown", "group"] {
        let failure = app
            .execute(CommandIntent::TestNodeDelay {
                node: node.into(),
                url: None,
                timeout_ms: None,
            })
            .await
            .unwrap_err();
        assert_eq!(failure.code, ErrorCode::InvalidInput);
    }
    assert!(gateway.probe_calls.lock().unwrap().is_empty());
    assert_eq!(gateway.switches.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn saved_parameters_drive_real_leaf_group_and_engine_probes_and_invalid_values_have_no_probe_effect()
 {
    let gateway = Arc::new(RecordingGeoGateway::default());
    *gateway.proxy_facts.lock().unwrap() = HashMap::from([
        (INSPECTION_NODE.into(), observed_proxy()),
        (
            "group".into(),
            Proxy::Selector(ProxyGroup {
                name: "group".into(),
                all: vec![INSPECTION_NODE.into()],
                now: INSPECTION_NODE.into(),
                ..Default::default()
            }),
        ),
    ]);
    let settings = Arc::new(ProbeSettings::default());
    let app = CommandApplication::new()
        .with_runtime(gateway.clone())
        .with_settings(SettingsApplication::new(settings));
    let options = ProxyProbeOptions {
        test_url: "https://probe.example.test/selected".into(),
        timeout_ms: 32767,
    };
    app.execute(CommandIntent::SetProxyProbeOptions {
        options: options.clone(),
    })
    .await
    .unwrap();
    app.execute(CommandIntent::TestNodeDelay {
        node: INSPECTION_NODE.into(),
        url: None,
        timeout_ms: None,
    })
    .await
    .unwrap();
    app.execute(CommandIntent::TestDelay {
        group: Some("group".into()),
        url: None,
        timeout_ms: None,
    })
    .await
    .unwrap();
    let engine = SpeedtestApplication::new(gateway.clone());
    app.clone()
        .with_speedtest(engine.clone())
        .execute(CommandIntent::TestDelay {
            group: Some("group".into()),
            url: None,
            timeout_ms: None,
        })
        .await
        .unwrap();
    assert_eq!(
        *gateway.probe_calls.lock().unwrap(),
        vec![
            (
                INSPECTION_NODE.into(),
                options.test_url.clone(),
                options.timeout_ms
            );
            3
        ]
    );
    assert_eq!(engine.snapshot().config.test_url, options.test_url);
    assert_eq!(engine.snapshot().config.timeout_ms, 32767);
    for timeout_ms in [0, 32768, 60000] {
        let failure = app
            .execute(CommandIntent::TestNodeDelay {
                node: INSPECTION_NODE.into(),
                url: None,
                timeout_ms: Some(timeout_ms),
            })
            .await
            .unwrap_err();
        assert_eq!(failure.code, ErrorCode::InvalidInput);
    }
    assert_eq!(gateway.probe_calls.lock().unwrap().len(), 3);
    assert_eq!(gateway.switches.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn group_order_transaction_requires_unique_complete_live_identities_and_never_selects_or_probes_nodes()
 {
    let gateway = Arc::new(RecordingGeoGateway::default());
    for name in ["A", "B", "C"] {
        gateway.proxy_facts.lock().unwrap().insert(
            name.into(),
            Proxy::Selector(ProxyGroup {
                name: name.into(),
                ..Default::default()
            }),
        );
    }
    gateway
        .proxy_facts
        .lock()
        .unwrap()
        .insert(INSPECTION_NODE.into(), observed_proxy());
    let preferences = ProxyPreferencesApplication::new();
    let application = CommandApplication::new()
        .with_runtime(gateway.clone())
        .with_proxy_preferences(preferences.clone());
    for malformed in [
        vec!["A"],
        vec!["A", "B", "B"],
        vec!["A", "B", "deleted"],
        vec!["A", "B", ""],
        vec!["A", "B", INSPECTION_NODE],
    ] {
        assert!(
            application
                .execute(CommandIntent::ReorderProxyGroups {
                    group_names: malformed.into_iter().map(str::to_owned).collect()
                })
                .await
                .is_err()
        );
        assert!(preferences.custom_group_order().unwrap().is_empty());
    }
    application
        .execute(CommandIntent::ReorderProxyGroups {
            group_names: vec!["C".into(), "A".into(), "B".into()],
        })
        .await
        .unwrap();
    assert_eq!(
        preferences.custom_group_order().unwrap(),
        vec!["C", "A", "B"]
    );
    gateway.proxy_facts.lock().unwrap().remove("B");
    let failure = application
        .execute(CommandIntent::ReorderProxyGroups {
            group_names: vec!["B".into(), "C".into(), "A".into()],
        })
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert_eq!(
        preferences.custom_group_order().unwrap(),
        vec!["C", "A", "B"]
    );
    assert_eq!(gateway.switches.load(Ordering::SeqCst), 0);
    assert!(gateway.probe_calls.lock().unwrap().is_empty());
}

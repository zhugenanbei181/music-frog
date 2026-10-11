//! test-intent: real_process

use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use infiltrator_contract::outbound::identity::OutboundId;
use infiltrator_contract::outbound::inventory::{
    OutboundInventoryState, OutboundSelectionState, SelectionAvailability,
};
use infiltrator_contract::profile_document::DocumentValidation;
use infiltrator_contract::routing_intent::{
    EngineDefinedRoutingIntent, RoutingIntent, RoutingIntentExtensionId,
};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::error::PortError;
use infiltrator_ports::outbound::{OutboundPort, OutboundScopeSource};
use infiltrator_ports::profile_document_codec::ProfileDocumentCodecPort;
use infiltrator_ports::routing_intent::RoutingIntentPort;
use singbox_adapter::clash_api::SingBoxClashApiClient;
use singbox_adapter::clash_runtime::{SingBoxOutboundAdapter, SingBoxRoutingIntentAdapter};
use singbox_adapter::config::SingBoxProfileConfigurationAdapter;
use singbox_adapter::http::SingBoxControllerAuthorization;
use singbox_adapter::process::SingBoxProcess;
use singbox_adapter::profile_document_codec::SingBoxProfileDocumentCodec;
use singbox_adapter::readiness::SingBoxControllerReadiness;
use std::env;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

fn required_environment(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set by the explicit sing-box smoke"))
}

struct FixedScope(CoreScope);

impl OutboundScopeSource for FixedScope {
    fn current_scope(&self) -> Option<CoreScope> {
        Some(self.0.clone())
    }
}

#[tokio::test]
#[ignore = "requires the explicit SHA-locked sing-box real-process smoke"]
async fn locked_sing_box_binary_checks_applies_starts_and_answers_loopback_readiness() {
    let binary_path = PathBuf::from(required_environment("SINGBOX_REAL_BINARY"));
    let expected_version = required_environment("SINGBOX_REAL_VERSION");
    let version = tokio::process::Command::new(&binary_path)
        .arg("version")
        .output()
        .await
        .expect("run the locked sing-box binary version command");
    assert!(version.status.success(), "sing-box version must run");
    let version_output = String::from_utf8_lossy(&version.stdout);
    assert!(
        version_output
            .lines()
            .next()
            .is_some_and(|line| { line == format!("sing-box version {expected_version}") }),
        "real binary must match the locked version"
    );

    let home = TempDir::new().expect("temporary sing-box smoke home");
    let config_path = home.path().join("config.json");
    let controller = TcpListener::bind("127.0.0.1:0").expect("reserve a loopback port");
    let controller_address = controller.local_addr().expect("read reserved port");
    drop(controller);
    let raw_config = serde_json::to_vec(&serde_json::json!({
        "log": {"level": "error"},
        "inbounds": [],
        "outbounds": [
            {"type": "direct", "tag": "direct-a"},
            {"type": "direct", "tag": "direct-b"},
            {
                "type": "selector",
                "tag": "selector",
                "outbounds": ["direct-a", "direct-b"],
                "default": "direct-a"
            }
        ],
        "experimental": {
            "clash_api": {
                "external_controller": controller_address.to_string()
            }
        }
    }))
    .expect("serialize the smoke configuration");

    let codec = SingBoxProfileDocumentCodec;
    assert_eq!(
        codec.inspect(&raw_config).unwrap(),
        DocumentValidation::Valid
    );
    let configuration =
        SingBoxProfileConfigurationAdapter::with_tokio(binary_path.clone(), config_path.clone());
    configuration
        .apply_and_readback(&raw_config)
        .await
        .expect("native check and exact config file readback");
    assert_eq!(
        configuration.readback().await.unwrap().as_slice(),
        raw_config.as_slice()
    );

    let process = SingBoxProcess::with_tokio(
        binary_path,
        config_path,
        CoreInstanceId::new("sing-box-real-process-smoke").expect("valid smoke identity"),
        controller_address,
    )
    .expect("construct loopback-only process adapter");
    let readiness =
        SingBoxControllerReadiness::with_reqwest(&format!("http://{controller_address}"))
            .expect("construct loopback readiness adapter");

    process.start().await.expect("start the locked real binary");
    let ready = tokio::time::timeout(
        Duration::from_secs(10),
        readiness.wait_until_ready(Duration::from_secs(9)),
    )
    .await
    .expect("readiness must finish within its bound");
    ready.expect("the live Clash API /version endpoint must respond");
    assert_eq!(
        process.status().await.unwrap(),
        CoreLifecycle::Running,
        "process adapter must observe the live child"
    );

    let observed_version = readiness
        .active_core_version()
        .await
        .expect("live API version request")
        .expect("sing-box reports its active version");
    let expected_core_version = CoreVersion::new(format!("v{}", expected_version))
        .expect("locked release version is valid");
    assert_eq!(observed_version, expected_core_version);
    let scope = CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("sing-box").expect("valid engine ID"),
            CoreInstanceId::new("sing-box-real-process-smoke").expect("valid instance ID"),
            Some(observed_version),
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    };
    let api = SingBoxClashApiClient::with_reqwest(
        &format!("http://{controller_address}"),
        SingBoxControllerAuthorization::NoSecret,
    )
    .expect("construct the loopback Clash API client");
    let scope_source = Arc::new(FixedScope(scope.clone()));
    let outbounds = SingBoxOutboundAdapter::new(api.clone(), scope_source.clone());
    let inventory = outbounds
        .inventory(scope.clone())
        .await
        .expect("observe the real sing-box outbound inventory");
    let selector_id = OutboundId::new(
        EngineId::new("sing-box").expect("valid engine ID"),
        "selector",
    )
    .expect("valid selector ID");
    let direct_a_id = OutboundId::new(
        EngineId::new("sing-box").expect("valid engine ID"),
        "direct-a",
    )
    .expect("valid outbound ID");
    let direct_b_id = OutboundId::new(
        EngineId::new("sing-box").expect("valid engine ID"),
        "direct-b",
    )
    .expect("valid outbound ID");
    let OutboundInventoryState::Ready { collections, .. } = inventory.inventory() else {
        panic!("real sing-box selector config should produce a non-empty inventory");
    };
    let selector = collections
        .iter()
        .find(|collection| collection.id == selector_id)
        .expect("real selector group appears in the inventory");
    assert_eq!(selector.availability, SelectionAvailability::Selectable);
    assert_eq!(
        selector.selection,
        OutboundSelectionState::Ready {
            selected: direct_a_id.clone()
        }
    );
    let switched = outbounds
        .select(
            scope.clone(),
            inventory.revision(),
            selector_id.clone(),
            direct_b_id.clone(),
        )
        .await
        .expect("real selector PUT must be confirmed by /proxies readback");
    let OutboundInventoryState::Ready {
        collections: switched_collections,
        ..
    } = switched.inventory()
    else {
        panic!("selector write should leave a ready outbound inventory");
    };
    assert_eq!(
        switched_collections
            .iter()
            .find(|collection| collection.id == selector_id)
            .expect("selector remains observed")
            .selection,
        OutboundSelectionState::Ready {
            selected: direct_b_id.clone()
        }
    );
    let restored = outbounds
        .select(
            scope.clone(),
            switched.revision(),
            selector_id.clone(),
            direct_a_id.clone(),
        )
        .await
        .expect("real selector original choice must read back after restoration");
    let OutboundInventoryState::Ready {
        collections: restored_collections,
        ..
    } = restored.inventory()
    else {
        panic!("selector restore should leave a ready outbound inventory");
    };
    assert_eq!(
        restored_collections
            .iter()
            .find(|collection| collection.id == selector_id)
            .expect("selector remains observed")
            .selection,
        OutboundSelectionState::Ready {
            selected: direct_a_id
        }
    );

    let routing = SingBoxRoutingIntentAdapter::new(api, scope_source);
    assert_eq!(
        routing
            .observe(&scope)
            .await
            .expect("observe engine-defined Clash mode"),
        RoutingIntent::EngineDefined(EngineDefinedRoutingIntent::new(
            EngineId::new("sing-box").expect("valid engine ID"),
            RoutingIntentExtensionId::new("mode.rule").expect("valid extension ID"),
        ))
    );
    assert!(matches!(
        routing.apply(&scope, RoutingIntent::FollowRules).await,
        Err(PortError::Unsupported { .. })
    ));

    process.stop().await.expect("stop the locked real binary");
    assert_eq!(
        process.status().await.unwrap(),
        CoreLifecycle::Stopped,
        "process adapter must finish the child before reporting stopped"
    );
}

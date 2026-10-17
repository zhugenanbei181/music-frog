use crate::backend::SingBoxBackendOptions;
use crate::backend::SingBoxEngineBackendProvider;
use lurker_core_contract::identity::CoreGeneration;
use lurker_core_contract::identity::CoreIdentity;
use lurker_core_contract::identity::CoreInstanceId;
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::core_scope::CoreScopeObservation;
use lurker_core_ports::core_scope::CoreScopeSource;
use lurker_core_ports::engine_backend::EngineBackendOwnerScopeBindingResult;
use lurker_core_ports::engine_backend::EngineBackendProvider;
use lurker_core_ports::engine_backend::EngineOperationPorts;
use lurker_core_ports::error::PortError;
use std::sync::Arc;

const INSTANCE_ID: &str = "singbox-backend-test";

struct FixedScope(CoreScope);

impl CoreScopeSource for FixedScope {
    fn scope_observation(&self) -> CoreScopeObservation {
        CoreScopeObservation::Observed(self.0.clone())
    }
}

fn options() -> SingBoxBackendOptions {
    SingBoxBackendOptions::new(
        "/opt/sing-box/sing-box",
        "/data/config.json",
        CoreInstanceId::new(INSTANCE_ID).expect("test instance is valid"),
        "127.0.0.1:9090",
        Some("test-secret".to_owned()),
    )
    .expect("valid sing-box backend options")
}

#[test]
fn provider_reports_the_sing_box_engine() {
    let provider = SingBoxEngineBackendProvider::new(options());

    assert_eq!(provider.engine_id(), EngineId::sing_box());
}

#[test]
fn provider_creates_an_available_sing_box_backend() {
    let provider = SingBoxEngineBackendProvider::new(options());

    let backend = provider
        .create_backend()
        .expect("valid options create a bundle");

    assert_eq!(backend.engine_id(), &EngineId::sing_box());
    assert_eq!(
        backend
            .process()
            .core_identity()
            .expect("process adapter publishes its identity")
            .engine,
        EngineId::sing_box()
    );
    assert_eq!(
        backend.configuration().scope().engine_id,
        EngineId::sing_box()
    );
    assert!(matches!(
        backend.operations(),
        EngineOperationPorts::Available { .. }
    ));
    assert!(backend.operations().outbound().is_ok());
    assert!(backend.operations().routing_intent().is_ok());
}

#[test]
fn owner_scope_stays_unavailable_until_the_host_binds_it() {
    let backend = SingBoxEngineBackendProvider::new(options())
        .create_backend()
        .expect("valid options create a bundle");
    let EngineOperationPorts::Available { owner_scope, .. } = backend.operations() else {
        panic!("sing-box backend operations must be available");
    };
    assert_eq!(
        owner_scope.scope_observation(),
        CoreScopeObservation::Unavailable
    );

    let scope = CoreScope {
        identity: CoreIdentity::new(
            EngineId::sing_box(),
            CoreInstanceId::new(INSTANCE_ID).expect("test instance is valid"),
            None,
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    };
    let source = Arc::new(FixedScope(scope.clone()));

    assert_eq!(
        backend
            .bind_owner_core_scope(source)
            .expect("the owner scope binds once"),
        EngineBackendOwnerScopeBindingResult::Bound
    );
    assert_eq!(
        owner_scope.scope_observation(),
        CoreScopeObservation::Observed(scope)
    );
}

#[test]
fn invalid_options_are_typed_failures() {
    let empty_binary = SingBoxBackendOptions::new(
        "",
        "/data/config.json",
        CoreInstanceId::new(INSTANCE_ID).expect("test instance is valid"),
        "127.0.0.1:9090",
        None,
    );
    assert!(matches!(empty_binary, Err(PortError::Failed(_))));

    let foreign_endpoint = SingBoxBackendOptions::new(
        "/opt/sing-box/sing-box",
        "/data/config.json",
        CoreInstanceId::new(INSTANCE_ID).expect("test instance is valid"),
        "0.0.0.0:9090",
        None,
    );
    assert!(matches!(foreign_endpoint, Err(PortError::Failed(_))));

    let malformed_endpoint = SingBoxBackendOptions::new(
        "/opt/sing-box/sing-box",
        "/data/config.json",
        CoreInstanceId::new(INSTANCE_ID).expect("test instance is valid"),
        "not an endpoint",
        None,
    );
    assert!(matches!(malformed_endpoint, Err(PortError::Failed(_))));
}

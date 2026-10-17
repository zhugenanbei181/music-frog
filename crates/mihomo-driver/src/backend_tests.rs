use crate::backend::MihomoBackendOptions;
use crate::backend::MihomoEngineBackendProvider;
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

struct FixedScope(CoreScope);

impl CoreScopeSource for FixedScope {
    fn scope_observation(&self) -> CoreScopeObservation {
        CoreScopeObservation::Observed(self.0.clone())
    }
}

fn options() -> MihomoBackendOptions {
    MihomoBackendOptions::new(
        "/opt/mihomo/mihomo",
        "/data/config.yaml",
        "/data/home",
        "http://127.0.0.1:9090",
        Some("test-secret".to_owned()),
    )
    .expect("valid Mihomo backend options")
}

#[test]
fn provider_reports_the_mihomo_engine() {
    let provider = MihomoEngineBackendProvider::new(options());

    assert_eq!(provider.engine_id(), EngineId::mihomo());
}

#[test]
fn provider_creates_an_available_mihomo_backend() {
    let provider = MihomoEngineBackendProvider::new(options());

    let backend = provider
        .create_backend()
        .expect("valid options create a bundle");

    assert_eq!(backend.engine_id(), &EngineId::mihomo());
    assert_eq!(
        backend
            .process()
            .core_identity()
            .expect("process adapter publishes its identity")
            .engine,
        EngineId::mihomo()
    );
    assert_eq!(
        backend.configuration().scope().engine_id,
        EngineId::mihomo()
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
    let backend = MihomoEngineBackendProvider::new(options())
        .create_backend()
        .expect("valid options create a bundle");
    let EngineOperationPorts::Available { owner_scope, .. } = backend.operations() else {
        panic!("Mihomo backend operations must be available");
    };
    assert_eq!(
        owner_scope.scope_observation(),
        CoreScopeObservation::Unavailable
    );

    let scope = CoreScope {
        identity: CoreIdentity::new(
            EngineId::mihomo(),
            CoreInstanceId::new("desktop-active").expect("test instance is valid"),
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
    let empty_binary = MihomoBackendOptions::new(
        "",
        "/data/config.yaml",
        "/data/home",
        "http://127.0.0.1:9090",
        None,
    );
    assert!(matches!(empty_binary, Err(PortError::Failed(_))));

    let foreign_endpoint = MihomoBackendOptions::new(
        "/opt/mihomo/mihomo",
        "/data/config.yaml",
        "/data/home",
        "http://192.0.2.10:9090",
        None,
    );
    assert!(matches!(foreign_endpoint, Err(PortError::Failed(_))));

    let malformed_endpoint = MihomoBackendOptions::new(
        "/opt/mihomo/mihomo",
        "/data/config.yaml",
        "/data/home",
        "not an endpoint",
        None,
    );
    assert!(matches!(malformed_endpoint, Err(PortError::Failed(_))));
}

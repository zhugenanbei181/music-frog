use super::*;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::{CoreVersion, EngineId};
use infiltrator_core::engine_version_backend_factory::MihomoEngineVersionBackendFactory;
use infiltrator_ports::core_binary_selection::{
    CoreBinarySelection, CoreBinarySelectionPolicy, CoreBinarySelectionSource,
};
use infiltrator_ports::engine_version_backend_factory::EngineVersionBackendFactory;
use mihomo_api::client::MihomoClient;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::TempDir;

fn service_manager() -> ServiceManager {
    ServiceManager::with_pid_file(
        PathBuf::from("unused-mihomo-binary"),
        PathBuf::from("unused-mihomo-config"),
        PathBuf::from("unused-mihomo-pid"),
    )
}

#[test]
fn desktop_backend_factory_returns_identity_scoped_mihomo_ports() {
    let engine_id = MihomoVersionPort::engine_identifier();
    let backend = desktop_backend(
        &service_manager(),
        "http://127.0.0.1:9090",
        Some("test-secret".to_owned()),
        &engine_id,
    )
    .expect("registered Mihomo provider creates its backend ports");

    assert_eq!(backend.ports.engine_id(), &engine_id);
    assert_eq!(
        backend
            .ports
            .process()
            .core_identity()
            .expect("desktop process adapter is engine scoped")
            .engine,
        engine_id
    );
    assert_eq!(
        backend.ports.configuration().scope().engine_id,
        MihomoVersionPort::engine_identifier()
    );
    let operations = backend.ports.operations();
    operations.outbound().expect("Mihomo outbound is composed");
    operations
        .routing_intent()
        .expect("Mihomo routing intent is composed");
    let _readiness = backend.ports.readiness();
    let _client = backend.client;
}

#[test]
fn desktop_mihomo_provider_can_create_backend_ports_repeatedly() {
    let engine_id = MihomoVersionPort::engine_identifier();
    let provider = Arc::new(DesktopMihomoBackendProvider {
        process: service_manager().core_process(),
        controller_url: "http://127.0.0.1:9090".to_owned(),
        secret: Some("test-secret".to_owned()),
        client: MihomoClient::new("http://127.0.0.1:9090", Some("test-secret".to_owned()))
            .expect("valid endpoint constructs a Mihomo client"),
    });

    for _ in 0..2 {
        let backend = desktop_backend_from_provider(Arc::clone(&provider), &engine_id)
            .expect("provider construction is repeatable and has no hidden one-shot state");
        assert_eq!(backend.ports.engine_id(), &engine_id);
        assert_eq!(
            backend.ports.process().core_identity().unwrap().engine,
            engine_id
        );
    }
}

#[test]
fn unknown_engine_is_unsupported_before_mihomo_ports_are_created() {
    let unknown_engine = EngineId::new("future-engine").expect("valid engine identifier");
    let error = match desktop_backend(&service_manager(), "not a URL", None, &unknown_engine) {
        Ok(_) => panic!("unregistered engine must not create a controller client or ports"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        PortError::Unsupported {
            capability: Capability::CoreLifecycle,
            ..
        }
    ));
}

#[tokio::test]
async fn unknown_product_engine_is_rejected_before_binary_or_runtime_setup() {
    let home = TempDir::new().expect("isolated composition home");
    let factory = MihomoEngineVersionBackendFactory::with_home(home.path().into());
    let engine_id = EngineId::new("future-engine").expect("valid engine identifier");

    let error = match open_product_backend_with_source(
        engine_id,
        SurfaceKind::IcedDesktop,
        &factory,
        infiltrator_ports::core_binary_selection::BundledBinarySource {
            candidates: vec![home.path().join("missing-bundle")],
            data_directory: home.path().join("runtime-assets"),
        },
    )
    .await
    {
        Ok(_) => panic!("unregistered engine must be rejected before runtime setup"),
        Err(error) => error,
    };

    assert!(matches!(
        error.downcast_ref::<PortError>(),
        Some(PortError::Unsupported {
            capability: Capability::CoreLifecycle,
            ..
        })
    ));
    assert!(
        !home.path().join("engines/future-engine").exists(),
        "unknown engine selection must not create an engine namespace"
    );
    assert!(
        !home.path().join("engines").exists(),
        "unknown engine selection must fail before creating any engine files"
    );
    assert!(
        !home.path().join("runtime-assets").exists(),
        "unknown engine selection must not copy or stage binary candidates"
    );
}

struct CountingDesktopRuntimeProvider {
    engine_id: EngineId,
    calls: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl DesktopEngineRuntimeProvider for CountingDesktopRuntimeProvider {
    fn engine_id(&self) -> EngineId {
        self.engine_id.clone()
    }

    async fn create(
        &self,
        _selection: CoreBinarySelection,
        _request: DesktopRuntimeRequest,
    ) -> anyhow::Result<DesktopRuntimeOutcome> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(anyhow::Error::new(PortError::Failed(
            "counting provider must not be called by lookup".to_owned(),
        )))
    }
}

#[test]
fn desktop_runtime_factory_rejects_unknown_engine_before_provider_creation() {
    let calls = Arc::new(AtomicUsize::new(0));
    let provider: Arc<dyn DesktopEngineRuntimeProvider> =
        Arc::new(CountingDesktopRuntimeProvider {
            engine_id: MihomoVersionPort::engine_identifier(),
            calls: Arc::clone(&calls),
        });
    let factory = DesktopEngineRuntimeFactory::new([provider]).unwrap();
    let unknown = EngineId::new("future-engine").unwrap();

    let error = match factory.provider(&unknown) {
        Ok(_) => panic!("an unregistered engine must not receive a desktop runtime provider"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        PortError::Unsupported {
            capability: Capability::CoreLifecycle,
            ..
        }
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn boot_host_materialization_rejects_unknown_engine_before_runtime_setup() {
    let home = TempDir::new().expect("isolated materialization home");
    let selection = CoreBinarySelection {
        engine_id: EngineId::new("future-engine").unwrap(),
        version: CoreVersion::new("v1.0.0").unwrap(),
        binary_path: home.path().join("future-engine"),
        related_asset_candidates: Vec::new(),
        source: CoreBinarySelectionSource::ManagedVersion,
        verification: Default::default(),
    };

    let error = match materialize_boot_host(selection).await {
        Ok(_) => panic!("an unknown engine must not materialize a desktop runtime"),
        Err(error) => error,
    };

    assert!(matches!(
        error.downcast_ref::<PortError>(),
        Some(PortError::Unsupported {
            capability: Capability::CoreLifecycle,
            ..
        })
    ));
    assert_eq!(std::fs::read_dir(home.path()).unwrap().count(), 0);
}

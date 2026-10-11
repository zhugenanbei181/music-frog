use super::*;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_ports::core_binary_selection::CoreBinarySelection;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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

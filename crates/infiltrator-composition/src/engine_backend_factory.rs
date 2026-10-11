//! Engine selection for product composition roots.

use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_ports::engine_backend_factory::{
    ENGINE_BACKEND_CAPABILITY, EngineBackendFactory, EngineBackendPorts, EngineBackendProvider,
};
use infiltrator_ports::error::PortError;
use std::collections::HashMap;
use std::sync::Arc;

/// Dispatches each engine identity to its registered process/config/runtime
/// adapter factory. A missing engine is rejected before any provider runs.
pub struct EngineBackendFactoryRegistry {
    providers: HashMap<EngineId, Arc<dyn EngineBackendProvider>>,
}

impl EngineBackendFactoryRegistry {
    pub fn new(
        providers: impl IntoIterator<Item = Arc<dyn EngineBackendProvider>>,
    ) -> Result<Self, PortError> {
        let mut by_engine = HashMap::new();
        for provider in providers {
            let engine_id = provider.engine_id();
            if by_engine.insert(engine_id.clone(), provider).is_some() {
                return Err(PortError::Failed(format!(
                    "duplicate backend factory for engine {}",
                    engine_id.as_str()
                )));
            }
        }
        Ok(Self {
            providers: by_engine,
        })
    }
}

impl EngineBackendFactory for EngineBackendFactoryRegistry {
    fn backend_ports(&self, engine_id: &EngineId) -> Result<EngineBackendPorts, PortError> {
        let provider = self.providers.get(engine_id).ok_or_else(|| {
            PortError::unsupported(
                ENGINE_BACKEND_CAPABILITY,
                format!(
                    "no process/config/runtime backend is registered for {}",
                    engine_id.as_str()
                ),
            )
        })?;
        let ports = provider.create_backend()?;
        if ports.engine_id() != engine_id {
            return Err(PortError::Failed(format!(
                "backend factory for {} returned ports for {}",
                engine_id.as_str(),
                ports.engine_id().as_str()
            )));
        }
        Ok(ports)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakeBackendProvider {
        engine_id: EngineId,
        calls: Arc<AtomicUsize>,
    }

    impl EngineBackendProvider for FakeBackendProvider {
        fn engine_id(&self) -> EngineId {
            self.engine_id.clone()
        }

        fn create_backend(&self) -> Result<EngineBackendPorts, PortError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(PortError::unsupported(
                Capability::CoreLifecycle,
                "fake backend intentionally unavailable",
            ))
        }
    }

    fn engine_id(value: &str) -> EngineId {
        EngineId::new(value).expect("test engine id is valid")
    }

    #[test]
    fn unknown_engine_is_typed_unsupported_without_constructing_any_backend() {
        let calls = Arc::new(AtomicUsize::new(0));
        let provider: Arc<dyn EngineBackendProvider> = Arc::new(FakeBackendProvider {
            engine_id: engine_id("mihomo"),
            calls: Arc::clone(&calls),
        });
        let factory = EngineBackendFactoryRegistry::new([provider]).unwrap();

        let error = match factory.backend_ports(&engine_id("sing-box")) {
            Ok(_) => panic!("an unknown engine must not receive ports"),
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

    #[test]
    fn registered_engine_dispatches_to_its_factory_once() {
        let calls = Arc::new(AtomicUsize::new(0));
        let provider: Arc<dyn EngineBackendProvider> = Arc::new(FakeBackendProvider {
            engine_id: engine_id("mihomo"),
            calls: Arc::clone(&calls),
        });
        let factory = EngineBackendFactoryRegistry::new([provider]).unwrap();

        let error = match factory.backend_ports(&engine_id("mihomo")) {
            Ok(_) => panic!("the fake provider reports an unavailable backend"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            PortError::Unsupported {
                capability: Capability::CoreLifecycle,
                ..
            }
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn duplicate_engine_registration_is_rejected() {
        let calls = Arc::new(AtomicUsize::new(0));
        let first: Arc<dyn EngineBackendProvider> = Arc::new(FakeBackendProvider {
            engine_id: engine_id("mihomo"),
            calls: Arc::clone(&calls),
        });
        let second: Arc<dyn EngineBackendProvider> = Arc::new(FakeBackendProvider {
            engine_id: engine_id("mihomo"),
            calls: Arc::clone(&calls),
        });
        let providers = vec![first, second];

        let error = match EngineBackendFactoryRegistry::new(providers) {
            Ok(_) => panic!("duplicate engine factories must be rejected"),
            Err(error) => error,
        };

        assert!(matches!(error, PortError::Failed(_)));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

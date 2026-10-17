//! Registry behavior tests: dispatch, availability and identity rejection.

use crate::error::EngineBackendRegistryError;
use crate::registry::EngineBackendProviderRegistry;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::engine_backend::{
    EngineBackendBundle, EngineBackendProvider, EngineBackendProviderAvailability,
};
use lurker_core_ports::error::PortError;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone)]
struct FakeBundle {
    engine_id: EngineId,
}

impl EngineBackendBundle for FakeBundle {
    fn engine_id(&self) -> &EngineId {
        &self.engine_id
    }
}

struct FakeProvider {
    engine_id: EngineId,
    bundle_engine_id: EngineId,
    calls: Arc<AtomicUsize>,
    outcome: FakeOutcome,
}

#[derive(Clone, Copy)]
enum FakeOutcome {
    Bundle,
    Unavailable,
}

impl FakeProvider {
    fn available(engine_id: EngineId, calls: Arc<AtomicUsize>) -> Self {
        Self {
            bundle_engine_id: engine_id.clone(),
            engine_id,
            calls,
            outcome: FakeOutcome::Bundle,
        }
    }

    fn unavailable(engine_id: EngineId, calls: Arc<AtomicUsize>) -> Self {
        Self {
            bundle_engine_id: engine_id.clone(),
            engine_id,
            calls,
            outcome: FakeOutcome::Unavailable,
        }
    }

    fn returning(engine_id: EngineId, bundle_engine_id: EngineId, calls: Arc<AtomicUsize>) -> Self {
        Self {
            engine_id,
            bundle_engine_id,
            calls,
            outcome: FakeOutcome::Bundle,
        }
    }
}

impl EngineBackendProvider<FakeBundle> for FakeProvider {
    fn engine_id(&self) -> EngineId {
        self.engine_id.clone()
    }

    fn create_backend(&self) -> Result<FakeBundle, PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.outcome {
            FakeOutcome::Bundle => Ok(FakeBundle {
                engine_id: self.bundle_engine_id.clone(),
            }),
            FakeOutcome::Unavailable => Err(PortError::unsupported(
                Capability::CoreLifecycle,
                "fake provider intentionally unavailable",
            )),
        }
    }
}

fn provider(
    provider: FakeProvider,
) -> (Arc<dyn EngineBackendProvider<FakeBundle>>, Arc<AtomicUsize>) {
    let calls = Arc::clone(&provider.calls);
    (Arc::new(provider), calls)
}

#[test]
fn unregistered_engine_is_typed_unsupported_without_invoking_a_provider() {
    let calls = Arc::new(AtomicUsize::new(0));
    let (mihomo, mihomo_calls) = provider(FakeProvider::available(EngineId::mihomo(), calls));
    let registry = EngineBackendProviderRegistry::new([mihomo]).unwrap();

    let error = match registry.backend(&EngineId::sing_box()) {
        Ok(_) => panic!("an unregistered engine must not receive a backend"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        PortError::Unsupported {
            capability: Capability::CoreLifecycle,
            ..
        }
    ));
    assert_eq!(mihomo_calls.load(Ordering::SeqCst), 0);
    assert!(matches!(
        registry.availability(&EngineId::sing_box()),
        EngineBackendProviderAvailability::Unsupported { .. }
    ));
}

#[test]
fn duplicate_registration_is_rejected_without_invoking_a_provider() {
    let calls = Arc::new(AtomicUsize::new(0));
    let (first, calls) = provider(FakeProvider::available(EngineId::mihomo(), calls));
    let (second, _) = provider(FakeProvider::available(
        EngineId::mihomo(),
        Arc::clone(&calls),
    ));

    let error = match EngineBackendProviderRegistry::new([first, second]) {
        Ok(_) => panic!("duplicate engine providers must be rejected"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        EngineBackendRegistryError::DuplicateProvider {
            engine_id: EngineId::mihomo()
        }
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let mut registry = EngineBackendProviderRegistry::empty();
    let (registered, _) = provider(FakeProvider::available(
        EngineId::mihomo(),
        Arc::clone(&calls),
    ));
    registry.register(registered).unwrap();
    let (duplicate, _) = provider(FakeProvider::available(
        EngineId::mihomo(),
        Arc::clone(&calls),
    ));
    assert!(matches!(
        registry.register(duplicate),
        Err(EngineBackendRegistryError::DuplicateProvider { .. })
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn register_and_unregister_change_availability() {
    let mut registry = EngineBackendProviderRegistry::<FakeBundle>::empty();
    let mihomo = EngineId::mihomo();
    assert!(matches!(
        registry.availability(&mihomo),
        EngineBackendProviderAvailability::Unsupported { .. }
    ));
    assert!(matches!(
        registry.backend(&mihomo),
        Err(PortError::Unsupported { .. })
    ));

    let counters = Arc::new(AtomicUsize::new(0));
    let (registered, provider_calls) = provider(FakeProvider::available(
        mihomo.clone(),
        Arc::clone(&counters),
    ));
    registry.register(registered).unwrap();
    assert_eq!(
        registry.availability(&mihomo),
        EngineBackendProviderAvailability::Registered
    );
    let bundle = registry.backend(&mihomo).unwrap();
    assert_eq!(bundle.engine_id(), &mihomo);
    assert_eq!(provider_calls.load(Ordering::SeqCst), 1);

    registry.unregister(&mihomo).unwrap();
    assert!(matches!(
        registry.availability(&mihomo),
        EngineBackendProviderAvailability::Unsupported { .. }
    ));
    assert!(matches!(
        registry.backend(&mihomo),
        Err(PortError::Unsupported { .. })
    ));
    assert_eq!(provider_calls.load(Ordering::SeqCst), 1);
    assert!(matches!(
        registry.unregister(&mihomo),
        Err(EngineBackendRegistryError::UnknownProvider { .. })
    ));
}

#[test]
fn provider_returning_a_foreign_engine_bundle_is_rejected() {
    let counters = Arc::new(AtomicUsize::new(0));
    let (provider, calls) = provider(FakeProvider::returning(
        EngineId::mihomo(),
        EngineId::sing_box(),
        Arc::clone(&counters),
    ));
    let registry = EngineBackendProviderRegistry::new([provider]).unwrap();

    let error = match registry.backend(&EngineId::mihomo()) {
        Ok(_) => panic!("a foreign bundle must never be accepted"),
        Err(error) => error,
    };

    assert!(matches!(error, PortError::Failed(_)));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn provider_failure_is_propagated_without_an_identity_swap() {
    let counters = Arc::new(AtomicUsize::new(0));
    let (provider, calls) = provider(FakeProvider::unavailable(
        EngineId::sing_box(),
        Arc::clone(&counters),
    ));
    let registry = EngineBackendProviderRegistry::new([provider]).unwrap();

    let error = match registry.backend(&EngineId::sing_box()) {
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

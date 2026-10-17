use super::*;
use infiltrator_contract::adapter_operation_declaration::AdapterOperationOwner;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, CoreLifecycleOperation, CoreOperation, EngineCapabilityDecision,
};
use infiltrator_ports::core_binary_selection::{
    BundledBinarySource, CoreBinarySelection, CoreBinarySelectionPolicy,
};
use infiltrator_ports::core_capability_provider::CoreCapabilityProviderError;
use lurker_core_contract::identity::EngineId;
use std::env;
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

    fn binary_selection_policy(
        &self,
        _bundled_source: BundledBinarySource,
    ) -> CoreBinarySelectionPolicy {
        CoreBinarySelectionPolicy::ManagedOnly
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
fn desktop_binary_policy_keeps_sing_box_managed_only_and_mihomo_bundle_aware() {
    let factory = desktop_runtime_factory().unwrap();
    let bundle = BundledBinarySource {
        candidates: Vec::new(),
        data_directory: env::temp_dir(),
    };
    let mihomo = factory
        .provider(&MihomoVersionPort::engine_identifier())
        .unwrap();
    assert!(matches!(
        mihomo.binary_selection_policy(bundle.clone()),
        CoreBinarySelectionPolicy::PreferBundled(_)
    ));

    let sing_box = factory.provider(&singbox_api::engine_id()).unwrap();
    assert_eq!(
        sing_box.binary_selection_policy(bundle),
        CoreBinarySelectionPolicy::ManagedOnly
    );
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

#[test]
fn desktop_capability_registry_uses_sing_box_lifecycle_owner_and_fails_closed_for_unknown_engine() {
    let provider =
        desktop_core_capability_provider(DesktopLogStreamAdapter::CoreOwnerScopeBound).unwrap();
    let sing_box = singbox_api::engine_id();
    let matrix = provider.matrix_for(&sing_box, None).unwrap();
    let start = matrix
        .entries
        .iter()
        .find(|entry| entry.operation == CoreOperation::Lifecycle(CoreLifecycleOperation::Start))
        .unwrap();
    assert_eq!(
        start.engine,
        EngineCapabilityDecision::OwnerManaged {
            owner: AdapterOperationOwner::CoreLifecycle,
        }
    );
    assert_eq!(start.adapter, AdapterCapabilityDecision::Supported);

    let unknown = EngineId::new("future-engine").unwrap();
    assert!(matches!(
        provider.matrix_for(&unknown, None),
        Err(CoreCapabilityProviderError::EngineUnavailable { .. })
    ));
}

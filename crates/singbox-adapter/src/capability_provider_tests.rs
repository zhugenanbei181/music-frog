use super::*;
use infiltrator_contract::adapter_operation_declaration::AdapterOperationOwner;
use infiltrator_contract::core_capabilities::{
    CapabilityAccess, CapabilityMaturity, CoreCapabilityKey, CoreCapabilityStatus,
    CoreLifecycleOperation, EngineCapabilityDecision, OutboundOperation,
};

fn desktop_provider() -> SingBoxCoreCapabilityProvider {
    SingBoxCoreCapabilityProvider::new_for_target(
        HostKind::Desktop,
        EngineBinaryTarget::X86_64UnknownLinuxGnu,
        [
            CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
            CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
        ],
    )
}

#[test]
fn desktop_lifecycle_is_owner_managed_while_unintegrated_operations_stay_unsupported() {
    let provider = desktop_provider();
    let engine = crate::engine_id();
    let matrix = provider
        .matrix_for(&engine, None)
        .expect("registered sing-box declaration creates a matrix");
    let start_key = CoreCapabilityKey::new(
        matrix.scope.clone(),
        CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
    );
    let start = matrix.resolve(start_key);
    assert!(matches!(
        start.engine,
        Some(EngineCapabilityDecision::OwnerManaged {
            owner: AdapterOperationOwner::CoreLifecycle
        })
    ));
    assert_eq!(
        start.status,
        CoreCapabilityStatus::Available {
            access: CapabilityAccess::Supported,
            maturity: CapabilityMaturity::Stable,
        }
    );

    let outbound_key = CoreCapabilityKey::new(
        matrix.scope.clone(),
        CoreOperation::Outbound(OutboundOperation::List),
    );
    assert!(matches!(
        matrix.resolve(outbound_key).status,
        CoreCapabilityStatus::Unsupported { .. }
    ));
}

#[test]
fn sing_box_capability_provider_rejects_foreign_engine_and_unregistered_target() {
    let provider = desktop_provider();
    let mihomo = EngineId::new("mihomo").expect("valid engine ID");
    assert!(matches!(
        provider.matrix_for(&mihomo, None),
        Err(CoreCapabilityProviderError::EngineUnavailable { .. })
    ));

    let unsupported_target = SingBoxCoreCapabilityProvider::new_for_target(
        HostKind::Desktop,
        EngineBinaryTarget::X86_64PcWindowsMsvc,
        [CoreOperation::Lifecycle(CoreLifecycleOperation::Start)],
    );
    let matrix = unsupported_target
        .matrix_for(&crate::engine_id(), None)
        .expect("unsupported target still produces an honest matrix");
    let key = CoreCapabilityKey::new(
        matrix.scope.clone(),
        CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
    );
    assert!(matches!(
        matrix.resolve(key).status,
        CoreCapabilityStatus::HostUnavailable { .. }
    ));
}

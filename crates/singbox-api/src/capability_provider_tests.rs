use super::*;
use infiltrator_contract::adapter_operation_declaration::AdapterOperationOwner;
use infiltrator_contract::core_capabilities::{
    CapabilityAccess, CapabilityMaturity, CoreCapabilityKey, CoreCapabilityStatus,
    CoreLifecycleOperation, EngineCapabilityDecision, OutboundOperation, VersionOperation,
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
    let engine = EngineId::sing_box();
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
fn sing_box_rollback_is_available_only_when_the_product_composes_its_version_owner() {
    let engine = EngineId::sing_box();
    let operations = [
        CoreOperation::Version(VersionOperation::Inspect),
        CoreOperation::Version(VersionOperation::Rollback),
    ];
    let uncomposed_matrix = desktop_provider()
        .matrix_for(&engine, None)
        .expect("registered sing-box declaration creates a matrix");
    for operation in operations {
        let uncomposed = uncomposed_matrix.resolve(CoreCapabilityKey::new(
            uncomposed_matrix.scope.clone(),
            operation,
        ));
        assert!(matches!(
            uncomposed.status,
            CoreCapabilityStatus::AdapterMissing { .. }
        ));
    }

    let composed_provider = SingBoxCoreCapabilityProvider::new_for_target(
        HostKind::Desktop,
        EngineBinaryTarget::X86_64UnknownLinuxGnu,
        [
            CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
            CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
            operations[0],
            operations[1],
        ],
    );
    let matrix = composed_provider
        .matrix_for(&engine, None)
        .expect("registered sing-box declaration creates a matrix");
    for operation in operations {
        let version = matrix.resolve(CoreCapabilityKey::new(matrix.scope.clone(), operation));
        assert!(matches!(
            version.engine,
            Some(EngineCapabilityDecision::OwnerManaged {
                owner: AdapterOperationOwner::VersionManagement
            })
        ));
        assert_eq!(
            version.status,
            CoreCapabilityStatus::Available {
                access: CapabilityAccess::Supported,
                maturity: CapabilityMaturity::Stable,
            }
        );
    }
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
        .matrix_for(&EngineId::sing_box(), None)
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

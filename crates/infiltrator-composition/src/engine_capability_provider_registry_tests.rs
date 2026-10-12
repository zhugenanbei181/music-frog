use super::*;
use infiltrator_contract::core_capabilities::{CoreLifecycleOperation, CoreOperation};
use infiltrator_contract::surface::HostKind;
use mihomo_api::core_capability_provider::MihomoCoreCapabilityProvider;
use singbox_api::capability_provider::SingBoxCoreCapabilityProvider;

fn lifecycle_provider(engine: &str) -> (EngineId, Arc<dyn CoreCapabilityProvider>) {
    let start = CoreOperation::Lifecycle(CoreLifecycleOperation::Start);
    match engine {
        "mihomo" => (
            EngineId::new(engine).unwrap(),
            Arc::new(MihomoCoreCapabilityProvider::new(
                HostKind::Desktop,
                [start],
            )),
        ),
        "sing-box" => (
            EngineId::new(engine).unwrap(),
            Arc::new(SingBoxCoreCapabilityProvider::new(
                HostKind::Desktop,
                [start],
            )),
        ),
        _ => panic!("test fixture engine is registered explicitly"),
    }
}

#[test]
fn capability_matrix_dispatch_is_engine_scoped_across_registered_adapters() {
    let (mihomo_id, mihomo) = lifecycle_provider("mihomo");
    let (sing_box_id, sing_box) = lifecycle_provider("sing-box");
    let registry = EngineCoreCapabilityProviderRegistry::new([
        (mihomo_id.clone(), mihomo),
        (sing_box_id.clone(), sing_box),
    ])
    .unwrap();

    let mihomo_matrix = registry.matrix_for(&mihomo_id, None).unwrap();
    let sing_box_matrix = registry.matrix_for(&sing_box_id, None).unwrap();
    assert_eq!(mihomo_matrix.scope.engine_id, mihomo_id);
    assert_eq!(sing_box_matrix.scope.engine_id, sing_box_id);
}

#[test]
fn missing_and_duplicate_engine_registrations_fail_closed() {
    let (engine_id, provider) = lifecycle_provider("mihomo");
    let duplicate = EngineCoreCapabilityProviderRegistry::new([
        (engine_id.clone(), Arc::clone(&provider)),
        (engine_id.clone(), provider),
    ]);
    assert!(matches!(duplicate, Err(PortError::Failed(_))));

    let (engine_id, provider) = lifecycle_provider("mihomo");
    let registry = EngineCoreCapabilityProviderRegistry::new([(engine_id, provider)]).unwrap();
    let unknown = EngineId::new("future-engine").unwrap();
    assert!(matches!(
        registry.matrix_for(&unknown, None),
        Err(CoreCapabilityProviderError::EngineUnavailable { .. })
    ));
}

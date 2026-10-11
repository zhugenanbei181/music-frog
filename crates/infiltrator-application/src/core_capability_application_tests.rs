use super::*;
use infiltrator_contract::adapter_operation_declaration::AdapterOperationOwner;
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, AdapterRevision, CoreCapabilityEntry, CoreOperation,
    EngineBinaryTarget, EngineCapabilityDecision, HostCapabilityDecision, ProfileOperation,
};
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use infiltrator_contract::surface::HostKind;
use infiltrator_ports::core_capability_provider::CoreCapabilityProviderError;

struct FixedMatrixProvider(CoreCapabilityMatrix);

impl CoreCapabilityProvider for FixedMatrixProvider {
    fn matrix_for(
        &self,
        _engine_id: &EngineId,
        _core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        Ok(self.0.clone())
    }
}

fn supported_matrix_for(scope: CoreCapabilityScope) -> CoreCapabilityMatrix {
    CoreCapabilityMatrix::new(
        scope,
        vec![CoreCapabilityEntry::new(
            CoreOperation::Profile(ProfileOperation::Validate),
            EngineCapabilityDecision::Supported,
            HostCapabilityDecision::Supported,
            AdapterCapabilityDecision::Supported,
        )],
    )
    .expect("test capability matrix has a unique operation")
}

#[test]
fn capability_matrix_with_another_engine_or_version_is_rejected() {
    let requested_engine = EngineId::new("mihomo").expect("valid engine ID");
    let requested_version = CoreVersion::new("v1.19.32").expect("valid version");
    let target = EngineBinaryTarget::current();
    let host = target.host_kind().expect("current target is registered");
    let stale_version = CoreVersion::new("v1.19.31").expect("valid version");
    let foreign_engine = EngineId::new("sing-box").expect("valid engine ID");

    for provider_scope in [
        CoreCapabilityScope::new_for_target(
            foreign_engine,
            Some(requested_version.clone()),
            host,
            target,
            AdapterRevision::new(1),
        ),
        CoreCapabilityScope::new_for_target(
            requested_engine.clone(),
            Some(stale_version),
            host,
            target,
            AdapterRevision::new(1),
        ),
        CoreCapabilityScope::new_for_target(
            requested_engine.clone(),
            None,
            host,
            target,
            AdapterRevision::new(1),
        ),
    ] {
        let application = CoreCapabilityApplication::new(Arc::new(FixedMatrixProvider(
            supported_matrix_for(provider_scope),
        )));
        assert!(
            application
                .matrix_for(&requested_engine, Some(&requested_version))
                .is_err()
        );
    }
}

#[test]
fn mismatched_host_and_binary_target_matrix_is_rejected_before_resolution() {
    let engine = EngineId::new("mihomo").expect("valid engine ID");
    let version = CoreVersion::new("v1.19.32").expect("valid version");
    let operation = CoreOperation::Profile(ProfileOperation::Validate);
    let matrix = CoreCapabilityMatrix::new(
        CoreCapabilityScope::new_for_target(
            engine.clone(),
            Some(version.clone()),
            HostKind::Android,
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(1),
        ),
        vec![CoreCapabilityEntry::new(
            operation,
            EngineCapabilityDecision::Supported,
            HostCapabilityDecision::Supported,
            AdapterCapabilityDecision::Supported,
        )],
    )
    .expect("matrix structure is valid so the application boundary must reject its scope");
    let application = CoreCapabilityApplication::new(Arc::new(FixedMatrixProvider(matrix)));
    let core_scope = CoreScope {
        identity: CoreIdentity::new(
            engine.clone(),
            CoreInstanceId::new("test-core").expect("valid instance ID"),
            Some(version.clone()),
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    };

    assert!(application.matrix_for(&engine, Some(&version)).is_err());
    assert!(matches!(
        application.resolve(Some(&core_scope), operation),
        CoreCapabilityLookup::InvalidProviderMatrix { failure, .. }
            if failure.code == ErrorCode::InvalidState
    ));
}

#[test]
fn capability_matrix_from_another_binary_target_is_rejected() {
    let engine = EngineId::new("mihomo").expect("valid engine ID");
    let version = CoreVersion::new("v1.19.32").expect("valid version");
    let operation = CoreOperation::Profile(ProfileOperation::Validate);
    let other_target = match EngineBinaryTarget::current() {
        EngineBinaryTarget::X86_64UnknownLinuxGnu => EngineBinaryTarget::Aarch64UnknownLinuxGnu,
        EngineBinaryTarget::Aarch64UnknownLinuxGnu => EngineBinaryTarget::X86_64UnknownLinuxGnu,
        EngineBinaryTarget::X86_64UnknownLinuxMusl => EngineBinaryTarget::Aarch64UnknownLinuxMusl,
        EngineBinaryTarget::Aarch64UnknownLinuxMusl => EngineBinaryTarget::X86_64UnknownLinuxMusl,
        EngineBinaryTarget::X86_64PcWindowsMsvc => EngineBinaryTarget::Aarch64PcWindowsMsvc,
        EngineBinaryTarget::Aarch64PcWindowsMsvc => EngineBinaryTarget::X86_64PcWindowsMsvc,
        EngineBinaryTarget::X86_64PcWindowsGnu => EngineBinaryTarget::X86_64PcWindowsGnullvm,
        EngineBinaryTarget::X86_64PcWindowsGnullvm => EngineBinaryTarget::X86_64PcWindowsGnu,
        EngineBinaryTarget::Aarch64PcWindowsGnullvm => EngineBinaryTarget::X86_64PcWindowsGnullvm,
        EngineBinaryTarget::X86_64AppleDarwin => EngineBinaryTarget::Aarch64AppleDarwin,
        EngineBinaryTarget::Aarch64AppleDarwin => EngineBinaryTarget::X86_64AppleDarwin,
        EngineBinaryTarget::X86_64LinuxAndroid => EngineBinaryTarget::Aarch64LinuxAndroid,
        EngineBinaryTarget::Aarch64LinuxAndroid => EngineBinaryTarget::X86_64LinuxAndroid,
        EngineBinaryTarget::X86_64AppleIos => EngineBinaryTarget::Aarch64AppleIos,
        EngineBinaryTarget::Aarch64AppleIos => EngineBinaryTarget::X86_64AppleIos,
        EngineBinaryTarget::Aarch64AppleIosSim => EngineBinaryTarget::Aarch64AppleIos,
        EngineBinaryTarget::Aarch64AppleIosMacabi => EngineBinaryTarget::Aarch64AppleIos,
        EngineBinaryTarget::Unregistered => EngineBinaryTarget::X86_64UnknownLinuxGnu,
    };
    let matrix = CoreCapabilityMatrix::new(
        CoreCapabilityScope::new_for_target(
            engine.clone(),
            Some(version.clone()),
            other_target.host_kind().expect("registered test target"),
            other_target,
            AdapterRevision::new(1),
        ),
        vec![CoreCapabilityEntry::new(
            operation,
            EngineCapabilityDecision::Supported,
            HostCapabilityDecision::Supported,
            AdapterCapabilityDecision::Supported,
        )],
    )
    .expect("matrix structure is valid so the application boundary must reject its scope");
    let application = CoreCapabilityApplication::new(Arc::new(FixedMatrixProvider(matrix)));

    assert!(application.matrix_for(&engine, Some(&version)).is_err());
}

#[test]
fn capability_matrix_with_zero_adapter_revision_is_rejected() {
    let engine = EngineId::new("mihomo").expect("valid engine ID");
    let version = CoreVersion::new("v1.19.32").expect("valid version");
    let operation = CoreOperation::Profile(ProfileOperation::Validate);
    let target = EngineBinaryTarget::current();
    let matrix = CoreCapabilityMatrix::new(
        CoreCapabilityScope::new_for_target(
            engine.clone(),
            Some(version.clone()),
            target.host_kind().expect("registered current target"),
            target,
            AdapterRevision::new(0),
        ),
        vec![CoreCapabilityEntry::new(
            operation,
            EngineCapabilityDecision::Supported,
            HostCapabilityDecision::Supported,
            AdapterCapabilityDecision::Supported,
        )],
    )
    .expect("matrix structure is valid so the application boundary must reject its scope");
    let application = CoreCapabilityApplication::new(Arc::new(FixedMatrixProvider(matrix)));

    assert!(application.matrix_for(&engine, Some(&version)).is_err());
}

#[test]
fn capability_application_revalidates_owner_authority_after_provider_return() {
    let engine = EngineId::new("mihomo").expect("valid engine ID");
    let version = CoreVersion::new("v1.19.32").expect("valid version");
    let target = EngineBinaryTarget::current();
    let operation = CoreOperation::Profile(ProfileOperation::Validate);
    let mut matrix = supported_matrix_for(CoreCapabilityScope::new_for_target(
        engine.clone(),
        Some(version.clone()),
        target.host_kind().expect("registered build target"),
        target,
        AdapterRevision::new(1),
    ));
    matrix.entries[0].engine = EngineCapabilityDecision::OwnerManaged {
        owner: AdapterOperationOwner::CoreLifecycle,
    };
    let application = CoreCapabilityApplication::new(Arc::new(FixedMatrixProvider(matrix)));

    assert!(matches!(
        application.resolve(
            Some(&CoreScope {
                identity: CoreIdentity::new(
                    engine,
                    CoreInstanceId::new("test-core").expect("valid instance ID"),
                    Some(version),
                ),
                generation: CoreGeneration::new(1),
                session_token: None,
            }),
            operation,
        ),
        CoreCapabilityLookup::InvalidProviderMatrix { failure, .. }
            if failure.code == ErrorCode::InvalidState
    ));
}

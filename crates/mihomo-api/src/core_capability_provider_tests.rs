use super::{
    MIHOMO_OPERATION_DECLARATION, MihomoCoreCapabilityProvider, evidence_manifest,
    is_canonical_mihomo_version, operation_declarations,
};
use infiltrator_contract::adapter_operation_declaration::{
    AdapterOperationDeclarationFile, AdapterOperationOwner, AdapterOperationSupport,
    OperationEvidenceAsset, OperationEvidenceId, OperationEvidenceLock,
    ValidatedAdapterOperationDeclarations,
};
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, AdapterRevision, ConnectionOperation, CoreCapabilityKey,
    CoreCapabilityStatus, CoreGarbageCollectionOperation, CoreLifecycleOperation, CoreOperation,
    CoreVersionRequirement, DnsOperation, EngineBinaryTarget, EngineCapabilityDecision,
    FakeIpCacheOperation, GeoOperation, LogsOperation, OutboundOperation, ProfileOperation,
    ProviderOperation, ProxyControlOperation, RulesOperation, SubscriptionOperation, TunOperation,
    VersionOperation,
};
use infiltrator_contract::surface::HostKind;
use infiltrator_ports::core_capability_provider::CoreCapabilityProvider;
use lurker_core_contract::identity::{CoreVersion, EngineId};
use std::collections::HashSet;

fn locked_versions_and_operations() -> Vec<(String, String, u64, Vec<String>)> {
    evidence_manifest()
        .and_then(|manifest| manifest.get("assets"))
        .and_then(serde_json::Value::as_array)
        .expect("valid embedded evidence manifest")
        .iter()
        .map(|asset| {
            let version = asset
                .get("version")
                .and_then(serde_json::Value::as_str)
                .expect("asset version")
                .to_string();
            let operations = asset
                .get("operations")
                .and_then(serde_json::Value::as_array)
                .expect("asset operations")
                .iter()
                .map(|operation| {
                    operation
                        .as_str()
                        .expect("operation identifier")
                        .to_string()
                })
                .collect();
            let target = asset
                .get("binary_target")
                .and_then(serde_json::Value::as_str)
                .expect("asset binary target")
                .to_owned();
            let adapter_revision = asset
                .get("adapter_revision")
                .and_then(serde_json::Value::as_u64)
                .expect("asset adapter revision");
            (version, target, adapter_revision, operations)
        })
        .collect()
}

fn next_patch_version(version: &str) -> String {
    let mut parts = version
        .strip_prefix('v')
        .expect("canonical version")
        .split('.')
        .map(|part| part.parse::<u64>().expect("numeric version component"));
    let major = parts.next().expect("major");
    let minor = parts.next().expect("minor");
    let patch = parts.next().expect("patch");
    format!("v{major}.{minor}.{}", patch + 1)
}

fn version_tuple(version: &str) -> (u64, u64, u64) {
    let mut parts = version
        .strip_prefix('v')
        .expect("canonical version")
        .split('.')
        .map(|part| part.parse::<u64>().expect("numeric version component"));
    (
        parts.next().expect("major"),
        parts.next().expect("minor"),
        parts.next().expect("patch"),
    )
}

fn version_install_declarations(
    support: AdapterOperationSupport,
) -> ValidatedAdapterOperationDeclarations {
    let mut declaration: AdapterOperationDeclarationFile =
        serde_json::from_str(MIHOMO_OPERATION_DECLARATION).expect("Mihomo declaration");
    let install = declaration
        .operations
        .iter_mut()
        .find(|entry| entry.operation == CoreOperation::Version(VersionOperation::Install))
        .expect("version install is registered");
    install.support = support;
    let registered_operations = declaration
        .operations
        .iter()
        .filter_map(|entry| match &entry.support {
            AdapterOperationSupport::Supported { evidence_id } => Some(evidence_id.clone()),
            AdapterOperationSupport::OwnerManaged { .. }
            | AdapterOperationSupport::EvidenceOpen { .. }
            | AdapterOperationSupport::Unsupported { .. } => None,
        })
        .collect::<HashSet<_>>();

    let evidence_lock = OperationEvidenceLock::new(
        EngineId::new("mihomo").expect("Mihomo engine ID"),
        [OperationEvidenceAsset {
            version: CoreVersion::new("v1.19.32").expect("version"),
            target: EngineBinaryTarget::X86_64UnknownLinuxGnu,
            adapter_revision: AdapterRevision::new(1),
            operations: registered_operations,
        }],
    )
    .expect("target-scoped evidence lock");
    ValidatedAdapterOperationDeclarations::validate(declaration, "mihomo", &evidence_lock)
        .expect("typed version install declaration")
}

#[test]
fn only_manifest_verified_operations_are_supported_at_exact_locked_versions() {
    let declarations = operation_declarations().expect("validated Mihomo declarations");
    let locked = locked_versions_and_operations();
    assert_eq!(
        locked.len(),
        2,
        "guarded evidence lock has exactly two releases"
    );
    assert!(
        locked
            .iter()
            .all(|(version, _, _, _)| is_canonical_mihomo_version(version))
    );

    let mut version_cases = vec![
        None,
        Some(next_patch_version(
            locked
                .iter()
                .map(|(version, _, _, _)| version.as_str())
                .max_by_key(|version| version_tuple(version))
                .expect("locked releases"),
        )),
    ];
    version_cases.extend(
        locked
            .iter()
            .map(|(version, _, _, _)| Some(version.clone())),
    );
    version_cases.push(Some(format!("{}-rc.1", locked[0].0)));
    version_cases.push(Some(format!("{}+build.1", locked[0].0)));
    version_cases.push(Some(locked[0].0.trim_start_matches('v').to_string()));

    let provider = MihomoCoreCapabilityProvider::new_for_target(
        HostKind::Desktop,
        EngineBinaryTarget::X86_64UnknownLinuxGnu,
        CoreOperation::all().iter().copied(),
    );
    let engine_id = EngineId::new("mihomo").expect("valid engine ID");

    for version_case in version_cases {
        let version = version_case
            .as_deref()
            .map(|value| CoreVersion::new(value).expect("nonempty version"));
        let matrix = provider
            .matrix_for(&engine_id, version.as_ref())
            .expect("Mihomo capability matrix");
        assert_eq!(matrix.entries.len(), CoreOperation::all().len());

        for operation in CoreOperation::all() {
            let entry = matrix
                .entries
                .iter()
                .find(|entry| entry.operation == *operation)
                .expect("every catalogue operation has an entry");

            match declarations.support(*operation) {
                AdapterOperationSupport::Unsupported { .. } => assert!(matches!(
                    &entry.engine,
                    EngineCapabilityDecision::Unsupported { .. }
                )),
                AdapterOperationSupport::OwnerManaged { owner } => {
                    assert!(matches!(
                        operation.version_requirement(),
                        CoreVersionRequirement::IndependentOfActiveVersion
                    ));
                    assert_eq!(
                        entry.engine,
                        EngineCapabilityDecision::OwnerManaged { owner: *owner }
                    );
                }
                AdapterOperationSupport::EvidenceOpen { .. } => {
                    match operation.version_requirement() {
                        CoreVersionRequirement::IndependentOfActiveVersion => {
                            assert_eq!(entry.engine, EngineCapabilityDecision::Supported)
                        }
                        CoreVersionRequirement::RequiresActiveVersion => assert!(matches!(
                            &entry.engine,
                            EngineCapabilityDecision::VersionUnavailable { .. }
                        )),
                    }
                }
                AdapterOperationSupport::Supported { evidence_id } => match operation
                    .version_requirement()
                {
                    CoreVersionRequirement::IndependentOfActiveVersion => {
                        assert_eq!(entry.engine, EngineCapabilityDecision::Supported)
                    }
                    CoreVersionRequirement::RequiresActiveVersion => {
                        let manifest_operations = version_case.as_deref().and_then(|version| {
                            locked
                                .iter()
                                .find(|(candidate, _, _, _)| candidate == version)
                                .map(|(_, target, revision, operations)| {
                                    (target.as_str(), *revision, operations)
                                })
                        });
                        let should_be_supported =
                            manifest_operations.is_some_and(|(target, revision, operations)| {
                                target == "x86_64-unknown-linux-gnu"
                                    && revision == 2
                                    && operations
                                        .iter()
                                        .any(|candidate| candidate == evidence_id.as_str())
                            });
                        if should_be_supported {
                            assert_eq!(
                                entry.engine,
                                EngineCapabilityDecision::Supported,
                                "{operation:?} at {version_case:?} has exact manifest evidence"
                            );
                        } else {
                            assert!(
                                matches!(
                                    &entry.engine,
                                    EngineCapabilityDecision::VersionUnavailable { .. }
                                ),
                                "{operation:?} at {version_case:?} must fail closed"
                            );
                        }
                    }
                },
            }
        }
    }
}

fn assert_evidence_open_operation_stays_closed_with_historical_locked_evidence(
    operation: CoreOperation,
    operation_name: &str,
) {
    let provider = MihomoCoreCapabilityProvider::new_for_target(
        HostKind::Desktop,
        EngineBinaryTarget::X86_64UnknownLinuxGnu,
        [operation],
    );
    let engine_id = EngineId::new("mihomo").expect("valid engine ID");

    for raw_version in ["v1.19.18", "v1.19.32"] {
        let version = CoreVersion::new(raw_version).expect("locked Mihomo version");
        let matrix = provider
            .matrix_for(&engine_id, Some(&version))
            .expect("Mihomo capability matrix");
        let resolution = matrix.resolve(CoreCapabilityKey::new(matrix.scope.clone(), operation));
        assert!(
            matches!(
                resolution.engine,
                Some(EngineCapabilityDecision::VersionUnavailable { .. })
            ),
            "{operation_name} has only evidence-open status at {raw_version}"
        );
        assert_eq!(
            resolution.adapter,
            Some(AdapterCapabilityDecision::Supported),
            "{operation_name} at {raw_version}"
        );
        assert!(
            matches!(
                resolution.status,
                CoreCapabilityStatus::VersionUnavailable { .. }
            ),
            "{operation_name} must remain closed until production composition is audited at {raw_version}"
        );
    }

    let unregistered_version = CoreVersion::new("v1.19.33").expect("unregistered version");
    let matrix = provider
        .matrix_for(&engine_id, Some(&unregistered_version))
        .expect("unregistered version remains representable");
    let resolution = matrix.resolve(CoreCapabilityKey::new(matrix.scope.clone(), operation));
    assert!(
        matches!(
            resolution.engine,
            Some(EngineCapabilityDecision::VersionUnavailable { .. })
        ),
        "{operation_name}"
    );
    assert!(
        !matches!(resolution.status, CoreCapabilityStatus::Available { .. }),
        "{operation_name}"
    );

    for (host, target, revision) in [
        (
            HostKind::Android,
            EngineBinaryTarget::Aarch64LinuxAndroid,
            AdapterRevision::new(2),
        ),
        (
            HostKind::Desktop,
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(1),
        ),
        (
            HostKind::Desktop,
            EngineBinaryTarget::Aarch64UnknownLinuxGnu,
            AdapterRevision::new(2),
        ),
    ] {
        let scoped_provider =
            MihomoCoreCapabilityProvider::new_with_scope(host, target, revision, [operation]);
        let version = CoreVersion::new("v1.19.32").expect("locked version");
        let matrix = scoped_provider
            .matrix_for(&engine_id, Some(&version))
            .expect("mismatched scope remains representable");
        let resolution = matrix.resolve(CoreCapabilityKey::new(matrix.scope.clone(), operation));
        assert!(
            matches!(
                resolution.status,
                CoreCapabilityStatus::HostUnavailable { .. }
            ),
            "{operation_name} at {host:?}/{target:?}/revision {}",
            revision.value()
        );
    }
}

#[test]
fn historical_dns_query_evidence_does_not_authorize_the_production_operation() {
    assert_evidence_open_operation_stays_closed_with_historical_locked_evidence(
        CoreOperation::Dns(DnsOperation::Query),
        "DNS query",
    );
}

#[test]
fn historical_dns_apply_evidence_does_not_authorize_the_production_operation() {
    assert_evidence_open_operation_stays_closed_with_historical_locked_evidence(
        CoreOperation::Dns(DnsOperation::ApplyConfiguration),
        "DNS configuration apply",
    );
}

#[test]
fn historical_rules_apply_evidence_does_not_authorize_the_production_operation() {
    assert_evidence_open_operation_stays_closed_with_historical_locked_evidence(
        CoreOperation::Rules(RulesOperation::Apply),
        "Rules.Apply",
    );
}

#[test]
fn historical_connection_close_evidence_does_not_authorize_the_production_operation() {
    assert_evidence_open_operation_stays_closed_with_historical_locked_evidence(
        CoreOperation::Connection(ConnectionOperation::Close),
        "Connection.Close",
    );
}

#[test]
fn historical_fake_ip_flush_evidence_does_not_authorize_the_production_operation() {
    assert_evidence_open_operation_stays_closed_with_historical_locked_evidence(
        CoreOperation::FakeIpCache(FakeIpCacheOperation::Flush),
        "FakeIpCache.Flush",
    );
}

#[test]
fn historical_rule_provider_list_evidence_does_not_authorize_the_production_operation() {
    assert_evidence_open_operation_stays_closed_with_historical_locked_evidence(
        CoreOperation::Provider(ProviderOperation::ListRule),
        "Provider.ListRule",
    );
}

#[test]
fn historical_rule_provider_update_evidence_does_not_authorize_the_production_operation() {
    assert_evidence_open_operation_stays_closed_with_historical_locked_evidence(
        CoreOperation::Provider(ProviderOperation::UpdateRule),
        "Provider.UpdateRule",
    );
}

#[test]
fn historical_core_garbage_collection_evidence_does_not_authorize_the_production_operation() {
    assert_evidence_open_operation_stays_closed_with_historical_locked_evidence(
        CoreOperation::CoreGarbageCollection(CoreGarbageCollectionOperation::Trigger),
        "CoreGarbageCollection.Trigger",
    );
}

#[test]
fn explicit_unsupported_declaration_cannot_become_available_from_composition_wiring() {
    let provider = MihomoCoreCapabilityProvider::new_for_target(
        HostKind::Desktop,
        EngineBinaryTarget::X86_64UnknownLinuxGnu,
        CoreOperation::all().iter().copied(),
    );
    let engine_id = EngineId::new("mihomo").expect("valid engine ID");
    let version = CoreVersion::new("v1.19.32").expect("valid version");
    let matrix = provider
        .matrix_for(&engine_id, Some(&version))
        .expect("Mihomo capability matrix");
    for operation in [
        CoreOperation::Profile(ProfileOperation::Validate),
        CoreOperation::Profile(ProfileOperation::Apply),
        CoreOperation::Version(VersionOperation::Install),
    ] {
        let resolution = matrix.resolve(CoreCapabilityKey::new(matrix.scope.clone(), operation));
        assert!(matches!(
            resolution.status,
            CoreCapabilityStatus::Unsupported { .. }
        ));
        assert!(matches!(
            resolution.engine,
            Some(EngineCapabilityDecision::Unsupported { .. })
        ));
        assert_eq!(
            resolution.adapter,
            Some(AdapterCapabilityDecision::Supported)
        );
    }
}

#[test]
fn supported_binary_evidence_still_requires_an_owner_composed_adapter_port() {
    let provider = MihomoCoreCapabilityProvider::new_for_target(
        HostKind::Desktop,
        EngineBinaryTarget::X86_64UnknownLinuxGnu,
        [CoreOperation::Outbound(OutboundOperation::List)],
    );
    let engine_id = EngineId::new("mihomo").expect("valid engine ID");
    let version = CoreVersion::new("v1.19.32").expect("valid version");
    let matrix = provider
        .matrix_for(&engine_id, Some(&version))
        .expect("Mihomo capability matrix");
    let operation = CoreOperation::Outbound(OutboundOperation::Probe);
    let resolution = matrix.resolve(CoreCapabilityKey::new(matrix.scope.clone(), operation));

    assert_eq!(resolution.engine, Some(EngineCapabilityDecision::Supported));
    assert!(matches!(
        resolution.status,
        CoreCapabilityStatus::AdapterMissing { .. }
    ));
}

#[path = "core_capability_provider_tests/matrix_tests.rs"]
mod matrix_tests;

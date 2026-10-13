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
    FakeIpCacheOperation, GeoOperation, OutboundOperation, ProfileOperation, ProviderOperation,
    RulesOperation, SubscriptionOperation, TunOperation, VersionOperation,
};
use infiltrator_contract::core_identity::{CoreVersion, EngineId};
use infiltrator_contract::surface::HostKind;
use infiltrator_ports::core_capability_provider::CoreCapabilityProvider;
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

fn assert_versioned_evidence_is_authorized_only_for_its_exact_mihomo_scope(
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
        assert_eq!(
            resolution.engine,
            Some(EngineCapabilityDecision::Supported),
            "{operation_name} at {raw_version}"
        );
        assert_eq!(
            resolution.adapter,
            Some(AdapterCapabilityDecision::Supported),
            "{operation_name} at {raw_version}"
        );
        assert!(
            matches!(resolution.status, CoreCapabilityStatus::Available { .. }),
            "{operation_name} at {raw_version}"
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
fn real_dns_query_evidence_authorizes_only_the_exact_locked_mihomo_scope() {
    assert_versioned_evidence_is_authorized_only_for_its_exact_mihomo_scope(
        CoreOperation::Dns(DnsOperation::Query),
        "DNS query",
    );
}

#[test]
fn real_dns_apply_configuration_evidence_authorizes_only_the_exact_locked_mihomo_scope() {
    assert_versioned_evidence_is_authorized_only_for_its_exact_mihomo_scope(
        CoreOperation::Dns(DnsOperation::ApplyConfiguration),
        "DNS configuration apply",
    );
}

#[test]
fn real_rules_apply_evidence_authorizes_only_the_exact_locked_mihomo_scope() {
    assert_versioned_evidence_is_authorized_only_for_its_exact_mihomo_scope(
        CoreOperation::Rules(RulesOperation::Apply),
        "Rules.Apply",
    );
}

#[test]
fn real_connection_close_evidence_authorizes_only_the_exact_locked_mihomo_scope() {
    assert_versioned_evidence_is_authorized_only_for_its_exact_mihomo_scope(
        CoreOperation::Connection(ConnectionOperation::Close),
        "Connection.Close",
    );
}

#[test]
fn real_fake_ip_flush_evidence_authorizes_only_the_exact_locked_mihomo_scope() {
    assert_versioned_evidence_is_authorized_only_for_its_exact_mihomo_scope(
        CoreOperation::FakeIpCache(FakeIpCacheOperation::Flush),
        "FakeIpCache.Flush",
    );
}

#[test]
fn real_rule_provider_list_evidence_authorizes_only_the_exact_locked_mihomo_scope() {
    assert_versioned_evidence_is_authorized_only_for_its_exact_mihomo_scope(
        CoreOperation::Provider(ProviderOperation::ListRule),
        "Provider.ListRule",
    );
}

#[test]
fn real_rule_provider_update_evidence_authorizes_only_the_exact_locked_mihomo_scope() {
    assert_versioned_evidence_is_authorized_only_for_its_exact_mihomo_scope(
        CoreOperation::Provider(ProviderOperation::UpdateRule),
        "Provider.UpdateRule",
    );
}

#[test]
fn real_core_garbage_collection_evidence_authorizes_only_the_exact_locked_mihomo_scope() {
    assert_versioned_evidence_is_authorized_only_for_its_exact_mihomo_scope(
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

#[test]
fn owner_managed_lifecycle_still_requires_an_owner_composed_port() {
    let operation = CoreOperation::Lifecycle(CoreLifecycleOperation::Start);
    let provider = MihomoCoreCapabilityProvider::new_for_target(
        HostKind::Desktop,
        EngineBinaryTarget::X86_64UnknownLinuxGnu,
        [],
    );
    let engine_id = EngineId::new("mihomo").expect("valid engine ID");
    let matrix = provider
        .matrix_for(&engine_id, None)
        .expect("Mihomo lifecycle capability matrix");
    let resolution = matrix.resolve(CoreCapabilityKey::new(matrix.scope.clone(), operation));

    assert_eq!(
        resolution.engine,
        Some(EngineCapabilityDecision::OwnerManaged {
            owner: AdapterOperationOwner::CoreLifecycle,
        })
    );
    assert!(matches!(
        resolution.adapter,
        Some(AdapterCapabilityDecision::AdapterMissing { .. })
    ));
    assert!(matches!(
        resolution.status,
        CoreCapabilityStatus::AdapterMissing { .. }
    ));
}

#[test]
fn linux_real_process_evidence_does_not_authorize_android_or_ios_operations() {
    let version = CoreVersion::new("v1.19.32").expect("valid version");
    for (host, target) in [
        (HostKind::Android, EngineBinaryTarget::Aarch64LinuxAndroid),
        (HostKind::Ios, EngineBinaryTarget::Aarch64AppleIos),
    ] {
        let provider = MihomoCoreCapabilityProvider::new_for_target(
            host,
            target,
            CoreOperation::all().iter().copied(),
        );
        let engine_id = EngineId::new("mihomo").expect("valid engine ID");
        let matrix = provider
            .matrix_for(&engine_id, Some(&version))
            .expect("Mihomo capability matrix");
        let resolution = matrix.resolve(CoreCapabilityKey::new(
            matrix.scope.clone(),
            CoreOperation::Outbound(OutboundOperation::Probe),
        ));
        assert!(
            matches!(
                resolution.status,
                CoreCapabilityStatus::HostUnavailable { .. }
            ),
            "{host:?} cannot inherit x86_64-unknown-linux-gnu process evidence"
        );
    }
}

#[test]
fn mismatched_host_target_and_adapter_revision_fail_closed() {
    let version = CoreVersion::new("v1.19.32").expect("valid version");
    let cases = [
        (
            HostKind::Android,
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(2),
        ),
        (
            HostKind::Desktop,
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
    ];
    for (host, target, revision) in cases {
        let provider = MihomoCoreCapabilityProvider::new_with_scope(
            host,
            target,
            revision,
            CoreOperation::all().iter().copied(),
        );
        let engine_id = EngineId::new("mihomo").expect("valid engine ID");
        let matrix = provider
            .matrix_for(&engine_id, Some(&version))
            .expect("Mihomo capability matrix");
        assert_eq!(matrix.scope.binary_target, target);
        assert_eq!(matrix.scope.adapter_revision, revision);
        let resolution = matrix.resolve(CoreCapabilityKey::new(
            matrix.scope.clone(),
            CoreOperation::Outbound(OutboundOperation::List),
        ));
        assert!(
            matches!(
                resolution.status,
                CoreCapabilityStatus::HostUnavailable { .. }
            ),
            "{host:?}/{target:?}/revision {} must not reuse x86_64-unknown-linux-gnu revision 2 evidence",
            revision.value()
        );
    }
}

#[test]
fn capability_matrix_keeps_unverified_operations_closed_and_verified_dns_available() {
    #[derive(Clone, Copy)]
    enum ExpectedStatus {
        Available,
        Unsupported,
    }

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
    let cases = [
        (
            CoreOperation::Profile(ProfileOperation::Validate),
            ExpectedStatus::Unsupported,
        ),
        (
            CoreOperation::Profile(ProfileOperation::Apply),
            ExpectedStatus::Unsupported,
        ),
        (
            CoreOperation::Profile(ProfileOperation::ApplyFeature),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::Dns(DnsOperation::Query),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::Dns(DnsOperation::ApplyConfiguration),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::Rules(RulesOperation::Trace),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::Rules(RulesOperation::Apply),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::Subscription(SubscriptionOperation::Refresh),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::Subscription(SubscriptionOperation::Import),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::Subscription(SubscriptionOperation::Update),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::Geo(GeoOperation::UpdateDatabases),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::Connection(ConnectionOperation::Close),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::Provider(ProviderOperation::ListRule),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::Provider(ProviderOperation::UpdateRule),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::CoreGarbageCollection(CoreGarbageCollectionOperation::Trigger),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::FakeIpCache(FakeIpCacheOperation::Flush),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::Connection(ConnectionOperation::List),
            ExpectedStatus::Unsupported,
        ),
        (
            CoreOperation::Tun(TunOperation::Configure),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::Version(VersionOperation::Install),
            ExpectedStatus::Unsupported,
        ),
    ];

    for (operation, expected) in cases {
        let resolution = matrix.resolve(CoreCapabilityKey::new(matrix.scope.clone(), operation));
        match expected {
            ExpectedStatus::Available => assert!(
                matches!(resolution.status, CoreCapabilityStatus::Available { .. }),
                "{operation:?}"
            ),
            ExpectedStatus::Unsupported => assert!(
                matches!(resolution.status, CoreCapabilityStatus::Unsupported { .. }),
                "{operation:?}"
            ),
        }
    }
}

#[test]
fn active_version_independent_engine_operations_still_require_exact_target_evidence() {
    let operation = CoreOperation::Version(VersionOperation::Install);
    let declarations = version_install_declarations(AdapterOperationSupport::Supported {
        evidence_id: OperationEvidenceId::new("version.install").expect("evidence ID"),
    });

    assert_eq!(
        declarations.engine_decision_for_scope(
            operation,
            None,
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(1),
        ),
        EngineCapabilityDecision::Supported
    );
    for (target, revision) in [
        (
            EngineBinaryTarget::Aarch64UnknownLinuxGnu,
            AdapterRevision::new(1),
        ),
        (
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(2),
        ),
    ] {
        assert!(matches!(
            declarations.engine_decision_for_scope(operation, None, target, revision),
            EngineCapabilityDecision::VersionUnavailable { .. }
        ));
    }
}

#[test]
fn evidence_open_is_never_available_for_a_version_independent_operation() {
    let operation = CoreOperation::Version(VersionOperation::Install);
    let declarations = version_install_declarations(AdapterOperationSupport::EvidenceOpen {
        reason: "candidate engine behavior is not verified".to_owned(),
    });

    assert!(matches!(
        declarations.engine_decision_for_scope(
            operation,
            None,
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(1),
        ),
        EngineCapabilityDecision::VersionUnavailable { .. }
    ));
}

#[test]
fn locked_binary_observations_do_not_override_typed_profile_unsupported() {
    let declarations = operation_declarations().expect("validated Mihomo declarations");
    let locked = locked_versions_and_operations();
    for (version, target, revision, operation_ids) in locked {
        assert_eq!(target, "x86_64-unknown-linux-gnu", "{version}");
        assert_eq!(revision, 2, "{version}");
        assert_eq!(operation_ids.len(), 24, "{version}");
        let core_version = CoreVersion::new(version.clone()).expect("locked version");
        let tun_operation = CoreOperation::Tun(TunOperation::Configure);
        assert!(
            operation_ids
                .iter()
                .any(|registered| registered == "tun.configure")
        );
        assert!(declarations.has_verified_support_for_scope(
            tun_operation,
            &core_version,
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(revision),
        ));
        let rule_trace_operation = CoreOperation::Rules(RulesOperation::Trace);
        assert!(
            operation_ids
                .iter()
                .any(|registered| registered == "rules.trace")
        );
        assert!(declarations.has_verified_support_for_scope(
            rule_trace_operation,
            &core_version,
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(revision),
        ));
        let profile_feature_operation = CoreOperation::Profile(ProfileOperation::ApplyFeature);
        assert!(
            operation_ids
                .iter()
                .any(|registered| registered == "profile.apply_feature")
        );
        assert!(declarations.has_verified_support_for_scope(
            profile_feature_operation,
            &core_version,
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(revision),
        ));
        for (operation, evidence_id) in [
            (
                CoreOperation::Subscription(SubscriptionOperation::Import),
                "subscription.import",
            ),
            (
                CoreOperation::Subscription(SubscriptionOperation::Update),
                "subscription.update",
            ),
            (
                CoreOperation::Subscription(SubscriptionOperation::Refresh),
                "subscription.refresh",
            ),
        ] {
            assert!(
                operation_ids
                    .iter()
                    .any(|registered| registered == evidence_id),
                "{version} must register {evidence_id}"
            );
            assert!(declarations.has_verified_support_for_scope(
                operation,
                &core_version,
                EngineBinaryTarget::X86_64UnknownLinuxGnu,
                AdapterRevision::new(revision),
            ));
        }
        assert!(
            operation_ids
                .iter()
                .any(|registered| registered == "geo.update_databases"),
            "{version} must register geo.update_databases"
        );
        assert!(declarations.has_verified_support_for_scope(
            CoreOperation::Geo(GeoOperation::UpdateDatabases),
            &core_version,
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(revision),
        ));
        for (operation, evidence_id) in [
            (
                CoreOperation::Profile(ProfileOperation::Validate),
                "profile.validate",
            ),
            (
                CoreOperation::Profile(ProfileOperation::Apply),
                "profile.apply",
            ),
        ] {
            assert!(
                operation_ids
                    .iter()
                    .any(|registered| registered == evidence_id)
            );
            assert!(!declarations.has_verified_support_for_scope(
                operation,
                &core_version,
                EngineBinaryTarget::X86_64UnknownLinuxGnu,
                AdapterRevision::new(revision),
            ));
            assert!(matches!(
                declarations.support(operation),
                AdapterOperationSupport::Unsupported { reason }
                    if reason.contains("CORE-013.4")
            ));
        }
    }
}

#[test]
fn canonical_mihomo_versions_require_exact_release_syntax() {
    for (version, _, _, _) in locked_versions_and_operations() {
        assert!(is_canonical_mihomo_version(&version), "{version}");
        assert!(!is_canonical_mihomo_version(&format!("{version}-rc.1")));
        assert!(!is_canonical_mihomo_version(&format!("{version}+build.1")));
        let components = version
            .strip_prefix('v')
            .expect("canonical version")
            .split('.')
            .collect::<Vec<_>>();
        assert!(!is_canonical_mihomo_version(&version[1..]));
        assert!(!is_canonical_mihomo_version(&format!(
            "v0{}.{}.{}",
            components[0], components[1], components[2]
        )));
        assert!(!is_canonical_mihomo_version(&format!(
            "v{}.{}.0{}",
            components[0], components[1], components[2]
        )));
        assert!(!is_canonical_mihomo_version(&format!(
            "v{}.{}",
            components[0], components[1]
        )));
        assert!(!is_canonical_mihomo_version(&format!("{version}.1")));
    }
}

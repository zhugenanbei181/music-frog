use super::adapter_operation_declaration::{
    AdapterOperationDeclarationError, AdapterOperationDeclarationFile, AdapterOperationOwner,
    AdapterOperationSupport, OperationEvidenceAsset, OperationEvidenceId, OperationEvidenceLock,
    ValidatedAdapterOperationDeclarations,
};
use super::core_capabilities::{
    AdapterRevision, CoreGarbageCollectionOperation, CoreLifecycleOperation, CoreOperation,
    DnsOperation, EngineBinaryTarget, EngineCapabilityDecision, FakeIpCacheOperation, GeoOperation,
    LogsOperation, OutboundOperation, ProfileOperation, ProviderOperation, ProxyControlOperation,
    RulesOperation, SubscriptionOperation, TunOperation,
};
use super::core_identity::{CoreVersion, EngineId};
use std::collections::HashSet;

const MIHOMO_DECLARATION: &str =
    include_str!("../../mihomo-api/adapter-operation-declarations.json");
const SING_BOX_DECLARATION: &str =
    include_str!("../../singbox-api/adapter-operation-declarations.json");
const MIHOMO_EVIDENCE_LOCK: &str = include_str!("../../../packaging/mihomo-evidence-assets.json");
const SING_BOX_EVIDENCE_LOCK: &str =
    include_str!("../../../packaging/sing-box-evidence-assets.json");

fn lock_from_manifest(engine_name: &str, source: &str) -> OperationEvidenceLock {
    OperationEvidenceLock::from_manifest_json(source, engine_name)
        .expect("target-scoped evidence manifest is valid")
}

fn parse_declaration(source: &str) -> AdapterOperationDeclarationFile {
    serde_json::from_str(source).expect("adapter declaration is valid JSON")
}

fn validate_source(
    declaration: &str,
    engine: &str,
    evidence_manifest: &str,
) -> ValidatedAdapterOperationDeclarations {
    ValidatedAdapterOperationDeclarations::validate(
        parse_declaration(declaration),
        engine,
        &lock_from_manifest(engine, evidence_manifest),
    )
    .expect("declaration is complete and backed by its evidence lock")
}

#[test]
fn both_engine_declarations_match_the_complete_unique_core_operation_catalogue() {
    let catalogue = CoreOperation::all().iter().copied().collect::<HashSet<_>>();
    assert_eq!(catalogue.len(), CoreOperation::all().len());

    for (engine, declaration, evidence_lock) in [
        ("mihomo", MIHOMO_DECLARATION, MIHOMO_EVIDENCE_LOCK),
        ("sing-box", SING_BOX_DECLARATION, SING_BOX_EVIDENCE_LOCK),
    ] {
        let validated = validate_source(declaration, engine, evidence_lock);
        let file = parse_declaration(declaration);
        let declared = file
            .operations
            .iter()
            .map(|item| item.operation)
            .collect::<HashSet<_>>();

        assert_eq!(
            declared.len(),
            file.operations.len(),
            "{engine} has no duplicates"
        );
        assert_eq!(declared, catalogue, "{engine} covers CoreOperation::all");
        assert_eq!(validated.engine().as_str(), engine);
        let lock = lock_from_manifest(engine, evidence_lock);
        assert_eq!(lock.engine().as_str(), engine);
        assert!(lock.versions().next().is_some());
    }
}

#[test]
fn only_explicitly_supported_operations_resolve_to_verified_support() {
    let mihomo_lock = lock_from_manifest("mihomo", MIHOMO_EVIDENCE_LOCK);
    let mihomo_file = parse_declaration(MIHOMO_DECLARATION);
    let validated = ValidatedAdapterOperationDeclarations::validate(
        mihomo_file.clone(),
        "mihomo",
        &mihomo_lock,
    )
    .expect("Mihomo supported claims match the pinned lock");
    let supported = CoreOperation::all()
        .iter()
        .filter(|operation| validated.verified_supported_evidence(**operation).is_some())
        .copied()
        .collect::<HashSet<_>>();
    let expected_supported = [
        CoreOperation::Outbound(OutboundOperation::List),
        CoreOperation::Outbound(OutboundOperation::Select),
        CoreOperation::Outbound(OutboundOperation::Probe),
        CoreOperation::Logs(LogsOperation::Stream),
        CoreOperation::Logs(LogsOperation::SetLevel),
        CoreOperation::ProxyControl(ProxyControlOperation::GetRoutingIntent),
        CoreOperation::ProxyControl(ProxyControlOperation::SetRoutingIntent),
    ]
    .into_iter()
    .collect::<HashSet<_>>();
    assert_eq!(supported, expected_supported);

    for operation in [
        CoreOperation::Profile(ProfileOperation::ApplyFeature),
        CoreOperation::Dns(DnsOperation::Query),
        CoreOperation::Dns(DnsOperation::ApplyConfiguration),
        CoreOperation::Rules(RulesOperation::Trace),
        CoreOperation::Rules(RulesOperation::Apply),
        CoreOperation::Subscription(SubscriptionOperation::Refresh),
        CoreOperation::Subscription(SubscriptionOperation::Import),
        CoreOperation::Subscription(SubscriptionOperation::Update),
        CoreOperation::Provider(ProviderOperation::ListRule),
        CoreOperation::Provider(ProviderOperation::UpdateRule),
        CoreOperation::Connection(super::core_capabilities::ConnectionOperation::Close),
        CoreOperation::Geo(GeoOperation::UpdateDatabases),
        CoreOperation::FakeIpCache(FakeIpCacheOperation::Flush),
        CoreOperation::Tun(TunOperation::Configure),
        CoreOperation::CoreGarbageCollection(CoreGarbageCollectionOperation::Trigger),
    ] {
        assert!(
            validated.verified_supported_evidence(operation).is_none(),
            "{operation:?} is evidence-open and needs a production owner audit"
        );
    }

    let sing_box = validate_source(SING_BOX_DECLARATION, "sing-box", SING_BOX_EVIDENCE_LOCK);
    assert!(
        CoreOperation::all()
            .iter()
            .all(|operation| sing_box.verified_supported_evidence(*operation).is_none())
    );

    let empty_lock = OperationEvidenceLock::new(
        EngineId::new("mihomo").expect("valid engine ID"),
        [OperationEvidenceAsset {
            version: CoreVersion::new("v1.19.32").expect("valid core version"),
            target: EngineBinaryTarget::X86_64UnknownLinuxGnu,
            adapter_revision: AdapterRevision::new(1),
            operations: HashSet::new(),
        }],
    )
    .expect("evidence lock has a version");
    assert!(matches!(
        ValidatedAdapterOperationDeclarations::validate(mihomo_file.clone(), "mihomo", &empty_lock,),
        Err(AdapterOperationDeclarationError::EvidenceNotRegistered { .. })
    ));

    let partially_registered_lock = OperationEvidenceLock::new(
        EngineId::new("mihomo").expect("valid engine ID"),
        [
            OperationEvidenceAsset {
                version: CoreVersion::new("v1.19.32").expect("valid core version"),
                target: EngineBinaryTarget::X86_64UnknownLinuxGnu,
                adapter_revision: AdapterRevision::new(1),
                operations: HashSet::from([
                    OperationEvidenceId::new("profile.validate").expect("evidence ID is non-empty")
                ]),
            },
            OperationEvidenceAsset {
                version: CoreVersion::new("v1.19.18").expect("valid core version"),
                target: EngineBinaryTarget::X86_64UnknownLinuxGnu,
                adapter_revision: AdapterRevision::new(1),
                operations: HashSet::new(),
            },
        ],
    )
    .expect("evidence lock has versions");
    assert!(matches!(
        ValidatedAdapterOperationDeclarations::validate(
            mihomo_file.clone(),
            "mihomo",
            &partially_registered_lock,
        ),
        Err(AdapterOperationDeclarationError::EvidenceNotRegistered { .. })
    ));

    let mut mismatched_id = serde_json::from_str::<serde_json::Value>(MIHOMO_DECLARATION)
        .expect("Mihomo declaration is JSON");
    let supported_index = mismatched_id["operations"]
        .as_array()
        .expect("operation list")
        .iter()
        .position(|item| item["support"]["status"] == "supported")
        .expect("at least one supported operation");
    mismatched_id["operations"][supported_index]["support"]["evidence_id"] =
        serde_json::Value::String("profile.apply".to_owned());
    let mismatch = serde_json::from_value::<AdapterOperationDeclarationFile>(mismatched_id)
        .expect("mismatched evidence ID still has a valid shape");
    assert!(matches!(
        ValidatedAdapterOperationDeclarations::validate(mismatch, "mihomo", &mihomo_lock),
        Err(AdapterOperationDeclarationError::EvidenceIdOperationMismatch { .. })
    ));

    let sing_box_lock = lock_from_manifest("sing-box", SING_BOX_EVIDENCE_LOCK);
    assert!(matches!(
        ValidatedAdapterOperationDeclarations::validate(mihomo_file, "mihomo", &sing_box_lock),
        Err(AdapterOperationDeclarationError::EvidenceLockEngineMismatch { .. })
    ));
}

#[test]
fn supported_evidence_is_bound_to_exact_version_target_and_adapter_revision() {
    let validated = validate_source(MIHOMO_DECLARATION, "mihomo", MIHOMO_EVIDENCE_LOCK);
    let operation = CoreOperation::Outbound(OutboundOperation::List);
    let version = CoreVersion::new("v1.19.32").expect("valid version");

    assert!(validated.has_verified_support_for_scope(
        operation,
        &version,
        EngineBinaryTarget::X86_64UnknownLinuxGnu,
        AdapterRevision::new(2),
    ));
    for (version, target, revision) in [
        (
            CoreVersion::new("v1.19.33").expect("valid version"),
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(2),
        ),
        (
            CoreVersion::new("v1.19.32").expect("valid version"),
            EngineBinaryTarget::Aarch64LinuxAndroid,
            AdapterRevision::new(2),
        ),
        (
            CoreVersion::new("v1.19.32").expect("valid version"),
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(1),
        ),
    ] {
        assert!(!validated.has_verified_support_for_scope(operation, &version, target, revision,));
    }
}

#[test]
fn engine_decision_is_shared_for_owner_managed_supported_and_unsupported_operations() {
    let sing_box = validate_source(SING_BOX_DECLARATION, "sing-box", SING_BOX_EVIDENCE_LOCK);
    let start = CoreOperation::Lifecycle(CoreLifecycleOperation::Start);
    assert_eq!(
        sing_box.engine_decision_for_scope(
            start,
            None,
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(1),
        ),
        EngineCapabilityDecision::OwnerManaged {
            owner: AdapterOperationOwner::CoreLifecycle,
        }
    );

    let outbound_list = CoreOperation::Outbound(OutboundOperation::List);
    assert!(matches!(
        sing_box.engine_decision_for_scope(
            outbound_list,
            None,
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(1),
        ),
        EngineCapabilityDecision::Unsupported { .. }
    ));

    let mihomo = validate_source(MIHOMO_DECLARATION, "mihomo", MIHOMO_EVIDENCE_LOCK);
    let version = CoreVersion::new("v1.19.32").expect("valid version");
    assert_eq!(
        mihomo.engine_decision_for_scope(
            outbound_list,
            Some(&version),
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(2),
        ),
        EngineCapabilityDecision::Supported
    );
    assert!(matches!(
        mihomo.engine_decision_for_scope(
            outbound_list,
            None,
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(2),
        ),
        EngineCapabilityDecision::VersionUnavailable { .. }
    ));
}

#[test]
fn evidence_lock_rejects_unregistered_targets_and_zero_adapter_revisions() {
    let engine = EngineId::new("fixture").expect("valid engine id");
    let version = CoreVersion::new("v1.0.0").expect("valid version");
    let unregistered = OperationEvidenceAsset {
        version: version.clone(),
        target: EngineBinaryTarget::Unregistered,
        adapter_revision: AdapterRevision::new(1),
        operations: HashSet::new(),
    };
    assert!(matches!(
        OperationEvidenceLock::new(engine.clone(), [unregistered]),
        Err(AdapterOperationDeclarationError::UnregisteredEvidenceTarget { .. })
    ));

    let zero_revision = OperationEvidenceAsset {
        version,
        target: EngineBinaryTarget::X86_64UnknownLinuxGnu,
        adapter_revision: AdapterRevision::new(0),
        operations: HashSet::new(),
    };
    assert!(matches!(
        OperationEvidenceLock::new(engine, [zero_revision]),
        Err(AdapterOperationDeclarationError::InvalidEvidenceAdapterRevision { .. })
    ));
}

#[test]
fn evidence_open_and_unsupported_never_resolve_to_verified_support() {
    for (engine, declaration, evidence_lock, expected_evidence_open_count) in [
        ("mihomo", MIHOMO_DECLARATION, MIHOMO_EVIDENCE_LOCK, 15),
        ("sing-box", SING_BOX_DECLARATION, SING_BOX_EVIDENCE_LOCK, 0),
    ] {
        let validated = validate_source(declaration, engine, evidence_lock);
        let mut evidence_open_count = 0;
        let mut unsupported_count = 0;
        let mut owner_managed_count = 0;

        for operation in CoreOperation::all() {
            match validated.support(*operation) {
                AdapterOperationSupport::Supported { .. } => {
                    assert!(validated.verified_supported_evidence(*operation).is_some());
                }
                AdapterOperationSupport::OwnerManaged { .. } => {
                    owner_managed_count += 1;
                    assert!(validated.verified_supported_evidence(*operation).is_none());
                }
                AdapterOperationSupport::EvidenceOpen { .. } => {
                    evidence_open_count += 1;
                    assert!(validated.verified_supported_evidence(*operation).is_none());
                }
                AdapterOperationSupport::Unsupported { .. } => {
                    unsupported_count += 1;
                    assert!(validated.verified_supported_evidence(*operation).is_none());
                }
            }
        }

        assert_eq!(
            evidence_open_count, expected_evidence_open_count,
            "{engine} evidence-open count must match the production audit"
        );
        assert!(unsupported_count > 0, "{engine} exercises Unsupported");
        assert!(owner_managed_count > 0, "{engine} exercises OwnerManaged");
    }
}

#[test]
fn owner_managed_declarations_are_restricted_to_their_typed_operation_owner() {
    let lock = lock_from_manifest("mihomo", MIHOMO_EVIDENCE_LOCK);
    let mut wrong_owner = parse_declaration(MIHOMO_DECLARATION);
    let lifecycle = wrong_owner
        .operations
        .iter_mut()
        .find(|entry| entry.operation == CoreOperation::Lifecycle(CoreLifecycleOperation::Start))
        .expect("lifecycle start declaration exists");
    lifecycle.support = AdapterOperationSupport::OwnerManaged {
        owner: AdapterOperationOwner::VersionManagement,
    };
    assert!(matches!(
        ValidatedAdapterOperationDeclarations::validate(wrong_owner, "mihomo", &lock),
        Err(AdapterOperationDeclarationError::OwnerManagedOperationMismatch { .. })
    ));

    let mut engine_operation = parse_declaration(MIHOMO_DECLARATION);
    let dns_query = engine_operation
        .operations
        .iter_mut()
        .find(|entry| entry.operation == CoreOperation::Dns(DnsOperation::Query))
        .expect("DNS query declaration exists");
    dns_query.support = AdapterOperationSupport::OwnerManaged {
        owner: AdapterOperationOwner::CoreLifecycle,
    };
    assert!(matches!(
        ValidatedAdapterOperationDeclarations::validate(engine_operation, "mihomo", &lock,),
        Err(
            AdapterOperationDeclarationError::OwnerManagedOperationNotAllowed(CoreOperation::Dns(
                DnsOperation::Query
            ))
        )
    ));
}

#[test]
fn incomplete_or_duplicate_operation_sets_fail_closed() {
    let lock = lock_from_manifest("mihomo", MIHOMO_EVIDENCE_LOCK);
    let mut incomplete = parse_declaration(MIHOMO_DECLARATION);
    incomplete.operations.pop();
    assert!(matches!(
        ValidatedAdapterOperationDeclarations::validate(incomplete, "mihomo", &lock),
        Err(AdapterOperationDeclarationError::MissingOperation(_))
    ));

    let mut duplicate = parse_declaration(MIHOMO_DECLARATION);
    let duplicate_entry = duplicate.operations[0].clone();
    duplicate.operations.push(duplicate_entry);
    assert!(matches!(
        ValidatedAdapterOperationDeclarations::validate(duplicate, "mihomo", &lock),
        Err(AdapterOperationDeclarationError::DuplicateOperation(_))
    ));
}

#[test]
fn unknown_missing_and_untyped_json_fields_fail_during_deserialization() {
    let original = serde_json::from_str::<serde_json::Value>(MIHOMO_DECLARATION)
        .expect("Mihomo declaration is JSON");
    let mut unknown_operation_category = original.clone();
    unknown_operation_category["operations"][0]["operation"]["category"] =
        serde_json::Value::String("future_area".to_owned());

    let mut unknown_operation_field = original.clone();
    unknown_operation_field["operations"][0]["operation"]["future"] =
        serde_json::Value::String("unknown".to_owned());

    let mut unknown_support_status = original.clone();
    unknown_support_status["operations"][0]["support"]["status"] =
        serde_json::Value::String("available".to_owned());

    let mut missing_supported_field = original.clone();
    let supported_index = missing_supported_field["operations"]
        .as_array()
        .expect("operation list")
        .iter()
        .position(|item| item["support"]["status"] == "supported")
        .expect("at least one supported operation");
    missing_supported_field["operations"][supported_index]["support"]
        .as_object_mut()
        .expect("support is an object")
        .remove("evidence_id");

    let mut missing_operation_field = original.clone();
    missing_operation_field["operations"][0]
        .as_object_mut()
        .expect("declaration entry is an object")
        .remove("operation");

    let mut unknown_top_level_field = original;
    unknown_top_level_field["unexpected"] = serde_json::Value::Bool(true);

    for invalid in [
        unknown_operation_category,
        unknown_operation_field,
        unknown_support_status,
        missing_supported_field,
        missing_operation_field,
        unknown_top_level_field,
    ] {
        assert!(
            serde_json::from_value::<AdapterOperationDeclarationFile>(invalid).is_err(),
            "malformed declaration must be rejected"
        );
    }
}

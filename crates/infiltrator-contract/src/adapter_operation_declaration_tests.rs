use super::adapter_operation_declaration::{
    AdapterOperationDeclarationError, AdapterOperationDeclarationFile, AdapterOperationOwner,
    AdapterOperationSupport, OperationEvidenceAsset, OperationEvidenceId, OperationEvidenceLock,
    ValidatedAdapterOperationDeclarations,
};
use super::core_capabilities::{
    AdapterRevision, CoreLifecycleOperation, CoreOperation, DnsOperation, EngineBinaryTarget,
    OutboundOperation,
};
use super::core_identity::{CoreVersion, EngineId};
use std::collections::HashSet;

const MIHOMO_DECLARATION: &str =
    include_str!("../../mihomo-api/adapter-operation-declarations.json");
const SING_BOX_DECLARATION: &str =
    include_str!("../../singbox-adapter/adapter-operation-declarations.json");
const MIHOMO_EVIDENCE_LOCK: &str = include_str!("../../../packaging/mihomo-evidence-assets.json");
const SING_BOX_EVIDENCE_LOCK: &str =
    include_str!("../../../packaging/sing-box-evidence-assets.json");

fn lock_from_manifest(engine_name: &str, source: &str) -> OperationEvidenceLock {
    let manifest: serde_json::Value =
        serde_json::from_str(source).expect("evidence manifest is valid JSON");
    let assets = manifest["assets"]
        .as_array()
        .expect("evidence manifest has an asset array");
    assert!(!assets.is_empty(), "evidence lock has at least one asset");
    if let Some(locked_engine) = manifest["engine"].as_str() {
        assert_eq!(locked_engine, engine_name, "evidence lock engine matches");
    }
    let engine = EngineId::new(engine_name.to_owned()).expect("evidence engine ID is valid");
    let release_version = manifest["release"]["version"].as_str();
    let evidence_assets = assets.iter().map(|asset| {
        let version = asset["version"]
            .as_str()
            .or(release_version)
            .expect("each evidence asset belongs to a pinned version");
        let registered = asset["operations"]
            .as_array()
            .map(|operations| {
                operations
                    .iter()
                    .map(|operation| {
                        let evidence_id = operation
                            .as_str()
                            .expect("operation evidence IDs are strings");
                        OperationEvidenceId::new(evidence_id.to_owned())
                            .expect("evidence IDs are non-empty")
                    })
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();
        let binary_target = asset["binary_target"]
            .as_str()
            .expect("each asset has a binary target");
        let target = EngineBinaryTarget::from_target_triple(binary_target)
            .expect("each asset target is registered in the typed target enum");
        let adapter_revision = asset["adapter_revision"]
            .as_u64()
            .expect("each asset has an adapter revision");
        OperationEvidenceAsset {
            version: CoreVersion::new(version.to_owned()).expect("asset version is non-empty"),
            target,
            adapter_revision: AdapterRevision::new(adapter_revision),
            operations: registered,
        }
    });
    OperationEvidenceLock::new(engine, evidence_assets).expect("evidence lock has a pinned version")
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
fn supported_operations_require_matching_ids_registered_in_each_engine_lock() {
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
        .count();
    assert_eq!(supported, 6);

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
        AdapterRevision::new(1),
    ));
    for (version, target, revision) in [
        (
            CoreVersion::new("v1.19.33").expect("valid version"),
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(1),
        ),
        (
            CoreVersion::new("v1.19.32").expect("valid version"),
            EngineBinaryTarget::Aarch64LinuxAndroid,
            AdapterRevision::new(1),
        ),
        (
            CoreVersion::new("v1.19.32").expect("valid version"),
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(2),
        ),
    ] {
        assert!(!validated.has_verified_support_for_scope(operation, &version, target, revision,));
    }
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
    for (engine, declaration, evidence_lock) in [
        ("mihomo", MIHOMO_DECLARATION, MIHOMO_EVIDENCE_LOCK),
        ("sing-box", SING_BOX_DECLARATION, SING_BOX_EVIDENCE_LOCK),
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

        if engine == "mihomo" {
            assert!(evidence_open_count > 0, "Mihomo exercises EvidenceOpen");
        } else {
            assert_eq!(evidence_open_count, 0, "sing-box claims no open evidence");
        }
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

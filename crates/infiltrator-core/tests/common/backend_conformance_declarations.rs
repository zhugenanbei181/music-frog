use super::backend_conformance::{DeclaredOperationExpectation, OperationDeclarationState};
use infiltrator_contract::adapter_operation_declaration::{
    AdapterOperationDeclarationFile, OperationEvidenceAsset, OperationEvidenceId,
    OperationEvidenceLock, ValidatedAdapterOperationDeclarations,
};
use infiltrator_contract::core_capabilities::{
    AdapterRevision, CoreLifecycleOperation, CoreOperation, EngineBinaryTarget, OutboundOperation,
    ProfileOperation, ProxyControlOperation, VersionOperation,
};
use infiltrator_contract::core_identity::{CoreVersion, EngineId};
use std::collections::HashSet;

const MIHOMO_DECLARATION: &str =
    include_str!("../../../mihomo-api/adapter-operation-declarations.json");
const SING_BOX_DECLARATION: &str =
    include_str!("../../../singbox-adapter/adapter-operation-declarations.json");
const MIHOMO_EVIDENCE_LOCK: &str =
    include_str!("../../../../packaging/mihomo-evidence-assets.json");
const SING_BOX_EVIDENCE_LOCK: &str =
    include_str!("../../../../packaging/sing-box-evidence-assets.json");

fn declaration_set(
    engine_name: &str,
    declaration_json: &str,
    evidence_json: &str,
) -> ValidatedAdapterOperationDeclarations {
    let declaration: AdapterOperationDeclarationFile =
        serde_json::from_str(declaration_json).expect("adapter declaration is valid JSON");
    let manifest: serde_json::Value =
        serde_json::from_str(evidence_json).expect("evidence lock is valid JSON");
    let assets = manifest["assets"]
        .as_array()
        .expect("evidence lock has an assets array");
    let release_version = manifest["release"]["version"].as_str();
    let engine = EngineId::new(engine_name).expect("evidence engine ID is valid");
    let locked_assets = assets.iter().map(|asset| {
        let version = asset["version"]
            .as_str()
            .or(release_version)
            .expect("asset version is pinned");
        let target_name = asset["binary_target"]
            .as_str()
            .expect("asset has a concrete binary_target");
        let target = EngineBinaryTarget::from_target_triple(target_name)
            .expect("asset binary target is recognized by the typed contract");
        let adapter_revision = AdapterRevision::new(
            asset["adapter_revision"]
                .as_u64()
                .expect("asset has an adapter revision"),
        );
        let registered = asset["operations"]
            .as_array()
            .map(|operations| {
                operations
                    .iter()
                    .map(|operation| {
                        OperationEvidenceId::new(
                            operation
                                .as_str()
                                .expect("evidence operation is a string")
                                .to_owned(),
                        )
                        .expect("evidence operation ID is non-empty")
                    })
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();
        OperationEvidenceAsset {
            version: CoreVersion::new(version.to_owned()).expect("asset version is valid"),
            target,
            adapter_revision,
            operations: registered,
        }
    });
    let lock = OperationEvidenceLock::new(engine, locked_assets)
        .expect("evidence lock has a concrete binary version and target");
    ValidatedAdapterOperationDeclarations::validate(declaration, engine_name, &lock)
        .expect("typed declarations match the engine operation catalogue and evidence lock")
}

pub(crate) fn mihomo_declarations() -> ValidatedAdapterOperationDeclarations {
    declaration_set("mihomo", MIHOMO_DECLARATION, MIHOMO_EVIDENCE_LOCK)
}

pub(crate) fn sing_box_declarations() -> ValidatedAdapterOperationDeclarations {
    declaration_set("sing-box", SING_BOX_DECLARATION, SING_BOX_EVIDENCE_LOCK)
}

pub(crate) fn declarations_for_engine(engine: &EngineId) -> ValidatedAdapterOperationDeclarations {
    match engine.as_str() {
        "mihomo" => mihomo_declarations(),
        "sing-box" => sing_box_declarations(),
        other => panic!("no conformance operation declarations registered for {other}"),
    }
}

pub(crate) fn supported_engine_operations() -> Vec<DeclaredOperationExpectation> {
    let state = OperationDeclarationState::Supported;
    let mut operations = vec![
        DeclaredOperationExpectation {
            operation: CoreOperation::Outbound(OutboundOperation::List),
            state,
        },
        DeclaredOperationExpectation {
            operation: CoreOperation::Outbound(OutboundOperation::Select),
            state,
        },
        DeclaredOperationExpectation {
            operation: CoreOperation::Outbound(OutboundOperation::Probe),
            state,
        },
        DeclaredOperationExpectation {
            operation: CoreOperation::ProxyControl(ProxyControlOperation::GetRoutingIntent),
            state,
        },
        DeclaredOperationExpectation {
            operation: CoreOperation::ProxyControl(ProxyControlOperation::SetRoutingIntent),
            state,
        },
    ];
    operations.extend(lifecycle_owner_managed_operations());
    operations.push(DeclaredOperationExpectation {
        operation: CoreOperation::Version(VersionOperation::Inspect),
        state: OperationDeclarationState::OwnerManaged,
    });
    operations
}

pub(crate) fn unsupported_profile_transaction_operations() -> Vec<DeclaredOperationExpectation> {
    let state = OperationDeclarationState::Unsupported;
    vec![
        DeclaredOperationExpectation {
            operation: CoreOperation::Profile(ProfileOperation::Validate),
            state,
        },
        DeclaredOperationExpectation {
            operation: CoreOperation::Profile(ProfileOperation::Apply),
            state,
        },
    ]
}

pub(crate) fn unsupported_engine_operations() -> Vec<DeclaredOperationExpectation> {
    let state = OperationDeclarationState::Unsupported;
    let mut operations = vec![
        DeclaredOperationExpectation {
            operation: CoreOperation::Profile(ProfileOperation::Validate),
            state,
        },
        DeclaredOperationExpectation {
            operation: CoreOperation::Profile(ProfileOperation::Apply),
            state,
        },
        DeclaredOperationExpectation {
            operation: CoreOperation::Outbound(OutboundOperation::List),
            state,
        },
        DeclaredOperationExpectation {
            operation: CoreOperation::Outbound(OutboundOperation::Select),
            state,
        },
        DeclaredOperationExpectation {
            operation: CoreOperation::Outbound(OutboundOperation::Probe),
            state,
        },
        DeclaredOperationExpectation {
            operation: CoreOperation::ProxyControl(ProxyControlOperation::GetRoutingIntent),
            state,
        },
        DeclaredOperationExpectation {
            operation: CoreOperation::ProxyControl(ProxyControlOperation::SetRoutingIntent),
            state,
        },
    ];
    operations.extend(lifecycle_owner_managed_operations());
    operations.push(DeclaredOperationExpectation {
        operation: CoreOperation::Version(VersionOperation::Inspect),
        state,
    });
    operations
}

fn lifecycle_owner_managed_operations() -> Vec<DeclaredOperationExpectation> {
    let state = OperationDeclarationState::OwnerManaged;
    vec![
        DeclaredOperationExpectation {
            operation: CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
            state,
        },
        DeclaredOperationExpectation {
            operation: CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
            state,
        },
    ]
}

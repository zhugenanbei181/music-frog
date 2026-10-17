//! Product fake adapters for the shared conformance suite.
//!
//! The pure adapter fakes come from the neutral conformance kit. This module
//! keeps only the product-owned native configuration transaction and profile
//! store fixtures, then runs the same product wrapper the real engines use.

use super::backend_conformance::profile_support::{
    ConformanceLifecycleSession, ConformanceProfileStore,
};
use super::backend_conformance::{
    BackendConformanceCase, BackendConformancePorts, OperationDeclarationState,
    RunningProfileTransactionBehavior, assert_declared_operation_differences_match_case,
    assert_evidence_open_or_unsupported, expected_support, run_backend_conformance,
};
use super::backend_conformance_declarations::{
    declarations_for_engine, mihomo_declarations, sing_box_declarations,
    supported_engine_operations, unsupported_engine_operations,
    unsupported_profile_transaction_operations,
};
use infiltrator_contract::adapter_operation_declaration::{
    AdapterOperationOwner, AdapterOperationSupport,
};
use infiltrator_contract::core_capabilities::{
    AdapterRevision, CoreLifecycleOperation, CoreOperation, EngineBinaryTarget, OutboundOperation,
    ProfileOperation, ProxyControlOperation, VersionOperation,
};
use infiltrator_ports::profile_configuration_transaction::ProfileConfigurationTransactionScope;
use infiltrator_ports::profile_store::ProfileStore;
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use lurker_core_contract::routing_intent::{
    EngineDefinedRoutingIntent, RoutingIntent, RoutingIntentExtensionId,
};
use lurker_core_ports::engine_backend::{
    EngineBackendOwnerScopeBinding, EngineBackendPorts, EngineOperationPorts,
};
use lurker_core_ports::outbound::{OutboundProbeRequest, ProbeTarget};
use lurker_core_ports::profile_document_codec::ProfileDocumentCodecScope;
use lurker_engine_runtime::conformance::case::{DetailsBehavior, ProbeBehavior, RoutingBehavior};
use lurker_engine_runtime::conformance::fake::{FakeCodec, FakeOutbound, FakeProcess, FakeRouting};
use std::sync::Arc;
use tokio::runtime::Builder;

#[path = "backend_conformance_fake_transaction.rs"]
mod fake_transaction;
use fake_transaction::FakeNativeProfile;

#[derive(Clone, Copy)]
enum FakeBackendKind {
    MihomoLike,
    SingBoxLike,
}

struct FakeBackend {
    case: BackendConformanceCase,
    backend: EngineBackendPorts,
    configuration_transaction: FakeNativeProfile,
    profile_store: Arc<ConformanceProfileStore>,
    transaction_owner: Arc<ConformanceLifecycleSession>,
}

impl FakeBackend {
    fn new(kind: FakeBackendKind) -> Self {
        let (engine_name, version, instance_name, details, probe, routing) = match kind {
            FakeBackendKind::MihomoLike => (
                "mihomo",
                "v1.19.32",
                "fake-mihomo",
                DetailsBehavior::Ready,
                ProbeBehavior::Measured,
                RoutingBehavior::Writable {
                    initial: RoutingIntent::FollowRules,
                    requested: RoutingIntent::RouteAllThroughProxy,
                },
            ),
            FakeBackendKind::SingBoxLike => (
                "sing-box",
                "v1.14.3",
                "fake-sing-box",
                DetailsBehavior::Unsupported,
                ProbeBehavior::Unsupported,
                RoutingBehavior::ReadOnlyUnsupported {
                    initial: RoutingIntent::EngineDefined(EngineDefinedRoutingIntent::new(
                        EngineId::new("sing-box").expect("static engine ID is valid"),
                        RoutingIntentExtensionId::new("mode.rule")
                            .expect("static extension ID is valid"),
                    )),
                    requested: RoutingIntent::FollowRules,
                },
            ),
        };
        let engine = EngineId::new(engine_name).expect("fake engine ID is valid");
        let expected_version = CoreVersion::new(version).expect("fake engine version is valid");
        let identity = CoreIdentity::new(
            engine.clone(),
            CoreInstanceId::new(instance_name).expect("fake instance ID is valid"),
            None,
        );
        let scope = CoreScope {
            identity: CoreIdentity::new(
                engine.clone(),
                identity.instance.clone(),
                Some(expected_version.clone()),
            ),
            generation: CoreGeneration::new(1),
            session_token: None,
        };
        let collection_id = id(&engine, "selector");
        let initial_member_id = id(&engine, "member-a");
        let alternate_member_id = id(&engine, "member-b");
        let probe_outbound_id = match kind {
            FakeBackendKind::MihomoLike => alternate_member_id.clone(),
            FakeBackendKind::SingBoxLike => initial_member_id.clone(),
        };
        let valid_profile = match kind {
            FakeBackendKind::MihomoLike => b"fake mihomo profile".to_vec(),
            FakeBackendKind::SingBoxLike => b"fake sing-box profile".to_vec(),
        };
        let invalid_profile = b"invalid document".to_vec();
        let native_invalid_profile = match kind {
            FakeBackendKind::MihomoLike => b"valid fake Mihomo YAML shape, native-invalid".to_vec(),
            FakeBackendKind::SingBoxLike => {
                b"valid fake sing-box JSON shape, native-invalid".to_vec()
            }
        };
        let probe_request = OutboundProbeRequest::new(
            ProbeTarget::http_url("http://probe.test/health").expect("valid fake probe URL"),
            5_000,
        )
        .expect("valid fake probe request");
        let case = BackendConformanceCase {
            engine: engine.clone(),
            expected_version: expected_version.clone(),
            // The fake models the exact Linux GNU evidence row used by the
            // shared declaration matrix; it does not claim to run that binary.
            binary_target: EngineBinaryTarget::X86_64UnknownLinuxGnu,
            adapter_revision: match kind {
                FakeBackendKind::MihomoLike => AdapterRevision::new(2),
                FakeBackendKind::SingBoxLike => AdapterRevision::new(1),
            },
            declared_operations: match kind {
                FakeBackendKind::MihomoLike => {
                    let mut operations = supported_engine_operations();
                    operations.extend(unsupported_profile_transaction_operations());
                    operations
                }
                FakeBackendKind::SingBoxLike => unsupported_engine_operations(),
            },
            valid_profile: valid_profile.clone(),
            invalid_profile,
            native_invalid_profile: native_invalid_profile.clone(),
            collection_id: collection_id.clone(),
            initial_member_id: initial_member_id.clone(),
            alternate_member_id: alternate_member_id.clone(),
            probe_outbound_id,
            probe_request,
            details,
            probe,
            routing: routing.clone(),
            running_profile_transaction: match kind {
                FakeBackendKind::MihomoLike => RunningProfileTransactionBehavior::NotExercised,
                FakeBackendKind::SingBoxLike => RunningProfileTransactionBehavior::Unsupported,
            },
        };
        let codec_scope = ProfileDocumentCodecScope {
            engine_id: engine.clone(),
            format_id: ProfileFormatId::new(format!("{engine_name}-fake"))
                .expect("fake format ID is valid"),
            schema_version: ProfileSchemaVersion::new(1).expect("fake schema version is valid"),
        };
        let adapter_case = case.adapter_case(&codec_scope);
        let process = Arc::new(FakeProcess::new(identity));
        let readiness = Arc::new(process.readiness(expected_version.clone()));
        let codec = Arc::new(FakeCodec::new(
            codec_scope.clone(),
            &case.valid_profile,
            &case.native_invalid_profile,
        ));
        let outbound = Arc::new(FakeOutbound::new(scope.clone(), &adapter_case));
        let routing_port = Arc::new(FakeRouting::new(scope, case.routing.clone()));
        let operations = EngineOperationPorts::available(
            outbound,
            routing_port,
            Arc::new(EngineBackendOwnerScopeBinding::new(engine.clone())),
        );
        let backend =
            EngineBackendPorts::new(engine, process.clone(), readiness, codec, operations)
                .expect("fake engine backend is coherent");
        let transaction_owner = Arc::new(
            ConformanceLifecycleSession::for_process(
                process.as_ref(),
                case.expected_version.clone(),
            )
            .expect("fake process and transaction owner share a typed identity"),
        );
        let profile_store = Arc::new(ConformanceProfileStore::new(
            &codec_scope,
            &case.valid_profile,
        ));
        let configuration_transaction = FakeNativeProfile {
            scope: ProfileConfigurationTransactionScope::new(codec_scope),
            valid_profile: case.valid_profile.clone(),
            native_invalid_profile: case.native_invalid_profile.clone(),
            store: Arc::clone(&profile_store),
            transaction_owner: Arc::clone(&transaction_owner),
        };
        Self {
            case,
            backend,
            configuration_transaction,
            profile_store,
            transaction_owner,
        }
    }

    async fn run(&self) -> Result<(), String> {
        let declarations = declarations_for_engine(&self.case.engine);
        run_backend_conformance(
            &self.case,
            &declarations,
            &BackendConformancePorts {
                backend: &self.backend,
                configuration_transaction: &self.configuration_transaction,
                profile_store: self.profile_store.as_ref(),
                transaction_owner: self.transaction_owner.as_ref(),
            },
        )
        .await
    }
}

fn id(engine: &EngineId, value: &str) -> OutboundId {
    OutboundId::new(engine.clone(), value).expect("fake outbound ID is valid")
}

pub fn fake_mihomo_and_sing_box_profiles_use_the_same_typed_behavior_suite() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("fake conformance runtime builds");
    for kind in [FakeBackendKind::MihomoLike, FakeBackendKind::SingBoxLike] {
        let backend = FakeBackend::new(kind);
        runtime
            .block_on(backend.run())
            .unwrap_or_else(|error| panic!("shared fake conformance failed: {error}"));
        let workspace = runtime
            .block_on(backend.profile_store.load_active_workspace())
            .expect("fake profile workspace remains readable");
        assert_eq!(workspace.document.raw_bytes, backend.case.valid_profile);
        assert_eq!(backend.profile_store.writes(), 1);
        assert_eq!(
            runtime
                .block_on(backend.backend.process().status())
                .expect("fake process state is available"),
            CoreLifecycle::Stopped
        );
    }
}

pub fn declaration_matrix_keeps_evidence_open_and_unsupported_out_of_verified_support() {
    let mihomo = mihomo_declarations();
    let sing_box = sing_box_declarations();
    // The shared fake suite exercises stand-alone adapter behavior; it does
    // not make sing-box operations available through a production owner.
    let mihomo_case = FakeBackend::new(FakeBackendKind::MihomoLike).case;
    let sing_box_case = FakeBackend::new(FakeBackendKind::SingBoxLike).case;
    assert_declared_operation_differences_match_case(&mihomo_case, &mihomo);
    assert_declared_operation_differences_match_case(&sing_box_case, &sing_box);

    let profile_validate = CoreOperation::Profile(ProfileOperation::Validate);
    let profile_apply = CoreOperation::Profile(ProfileOperation::Apply);
    let outbound_list = CoreOperation::Outbound(OutboundOperation::List);
    let outbound_select = CoreOperation::Outbound(OutboundOperation::Select);
    let outbound_probe = CoreOperation::Outbound(OutboundOperation::Probe);
    let routing_read = CoreOperation::ProxyControl(ProxyControlOperation::GetRoutingIntent);
    let routing_write = CoreOperation::ProxyControl(ProxyControlOperation::SetRoutingIntent);

    for operation in [
        outbound_list,
        outbound_select,
        outbound_probe,
        routing_read,
        routing_write,
    ] {
        assert!(matches!(
            mihomo.support(operation),
            AdapterOperationSupport::Supported { .. }
        ));
    }
    for operation in [profile_validate, profile_apply] {
        assert!(matches!(
            mihomo.support(operation),
            AdapterOperationSupport::Unsupported { .. }
        ));
    }
    let mihomo_version = CoreVersion::new("v1.19.32").expect("locked Mihomo version is valid");
    for operation in [
        outbound_list,
        outbound_select,
        outbound_probe,
        routing_read,
        routing_write,
    ] {
        assert!(mihomo.has_verified_support_for_scope(
            operation,
            &mihomo_version,
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(2),
        ));
    }
    // The asset lock alone does not promote a binary check to a product
    // capability; the production adapter declaration still gates it.
    assert!(!mihomo.has_verified_support_for_scope(
        profile_validate,
        &mihomo_version,
        EngineBinaryTarget::X86_64UnknownLinuxGnu,
        AdapterRevision::new(2),
    ));
    assert!(!mihomo.has_verified_support_for_scope(
        profile_apply,
        &mihomo_version,
        EngineBinaryTarget::X86_64UnknownLinuxGnu,
        AdapterRevision::new(2),
    ));
    assert!(!mihomo.has_verified_support_for_scope(
        outbound_probe,
        &mihomo_version,
        EngineBinaryTarget::Aarch64LinuxAndroid,
        AdapterRevision::new(2),
    ));
    assert!(!mihomo.has_verified_support_for_scope(
        outbound_probe,
        &mihomo_version,
        EngineBinaryTarget::X86_64UnknownLinuxGnu,
        AdapterRevision::new(1),
    ));
    assert_eq!(
        mihomo.engine().as_str(),
        "mihomo",
        "Mihomo declarations are checked against their own evidence lock"
    );
    assert_eq!(
        expected_support(&sing_box, profile_validate),
        OperationDeclarationState::Unsupported
    );
    assert_eq!(
        expected_support(&sing_box, profile_apply),
        OperationDeclarationState::Unsupported
    );
    assert_eq!(
        expected_support(&sing_box, outbound_list),
        OperationDeclarationState::Unsupported
    );
    assert_eq!(
        expected_support(&sing_box, outbound_select),
        OperationDeclarationState::Unsupported
    );
    assert_eq!(
        expected_support(&sing_box, outbound_probe),
        OperationDeclarationState::Unsupported
    );
    assert_eq!(
        expected_support(&sing_box, routing_read),
        OperationDeclarationState::Unsupported
    );
    assert_eq!(
        expected_support(&sing_box, routing_write),
        OperationDeclarationState::Unsupported
    );
    for operation in [
        profile_validate,
        profile_apply,
        outbound_list,
        outbound_select,
        routing_read,
        outbound_probe,
        routing_write,
    ] {
        assert_evidence_open_or_unsupported(&sing_box, operation);
        assert!(!sing_box.has_verified_support_for_scope(
            operation,
            &CoreVersion::new("v1.14.3").expect("locked sing-box version is valid"),
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(1),
        ));
    }
    assert!(matches!(
        sing_box.support(outbound_probe),
        AdapterOperationSupport::Unsupported { reason } if !reason.trim().is_empty()
    ));
    for (declarations, engine) in [(&mihomo, "mihomo"), (&sing_box, "sing-box")] {
        for operation in [
            CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
            CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
        ] {
            assert!(
                matches!(
                    declarations.support(operation),
                    AdapterOperationSupport::OwnerManaged {
                        owner: AdapterOperationOwner::CoreLifecycle
                    }
                ),
                "{engine} process lifecycle remains owned outside binary evidence"
            );
            assert!(
                declarations
                    .verified_supported_evidence(operation)
                    .is_none()
            );
        }
    }
    assert_eq!(
        expected_support(
            &mihomo,
            CoreOperation::Lifecycle(CoreLifecycleOperation::Start)
        ),
        OperationDeclarationState::OwnerManaged
    );
    assert_eq!(
        expected_support(
            &sing_box,
            CoreOperation::Lifecycle(CoreLifecycleOperation::Stop)
        ),
        OperationDeclarationState::OwnerManaged
    );
    let version_inspect = CoreOperation::Version(VersionOperation::Inspect);
    assert!(matches!(
        mihomo.support(version_inspect),
        AdapterOperationSupport::OwnerManaged {
            owner: AdapterOperationOwner::VersionManagement
        }
    ));
    assert!(
        mihomo
            .verified_supported_evidence(version_inspect)
            .is_none()
    );
    assert_eq!(
        expected_support(&sing_box, version_inspect),
        OperationDeclarationState::OwnerManaged,
        "version inspection belongs to the shared version-management owner"
    );
    assert_eq!(
        expected_support(&sing_box, CoreOperation::Version(VersionOperation::Install)),
        OperationDeclarationState::Unsupported,
        "sing-box installation has not been connected through a product owner"
    );
    assert_eq!(
        expected_support(
            &sing_box,
            CoreOperation::Version(VersionOperation::Activate)
        ),
        OperationDeclarationState::Unsupported,
        "sing-box activation has not been connected through a product owner"
    );
    assert_eq!(
        expected_support(
            &sing_box,
            CoreOperation::Version(VersionOperation::Uninstall)
        ),
        OperationDeclarationState::Unsupported,
        "sing-box uninstall has not been connected through a product owner"
    );
    assert_eq!(
        expected_support(
            &sing_box,
            CoreOperation::Version(VersionOperation::Rollback)
        ),
        OperationDeclarationState::OwnerManaged,
        "sing-box rollback belongs to the shared version-management owner"
    );
    assert!(
        sing_box
            .verified_supported_evidence(version_inspect)
            .is_none()
    );
}

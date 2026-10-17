use super::backend_conformance::profile_support::{
    ConformanceLifecycleSession, ConformanceProfileStore,
};
use super::backend_conformance::{
    BackendConformanceCase, BackendConformancePorts, DetailsBehavior, OperationDeclarationState,
    ProbeBehavior, RoutingBehavior, RunningProfileTransactionBehavior,
    assert_declared_operation_differences_match_case, assert_evidence_open_or_unsupported,
    expected_support, run_backend_conformance,
};
use super::backend_conformance_declarations::{
    declarations_for_engine, mihomo_declarations, sing_box_declarations,
    supported_engine_operations, unsupported_engine_operations,
    unsupported_profile_transaction_operations,
};
use async_trait::async_trait;
use infiltrator_contract::adapter_operation_declaration::{
    AdapterOperationOwner, AdapterOperationSupport,
};
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_capabilities::{
    AdapterRevision, CoreLifecycleOperation, CoreOperation, EngineBinaryTarget, OutboundOperation,
    ProfileOperation, ProxyControlOperation, VersionOperation,
};
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use infiltrator_contract::core_process::CoreProcessStartContext;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::outbound::details::{
    OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState, OutboundHealthObservation,
    OutboundObservation, OutboundProbeAvailability, OutboundProtocolObservation, OutboundSnapshot,
    OutboundUdpSupport,
};
use infiltrator_contract::outbound::identity::{OutboundId, SnapshotRevision};
use infiltrator_contract::outbound::inventory::{
    OutboundInventorySnapshot, OutboundInventoryState, OutboundProbeSnapshot, OutboundProbeState,
    OutboundSelectionSet, OutboundSelectionState, SelectionAvailability,
};
use infiltrator_contract::profile_document::DocumentValidation;
use infiltrator_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use infiltrator_contract::routing_intent::{
    EngineDefinedRoutingIntent, RoutingIntent, RoutingIntentExtensionId,
};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::error::PortError;
use infiltrator_ports::outbound::{OutboundPort, OutboundProbeRequest, ProbeTarget};
use infiltrator_ports::profile_configuration_transaction::ProfileConfigurationTransactionScope;
use infiltrator_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::routing_intent::RoutingIntentPort;
use std::sync::{Arc, Mutex};
use tokio::runtime::Builder;

#[path = "backend_conformance_fake_transaction.rs"]
mod fake_transaction;
use fake_transaction::FakeNativeProfile;

#[path = "backend_conformance_fakes/outbound_routing.rs"]
mod outbound_routing;
use outbound_routing::*;

#[derive(Clone, Copy)]
enum FakeBackendKind {
    MihomoLike,
    SingBoxLike,
}

struct FakeBackend {
    case: BackendConformanceCase,
    process: FakeProcess,
    readiness: FakeReadiness,
    codec: FakeCodec,
    configuration_transaction: FakeNativeProfile,
    profile_store: Arc<ConformanceProfileStore>,
    transaction_owner: Arc<ConformanceLifecycleSession>,
    outbound: FakeOutbound,
    routing: FakeRouting,
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
            probe_outbound_id: probe_outbound_id.clone(),
            probe_request,
            details: details.clone(),
            probe: probe.clone(),
            routing: routing.clone(),
            running_profile_transaction: match kind {
                FakeBackendKind::MihomoLike => RunningProfileTransactionBehavior::NotExercised,
                FakeBackendKind::SingBoxLike => RunningProfileTransactionBehavior::Unsupported,
            },
        };
        let phase = Arc::new(Mutex::new(FakeProcessPhase::Stopped));
        let profile_store = Arc::new(ConformanceProfileStore::new(
            &ProfileDocumentCodecPort::scope(&FakeCodec {
                engine: engine.clone(),
                valid_profile: valid_profile.clone(),
                native_invalid_profile: native_invalid_profile.clone(),
            }),
            &valid_profile,
        ));
        let process = FakeProcess {
            identity,
            phase: Arc::clone(&phase),
        };
        let transaction_owner = Arc::new(
            ConformanceLifecycleSession::for_process(&process, expected_version.clone())
                .expect("fake process and transaction owner share a typed identity"),
        );
        let routing_scope = scope.clone();
        let outbound = FakeOutbound {
            engine: engine.clone(),
            expected_scope: scope,
            collection_id,
            members: vec![initial_member_id, alternate_member_id],
            selected: Mutex::new(case.initial_member_id.clone()),
            revision: Mutex::new(1),
            probe_id: probe_outbound_id,
            details_behavior: details,
            probe_behavior: probe,
        };
        Self {
            process,
            readiness: FakeReadiness {
                phase: Arc::clone(&phase),
                version: expected_version,
            },
            codec: FakeCodec {
                engine: engine.clone(),
                valid_profile: valid_profile.clone(),
                native_invalid_profile: native_invalid_profile.clone(),
            },
            configuration_transaction: FakeNativeProfile {
                scope: ProfileConfigurationTransactionScope::new(ProfileDocumentCodecPort::scope(
                    &FakeCodec {
                        engine: engine.clone(),
                        valid_profile: valid_profile.clone(),
                        native_invalid_profile: native_invalid_profile.clone(),
                    },
                )),
                valid_profile,
                native_invalid_profile,
                store: Arc::clone(&profile_store),
                transaction_owner: Arc::clone(&transaction_owner),
            },
            outbound,
            routing: FakeRouting {
                expected_scope: routing_scope,
                behavior: routing.clone(),
                current: Mutex::new(match routing {
                    RoutingBehavior::Writable { initial, .. }
                    | RoutingBehavior::ReadOnlyUnsupported { initial, .. } => initial,
                }),
            },
            profile_store,
            transaction_owner,
            case,
        }
    }

    async fn run(&self) -> Result<(), String> {
        let declarations = declarations_for_engine(&self.case.engine);
        run_backend_conformance(
            &self.case,
            &declarations,
            &BackendConformancePorts {
                process: &self.process,
                readiness: &self.readiness,
                codec: &self.codec,
                configuration_transaction: &self.configuration_transaction,
                profile_store: self.profile_store.as_ref(),
                transaction_owner: self.transaction_owner.as_ref(),
                outbound: &self.outbound,
                routing: &self.routing,
            },
        )
        .await
    }
}

#[derive(Clone, Copy)]
enum FakeProcessPhase {
    Stopped,
    Running,
}

struct FakeProcess {
    identity: CoreIdentity,
    phase: Arc<Mutex<FakeProcessPhase>>,
}

#[async_trait]
impl CoreProcess for FakeProcess {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(self.identity.clone())
    }

    async fn start(&self, _context: CoreProcessStartContext) -> Result<(), PortError> {
        let mut phase = self.phase.lock().expect("fake process phase lock");
        match *phase {
            FakeProcessPhase::Stopped => {
                *phase = FakeProcessPhase::Running;
                Ok(())
            }
            FakeProcessPhase::Running => {
                Err(PortError::Failed("fake process already running".into()))
            }
        }
    }

    async fn stop(&self) -> Result<(), PortError> {
        *self.phase.lock().expect("fake process phase lock") = FakeProcessPhase::Stopped;
        Ok(())
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        match *self.phase.lock().expect("fake process phase lock") {
            FakeProcessPhase::Stopped => Ok(CoreLifecycle::Stopped),
            FakeProcessPhase::Running => Ok(CoreLifecycle::Running),
        }
    }

    fn controller_endpoint(&self) -> Option<String> {
        Some("http://127.0.0.1:0".to_owned())
    }
}

struct FakeReadiness {
    phase: Arc<Mutex<FakeProcessPhase>>,
    version: CoreVersion,
}

#[async_trait]
impl CoreReadiness for FakeReadiness {
    async fn probe(&self) -> Result<String, PortError> {
        match *self.phase.lock().expect("fake process phase lock") {
            FakeProcessPhase::Stopped => Err(PortError::Network("fake process is stopped".into())),
            FakeProcessPhase::Running => Ok("fake-loopback-controller".to_owned()),
        }
    }

    async fn active_core_version(&self) -> Result<Option<CoreVersion>, PortError> {
        match *self.phase.lock().expect("fake process phase lock") {
            FakeProcessPhase::Stopped => Ok(None),
            FakeProcessPhase::Running => Ok(Some(self.version.clone())),
        }
    }
}

struct FakeCodec {
    engine: EngineId,
    valid_profile: Vec<u8>,
    native_invalid_profile: Vec<u8>,
}

impl ProfileDocumentCodecPort for FakeCodec {
    fn scope(&self) -> ProfileDocumentCodecScope {
        ProfileDocumentCodecScope {
            engine_id: self.engine.clone(),
            format_id: ProfileFormatId::new(format!("{}-fake", self.engine.as_str()))
                .expect("fake format ID is valid"),
            schema_version: ProfileSchemaVersion::new(1).expect("fake schema version is valid"),
        }
    }

    fn inspect(&self, raw_bytes: &[u8]) -> Result<DocumentValidation, PortError> {
        if raw_bytes == self.valid_profile || raw_bytes == self.native_invalid_profile {
            Ok(DocumentValidation::Valid)
        } else {
            Ok(DocumentValidation::Invalid {
                diagnostics: vec![],
            })
        }
    }
}

fn fake_detail(id: OutboundId) -> OutboundDetail {
    OutboundDetail {
        id,
        protocol: OutboundProtocolObservation::Unknown,
        protocol_display_name: None,
        cipher: OutboundObservation::Unknown,
        cipher_display_name: None,
        collection_kind: OutboundObservation::Unknown,
        features: OutboundObservation::Unknown,
        endpoint: OutboundObservation::Unknown,
        udp: OutboundUdpSupport::Unknown,
        health: OutboundHealthObservation::Unknown,
        current_latency_ms: OutboundObservation::Unknown,
        latency_history: OutboundObservation::Unknown,
        rtt: OutboundObservation::Unknown,
        historical_egress: OutboundObservation::Unknown,
        probe_availability: OutboundProbeAvailability::Unknown,
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
                .block_on(backend.process.status())
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

use super::backend_conformance::{
    BackendConformanceCase, BackendConformancePorts, DeclaredOperationExpectation, DetailsBehavior,
    NativeProfileFixtureApplyOutcome, NativeProfileFixturePort, OperationDeclarationState,
    ProbeBehavior, RoutingBehavior, assert_declared_operation_differences_match_case,
    assert_evidence_open_or_unsupported, expected_support, run_backend_conformance,
};
use async_trait::async_trait;
use infiltrator_contract::adapter_operation_declaration::{
    AdapterOperationDeclarationFile, AdapterOperationOwner, AdapterOperationSupport,
    OperationEvidenceAsset, OperationEvidenceId, OperationEvidenceLock,
    ValidatedAdapterOperationDeclarations,
};
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_capabilities::{
    AdapterRevision, CoreLifecycleOperation, CoreOperation, EngineBinaryTarget, OutboundOperation,
    ProfileOperation, ProxyControlOperation, VersionOperation,
};
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::outbound::details::{
    OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState, OutboundHealthObservation,
    OutboundObservation, OutboundProbeAvailability, OutboundProtocolObservation,
    OutboundUdpSupport,
};
use infiltrator_contract::outbound::identity::{OutboundId, SnapshotRevision};
use infiltrator_contract::outbound::inventory::{
    OutboundInventorySnapshot, OutboundInventoryState, OutboundProbeSnapshot, OutboundProbeState,
    OutboundSelectionSet, OutboundSelectionState, SelectionAvailability,
};
use infiltrator_contract::profile_document::DocumentValidation;
use infiltrator_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use infiltrator_contract::routing_intent::RoutingIntent;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::error::PortError;
use infiltrator_ports::outbound::{OutboundPort, OutboundProbeRequest, ProbeTarget};
use infiltrator_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use infiltrator_ports::routing_intent::RoutingIntentPort;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

const MIHOMO_DECLARATION: &str =
    include_str!("../../../mihomo-api/adapter-operation-declarations.json");
const SING_BOX_DECLARATION: &str =
    include_str!("../../../singbox-adapter/adapter-operation-declarations.json");
const MIHOMO_EVIDENCE_LOCK: &str =
    include_str!("../../../../packaging/mihomo-evidence-assets.json");
const SING_BOX_EVIDENCE_LOCK: &str =
    include_str!("../../../../packaging/sing-box-evidence-assets.json");

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
    native_profile: FakeNativeProfile,
    outbound: FakeOutbound,
    routing: FakeRouting,
    profile_bytes: Arc<Mutex<Vec<u8>>>,
    profile_apply_count: Arc<Mutex<usize>>,
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
                    initial: RoutingIntent::EngineDefined(
                        infiltrator_contract::routing_intent::EngineDefinedRoutingIntent::new(
                            EngineId::new("sing-box").expect("static engine ID is valid"),
                            infiltrator_contract::routing_intent::RoutingIntentExtensionId::new(
                                "mode.rule",
                            )
                            .expect("static extension ID is valid"),
                        ),
                    ),
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
            adapter_revision: AdapterRevision::new(1),
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
        };
        let phase = Arc::new(Mutex::new(FakeProcessPhase::Stopped));
        let profile_bytes = Arc::new(Mutex::new(Vec::new()));
        let profile_apply_count = Arc::new(Mutex::new(0));
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
            process: FakeProcess {
                identity,
                phase: Arc::clone(&phase),
            },
            readiness: FakeReadiness {
                phase,
                version: expected_version,
            },
            codec: FakeCodec {
                engine: engine.clone(),
                valid_profile: valid_profile.clone(),
                native_invalid_profile: native_invalid_profile.clone(),
            },
            native_profile: FakeNativeProfile {
                engine,
                valid_profile,
                native_invalid_profile,
                bytes: Arc::clone(&profile_bytes),
                apply_count: Arc::clone(&profile_apply_count),
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
            profile_bytes,
            profile_apply_count,
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
                native_profile: &self.native_profile,
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

    async fn start(&self) -> Result<(), PortError> {
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

struct FakeNativeProfile {
    engine: EngineId,
    valid_profile: Vec<u8>,
    native_invalid_profile: Vec<u8>,
    bytes: Arc<Mutex<Vec<u8>>>,
    apply_count: Arc<Mutex<usize>>,
}

#[async_trait]
impl NativeProfileFixturePort for FakeNativeProfile {
    fn engine(&self) -> EngineId {
        self.engine.clone()
    }

    async fn validate_native(&self, raw_bytes: &[u8]) -> Result<DocumentValidation, String> {
        if raw_bytes == self.valid_profile {
            Ok(DocumentValidation::Valid)
        } else if raw_bytes == self.native_invalid_profile {
            Ok(DocumentValidation::Invalid {
                diagnostics: vec![],
            })
        } else {
            Err("fake native validator received an unregistered fixture".to_owned())
        }
    }

    async fn apply_and_readback(
        &self,
        raw_bytes: &[u8],
    ) -> Result<NativeProfileFixtureApplyOutcome, String> {
        if raw_bytes == self.native_invalid_profile {
            return Ok(NativeProfileFixtureApplyOutcome::Rejected {
                reason: "fake native validator rejected semantic-invalid document".to_owned(),
            });
        }
        if raw_bytes != self.valid_profile {
            return Err("fake native fixture received an unregistered document".to_owned());
        }
        *self.bytes.lock().expect("fake profile readback lock") = raw_bytes.to_vec();
        let mut apply_count = self.apply_count.lock().expect("fake profile count lock");
        *apply_count += 1;
        Ok(NativeProfileFixtureApplyOutcome::Applied {
            readback: self
                .bytes
                .lock()
                .expect("fake profile readback lock")
                .clone(),
        })
    }

    async fn readback(&self) -> Result<Vec<u8>, String> {
        Ok(self
            .bytes
            .lock()
            .expect("fake profile readback lock")
            .clone())
    }
}

struct FakeOutbound {
    engine: EngineId,
    expected_scope: CoreScope,
    collection_id: OutboundId,
    members: Vec<OutboundId>,
    selected: Mutex<OutboundId>,
    revision: Mutex<u64>,
    probe_id: OutboundId,
    details_behavior: DetailsBehavior,
    probe_behavior: ProbeBehavior,
}

#[async_trait]
impl OutboundPort for FakeOutbound {
    async fn inventory(&self, scope: CoreScope) -> Result<OutboundInventorySnapshot, PortError> {
        self.verify_scope(&scope)?;
        let selected = self
            .selected
            .lock()
            .expect("fake selected member lock")
            .clone();
        let revision = *self.revision.lock().expect("fake inventory revision lock");
        OutboundInventorySnapshot::new(
            scope,
            SnapshotRevision::new(revision),
            OutboundInventoryState::Ready {
                entries: self
                    .members
                    .iter()
                    .map(
                        |id| infiltrator_contract::outbound::details::OutboundSnapshot {
                            id: id.clone(),
                            display_name: Some(id.value().to_owned()),
                        },
                    )
                    .collect(),
                collections: vec![OutboundSelectionSet {
                    id: self.collection_id.clone(),
                    member_ids: self.members.clone(),
                    availability: SelectionAvailability::Selectable,
                    selection: OutboundSelectionState::Ready { selected },
                }],
            },
        )
        .map_err(|error| PortError::Failed(error.to_string()))
    }

    async fn details(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
    ) -> Result<OutboundDetailsSnapshot, PortError> {
        self.verify_scope(&scope)?;
        self.verify_revision(expected_revision)?;
        let state = match self.details_behavior {
            DetailsBehavior::Ready => OutboundDetailsState::Ready {
                details: vec![fake_detail(self.probe_id.clone())],
            },
            DetailsBehavior::Unsupported => OutboundDetailsState::Unsupported,
        };
        OutboundDetailsSnapshot::new(scope, SnapshotRevision::new(1), state)
            .map_err(|error| PortError::Failed(error.to_string()))
    }

    async fn select(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
        collection_id: OutboundId,
        outbound_id: OutboundId,
    ) -> Result<OutboundInventorySnapshot, PortError> {
        self.verify_scope(&scope)?;
        self.verify_revision(expected_revision)?;
        if collection_id != self.collection_id || !self.members.contains(&outbound_id) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake selector request was not in its current inventory",
                false,
            )));
        }
        *self.selected.lock().expect("fake selected member lock") = outbound_id;
        {
            let mut revision = self.revision.lock().expect("fake inventory revision lock");
            *revision = revision
                .checked_add(1)
                .ok_or_else(|| PortError::Failed("fake revision exhausted".to_owned()))?;
        }
        self.inventory(scope).await
    }

    async fn probe(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
        outbound_id: OutboundId,
        _request: OutboundProbeRequest,
    ) -> Result<OutboundProbeSnapshot, PortError> {
        self.verify_scope(&scope)?;
        self.verify_revision(expected_revision)?;
        if outbound_id != self.probe_id {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake probe target was not registered",
                false,
            )));
        }
        let state = match self.probe_behavior {
            ProbeBehavior::Measured => OutboundProbeState::Measured { delay_ms: 1 },
            ProbeBehavior::Unsupported => OutboundProbeState::Unsupported,
        };
        OutboundProbeSnapshot::new(
            scope,
            expected_revision,
            SnapshotRevision::new(1),
            outbound_id,
            state,
        )
        .map_err(|error| PortError::Failed(error.to_string()))
    }
}

impl FakeOutbound {
    fn verify_scope(&self, scope: &CoreScope) -> Result<(), PortError> {
        if scope == &self.expected_scope && scope.identity.engine == self.engine {
            Ok(())
        } else {
            Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake adapter received a foreign or stale scope",
                false,
            )))
        }
    }

    fn verify_revision(&self, expected: SnapshotRevision) -> Result<(), PortError> {
        let actual = *self.revision.lock().expect("fake revision lock");
        if expected == SnapshotRevision::new(actual) {
            Ok(())
        } else {
            Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake adapter rejected a stale inventory revision",
                false,
            )))
        }
    }
}

struct FakeRouting {
    expected_scope: CoreScope,
    behavior: RoutingBehavior,
    current: Mutex<RoutingIntent>,
}

#[async_trait]
impl RoutingIntentPort for FakeRouting {
    async fn observe(&self, scope: &CoreScope) -> Result<RoutingIntent, PortError> {
        self.verify_scope(scope)?;
        Ok(self
            .current
            .lock()
            .expect("fake routing state lock")
            .clone())
    }

    async fn apply(
        &self,
        scope: &CoreScope,
        intent: RoutingIntent,
    ) -> Result<RoutingIntent, PortError> {
        self.verify_scope(scope)?;
        match &self.behavior {
            RoutingBehavior::Writable { initial, requested }
                if &intent == initial || &intent == requested =>
            {
                *self.current.lock().expect("fake routing state lock") = intent.clone();
                Ok(intent)
            }
            RoutingBehavior::ReadOnlyUnsupported { .. } => Err(PortError::unsupported(
                Capability::RoutingIntent,
                "fake sing-box-like backend preserves EngineDefined mode only",
            )),
            RoutingBehavior::Writable { .. } => Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake routing adapter received an unregistered intent",
                false,
            ))),
        }
    }
}

impl FakeRouting {
    fn verify_scope(&self, scope: &CoreScope) -> Result<(), PortError> {
        if scope == &self.expected_scope {
            Ok(())
        } else {
            Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake routing adapter received a foreign scope",
                false,
            )))
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

fn mihomo_declarations() -> ValidatedAdapterOperationDeclarations {
    declaration_set("mihomo", MIHOMO_DECLARATION, MIHOMO_EVIDENCE_LOCK)
}

fn sing_box_declarations() -> ValidatedAdapterOperationDeclarations {
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

pub fn fake_mihomo_and_sing_box_profiles_use_the_same_typed_behavior_suite() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("fake conformance runtime builds");
    for kind in [FakeBackendKind::MihomoLike, FakeBackendKind::SingBoxLike] {
        let backend = FakeBackend::new(kind);
        runtime
            .block_on(backend.run())
            .unwrap_or_else(|error| panic!("shared fake conformance failed: {error}"));
        assert_eq!(
            *backend
                .profile_bytes
                .lock()
                .expect("fake profile bytes lock"),
            backend.case.valid_profile
        );
        assert_eq!(
            *backend
                .profile_apply_count
                .lock()
                .expect("fake profile count lock"),
            1
        );
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
            AdapterRevision::new(1),
        ));
    }
    // The asset lock alone does not promote a binary check to a product
    // capability; the production adapter declaration still gates it.
    assert!(!mihomo.has_verified_support_for_scope(
        profile_validate,
        &mihomo_version,
        EngineBinaryTarget::X86_64UnknownLinuxGnu,
        AdapterRevision::new(1),
    ));
    assert!(!mihomo.has_verified_support_for_scope(
        profile_apply,
        &mihomo_version,
        EngineBinaryTarget::X86_64UnknownLinuxGnu,
        AdapterRevision::new(1),
    ));
    assert!(!mihomo.has_verified_support_for_scope(
        outbound_probe,
        &mihomo_version,
        EngineBinaryTarget::Aarch64LinuxAndroid,
        AdapterRevision::new(1),
    ));
    assert!(!mihomo.has_verified_support_for_scope(
        outbound_probe,
        &mihomo_version,
        EngineBinaryTarget::X86_64UnknownLinuxGnu,
        AdapterRevision::new(2),
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
        OperationDeclarationState::Unsupported,
        "live readiness version observation is not sing-box version-management support"
    );
    assert!(
        sing_box
            .verified_supported_evidence(version_inspect)
            .is_none()
    );
}

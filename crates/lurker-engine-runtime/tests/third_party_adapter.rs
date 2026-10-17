#![cfg(feature = "conformance")]
//! Proof that a third-party adapter can use the neutral kit.
//!
//! The provider below is intentionally defined in this test, not in the kit:
//! it composes [`EngineBackendPorts`] from the neutral fakes and then runs the
//! same adapter and owner-switch conformance the product harness runs.

use futures_executor::block_on;
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use lurker_core_contract::routing_intent::RoutingIntent;
use lurker_core_ports::engine_backend::{
    EngineBackendOwnerScopeBinding, EngineBackendPorts, EngineBackendProvider, EngineOperationPorts,
};
use lurker_core_ports::error::PortError;
use lurker_core_ports::outbound::{OutboundProbeRequest, ProbeTarget};
use lurker_core_ports::profile_document_codec::ProfileDocumentCodecScope;
use lurker_engine_runtime::conformance::case::{
    AdapterConformanceCase, DEFAULT_CONFORMANCE_READINESS_TIMEOUT, DetailsBehavior,
    LifecycleExpectation, ProbeBehavior, RoutingBehavior,
};
use lurker_engine_runtime::conformance::failure::{ConformanceCheck, ConformanceFailure};
use lurker_engine_runtime::conformance::fake::{
    FakeCodec, FakeOutbound, FakeProcess, FakeReadiness, FakeRouting, InMemoryEngineSwitchJournal,
};
use lurker_engine_runtime::conformance::group::ConformanceGroup;
use lurker_engine_runtime::conformance::runner::run_adapter_conformance;
use lurker_engine_runtime::conformance::switch::run_switch_conformance;
use lurker_engine_runtime::registry::EngineBackendProviderRegistry;
use std::sync::Arc;

struct ExampleEngineProvider {
    engine: EngineId,
    process: Arc<FakeProcess>,
    readiness: Arc<FakeReadiness>,
    codec: Arc<FakeCodec>,
    outbound: Arc<FakeOutbound>,
    routing: Arc<FakeRouting>,
}

impl EngineBackendProvider<EngineBackendPorts> for ExampleEngineProvider {
    fn engine_id(&self) -> EngineId {
        self.engine.clone()
    }

    fn create_backend(&self) -> Result<EngineBackendPorts, PortError> {
        EngineBackendPorts::new(
            self.engine.clone(),
            self.process.clone(),
            self.readiness.clone(),
            self.codec.clone(),
            EngineOperationPorts::available(
                self.outbound.clone(),
                self.routing.clone(),
                Arc::new(EngineBackendOwnerScopeBinding::new(self.engine.clone())),
            ),
        )
    }
}

struct Fixture {
    case: AdapterConformanceCase,
    provider: Arc<dyn EngineBackendProvider<EngineBackendPorts>>,
}

fn id(engine: &EngineId, value: &str) -> OutboundId {
    OutboundId::new(engine.clone(), value).expect("the static outbound ID is valid")
}

fn fixture(engine_name: &str, version: &str, documents: &[u8]) -> Fixture {
    let engine = EngineId::new(engine_name).expect("the example engine ID is valid");
    let expected_version = CoreVersion::new(version).expect("the example version is valid");
    let instance = CoreInstanceId::new(format!("{engine_name}-instance"))
        .expect("the example instance is valid");
    let identity = CoreIdentity::new(engine.clone(), instance.clone(), None);
    let scope = CoreScope {
        identity: CoreIdentity::new(engine.clone(), instance, Some(expected_version.clone())),
        generation: CoreGeneration::new(1),
        session_token: None,
    };
    let collection_id = id(&engine, "selector");
    let initial_member_id = id(&engine, "member-a");
    let alternate_member_id = id(&engine, "member-b");
    let probe_outbound_id = alternate_member_id.clone();
    let format =
        ProfileFormatId::new(format!("{engine_name}-format")).expect("the example format is valid");
    let schema = ProfileSchemaVersion::new(1).expect("the example schema version is valid");
    let case = AdapterConformanceCase {
        engine: engine.clone(),
        expected_version: expected_version.clone(),
        profile_format: format,
        profile_schema: schema,
        valid_document: documents.to_vec(),
        invalid_document: b"this is not a valid document: [".to_vec(),
        native_invalid_document: b"native-invalid but syntactically valid".to_vec(),
        collection_id,
        initial_member_id,
        alternate_member_id,
        probe_outbound_id,
        probe_request: OutboundProbeRequest::new(
            ProbeTarget::http_url("http://probe.test/health")
                .expect("the example probe URL is valid"),
            1_000,
        )
        .expect("the example probe request is valid"),
        details: DetailsBehavior::Ready,
        probe: ProbeBehavior::Measured,
        routing: RoutingBehavior::Writable {
            initial: RoutingIntent::FollowRules,
            requested: RoutingIntent::RouteAllThroughProxy,
        },
        lifecycle: LifecycleExpectation::ManagedStartStop,
        readiness_timeout: DEFAULT_CONFORMANCE_READINESS_TIMEOUT,
    };
    let codec_scope = ProfileDocumentCodecScope {
        engine_id: engine.clone(),
        format_id: case.profile_format.clone(),
        schema_version: case.profile_schema,
    };
    let process = Arc::new(FakeProcess::new(identity));
    let readiness = Arc::new(process.readiness(expected_version));
    let codec = Arc::new(FakeCodec::new(
        codec_scope,
        &case.valid_document,
        &case.native_invalid_document,
    ));
    let outbound = Arc::new(FakeOutbound::new(scope.clone(), &case));
    let routing = Arc::new(FakeRouting::new(scope, case.routing.clone()));
    let provider: Arc<dyn EngineBackendProvider<EngineBackendPorts>> =
        Arc::new(ExampleEngineProvider {
            engine,
            process,
            readiness,
            codec,
            outbound,
            routing,
        });
    Fixture { case, provider }
}

#[test]
fn a_third_party_provider_passes_every_adapter_group() {
    let fixture = fixture("example-engine", "v1.0.0", b"example-engine-profile");
    let registry = EngineBackendProviderRegistry::new([Arc::clone(&fixture.provider)])
        .expect("registry builds");
    let backend = registry
        .backend(&fixture.case.engine)
        .expect("the example backend builds");

    block_on(run_adapter_conformance(
        &fixture.case,
        &backend,
        &ConformanceGroup::ALL,
    ))
    .expect("the third-party adapter passes the shared conformance kit");
}

#[test]
fn two_third_party_engines_pass_the_switch_kit() {
    let first = fixture("example-engine", "v1.0.0", b"example-engine-profile");
    let second = fixture("example-partner", "v2.0.0", b"example-partner-profile");
    let registry = Arc::new(
        EngineBackendProviderRegistry::new([
            Arc::clone(&first.provider),
            Arc::clone(&second.provider),
        ])
        .expect("two-provider registry builds"),
    );
    let journal = Arc::new(InMemoryEngineSwitchJournal::new(first.case.engine.clone()));

    block_on(run_switch_conformance(
        registry,
        journal,
        &first.case.engine,
        &second.case.engine,
    ))
    .expect("the two third-party engines pass the owner-switch kit");
}

#[test]
fn a_stopped_only_adapter_can_assert_its_initial_terminal() {
    let mut fixture = fixture("example-document", "v1.0.0", b"example-document-profile");
    fixture.case.lifecycle = LifecycleExpectation::ObserveStoppedOnly;
    let registry = EngineBackendProviderRegistry::new([Arc::clone(&fixture.provider)])
        .expect("registry builds");
    let backend = registry
        .backend(&fixture.case.engine)
        .expect("the example backend builds");

    block_on(run_adapter_conformance(
        &fixture.case,
        &backend,
        &[ConformanceGroup::Document, ConformanceGroup::Lifecycle],
    ))
    .expect("a stopped-only adapter passes its document and initial-terminal groups");
}

#[test]
fn a_wrong_expected_version_is_a_typed_failure() {
    let fixture = fixture("example-engine", "v1.0.0", b"example-engine-profile");
    let mut wrong_case = fixture.case.clone();
    wrong_case.expected_version =
        CoreVersion::new("v9.9.9").expect("the wrong version is still a valid CoreVersion");
    let registry = EngineBackendProviderRegistry::new([Arc::clone(&fixture.provider)])
        .expect("registry builds");
    let backend = registry
        .backend(&fixture.case.engine)
        .expect("the example backend builds");

    let failure = block_on(run_adapter_conformance(
        &wrong_case,
        &backend,
        &[ConformanceGroup::Lifecycle],
    ))
    .expect_err("a mismatching readiness version fails conformance");

    assert_eq!(failure.check(), ConformanceCheck::LifecycleVersion);
    assert!(matches!(failure, ConformanceFailure::Mismatch { .. }));
}

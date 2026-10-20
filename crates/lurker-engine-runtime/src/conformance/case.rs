//! The data one adapter conformance run needs.

use super::failure::{ConformanceCheck, ConformanceFailure};
use lurker_core_contract::identity::{CoreVersion, EngineId};
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use lurker_core_contract::routing_intent::RoutingIntent;
use lurker_core_ports::outbound::OutboundProbeRequest;
use std::time::Duration;

/// Default bound for the readiness wait used by the kit.
pub const DEFAULT_CONFORMANCE_READINESS_TIMEOUT: Duration = Duration::from_secs(10);

/// How the adapter is expected to expose the Core process lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleExpectation {
    /// The adapter starts a Core, becomes ready, reports the expected version
    /// and stops to an exact `Stopped`/no-PID terminal.
    ManagedStartStop,
    /// The adapter only exposes an externally managed Core. Only the initial
    /// stopped state and pure document checks may be asserted.
    ObserveStoppedOnly,
}

/// Expected neutral detail support.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetailsBehavior {
    /// Ready details must contain the probe outbound.
    Ready,
    /// Details must read back the typed unsupported state.
    Unsupported,
}

/// Expected neutral probe support.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeBehavior {
    /// A measured result is expected.
    Measured,
    /// The typed unsupported state is expected.
    Unsupported,
}

/// Expected routing-intent support.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoutingBehavior {
    /// Observation and read-back-after-write are supported.
    Writable {
        /// The routing intent the adapter must report before the write.
        initial: RoutingIntent,
        /// The routing intent the check writes and reads back.
        requested: RoutingIntent,
    },
    /// Observation is supported but writes are typed unsupported.
    ReadOnlyUnsupported {
        /// The routing intent the adapter must report before the write.
        initial: RoutingIntent,
        /// The routing intent the check tries to write.
        requested: RoutingIntent,
    },
}

/// Everything the neutral kit needs to exercise one adapter.
#[derive(Clone, Debug)]
pub struct AdapterConformanceCase {
    /// Engine the adapter under test serves.
    pub engine: EngineId,
    /// Version the adapter must report once the core is ready.
    pub expected_version: CoreVersion,
    /// Native format the document codec speaks.
    pub profile_format: ProfileFormatId,
    /// Schema version the document codec speaks.
    pub profile_schema: ProfileSchemaVersion,
    /// Bytes the codec and the engine must both accept.
    pub valid_document: Vec<u8>,
    /// Bytes the codec must reject.
    pub invalid_document: Vec<u8>,
    /// Syntactically valid for the neutral codec but rejected by the engine's
    /// native validator; it distinguishes native validation from parsing.
    pub native_invalid_document: Vec<u8>,
    /// The selector collection the selection check operates on.
    pub collection_id: OutboundId,
    /// The member the selector starts on.
    pub initial_member_id: OutboundId,
    /// The member the selection check switches to.
    pub alternate_member_id: OutboundId,
    /// The outbound the probe check measures.
    pub probe_outbound_id: OutboundId,
    /// The probe request the probe check issues.
    pub probe_request: OutboundProbeRequest,
    /// What the adapter must report for neutral outbound details.
    pub details: DetailsBehavior,
    /// What the adapter must report for a neutral outbound probe.
    pub probe: ProbeBehavior,
    /// What the adapter must report for the shared routing intent.
    pub routing: RoutingBehavior,
    /// How the adapter is expected to expose the Core process lifecycle.
    pub lifecycle: LifecycleExpectation,
    /// Bound for the readiness wait of this run.
    pub readiness_timeout: Duration,
}

impl AdapterConformanceCase {
    /// Reject a case that cannot describe a meaningful conformance run.
    pub fn validate(&self) -> Result<(), ConformanceFailure> {
        if self.initial_member_id == self.alternate_member_id {
            return Err(ConformanceFailure::state(
                ConformanceCheck::Case,
                "two distinct selector members",
                format!(
                    "{} and {}",
                    self.initial_member_id.value(),
                    self.alternate_member_id.value()
                ),
                "the initial and alternate members must differ",
            ));
        }
        if self.valid_document == self.invalid_document
            || self.valid_document == self.native_invalid_document
            || self.invalid_document == self.native_invalid_document
        {
            return Err(ConformanceFailure::state(
                ConformanceCheck::Case,
                "three distinct document fixtures",
                "duplicated document bytes",
                "valid, invalid and native-invalid fixtures must be distinct",
            ));
        }
        for id in [
            &self.collection_id,
            &self.initial_member_id,
            &self.alternate_member_id,
            &self.probe_outbound_id,
        ] {
            if id.engine_id() != &self.engine {
                return Err(ConformanceFailure::state(
                    ConformanceCheck::Case,
                    format!("outbound IDs bound to {}", self.engine.as_str()),
                    format!("{} is bound to {}", id.value(), id.engine_id().as_str()),
                    "every outbound ID must belong to the conformance engine",
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lurker_core_contract::identity::CoreVersion;
    use lurker_core_contract::outbound::identity::OutboundId;
    use lurker_core_ports::outbound::ProbeTarget;

    fn engine(value: &str) -> EngineId {
        EngineId::new(value).expect("test engine ID is valid")
    }

    fn case() -> AdapterConformanceCase {
        let engine = engine("conformance-engine");
        AdapterConformanceCase {
            engine: engine.clone(),
            expected_version: CoreVersion::new("v1.0.0").expect("test version is valid"),
            profile_format: ProfileFormatId::new("conformance-yaml").expect("test format is valid"),
            profile_schema: ProfileSchemaVersion::new(1).expect("test schema is valid"),
            valid_document: b"valid".to_vec(),
            invalid_document: b"invalid: [".to_vec(),
            native_invalid_document: b"native-invalid".to_vec(),
            collection_id: OutboundId::new(engine.clone(), "selector").expect("test id is valid"),
            initial_member_id: OutboundId::new(engine.clone(), "member-a")
                .expect("test id is valid"),
            alternate_member_id: OutboundId::new(engine.clone(), "member-b")
                .expect("test id is valid"),
            probe_outbound_id: OutboundId::new(engine, "member-b").expect("test id is valid"),
            probe_request: OutboundProbeRequest::new(
                ProbeTarget::http_url("http://probe.test/health").expect("test URL is valid"),
                1_000,
            )
            .expect("test probe request is valid"),
            details: DetailsBehavior::Ready,
            probe: ProbeBehavior::Measured,
            routing: RoutingBehavior::Writable {
                initial: RoutingIntent::FollowRules,
                requested: RoutingIntent::RouteAllThroughProxy,
            },
            lifecycle: LifecycleExpectation::ManagedStartStop,
            readiness_timeout: DEFAULT_CONFORMANCE_READINESS_TIMEOUT,
        }
    }

    #[test]
    fn a_complete_case_validates() {
        case().validate().expect("the complete case is valid");
    }

    #[test]
    fn duplicated_fixtures_and_members_are_rejected() {
        let mut duplicated_members = case();
        duplicated_members.alternate_member_id = duplicated_members.initial_member_id.clone();
        assert_eq!(
            duplicated_members
                .validate()
                .expect_err("duplicate members are rejected")
                .check(),
            ConformanceCheck::Case
        );

        let mut duplicated_documents = case();
        duplicated_documents.invalid_document = duplicated_documents.valid_document.clone();
        assert_eq!(
            duplicated_documents
                .validate()
                .expect_err("duplicate fixtures are rejected")
                .check(),
            ConformanceCheck::Case
        );
    }
}

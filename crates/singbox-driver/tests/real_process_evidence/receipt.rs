//! The machine-readable receipt of one real-binary evidence run.
//!
//! The receipt is the single artifact the local entry point consumes. It
//! names the engine, the typed outcome of every proof and the live version
//! the engine reported. A run that proved nothing says so in the receipt
//! instead of reporting an empty success: the outcome is derived from the
//! proof records, so the receipt can never claim more than the proofs did.

use serde_json::Value;
use serde_json::json;

/// The schema of the machine-readable receipt.
pub const RECEIPT_SCHEMA: &str = "music-frog.real-process-evidence/1";

/// The capabilities the real-binary evidence must prove.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Proof {
    /// The controller handshake and the live core version readback.
    ApiHandshake,
    /// A configuration write to the running engine and its readback.
    ConfigurationWrite,
    /// The typed node speed test of one outbound.
    NodeSpeedTest,
    /// A real traffic flow through the engine's own listener.
    TrafficDispatch,
}

impl Proof {
    /// Every proof the real-binary evidence must produce.
    pub const ALL: [Proof; 4] = [
        Proof::ApiHandshake,
        Proof::ConfigurationWrite,
        Proof::NodeSpeedTest,
        Proof::TrafficDispatch,
    ];

    /// The stable machine-readable name of the proof.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ApiHandshake => "api_handshake",
            Self::ConfigurationWrite => "configuration_write",
            Self::NodeSpeedTest => "node_speed_test",
            Self::TrafficDispatch => "traffic_dispatch",
        }
    }
}

/// The typed outcome of one proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProofOutcome {
    /// The proof produced the typed outcome its engine contract declares.
    Proven {
        /// The evidence the proof observed.
        detail: String,
    },
    /// The engine's typed port declares the capability unsupported.
    Unsupported {
        /// Why the typed port does not model the capability.
        reason: String,
    },
    /// The proof did not reach its expected typed outcome.
    Failed {
        /// Why the proof failed.
        reason: String,
    },
}

/// One proof and the typed outcome it produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofRecord {
    proof: Proof,
    outcome: ProofOutcome,
}

impl ProofRecord {
    /// Builds one record.
    pub fn new(proof: Proof, outcome: ProofOutcome) -> Self {
        Self { proof, outcome }
    }

    /// The proof this record belongs to.
    pub const fn proof(&self) -> Proof {
        self.proof
    }

    /// The typed outcome the proof produced.
    pub const fn outcome(&self) -> &ProofOutcome {
        &self.outcome
    }
}

/// The overall outcome of one real-binary evidence run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EvidenceOutcome {
    /// The environment provides no engine binary, so nothing was proven.
    EnvironmentAbsent {
        /// The environment keys that are missing.
        missing: Vec<String>,
    },
    /// The environment names an engine that cannot run.
    InvalidEnvironment {
        /// Why the environment cannot run an engine.
        reason: String,
    },
    /// Every required proof produced its expected typed outcome.
    Complete,
    /// One proof reported a capability the engine's typed port declares
    /// unsupported; the remaining proofs are complete.
    UnsupportedProof {
        /// The proof that reported the unsupported capability.
        proof: Proof,
    },
    /// The run could not produce the required evidence.
    Defect {
        /// Why the evidence is defective.
        reason: String,
    },
}

/// The machine-readable receipt of one real-binary evidence run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    engine: &'static str,
    outcome: EvidenceOutcome,
    records: Vec<ProofRecord>,
    engine_version: Option<String>,
    binary: String,
}

impl Receipt {
    /// Builds the receipt of a run that never reached a proof because the
    /// environment provides no engine binary.
    pub fn environment_absent(engine: &'static str, missing: Vec<String>) -> Self {
        Self {
            engine,
            outcome: EvidenceOutcome::EnvironmentAbsent { missing },
            records: Vec::new(),
            engine_version: None,
            binary: String::new(),
        }
    }

    /// Builds the receipt of a run that never reached a proof because the
    /// environment names an unusable engine.
    pub fn invalid_environment(engine: &'static str, reason: String) -> Self {
        Self {
            engine,
            outcome: EvidenceOutcome::InvalidEnvironment { reason },
            records: Vec::new(),
            engine_version: None,
            binary: String::new(),
        }
    }

    /// Builds the receipt of a run that could not start or stop the engine.
    pub fn defect(engine: &'static str, reason: String) -> Self {
        Self {
            engine,
            outcome: EvidenceOutcome::Defect { reason },
            records: Vec::new(),
            engine_version: None,
            binary: String::new(),
        }
    }

    /// Builds the receipt of a finished run from its proof records.
    ///
    /// The overall outcome is derived from the records, so the receipt can
    /// never report more than the proofs actually observed.
    pub fn from_records(
        engine: &'static str,
        binary: String,
        engine_version: Option<String>,
        records: Vec<ProofRecord>,
    ) -> Self {
        let outcome = derive_outcome(&records);
        Self {
            engine,
            outcome,
            records,
            engine_version,
            binary,
        }
    }

    /// The overall outcome of the run.
    pub const fn outcome(&self) -> &EvidenceOutcome {
        &self.outcome
    }

    /// Renders the receipt as one line of JSON.
    pub fn to_json(&self) -> String {
        let outcome = match &self.outcome {
            EvidenceOutcome::EnvironmentAbsent { missing } => {
                json!({"kind": "environment_absent", "missing": missing})
            }
            EvidenceOutcome::InvalidEnvironment { reason } => {
                json!({"kind": "invalid_environment", "reason": reason})
            }
            EvidenceOutcome::Complete => json!({"kind": "complete"}),
            EvidenceOutcome::UnsupportedProof { proof } => {
                json!({"kind": "unsupported_proof", "proof": proof.as_str()})
            }
            EvidenceOutcome::Defect { reason } => json!({"kind": "defect", "reason": reason}),
        };
        let proofs: Vec<Value> = self
            .records
            .iter()
            .map(|record| {
                let outcome = match record.outcome() {
                    ProofOutcome::Proven { detail } => json!({"kind": "proven", "detail": detail}),
                    ProofOutcome::Unsupported { reason } => {
                        json!({"kind": "unsupported", "reason": reason})
                    }
                    ProofOutcome::Failed { reason } => json!({"kind": "failed", "reason": reason}),
                };
                json!({"proof": record.proof().as_str(), "outcome": outcome})
            })
            .collect();
        let payload = json!({
            "schema": RECEIPT_SCHEMA,
            "engine": self.engine,
            "outcome": outcome,
            "engine_version": self.engine_version,
            "binary": self.binary,
            "proofs": proofs,
        });
        serde_json::to_string(&payload).unwrap_or_else(|_| {
            format!(
                "{{\"schema\":\"{RECEIPT_SCHEMA}\",\"engine\":\"{}\",\"outcome\":{{\"kind\":\"defect\",\"reason\":\"the receipt could not be encoded\"}}}}",
                self.engine
            )
        })
    }
}

/// Derives the overall outcome from the proof records.
fn derive_outcome(records: &[ProofRecord]) -> EvidenceOutcome {
    for record in records {
        if let ProofOutcome::Failed { reason } = record.outcome() {
            return EvidenceOutcome::Defect {
                reason: reason.clone(),
            };
        }
    }
    for record in records {
        if matches!(record.outcome(), ProofOutcome::Unsupported { .. }) {
            return EvidenceOutcome::UnsupportedProof {
                proof: record.proof(),
            };
        }
    }
    for proof in Proof::ALL {
        if !records.iter().any(|record| record.proof() == proof) {
            return EvidenceOutcome::Defect {
                reason: format!("the {} proof did not run", proof.as_str()),
            };
        }
    }
    EvidenceOutcome::Complete
}

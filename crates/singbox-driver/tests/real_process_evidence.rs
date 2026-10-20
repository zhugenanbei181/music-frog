//! Explicit real-binary evidence harness for the sing-box driver.
//!
//! This is the named entry point of PUB-006.8 / PUB-002.3: it proves the
//! driver against a real sing-box process. The ordinary quality gate only
//! covers mock/fake adapters, so the test is `#[ignore]`d and never runs
//! unless it is requested explicitly:
//!
//! ```text
//! SINGBOX_REAL_BINARY=… SINGBOX_REAL_CONFIG=… SINGBOX_REAL_INSTANCE=… \
//! SINGBOX_REAL_CONTROLLER=127.0.0.1:9090 \
//! SINGBOX_REAL_PROXY=127.0.0.1:1080 SINGBOX_REAL_FIXTURE=127.0.0.1:9099 \
//! SINGBOX_REAL_PROBE_OUTBOUND=DIRECT SINGBOX_REAL_SELECTOR=SELECT \
//! cargo nextest run -p singbox-driver --test real_process_evidence --run-ignored only
//! ```
//!
//! The harness builds the public `SingBoxEngineBackendProvider`, starts one
//! real process through `CoreProcess` and proves four capabilities against the
//! running engine: the controller handshake, the typed node speed test, a real
//! traffic flow through the engine's own listener and a configuration write
//! with its readback.
//!
//! The environment contract is typed: an environment without an engine binary
//! produces an `environment_absent` receipt and never a silent pass, and an
//! environment that names an unusable engine produces an
//! `invalid_environment` receipt. The receipt is written to the file
//! `SINGBOX_REAL_RECEIPT` names and printed to stdout in any case.

#![cfg(feature = "backend")]

#[path = "real_process_evidence/contract.rs"]
mod contract;
#[path = "real_process_evidence/proofs.rs"]
mod proofs;
#[path = "real_process_evidence/receipt.rs"]
mod receipt;

use crate::contract::RealProcessEnvironment;
use crate::receipt::{EvidenceOutcome, Receipt};
use std::env::var;
use std::fs::write;
use std::path::Path;

/// The marker that prefixes the machine-readable receipt on stdout.
const RECEIPT_MARKER: &str = "MUSIC_FROG_REAL_PROCESS_RECEIPT";

#[tokio::test]
#[ignore = "requires the SINGBOX_REAL_* environment contract of a real sing-box binary"]
async fn real_process_evidence() {
    match contract::resolve(|key| var(key).ok()) {
        RealProcessEnvironment::Absent { missing } => {
            let receipt = Receipt::environment_absent(
                "sing-box",
                missing.iter().map(|key| (*key).to_owned()).collect(),
            );
            publish(&receipt, contract::receipt_path().as_deref());
            panic!(
                "the sing-box real-binary evidence is absent: {}",
                missing.join(", ")
            );
        }
        RealProcessEnvironment::Invalid { reason } => {
            let receipt = Receipt::invalid_environment("sing-box", reason.clone());
            publish(&receipt, contract::receipt_path().as_deref());
            panic!("the sing-box real-binary environment is invalid: {reason}");
        }
        RealProcessEnvironment::Present(target) => {
            let receipt = match proofs::run(&target).await {
                Ok(evidence) => Receipt::from_records(
                    "sing-box",
                    target.binary().display().to_string(),
                    evidence.engine_version,
                    evidence.records,
                ),
                Err(reason) => Receipt::defect("sing-box", reason),
            };
            publish(&receipt, target.receipt());
            match receipt.outcome() {
                EvidenceOutcome::Complete => {}
                EvidenceOutcome::UnsupportedProof { proof } => {
                    panic!(
                        "the sing-box real-binary evidence is incomplete: the {} proof reported an unsupported capability",
                        proof.as_str()
                    );
                }
                EvidenceOutcome::Defect { reason } => {
                    panic!("the sing-box real-binary evidence is defective: {reason}");
                }
                EvidenceOutcome::EnvironmentAbsent { .. }
                | EvidenceOutcome::InvalidEnvironment { .. } => {}
            }
        }
    }
}

/// Publishes the machine-readable receipt on stdout and in the receipt file.
fn publish(receipt: &Receipt, path: Option<&Path>) {
    let json = receipt.to_json();
    println!("{RECEIPT_MARKER} {json}");
    let Some(path) = path else {
        return;
    };
    if let Err(error) = write(path, format!("{json}\n")) {
        eprintln!(
            "the sing-box real-binary receipt could not be written to {}: {error}",
            path.display()
        );
    }
}

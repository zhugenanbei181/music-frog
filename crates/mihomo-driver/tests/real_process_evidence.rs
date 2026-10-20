//! Explicit real-binary evidence harness for the Mihomo driver.
//!
//! This is the named entry point of PUB-006.8 / PUB-002.1: it proves the
//! driver against a real Mihomo process. The ordinary quality gate only
//! covers mock/fake adapters, so the test is `#[ignore]`d and never runs
//! unless it is requested explicitly:
//!
//! ```text
//! MIHOMO_REAL_BINARY=… MIHOMO_REAL_CONFIG=… MIHOMO_REAL_HOME=… \
//! MIHOMO_REAL_CONTROLLER=http://127.0.0.1:9090 \
//! MIHOMO_REAL_PROXY=127.0.0.1:1080 MIHOMO_REAL_FIXTURE=127.0.0.1:9099 \
//! MIHOMO_REAL_PROBE_OUTBOUND=CORE-SMOKE-HTTP \
//! MIHOMO_REAL_SELECTOR=CORE-SMOKE-SELECTOR \
//! cargo nextest run -p mihomo-driver --test real_process_evidence --run-ignored only
//! ```
//!
//! The harness builds the public `MihomoEngineBackendProvider`, starts one
//! real process through `CoreProcess` and proves four capabilities against the
//! running engine: the controller handshake, the typed node speed test, a real
//! traffic flow through the engine's own listener and a configuration write
//! with its readback.
//!
//! The environment contract is typed: an environment without an engine binary
//! produces an `environment_absent` receipt and never a silent pass, and an
//! environment that names an unusable engine produces an
//! `invalid_environment` receipt. The receipt is written to the file
//! `MIHOMO_REAL_RECEIPT` names and printed to stdout in any case.

#![cfg(feature = "backend")]

mod contract;
mod proofs;
mod receipt;

use crate::contract::RealProcessEnvironment;
use crate::receipt::{EvidenceOutcome, Receipt};
use std::env::var;
use std::fs::{create_dir_all, write};
use std::path::Path;

/// The marker that prefixes the machine-readable receipt on stdout.
const RECEIPT_MARKER: &str = "MUSIC_FROG_REAL_PROCESS_RECEIPT";

#[tokio::test]
#[ignore = "requires the MIHOMO_REAL_* environment contract of a real Mihomo binary"]
async fn real_process_evidence() {
    match contract::resolve(|key| var(key).ok()) {
        RealProcessEnvironment::Absent { missing } => {
            let receipt = Receipt::environment_absent(
                "mihomo",
                missing.iter().map(|key| (*key).to_owned()).collect(),
            );
            publish(&receipt, contract::receipt_path().as_deref());
            panic!(
                "the Mihomo real-binary evidence is absent: {}",
                missing.join(", ")
            );
        }
        RealProcessEnvironment::Invalid { reason } => {
            let receipt = Receipt::invalid_environment("mihomo", reason.clone());
            publish(&receipt, contract::receipt_path().as_deref());
            panic!("the Mihomo real-binary environment is invalid: {reason}");
        }
        RealProcessEnvironment::Present(target) => {
            create_dir_all(target.home())
                .expect("the Mihomo real-binary home directory must be creatable");
            let receipt = match proofs::run(&target).await {
                Ok(evidence) => Receipt::from_records(
                    "mihomo",
                    target.binary().display().to_string(),
                    evidence.engine_version,
                    evidence.records,
                ),
                Err(reason) => Receipt::defect("mihomo", reason),
            };
            publish(&receipt, target.receipt());
            match receipt.outcome() {
                EvidenceOutcome::Complete => {}
                EvidenceOutcome::UnsupportedProof { proof } => {
                    panic!(
                        "the Mihomo real-binary evidence is incomplete: the {} proof reported an unsupported capability",
                        proof.as_str()
                    );
                }
                EvidenceOutcome::Defect { reason } => {
                    panic!("the Mihomo real-binary evidence is defective: {reason}");
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
            "the Mihomo real-binary receipt could not be written to {}: {error}",
            path.display()
        );
    }
}

//! Shared behavior assertions for engine adapters.
//!
//! Fake cases run in the ordinary focused nextest target. The ignored cases
//! require the SHA-locked Linux binaries and are driven by the unified smoke
//! script; they have not been run as part of the source implementation.

#[path = "common/backend_conformance.rs"]
mod backend_conformance;

#[path = "common/backend_conformance_fakes.rs"]
mod backend_conformance_fakes;

#[path = "common/backend_conformance_real.rs"]
mod backend_conformance_real;

#[test]
fn fake_mihomo_and_sing_box_profiles_use_the_same_typed_behavior_suite() {
    backend_conformance_fakes::fake_mihomo_and_sing_box_profiles_use_the_same_typed_behavior_suite(
    );
}

#[test]
fn declaration_matrix_keeps_evidence_open_and_unsupported_out_of_verified_support() {
    backend_conformance_fakes::declaration_matrix_keeps_evidence_open_and_unsupported_out_of_verified_support();
}

#[tokio::test]
#[ignore = "requires the explicit SHA-locked Mihomo Linux amd64 real-process smoke"]
async fn locked_real_mihomo_runs_the_shared_core_conformance_suite() {
    backend_conformance_real::locked_real_mihomo_runs_the_shared_core_conformance_suite()
        .await
        .unwrap_or_else(|error| panic!("locked real Mihomo conformance failed: {error}"));
}

#[tokio::test]
#[ignore = "requires the explicit SHA-locked sing-box Linux amd64 real-process smoke"]
async fn locked_real_sing_box_runs_the_shared_core_conformance_suite() {
    backend_conformance_real::locked_real_sing_box_runs_the_shared_core_conformance_suite()
        .await
        .unwrap_or_else(|error| panic!("locked real sing-box conformance failed: {error}"));
}

//! Engine version knowledge: identity, capability matrix, upstream target
//! inventory and fixture generation.
//!
//! Live version probing itself is the controller `GET /version` call in
//! [`crate::api::client::MihomoClient`], surfaced to the application boundary as
//! [`crate::driver::ControllerReadiness`].

/// The kernel capability matrix of one Mihomo version.
pub mod capability;
/// The engine identity of the Mihomo driver.
pub mod engine;
/// The fixture generator of the version capability snapshots.
pub mod fixture_automation;
/// The upstream target and artifact inventory of Mihomo releases.
pub mod target_matrix;

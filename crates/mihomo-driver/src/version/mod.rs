//! Engine version knowledge: identity, capability matrix, upstream target
//! inventory and fixture generation.
//!
//! Live version probing itself is the controller `GET /version` call in
//! [`crate::api::client::MihomoClient`], surfaced to the application boundary as
//! [`crate::driver::ControllerReadiness`].

pub mod capability;
pub mod engine;
pub mod fixture_automation;
pub mod target_matrix;

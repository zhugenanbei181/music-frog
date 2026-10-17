//! Engine-neutral adapter conformance kit.
//!
//! This module is compiled only with the `conformance` feature. It depends on
//! the neutral contract and ports only: no executor, product crate, engine
//! adapter or UI toolkit is required. A third party can build
//! [`EngineBackendPorts`] from its own adapters, or from the neutral fakes in
//! [`fake`], and run the exact checks the product harness runs.
//!
//! - [`runner::run_adapter_conformance`] covers the Core process lifecycle,
//!   document inspection, outbound inventory/observation, routing readback and
//!   typed error mapping.
//! - [`switch::run_switch_conformance`] drives a full owner switch through the
//!   coordinator, including the pre-reservation rollback and the
//!   `RecoveryRequired` path.
//!
//! [`EngineBackendPorts`]: lurker_core_ports::engine_backend::EngineBackendPorts

pub mod case;
pub mod failure;
pub mod fake;
pub mod group;
mod readiness;
pub mod runner;
pub mod switch;

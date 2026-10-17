//! Product composition surface for the sing-box backend.
//!
//! The engine itself (control API client, readiness probe, outbound/routing
//! adapters, process supervision, configuration codec and version probe) lives
//! in the neutral `singbox-driver` crate. This crate keeps only the product
//! capability declaration: the validated per-operation evidence that the
//! product composition is allowed to advertise for sing-box.
pub mod capability_provider;

use lurker_core_contract::identity::EngineId;

pub fn engine_id() -> EngineId {
    EngineId::sing_box()
}

//! Independent sing-box process and native JSON adapters.
//!
//! The crate exposes the engine-owned process, readiness, and document
//! boundaries. Application ownership, scope fencing, and VPN state remain
//! outside this crate.

pub mod clash_api;
pub mod clash_runtime;
pub mod command;
pub mod config;
pub mod http;
pub mod process;
pub mod profile_configuration_transaction;
pub mod profile_document_codec;
pub mod readiness;
pub mod version;

pub fn engine_id() -> infiltrator_contract::core_identity::EngineId {
    infiltrator_contract::core_identity::EngineId::new("sing-box")
        .expect("sing-box engine identifier is valid")
}

#[cfg(test)]
mod clash_runtime_tests;

#[cfg(test)]
mod tests;

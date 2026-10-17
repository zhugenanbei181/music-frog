#![allow(clippy::question_mark)]
pub mod capability_provider;
pub mod clash_api;
pub mod clash_runtime;
pub mod controller_configuration;
pub mod http;
pub mod readiness;

use lurker_core_contract::identity::EngineId;

pub fn engine_id() -> EngineId {
    EngineId::sing_box()
}

#[cfg(test)]
mod clash_runtime_tests;

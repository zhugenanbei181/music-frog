pub mod capability_provider;
pub mod clash_api;
pub mod clash_runtime;
pub mod controller_configuration;
pub mod http;
pub mod readiness;

pub fn engine_id() -> infiltrator_contract::core_identity::EngineId {
    infiltrator_contract::core_identity::EngineId::sing_box()
}

#[cfg(test)]
mod clash_runtime_tests;

use infiltrator_contract::core_identity::EngineId;

pub mod endpoint;
pub mod manager;
pub mod port;
pub mod profile;
pub mod profile_configuration_codec;
pub mod profile_document_codec;
pub mod profile_option_store;
pub mod profile_options_codec;
pub mod profile_protocol_codec;
mod profile_workspace_store;
pub mod subscription;
pub mod yaml;
pub mod yaml_ast;

mod apply_observations;

/// Engine identity authored by this adapter crate.
pub fn engine_id() -> EngineId {
    EngineId::new("mihomo").expect("the Mihomo engine identifier is valid")
}

#[cfg(test)]
#[path = "profile_document_codec_tests.rs"]
mod profile_document_codec_tests;

#[cfg(test)]
#[path = "profile_store_owner_tests.rs"]
mod profile_store_owner_tests;

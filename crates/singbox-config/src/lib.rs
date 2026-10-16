#![allow(clippy::useless_conversion)]
use infiltrator_contract::core_identity::EngineId;

pub mod config;
pub mod profile_configuration_transaction;
pub mod profile_document_codec;

pub fn engine_id() -> EngineId {
    EngineId::sing_box()
}

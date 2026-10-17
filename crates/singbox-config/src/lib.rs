#![allow(clippy::useless_conversion)]
use lurker_core_contract::identity::EngineId;

pub mod config;
pub mod profile_configuration_transaction;
pub mod profile_document_codec;

pub fn engine_id() -> EngineId {
    EngineId::sing_box()
}

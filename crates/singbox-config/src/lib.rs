pub mod config;
pub mod profile_configuration_transaction;
pub mod profile_document_codec;

pub fn engine_id() -> infiltrator_contract::core_identity::EngineId {
    infiltrator_contract::core_identity::EngineId::sing_box()
}

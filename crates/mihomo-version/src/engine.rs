use infiltrator_contract::core_identity::EngineId;

pub fn engine_id() -> EngineId {
    EngineId::new("mihomo").expect("Mihomo engine ID is valid")
}

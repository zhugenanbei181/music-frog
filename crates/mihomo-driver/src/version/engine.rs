use lurker_core_contract::identity::EngineId;

pub fn engine_id() -> EngineId {
    EngineId::new("mihomo").expect("Mihomo engine ID is valid")
}

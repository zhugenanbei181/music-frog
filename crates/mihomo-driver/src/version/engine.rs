use lurker_core_contract::identity::EngineId;

/// Returns the engine identity of the Mihomo driver.
pub fn engine_id() -> EngineId {
    EngineId::new("mihomo").expect("Mihomo engine ID is valid")
}

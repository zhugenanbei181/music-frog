//! Dispatches capability declarations by the selected engine identity.

use infiltrator_contract::core_capabilities::CoreCapabilityMatrix;
use infiltrator_ports::core_capability_provider::{
    CoreCapabilityProvider, CoreCapabilityProviderError,
};
use infiltrator_ports::error::PortError;
use lurker_core_contract::identity::{CoreVersion, EngineId};
use std::collections::HashMap;
use std::sync::Arc;

pub struct EngineCoreCapabilityProviderRegistry {
    providers: HashMap<EngineId, Arc<dyn CoreCapabilityProvider>>,
}

impl EngineCoreCapabilityProviderRegistry {
    pub fn new(
        providers: impl IntoIterator<Item = (EngineId, Arc<dyn CoreCapabilityProvider>)>,
    ) -> Result<Self, PortError> {
        let mut by_engine = HashMap::new();
        for (engine_id, provider) in providers {
            if by_engine.insert(engine_id.clone(), provider).is_some() {
                return Err(PortError::Failed(format!(
                    "duplicate Core capability provider for engine {}",
                    engine_id.as_str()
                )));
            }
        }
        Ok(Self {
            providers: by_engine,
        })
    }
}

impl CoreCapabilityProvider for EngineCoreCapabilityProviderRegistry {
    fn matrix_for(
        &self,
        engine_id: &EngineId,
        core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        let provider = self.providers.get(engine_id).ok_or_else(|| {
            CoreCapabilityProviderError::EngineUnavailable {
                engine_id: engine_id.clone(),
                reason: "no Core capability provider is registered for this engine".to_owned(),
            }
        })?;
        let matrix = provider.matrix_for(engine_id, core_version)?;
        if matrix.scope.engine_id != *engine_id {
            return Err(CoreCapabilityProviderError::MatrixUnavailable {
                reason: format!(
                    "capability provider registered for {} returned a matrix for {}",
                    engine_id.as_str(),
                    matrix.scope.engine_id.as_str()
                ),
            });
        }
        Ok(matrix)
    }
}

#[cfg(test)]
#[path = "engine_capability_provider_registry_tests.rs"]
mod tests;

//! Engine-keyed backend provider dispatch.
//!
//! A request for an unregistered engine is rejected before any provider runs,
//! so a provider's adapter construction and side effects stay strictly behind
//! a successful engine selection.

use crate::error::EngineBackendRegistryError;
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::engine_backend::{
    ENGINE_BACKEND_CAPABILITY, EngineBackendBundle, EngineBackendProvider,
    EngineBackendProviderAvailability,
};
use lurker_core_ports::error::PortError;
use std::collections::HashMap;
use std::sync::Arc;

/// Dispatches each engine identity to its registered backend provider.
///
/// Providers are keyed by the engine they declare. Creating a backend cannot
/// change the registration set; mutation happens only through `register` and
/// `unregister`.
pub struct EngineBackendProviderRegistry<B: EngineBackendBundle> {
    providers: HashMap<EngineId, Arc<dyn EngineBackendProvider<B>>>,
}

impl<B: EngineBackendBundle> EngineBackendProviderRegistry<B> {
    /// Build a registry from a provider set, rejecting duplicate engines.
    pub fn new(
        providers: impl IntoIterator<Item = Arc<dyn EngineBackendProvider<B>>>,
    ) -> Result<Self, EngineBackendRegistryError> {
        let mut registry = Self::empty();
        for provider in providers {
            registry.register(provider)?;
        }
        Ok(registry)
    }

    /// A registry that supports no engine until providers are registered.
    pub fn empty() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Register one provider for the engine it declares.
    pub fn register(
        &mut self,
        provider: Arc<dyn EngineBackendProvider<B>>,
    ) -> Result<(), EngineBackendRegistryError> {
        let engine_id = provider.engine_id();
        if self.providers.contains_key(&engine_id) {
            return Err(EngineBackendRegistryError::DuplicateProvider { engine_id });
        }
        self.providers.insert(engine_id, provider);
        Ok(())
    }

    /// Remove and return the provider registered for `engine_id`.
    pub fn unregister(
        &mut self,
        engine_id: &EngineId,
    ) -> Result<Arc<dyn EngineBackendProvider<B>>, EngineBackendRegistryError> {
        self.providers.remove(engine_id).ok_or_else(|| {
            EngineBackendRegistryError::UnknownProvider {
                engine_id: engine_id.clone(),
            }
        })
    }

    /// Inspect registration without invoking a provider. Switch coordinators
    /// use this to reject missing engines before terminating the current Core.
    pub fn availability(&self, engine_id: &EngineId) -> EngineBackendProviderAvailability {
        match self.providers.contains_key(engine_id) {
            true => EngineBackendProviderAvailability::Registered,
            false => EngineBackendProviderAvailability::Unsupported {
                reason: unregistered_reason(engine_id),
            },
        }
    }

    /// Create the backend bundle for `engine_id`.
    ///
    /// An unregistered engine is a typed `Unsupported` failure and invokes
    /// zero providers. A provider that returns a bundle for another engine is
    /// rejected instead of being accepted under the requested identity.
    pub fn backend(&self, engine_id: &EngineId) -> Result<B, PortError> {
        let provider = self.providers.get(engine_id).ok_or_else(|| {
            PortError::unsupported(ENGINE_BACKEND_CAPABILITY, unregistered_reason(engine_id))
        })?;
        let bundle = provider.create_backend()?;
        if bundle.engine_id() != engine_id {
            return Err(PortError::Failed(format!(
                "engine backend provider for {} returned a bundle for {}",
                engine_id.as_str(),
                bundle.engine_id().as_str()
            )));
        }
        Ok(bundle)
    }
}

fn unregistered_reason(engine_id: &EngineId) -> String {
    format!(
        "no engine backend provider is registered for {}",
        engine_id.as_str()
    )
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;

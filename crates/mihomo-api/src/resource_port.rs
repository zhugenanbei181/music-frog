//! Owner-scoped Mihomo memory/GC adapter with verified post-GC readback.

use crate::runtime_configuration::RuntimeScopeResolver;
use infiltrator_contract::resources::ResourceAvailability;
use infiltrator_ports::resources::{
    CoreResourcePort, CpuResourceObserver, GarbageCollectionOutcome, ScopedGarbageCollection,
    ScopedResourceObservation,
};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use lurker_core_ports::error::PortError;
use mihomo_driver::api::client::MihomoClient;
use mihomo_driver::error::MihomoError;
use mihomo_driver::network_error::network_error;
use std::sync::Arc;

#[derive(Clone)]
pub struct MihomoRuntimeResourcePort {
    client: MihomoClient,
    scope_resolver: Option<RuntimeScopeResolver>,
    cpu_observer: Option<Arc<dyn CpuResourceObserver>>,
}

impl MihomoRuntimeResourcePort {
    pub fn new(client: MihomoClient, scope_resolver: Option<RuntimeScopeResolver>) -> Self {
        Self {
            client,
            scope_resolver,
            cpu_observer: None,
        }
    }

    pub fn with_cpu_observer(mut self, observer: Arc<dyn CpuResourceObserver>) -> Self {
        self.cpu_observer = Some(observer);
        self
    }

    fn validate_scope(&self, requested: &CoreScope) -> Result<(), PortError> {
        if requested.identity.engine.as_str() != "mihomo" {
            return Err(PortError::unsupported(
                Capability::CoreLifecycle,
                format!(
                    "Mihomo resources cannot serve engine {}",
                    requested.identity.engine.as_str()
                ),
            ));
        }
        let resolver = self.scope_resolver.as_ref().ok_or_else(|| {
            PortError::unsupported(
                Capability::CoreLifecycle,
                "Mihomo resource access has no Core scope resolver",
            )
        })?;
        match resolver()? {
            active if active == *requested => Ok(()),
            _ => Err(stale_scope()),
        }
    }
}

#[async_trait::async_trait]
impl CoreResourcePort for MihomoRuntimeResourcePort {
    async fn observe(&self, scope: CoreScope) -> Result<ScopedResourceObservation, PortError> {
        self.validate_scope(&scope)?;
        let memory_bytes = match self.client.get_memory().await {
            Ok(memory) => ResourceAvailability::Available(memory.in_use),
            Err(error) => ResourceAvailability::Failed(Failure::from(network_error(error))),
        };
        let cpu_percent = match &self.cpu_observer {
            Some(observer) => observer.observe_cpu_percent().await,
            None => ResourceAvailability::Unsupported,
        };
        self.validate_scope(&scope)?;
        Ok(ScopedResourceObservation {
            scope,
            memory_bytes,
            cpu_percent,
        })
    }

    async fn collect_garbage(
        &self,
        scope: CoreScope,
        before_bytes: u64,
    ) -> Result<ScopedGarbageCollection, PortError> {
        self.validate_scope(&scope)?;
        let outcome = match self.client.trigger_gc().await {
            Ok(()) => {
                self.validate_scope(&scope)?;
                match self.client.get_memory().await {
                    Ok(memory) => GarbageCollectionOutcome::Collected {
                        before_bytes,
                        after_bytes: memory.in_use,
                    },
                    Err(error) => {
                        GarbageCollectionOutcome::Failed(Failure::from(network_error(error)))
                    }
                }
            }
            Err(MihomoError::Http(error))
                if error.status().is_some_and(|status| status.as_u16() == 404) =>
            {
                GarbageCollectionOutcome::Unsupported
            }
            Err(error) => GarbageCollectionOutcome::Failed(Failure::from(network_error(error))),
        };
        self.validate_scope(&scope)?;
        Ok(ScopedGarbageCollection { scope, outcome })
    }
}

fn stale_scope() -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::InvalidState,
        "Mihomo resource request belongs to a retired Core scope",
        false,
    ))
}

#[cfg(test)]
#[path = "resource_port_tests.rs"]
mod tests;

//! DUAL-07-09: the narrow host seam that hands reloaded configuration to a
//! running core.
//!
//! The shared subscription refresh consults this port after a successful
//! update of the active profile. A host without one keeps the capability
//! explicitly unsupported, so the refresh reports a typed
//! `CoreReloadOutcome::Unsupported` instead of pretending the reload happened.

use crate::error::PortError;
use crate::runtime_gateway::ManagedRuntime;
use async_trait::async_trait;
use infiltrator_domain::apply::ApplyStrategy;
use std::sync::Arc;

/// Host capability that applies the current profile to the running core.
///
/// The reload goes through the existing managed-runtime apply transaction
/// (atomic write, reload-or-restart, readiness, rollback); this port only
/// narrows the surface to the single reload use-case the refresh needs.
#[async_trait]
pub trait CoreReloadPort: Send + Sync {
    /// Apply the current profile to the running core, preferring a hot reload
    /// over a process restart.
    async fn reload_active_profile(&self) -> Result<(), PortError>;
}

#[async_trait]
impl<T: ManagedRuntime + ?Sized> CoreReloadPort for Arc<T> {
    async fn reload_active_profile(&self) -> Result<(), PortError> {
        self.as_ref()
            .apply_current_config(ApplyStrategy::PreferReload)
            .await
            .map(|_| ())
    }
}

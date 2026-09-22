//! Host OS resolver cache refresh capability.
//!
//! The Fake-IP table lives inside the controller; the OS resolver cache does
//! not. Hosts that can drive their platform's resolver cache flush implement
//! this port; hosts that cannot must answer `Ok(false)` — a typed unsupported
//! that the application reports honestly instead of fabricating a refresh.

use crate::error::PortError;

#[async_trait::async_trait]
pub trait SystemDnsCachePort: Send + Sync {
    /// Refresh the operating system's DNS resolver cache.
    ///
    /// * `Ok(true)` — the platform flush ran and succeeded;
    /// * `Ok(false)` — this host has no drivable resolver-cache flush;
    /// * `Err(..)` — a flush was attempted and failed.
    async fn flush_system_cache(&self) -> Result<bool, PortError>;
}

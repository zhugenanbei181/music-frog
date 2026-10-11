//! Host OS resolver cache refresh capability.
//!
//! The Fake-IP table lives inside the controller; the OS resolver cache does
//! not. Hosts that can drive their platform's resolver cache flush implement
//! this port; hosts that cannot return the explicit `Unsupported` variant,
//! which the application reports honestly instead of fabricating a refresh.

use crate::error::PortError;

/// One system DNS-cache flush capability outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemDnsCacheFlush {
    Flushed,
    Unsupported,
}

#[async_trait::async_trait]
pub trait SystemDnsCachePort: Send + Sync {
    /// Refresh the operating system's DNS resolver cache.
    ///
    /// * `Ok(SystemDnsCacheFlush::Flushed)` — the platform flush ran and succeeded;
    /// * `Ok(SystemDnsCacheFlush::Unsupported)` — this host has no drivable resolver-cache flush;
    /// * `Err(..)` — a flush was attempted and failed.
    async fn flush_system_cache(&self) -> Result<SystemDnsCacheFlush, PortError>;
}

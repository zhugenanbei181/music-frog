//! Typed control for the running core's log verbosity.

use async_trait::async_trait;
use infiltrator_contract::command::CoreLogLevel;
use lurker_core_ports::error::PortError;

/// Writes a core log level and returns the adapter's typed readback.
///
/// Adapters must verify the live value after writing. Engine-specific wire
/// keys and configuration representations stay behind this port.
#[async_trait]
pub trait CoreLogLevelPort: Send + Sync {
    async fn set_core_log_level(&self, level: CoreLogLevel) -> Result<CoreLogLevel, PortError>;
}

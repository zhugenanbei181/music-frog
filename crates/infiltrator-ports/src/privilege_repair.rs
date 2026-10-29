//! Outbound port definition for privilege inspection and conflict repair (FLPARITY-034).

use async_trait::async_trait;
use infiltrator_contract::privilege_repair::{PrivilegeElevationAction, PrivilegeRepairSnapshot};
use lurker_core_ports::error::PortError;

/// Port abstraction for host environment privilege inspection, watchdog checks, and automated repairs.
#[async_trait]
pub trait PrivilegeRepairPort: Send + Sync {
    /// Inspect current host environment for missing TUN permissions, port collisions, or proxy tampering.
    async fn inspect_environment(&self) -> Result<PrivilegeRepairSnapshot, PortError>;

    /// Execute an automated repair or elevation action (e.g. pkexec setcap, kill conflicting process, restore proxy).
    async fn execute_repair(
        &self,
        action: PrivilegeElevationAction,
    ) -> Result<PrivilegeRepairSnapshot, PortError>;
}

//! The closed-connection-history command, on the shared owner.

use super::CommandApplication;
use lurker_core_contract::error::{ErrorCode, Failure};

impl CommandApplication {
    /// Clear the retained rows and confirm the owner's readback settled.
    pub(super) fn clear_closed_connections(&self) -> Result<(), Failure> {
        let Some(owner) = self.closed_connections.as_ref() else {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "clear closed connections has no composed closed-history owner",
                true,
            ));
        };
        let readback = owner.clear();
        if readback.total_closed == 0 && readback.records.is_empty() {
            Ok(())
        } else {
            Err(Failure::new(
                ErrorCode::InvalidState,
                "closed connection history readback did not settle after clear",
                false,
            ))
        }
    }
}

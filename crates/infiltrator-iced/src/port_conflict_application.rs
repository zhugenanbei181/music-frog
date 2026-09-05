//! Iced host access to the shared port-conflict application.

use infiltrator_application::port_conflict_application::PortConflictApplication;
use infiltrator_contract::error::InfiltratorError;

pub fn application() -> Result<PortConflictApplication, InfiltratorError> {
    let port = crate::host::storage::port_conflict()
        .map_err(|error| InfiltratorError::Internal(error.to_string()))?;
    Ok(PortConflictApplication::new(std::sync::Arc::new(port)))
}

//! Iced host access to the shared port-conflict application.

use crate::host::storage::port_conflict;
use infiltrator_application::port_conflict_application::PortConflictApplication;
use infiltrator_contract::error::InfiltratorError;
use std::sync::Arc;

pub fn application() -> Result<PortConflictApplication, InfiltratorError> {
    let port = port_conflict().map_err(|error| InfiltratorError::Internal(error.to_string()))?;
    Ok(PortConflictApplication::new(Arc::new(port)))
}

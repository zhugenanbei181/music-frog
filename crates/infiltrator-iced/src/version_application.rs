//! Iced host access to the shared core-version application.

use crate::host::storage::version;
use infiltrator_application::version_application::VersionApplication;
use infiltrator_contract::error::InfiltratorError;
use std::sync::Arc;

pub fn application() -> Result<VersionApplication, InfiltratorError> {
    let port = version().map_err(|error| InfiltratorError::Download(error.to_string()))?;
    Ok(VersionApplication::new(Arc::new(port)))
}

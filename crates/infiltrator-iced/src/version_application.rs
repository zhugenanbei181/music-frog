//! Iced host access to the shared core-version application.

use infiltrator_application::version_application::VersionApplication;
use infiltrator_contract::error::InfiltratorError;

pub fn application() -> Result<VersionApplication, InfiltratorError> {
    let port = infiltrator_desktop::storage::version()
        .map_err(|error| InfiltratorError::Download(error.to_string()))?;
    Ok(VersionApplication::new(std::sync::Arc::new(port)))
}

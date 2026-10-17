//! Safe engine-scoped profile document paths.
//!
//! Every path this module returns is composed from the injected
//! [`crate::layout::ProfileStoreLayout`]; the kernel owns no path literal of its
//! own.

use crate::store::ProfileStoreCore;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::profile_document_identity::ProfileId;
use lurker_core_contract::profile_name::validate_profile_name;
use lurker_core_ports::error::PortError;
use std::io::ErrorKind;
use std::path::PathBuf;
use tokio::fs;

impl ProfileStoreCore {
    /// Resolve a validated profile ID to a path contained by this engine's
    /// canonical document directory.
    pub async fn profile_document_path(&self, profile: &ProfileId) -> Result<PathBuf, PortError> {
        validate_document_profile_id(profile)?;
        fs::create_dir_all(self.config_dir())
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let base = fs::canonicalize(self.config_dir())
            .await
            .map_err(|error| PortError::Io(format!("config dir unavailable: {error}")))?;
        let path = self.layout().profile_document_path(
            &base,
            profile.as_str(),
            self.document_extension().as_str(),
        );
        if !path.starts_with(&base) {
            return Err(invalid_profile_path("profile path escapes config dir"));
        }
        Ok(path)
    }

    /// Resolve an existing document and reject symbolic links that escape the
    /// engine namespace.
    pub async fn existing_profile_document_path(
        &self,
        profile: &ProfileId,
    ) -> Result<PathBuf, PortError> {
        validate_document_profile_id(profile)?;
        let base = match fs::canonicalize(self.config_dir()).await {
            Ok(base) => base,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Err(PortError::NotFound(format!(
                    "Profile '{}' not found",
                    profile.as_str()
                )));
            }
            Err(error) => return Err(PortError::Io(error.to_string())),
        };
        let path = self.profile_document_path(profile).await?;
        let canonical = match fs::canonicalize(&path).await {
            Ok(path) => path,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Err(PortError::NotFound(format!(
                    "Profile '{}' not found",
                    profile.as_str()
                )));
            }
            Err(error) => return Err(PortError::Io(error.to_string())),
        };
        if !canonical.starts_with(&base) {
            return Err(invalid_profile_path("profile path escapes config dir"));
        }
        Ok(canonical)
    }
}

/// Reject profile IDs that cannot be a single relative path component before
/// any filesystem access happens.
pub fn validate_document_profile_id(profile: &ProfileId) -> Result<(), PortError> {
    validate_profile_name(profile.as_str())
        .map_err(|error| invalid_profile_path(format!("Invalid profile name: {error}")))
}

fn invalid_profile_path(message: impl Into<String>) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::InvalidInput, message, false))
}

#[cfg(test)]
#[path = "document_path_tests.rs"]
mod tests;

//! Mihomo namespace initialization before engine-neutral workspace reads.

use crate::manager::ConfigManager;
#[cfg(test)]
use infiltrator_contract::capability::Capability;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_workspace::ProfileWorkspace;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::error::MihomoError;
use std::io::ErrorKind;

impl<S: SecureStore> ConfigManager<S> {
    pub(crate) async fn read_workspace(
        &self,
        profile: &ProfileId,
    ) -> Result<ProfileWorkspace, PortError> {
        self.initialize_profile_namespace()
            .await
            .map_err(config_error)?;
        self.profile_store.read_profile_workspace(profile).await
    }
}

pub(crate) fn config_error(error: MihomoError) -> PortError {
    match error {
        MihomoError::Io(error) if error.kind() == ErrorKind::PermissionDenied => {
            PortError::PermissionDenied(error.to_string())
        }
        MihomoError::Io(error) => PortError::Io(error.to_string()),
        MihomoError::NotFound(reason) => PortError::NotFound(reason),
        other => PortError::Rejected(Failure::new(
            ErrorCode::Configuration,
            other.to_string(),
            false,
        )),
    }
}

#[cfg(test)]
#[path = "profile_workspace_store_tests.rs"]
mod tests;

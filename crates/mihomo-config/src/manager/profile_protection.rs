//! Protection comes from persisted provider ownership, even when credentials are unavailable.
use super::ConfigManager;
use super::paths::sanitized_profile_key;
use crate::profile_workspace_store::config_error;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use toml::Value;

impl<S: SecureStore> ConfigManager<S> {
    /// Caller owns the profile write boundary; no credential read or fallback can remove protection.
    pub(crate) async fn workspace_protection(
        &self,
        profile: &str,
    ) -> Result<ProfileWriteProtection, PortError> {
        let key = sanitized_profile_key(profile).map_err(config_error)?;
        let settings = self.read_settings_value().await.map_err(config_error)?;
        let Some(profiles) = settings.get("profiles") else {
            return Ok(ProfileWriteProtection::Editable);
        };
        let Some(metadata) = profiles.as_table().ok_or_else(invalid_metadata)?.get(&key) else {
            return Ok(ProfileWriteProtection::Editable);
        };
        let metadata = metadata.as_table().ok_or_else(invalid_metadata)?;
        for field in ["subscription_url", "subscription_url_key"] {
            if let Some(value) = metadata.get(field) {
                let value = Value::as_str(value).ok_or_else(invalid_metadata)?;
                if !value.trim().is_empty() {
                    return Ok(ProfileWriteProtection::RemoteSubscription);
                }
            }
        }
        Ok(ProfileWriteProtection::Editable)
    }
}
fn invalid_metadata() -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::Storage,
        "Invalid persisted profile ownership metadata",
        false,
    ))
}

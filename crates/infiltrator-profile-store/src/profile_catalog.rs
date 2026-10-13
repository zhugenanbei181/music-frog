//! Engine-scoped profile enumeration and neutral catalog projection.

use crate::metadata::{default_profile_metadata, read_profile_metadata};
use crate::store::EngineProfileStore;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceState;
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_domain::profiles::{ProfileInfo, validate_profile_name};
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use tokio::fs;

impl<S: SecureStore> EngineProfileStore<S> {
    /// Enumerate engine-owned profile files and project metadata while the
    /// caller holds the active-reference selection lease and profile-write
    /// lock.
    pub async fn list_profile_info_locked(
        &self,
        selection: &ActiveProfileReferenceState,
    ) -> Result<Vec<ProfileInfo>, PortError> {
        if !self.config_dir().exists() {
            return Ok(Vec::new());
        }
        let settings = self.read_settings_value().await.ok();
        let metadata_table = settings
            .as_ref()
            .and_then(|value| value.get("profiles"))
            .and_then(toml::Value::as_table);
        let engine = self.engine_id();
        let mut profiles = Vec::new();
        let mut entries = fs::read_dir(self.config_dir())
            .await
            .map_err(map_io_error)?;
        while let Some(entry) = entries.next_entry().await.map_err(map_io_error)? {
            let path = entry.path();
            if path.extension().and_then(|suffix| suffix.to_str())
                != Some(self.document_extension().as_str())
            {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("")
                .to_owned();
            let active = match selection {
                ActiveProfileReferenceState::Unselected { .. } => false,
                ActiveProfileReferenceState::Selected(reference) => {
                    reference.engine_id == engine && reference.profile_id.as_str() == name
                }
            };
            let mut info = ProfileInfo {
                name: name.clone(),
                active,
                path: path.to_string_lossy().to_string(),
                ..ProfileInfo::default()
            };
            let metadata = match metadata_table
                .and_then(|table| table.get(&name))
                .and_then(toml::Value::as_table)
            {
                Some(table) => {
                    read_profile_metadata(self.credential_store(), &engine, &name, table).await
                }
                None => default_profile_metadata(),
            };
            info.subscription_url = metadata.subscription_url;
            info.auto_update_enabled = metadata.auto_update_enabled;
            info.update_interval_hours = metadata.update_interval_hours;
            info.last_updated = metadata.last_updated;
            info.next_update = metadata.next_update;
            info.traffic_upload = metadata.traffic_upload;
            info.traffic_download = metadata.traffic_download;
            info.traffic_total = metadata.traffic_total;
            info.expire_at = metadata.expire_at;
            info.user_agent = metadata.user_agent;
            info.etag = metadata.etag;
            info.last_modified = metadata.last_modified;
            info.cron_expression = metadata.cron_expression;
            info.insecure_skip_verify = metadata.insecure_skip_verify;
            info.auto_reload_core = metadata.auto_reload_core;
            info.has_backup = if validate_profile_name(&name).is_ok() {
                match ProfileId::new(name) {
                    Ok(profile_id) => self
                        .profile_document_has_backup(&profile_id)
                        .await
                        .unwrap_or(false),
                    Err(_) => false,
                }
            } else {
                false
            };
            profiles.push(info);
        }
        profiles.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(profiles)
    }
}

fn map_io_error(error: std::io::Error) -> PortError {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        PortError::PermissionDenied(error.to_string())
    } else {
        PortError::Io(error.to_string())
    }
}

#[cfg(test)]
#[path = "profile_catalog_tests.rs"]
mod tests;

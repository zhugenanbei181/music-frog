//! Engine-neutral helpers for the durable profile-settings document.
//!
//! The document is plain TOML owned by the store; mapping its tables into a
//! product metadata model (subscription URL, credentials, traffic counters)
//! stays in the product crate. All filesystem access goes through the store's
//! [`StorePlatform`].

use crate::document_store::{
    read_optional_with_platform, write_document_bytes_durable_with_platform,
};
use crate::platform::{StorePlatform, native_platform, run_platform_io};
use lurker_core_ports::error::PortError;
use std::path::Path;
use std::sync::Arc;
use toml::map::Map;

/// Return the value as a table, replacing a non-table node with an empty one.
pub fn ensure_table(value: &mut toml::Value) -> Result<&mut Map<String, toml::Value>, PortError> {
    if !matches!(value, toml::Value::Table(_)) {
        *value = toml::Value::Table(Map::new());
    }
    match value {
        toml::Value::Table(table) => Ok(table),
        _ => Err(PortError::Io(
            "profile settings table is invalid".to_owned(),
        )),
    }
}

/// Read the settings document; a missing file is an empty table.
pub async fn read_settings_file(path: &Path) -> Result<toml::Value, PortError> {
    let platform = native_platform();
    read_settings_file_with_platform(&platform, path).await
}

/// [`read_settings_file`] through an injected platform.
pub async fn read_settings_file_with_platform(
    platform: &Arc<dyn StorePlatform>,
    path: &Path,
) -> Result<toml::Value, PortError> {
    let Some(content) = read_optional_with_platform(platform, path).await? else {
        return Ok(toml::Value::Table(Map::new()));
    };
    toml::from_str(&content)
        .map_err(|error| PortError::Io(format!("invalid profile settings: {error}")))
}

/// Replace the settings document through the durable byte path.
pub async fn write_settings_file(path: &Path, settings: &toml::Value) -> Result<(), PortError> {
    let platform = native_platform();
    write_settings_file_with_platform(&platform, path, settings).await
}

/// [`write_settings_file`] through an injected platform.
pub async fn write_settings_file_with_platform(
    platform: &Arc<dyn StorePlatform>,
    path: &Path,
    settings: &toml::Value,
) -> Result<(), PortError> {
    let content = toml::to_string(settings)
        .map_err(|error| PortError::Io(format!("failed to serialize profile settings: {error}")))?;
    write_document_bytes_durable_with_platform(platform, path, content.as_bytes()).await
}

/// Drop one profile's row from the `[profiles]` table, keeping other rows.
pub async fn remove_profile_metadata(path: &Path, profile: &str) -> Result<(), PortError> {
    let platform = native_platform();
    remove_profile_metadata_with_platform(&platform, path, profile).await
}

/// [`remove_profile_metadata`] through an injected platform.
pub async fn remove_profile_metadata_with_platform(
    platform: &Arc<dyn StorePlatform>,
    path: &Path,
    profile: &str,
) -> Result<(), PortError> {
    let path_owned = path.to_path_buf();
    let exists = run_platform_io(platform, move |platform| platform.exists(&path_owned)).await?;
    if !exists {
        return Ok(());
    }
    let mut settings = read_settings_file_with_platform(platform, path).await?;
    if let toml::Value::Table(root) = &mut settings
        && let Some(toml::Value::Table(profiles)) = root.get_mut("profiles")
    {
        profiles.remove(profile);
    }
    write_settings_file_with_platform(platform, path, &settings).await
}

/// Interpret an optional TOML integer as an unsigned value.
pub fn parse_u64(value: Option<&toml::Value>) -> Option<u64> {
    value
        .and_then(toml::Value::as_integer)
        .and_then(|value| u64::try_from(value).ok())
}

pub fn set_optional_string(table: &mut Map<String, toml::Value>, key: &str, value: Option<String>) {
    match value {
        Some(value) => {
            table.insert(key.to_owned(), toml::Value::String(value));
        }
        None => {
            table.remove(key);
        }
    }
}

pub fn set_optional_u32(table: &mut Map<String, toml::Value>, key: &str, value: Option<u32>) {
    match value {
        Some(value) => {
            table.insert(key.to_owned(), toml::Value::Integer(value as i64));
        }
        None => {
            table.remove(key);
        }
    }
}

pub fn set_optional_u64(table: &mut Map<String, toml::Value>, key: &str, value: Option<u64>) {
    match value {
        Some(value) => {
            let value = i64::try_from(value).unwrap_or(i64::MAX);
            table.insert(key.to_owned(), toml::Value::Integer(value));
        }
        None => {
            table.remove(key);
        }
    }
}

pub fn set_optional_i64(table: &mut Map<String, toml::Value>, key: &str, value: Option<i64>) {
    match value {
        Some(value) => {
            table.insert(key.to_owned(), toml::Value::Integer(value));
        }
        None => {
            table.remove(key);
        }
    }
}

pub fn set_bool(table: &mut Map<String, toml::Value>, key: &str, value: bool) {
    table.insert(key.to_owned(), toml::Value::Boolean(value));
}

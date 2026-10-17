//! Engine-neutral helpers for the durable profile-settings document.
//!
//! The document is plain TOML owned by the store; mapping its tables into a
//! product metadata model (subscription URL, credentials, traffic counters)
//! stays in the product crate.

use crate::document_store::write_document_bytes_durable;
use lurker_core_ports::error::PortError;
use std::io::ErrorKind;
use std::path::Path;
use tokio::fs;
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
    let content = match fs::read_to_string(path).await {
        Ok(content) => content,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(toml::Value::Table(Map::new()));
        }
        Err(error) => return Err(PortError::Io(error.to_string())),
    };
    toml::from_str(&content)
        .map_err(|error| PortError::Io(format!("invalid profile settings: {error}")))
}

/// Replace the settings document through the durable byte path.
pub async fn write_settings_file(path: &Path, settings: &toml::Value) -> Result<(), PortError> {
    let content = toml::to_string(settings)
        .map_err(|error| PortError::Io(format!("failed to serialize profile settings: {error}")))?;
    write_document_bytes_durable(path, content.as_bytes()).await
}

/// Drop one profile's row from the `[profiles]` table, keeping other rows.
pub async fn remove_profile_metadata(path: &Path, profile: &str) -> Result<(), PortError> {
    if !fs::try_exists(path)
        .await
        .map_err(|error| PortError::Io(error.to_string()))?
    {
        return Ok(());
    }
    let mut settings = read_settings_file(path).await?;
    if let toml::Value::Table(root) = &mut settings
        && let Some(toml::Value::Table(profiles)) = root.get_mut("profiles")
    {
        profiles.remove(profile);
    }
    write_settings_file(path, &settings).await
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

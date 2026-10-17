//! Engine-scoped profile metadata and subscription-credential persistence.

use crate::document_store::write_document_bytes_durable;
use crate::store::EngineProfileStore;
use chrono::{DateTime, Utc};
use infiltrator_domain::profiles::ProfileMetadata;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;
use std::io::ErrorKind;
use std::path::Path;
use tokio::fs;
use toml::map::Map;

pub const SUBSCRIPTION_SERVICE: &str = "MusicFrog-Infiltrator";
const SUBSCRIPTION_KEY_PREFIX: &str = "subscription";

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

/// Decode one engine-scoped metadata row, restoring a legacy plaintext URL
/// into the secure store when the row predates credential indirection.
pub async fn read_profile_metadata<S: SecureStore>(
    credential_store: &S,
    engine: &EngineId,
    profile: &str,
    table: &Map<String, toml::Value>,
) -> ProfileMetadata {
    let fallback_url = table
        .get("subscription_url")
        .and_then(|value| value.as_str())
        .map(str::to_owned);
    let mut key = table
        .get("subscription_url_key")
        .and_then(|value| value.as_str())
        .map(str::to_owned);
    if key.is_none() && fallback_url.is_some() {
        key = Some(subscription_key(engine, profile));
    }
    let mut resolved = load_subscription_url(credential_store, profile, key.as_deref()).await;
    if resolved.is_none()
        && let Some(url) = fallback_url.as_ref()
    {
        if let Err(error) = store_subscription_url(credential_store, engine, profile, url).await {
            log::warn!("failed to restore subscription url to store: {error}");
        } else {
            resolved = Some(url.clone());
        }
    }

    ProfileMetadata {
        subscription_url: resolved.or(fallback_url),
        auto_update_enabled: table
            .get("auto_update_enabled")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false),
        update_interval_hours: table
            .get("update_interval_hours")
            .and_then(toml::Value::as_integer)
            .and_then(|value| u32::try_from(value).ok()),
        last_updated: parse_datetime(table.get("last_updated")),
        next_update: parse_datetime(table.get("next_update")),
        traffic_upload: parse_u64(table.get("traffic_upload")),
        traffic_download: parse_u64(table.get("traffic_download")),
        traffic_total: parse_u64(table.get("traffic_total")),
        expire_at: table
            .get("expire_at")
            .and_then(toml::Value::as_integer)
            .filter(|value| *value >= 0),
        user_agent: table
            .get("user_agent")
            .and_then(toml::Value::as_str)
            .map(str::to_owned),
        etag: table
            .get("etag")
            .and_then(toml::Value::as_str)
            .map(str::to_owned),
        last_modified: table
            .get("last_modified")
            .and_then(toml::Value::as_str)
            .map(str::to_owned),
        cron_expression: table
            .get("cron_expression")
            .and_then(toml::Value::as_str)
            .map(str::to_owned),
        insecure_skip_verify: table
            .get("insecure_skip_verify")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false),
        auto_reload_core: table
            .get("auto_reload_core")
            .and_then(toml::Value::as_bool)
            .unwrap_or(true),
    }
}

/// Persist metadata fields and secure-store the subscription URL where
/// possible. A plaintext fallback is retained only if the credential store
/// rejects the value, preserving the established recovery behavior.
pub async fn write_profile_metadata<S: SecureStore>(
    credential_store: &S,
    engine: &EngineId,
    profile: &str,
    metadata: &ProfileMetadata,
    table: &mut Map<String, toml::Value>,
) {
    let mut subscription_url_key = None;
    let mut subscription_url_fallback = metadata.subscription_url.clone();
    if let Some(url) = metadata.subscription_url.as_deref() {
        match store_subscription_url(credential_store, engine, profile, url).await {
            Ok(key) => {
                subscription_url_key = Some(key);
                subscription_url_fallback = None;
            }
            Err(error) => log::warn!("failed to store subscription url securely: {error}"),
        }
    } else if let Err(error) = delete_subscription_url(credential_store, engine, profile).await {
        log::warn!("failed to delete subscription url: {error}");
    }
    set_optional_string(table, "subscription_url_key", subscription_url_key);
    set_optional_string(table, "subscription_url", subscription_url_fallback);
    set_bool(table, "auto_update_enabled", metadata.auto_update_enabled);
    set_optional_u32(
        table,
        "update_interval_hours",
        metadata.update_interval_hours,
    );
    set_optional_datetime(table, "last_updated", metadata.last_updated);
    set_optional_datetime(table, "next_update", metadata.next_update);
    set_optional_u64(table, "traffic_upload", metadata.traffic_upload);
    set_optional_u64(table, "traffic_download", metadata.traffic_download);
    set_optional_u64(table, "traffic_total", metadata.traffic_total);
    set_optional_i64(table, "expire_at", metadata.expire_at);
    set_optional_string(table, "user_agent", metadata.user_agent.clone());
    set_optional_string(table, "etag", metadata.etag.clone());
    set_optional_string(table, "last_modified", metadata.last_modified.clone());
    set_optional_string(table, "cron_expression", metadata.cron_expression.clone());
    set_bool(table, "insecure_skip_verify", metadata.insecure_skip_verify);
    set_bool(table, "auto_reload_core", metadata.auto_reload_core);
}

pub async fn store_subscription_url<S: SecureStore>(
    credential_store: &S,
    engine: &EngineId,
    profile: &str,
    url: &str,
) -> Result<String, PortError> {
    let key = subscription_key(engine, profile);
    credential_store
        .set(SUBSCRIPTION_SERVICE, &key, url)
        .await
        .map_err(|error| PortError::Io(error.to_string()))?;
    Ok(key)
}

pub async fn load_subscription_url<S: SecureStore>(
    credential_store: &S,
    profile: &str,
    key: Option<&str>,
) -> Option<String> {
    let key = match key {
        Some(key) if !key.trim().is_empty() => key,
        _ => return None,
    };
    match credential_store.get(SUBSCRIPTION_SERVICE, key).await {
        Ok(value) => value,
        Err(error) => {
            log::warn!("subscription get failed for profile {profile}: {error}");
            None
        }
    }
}

pub async fn delete_subscription_url<S: SecureStore>(
    credential_store: &S,
    engine: &EngineId,
    profile: &str,
) -> Result<(), PortError> {
    let key = subscription_key(engine, profile);
    credential_store
        .delete(SUBSCRIPTION_SERVICE, &key)
        .await
        .map_err(|error| PortError::Io(error.to_string()))
}

pub fn subscription_key(engine: &EngineId, profile: &str) -> String {
    format!("{SUBSCRIPTION_KEY_PREFIX}:{}:{profile}", engine.as_str())
}

pub fn legacy_subscription_key(profile: &str) -> String {
    format!("{SUBSCRIPTION_KEY_PREFIX}:{profile}")
}

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

pub async fn write_settings_file(path: &Path, settings: &toml::Value) -> Result<(), PortError> {
    let content = toml::to_string(settings)
        .map_err(|error| PortError::Io(format!("failed to serialize profile settings: {error}")))?;
    write_document_bytes_durable(path, content.as_bytes()).await
}

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

#[cfg(test)]
#[path = "metadata_tests.rs"]
mod tests;

impl<S: SecureStore> EngineProfileStore<S> {
    pub async fn read_settings_value(&self) -> Result<toml::Value, PortError> {
        read_settings_file(self.settings_file()).await
    }

    pub async fn write_settings_value(&self, settings: &toml::Value) -> Result<(), PortError> {
        write_settings_file(self.settings_file(), settings).await
    }

    pub async fn remove_profile_metadata(&self, profile: &str) -> Result<(), PortError> {
        remove_profile_metadata(self.settings_file(), profile).await
    }

    pub async fn get_profile_metadata(&self, profile: &str) -> Result<ProfileMetadata, PortError> {
        let settings = self.read_settings_value().await?;
        let table = settings
            .get("profiles")
            .and_then(toml::Value::as_table)
            .and_then(|profiles| profiles.get(profile))
            .and_then(toml::Value::as_table);
        match table {
            Some(table) => Ok(read_profile_metadata(
                self.credential_store(),
                &self.engine_id(),
                profile,
                table,
            )
            .await),
            None => Ok(default_profile_metadata()),
        }
    }

    pub async fn update_profile_metadata(
        &self,
        profile: &str,
        metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        let mut settings = self.read_settings_value().await?;
        let root = ensure_table(&mut settings)?;
        let profiles = root
            .entry("profiles".to_owned())
            .or_insert_with(|| toml::Value::Table(Map::new()));
        let profiles = ensure_table(profiles)?;
        let profile_table = profiles
            .entry(profile.to_owned())
            .or_insert_with(|| toml::Value::Table(Map::new()));
        let profile_table = ensure_table(profile_table)?;
        write_profile_metadata(
            self.credential_store(),
            &self.engine_id(),
            profile,
            metadata,
            profile_table,
        )
        .await;
        self.write_settings_value(&settings).await
    }
}

pub(crate) fn default_profile_metadata() -> ProfileMetadata {
    ProfileMetadata {
        auto_reload_core: true,
        ..ProfileMetadata::default()
    }
}

fn parse_u64(value: Option<&toml::Value>) -> Option<u64> {
    value
        .and_then(toml::Value::as_integer)
        .and_then(|value| u64::try_from(value).ok())
}

fn parse_datetime(value: Option<&toml::Value>) -> Option<DateTime<Utc>> {
    value
        .and_then(toml::Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|parsed| parsed.with_timezone(&Utc))
}

fn set_optional_string(table: &mut Map<String, toml::Value>, key: &str, value: Option<String>) {
    match value {
        Some(value) => {
            table.insert(key.to_owned(), toml::Value::String(value));
        }
        None => {
            table.remove(key);
        }
    }
}

fn set_optional_u32(table: &mut Map<String, toml::Value>, key: &str, value: Option<u32>) {
    match value {
        Some(value) => {
            table.insert(key.to_owned(), toml::Value::Integer(value as i64));
        }
        None => {
            table.remove(key);
        }
    }
}

fn set_optional_u64(table: &mut Map<String, toml::Value>, key: &str, value: Option<u64>) {
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

fn set_optional_i64(table: &mut Map<String, toml::Value>, key: &str, value: Option<i64>) {
    match value {
        Some(value) => {
            table.insert(key.to_owned(), toml::Value::Integer(value));
        }
        None => {
            table.remove(key);
        }
    }
}

fn set_bool(table: &mut Map<String, toml::Value>, key: &str, value: bool) {
    table.insert(key.to_owned(), toml::Value::Boolean(value));
}

fn set_optional_datetime(
    table: &mut Map<String, toml::Value>,
    key: &str,
    value: Option<DateTime<Utc>>,
) {
    match value {
        Some(value) => {
            table.insert(key.to_owned(), toml::Value::String(value.to_rfc3339()));
        }
        None => {
            table.remove(key);
        }
    }
}

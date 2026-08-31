//! Mapping between the `profiles` table of the settings TOML and
//! [`Profile`] metadata fields, plus small typed TOML value setters.

use chrono::{DateTime, Utc};
use mihomo_api::error::{MihomoError, Result};
use mihomo_platform::traits::CredentialStore;

use super::subscription_store::{load_subscription_url, store_subscription_url, subscription_key};
use crate::profile::Profile;

pub(super) fn ensure_table(
    value: &mut toml::Value,
) -> Result<&mut toml::map::Map<String, toml::Value>> {
    if !matches!(value, toml::Value::Table(_)) {
        *value = toml::Value::Table(toml::map::Map::new());
    }
    match value {
        toml::Value::Table(table) => Ok(table),
        _ => Err(MihomoError::Config("Invalid settings table".to_string())),
    }
}

pub(super) async fn apply_profile_metadata<S: CredentialStore>(
    credential_store: &S,
    profile: &mut Profile,
    table: &toml::map::Map<String, toml::Value>,
) {
    let fallback_url = table
        .get("subscription_url")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    let mut key = table
        .get("subscription_url_key")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    if key.is_none() && fallback_url.is_some() {
        key = Some(subscription_key(&profile.name));
    }
    let mut resolved = load_subscription_url(credential_store, &profile.name, key.as_deref()).await;
    if resolved.is_none()
        && let Some(url) = fallback_url.as_ref()
    {
        if let Err(err) = store_subscription_url(credential_store, &profile.name, url).await {
            log::warn!("failed to restore subscription url to store: {err}");
        } else {
            resolved = Some(url.clone());
        }
    }
    profile.subscription_url = resolved.or(fallback_url);
    profile.auto_update_enabled = table
        .get("auto_update_enabled")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    profile.update_interval_hours = table
        .get("update_interval_hours")
        .and_then(|value| value.as_integer())
        .and_then(|value| {
            if value >= 0 && value <= u32::MAX as i64 {
                Some(value as u32)
            } else {
                None
            }
        });
    profile.last_updated = parse_datetime(table.get("last_updated"));
    profile.next_update = parse_datetime(table.get("next_update"));
    profile.traffic_upload = parse_u64(table.get("traffic_upload"));
    profile.traffic_download = parse_u64(table.get("traffic_download"));
    profile.traffic_total = parse_u64(table.get("traffic_total"));
    profile.expire_at = table
        .get("expire_at")
        .and_then(|value| value.as_integer())
        .filter(|value| *value >= 0);
}

fn parse_u64(value: Option<&toml::Value>) -> Option<u64> {
    value
        .and_then(|value| value.as_integer())
        .and_then(|value| if value >= 0 { Some(value as u64) } else { None })
}

fn parse_datetime(value: Option<&toml::Value>) -> Option<DateTime<Utc>> {
    value
        .and_then(|value| value.as_str())
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|parsed| parsed.with_timezone(&Utc))
}

pub(super) fn set_optional_string(
    table: &mut toml::map::Map<String, toml::Value>,
    key: &str,
    value: Option<String>,
) {
    match value {
        Some(value) => {
            table.insert(key.to_string(), toml::Value::String(value));
        }
        None => {
            table.remove(key);
        }
    }
}

pub(super) fn set_optional_u32(
    table: &mut toml::map::Map<String, toml::Value>,
    key: &str,
    value: Option<u32>,
) {
    match value {
        Some(value) => {
            table.insert(key.to_string(), toml::Value::Integer(value as i64));
        }
        None => {
            table.remove(key);
        }
    }
}

pub(super) fn set_optional_u64(
    table: &mut toml::map::Map<String, toml::Value>,
    key: &str,
    value: Option<u64>,
) {
    match value {
        Some(value) => {
            let value = i64::try_from(value).unwrap_or(i64::MAX);
            table.insert(key.to_string(), toml::Value::Integer(value));
        }
        None => {
            table.remove(key);
        }
    }
}

pub(super) fn set_optional_i64(
    table: &mut toml::map::Map<String, toml::Value>,
    key: &str,
    value: Option<i64>,
) {
    match value {
        Some(value) => {
            table.insert(key.to_string(), toml::Value::Integer(value));
        }
        None => {
            table.remove(key);
        }
    }
}

pub(super) fn set_bool(table: &mut toml::map::Map<String, toml::Value>, key: &str, value: bool) {
    table.insert(key.to_string(), toml::Value::Boolean(value));
}

pub(super) fn set_optional_datetime(
    table: &mut toml::map::Map<String, toml::Value>,
    key: &str,
    value: Option<DateTime<Utc>>,
) {
    match value {
        Some(value) => {
            table.insert(key.to_string(), toml::Value::String(value.to_rfc3339()));
        }
        None => {
            table.remove(key);
        }
    }
}

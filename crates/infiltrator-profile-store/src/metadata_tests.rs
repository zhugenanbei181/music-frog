use super::{read_profile_metadata, write_profile_metadata};
use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use infiltrator_contract::core_identity::EngineId;
use infiltrator_domain::profiles::ProfileMetadata;
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use toml::map::Map;

#[derive(Clone, Default)]
struct MemorySecureStore(Arc<Mutex<HashMap<String, String>>>);

#[async_trait]
impl SecureStore for MemorySecureStore {
    async fn get(&self, _namespace: &str, key: &str) -> Result<Option<String>, PortError> {
        Ok(self.0.lock().expect("memory store").get(key).cloned())
    }

    async fn set(&self, _namespace: &str, key: &str, value: &str) -> Result<(), PortError> {
        self.0
            .lock()
            .expect("memory store")
            .insert(key.to_owned(), value.to_owned());
        Ok(())
    }

    async fn delete(&self, _namespace: &str, key: &str) -> Result<(), PortError> {
        self.0.lock().expect("memory store").remove(key);
        Ok(())
    }
}

#[tokio::test]
async fn metadata_round_trip_keeps_subscription_url_out_of_settings() {
    let store = MemorySecureStore::default();
    let engine = EngineId::new("mihomo").unwrap();
    let metadata = ProfileMetadata {
        subscription_url: Some("https://secret.example/profile?token=abc".into()),
        auto_update_enabled: true,
        update_interval_hours: Some(12),
        last_updated: Some(Utc.with_ymd_and_hms(2026, 10, 12, 3, 4, 5).unwrap()),
        next_update: None,
        traffic_upload: Some(101),
        traffic_download: Some(202),
        traffic_total: Some(303),
        expire_at: Some(1_800_000_000),
        user_agent: Some("MusicFrog/1".into()),
        etag: Some("etag-1".into()),
        last_modified: Some("yesterday".into()),
        cron_expression: Some("0 */12 * * *".into()),
        insecure_skip_verify: true,
        auto_reload_core: false,
    };
    let mut row = Map::new();

    write_profile_metadata(&store, &engine, "home", &metadata, &mut row).await;

    assert!(!row.contains_key("subscription_url"));
    assert_eq!(
        row.get("subscription_url_key")
            .and_then(toml::Value::as_str),
        Some("subscription:mihomo:home")
    );
    assert_eq!(
        store
            .get("ignored", "subscription:mihomo:home")
            .await
            .unwrap()
            .as_deref(),
        metadata.subscription_url.as_deref()
    );
    assert_eq!(
        read_profile_metadata(&store, &engine, "home", &row).await,
        metadata
    );
}

#[tokio::test]
async fn plaintext_subscription_fallback_is_restored_into_secure_store() {
    let store = MemorySecureStore::default();
    let engine = EngineId::new("sing-box").unwrap();
    let row = toml::from_str::<toml::Value>(
        "subscription_url = 'https://legacy.example/sub'\nauto_reload_core = false\n",
    )
    .unwrap();

    let metadata =
        read_profile_metadata(&store, &engine, "profile", row.as_table().expect("table")).await;

    assert_eq!(
        metadata.subscription_url.as_deref(),
        Some("https://legacy.example/sub")
    );
    assert!(!metadata.auto_reload_core);
    assert_eq!(
        store
            .get("ignored", "subscription:sing-box:profile")
            .await
            .unwrap()
            .as_deref(),
        Some("https://legacy.example/sub")
    );
}

//! Credential-store persistence of per-profile subscription URLs.
//!
//! URLs live in the OS credential store keyed by
//! `subscription:<sanitized profile name>`; the settings TOML only keeps a
//! reference (`subscription_url_key`) plus a plaintext fallback
//! (`subscription_url`) for stores that cannot persist the value.

use mihomo_api::error::Result;
use mihomo_platform::traits::CredentialStore;

const SUBSCRIPTION_SERVICE: &str = "MusicFrog-Despicable-Infiltrator";
const SUBSCRIPTION_KEY_PREFIX: &str = "subscription";

pub(super) fn subscription_key(profile: &str) -> String {
    format!("{SUBSCRIPTION_KEY_PREFIX}:{profile}")
}

pub(super) async fn store_subscription_url<S: CredentialStore>(
    credential_store: &S,
    profile: &str,
    url: &str,
) -> Result<String> {
    let key = subscription_key(profile);
    credential_store
        .set(SUBSCRIPTION_SERVICE, &key, url)
        .await?;
    Ok(key)
}

pub(super) async fn load_subscription_url<S: CredentialStore>(
    credential_store: &S,
    profile: &str,
    key: Option<&str>,
) -> Option<String> {
    let key = match key {
        Some(key) if !key.trim().is_empty() => key.to_string(),
        _ => return None,
    };
    match credential_store.get(SUBSCRIPTION_SERVICE, &key).await {
        Ok(value) => value,
        Err(err) => {
            log::warn!("subscription get failed for profile {}: {err}", profile);
            None
        }
    }
}

pub(super) async fn delete_subscription_url<S: CredentialStore>(
    credential_store: &S,
    profile: &str,
) -> Result<()> {
    let key = subscription_key(profile);
    credential_store.delete(SUBSCRIPTION_SERVICE, &key).await?;
    Ok(())
}

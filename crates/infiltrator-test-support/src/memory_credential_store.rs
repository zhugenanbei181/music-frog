//! In-memory [`SecureStore`] for tests that must not touch the host keyring.
//!
//! Install it with `infiltrator_platform::credentials::set_credential_store_override`
//! whenever a test isolates the home directory but still exercises the profile
//! namespace migration (which moves subscription credentials through the secure
//! store). Headless CI runners have no keyring, so the default store fails there.

use async_trait::async_trait;
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

#[derive(Default)]
pub struct MemoryCredentialStore {
    entries: Mutex<HashMap<(String, String), String>>,
}

impl MemoryCredentialStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn entries(&self) -> MutexGuard<'_, HashMap<(String, String), String>> {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[async_trait]
impl SecureStore for MemoryCredentialStore {
    async fn get(&self, namespace: &str, key: &str) -> Result<Option<String>, PortError> {
        Ok(self
            .entries()
            .get(&(namespace.to_owned(), key.to_owned()))
            .cloned())
    }

    async fn set(&self, namespace: &str, key: &str, value: &str) -> Result<(), PortError> {
        self.entries()
            .insert((namespace.to_owned(), key.to_owned()), value.to_owned());
        Ok(())
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<(), PortError> {
        self.entries()
            .remove(&(namespace.to_owned(), key.to_owned()));
        Ok(())
    }
}

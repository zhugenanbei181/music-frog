//! Process-global override for the host credential store.
//!
//! Native products select [`crate::defaults::DefaultCredentialStore`] — the OS
//! keyring on desktop, the Android bridge on mobile. Tests that exercise the
//! profile namespace migration must never touch the host keyring: it does not
//! exist on a headless runner, so the migration would fail there while passing on
//! a developer machine. They install an in-memory store for the duration of the
//! test, mirroring [`crate::paths::set_home_dir_override`].

use infiltrator_ports::secure_store::SecureStore;
use std::sync::{Arc, LazyLock, RwLock};

static CREDENTIAL_STORE_OVERRIDE: LazyLock<RwLock<Option<Arc<dyn SecureStore>>>> =
    LazyLock::new(|| RwLock::new(None));

/// Install a process-wide credential store. Returns false if the lock is poisoned.
pub fn set_credential_store_override(store: Arc<dyn SecureStore>) -> bool {
    match CREDENTIAL_STORE_OVERRIDE.write() {
        Ok(mut guard) => {
            *guard = Some(store);
            true
        }
        Err(_) => false,
    }
}

/// Remove any installed override, restoring the platform default store.
pub fn clear_credential_store_override() {
    if let Ok(mut guard) = CREDENTIAL_STORE_OVERRIDE.write() {
        *guard = None;
    }
}

/// The installed override, if any. Host adapters consult this before their own store.
pub(crate) fn credential_store_override() -> Option<Arc<dyn SecureStore>> {
    CREDENTIAL_STORE_OVERRIDE
        .read()
        .ok()
        .and_then(|guard| guard.clone())
}

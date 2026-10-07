//! Shared helpers for offline handler tests. Only compiled under `cfg(test)`.

use mihomo_config::manager::paths::CONFIGS_DIR_ENV;
use mihomo_platform::TEST_LOCK;
use std::env::{remove_var, set_var, var};
use tokio::sync::MutexGuard;

/// `INFILTRATOR_CONFIGS_DIR` is process-global state read by every
/// ConfigManager construction; hold the shared test lock and pin a known
/// value for the duration of a test (same pattern as the mihomo-config path
/// tests). The previous value is restored on drop, while the lock is still
/// held.
pub(crate) struct EnvGuard {
    _lock: MutexGuard<'static, ()>,
    previous: Option<String>,
}

impl EnvGuard {
    pub(crate) async fn acquire() -> Self {
        let lock = TEST_LOCK.lock().await;
        let key = CONFIGS_DIR_ENV;
        let previous = var(key).ok();
        // SAFETY: single-threaded mutation of process env, serialized by
        // TEST_LOCK exactly like in mihomo-config's own tests.
        unsafe { remove_var(key) };
        Self {
            _lock: lock,
            previous,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        let key = CONFIGS_DIR_ENV;
        // SAFETY: see EnvGuard::acquire.
        unsafe {
            if let Some(value) = self.previous.take() {
                set_var(key, value);
            } else {
                remove_var(key);
            }
        }
    }
}

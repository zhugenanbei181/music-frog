//! test-intent: behavior
use async_trait::async_trait;
use infiltrator_domain::settings::AppSettings;
use infiltrator_ports::error::PortError;
use infiltrator_ports::settings_store::SettingsStore;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Default)]
pub struct ProbeSettingsStore {
    pub settings: Mutex<AppSettings>,
    pub reject: AtomicBool,
    pub writes: AtomicUsize,
}
#[async_trait]
impl SettingsStore for ProbeSettingsStore {
    async fn load(&self) -> Result<AppSettings, PortError> {
        Ok(self.settings.lock().unwrap().clone())
    }
    async fn load_hydrated(&self) -> Result<AppSettings, PortError> {
        self.load().await
    }
    async fn save(&self, settings: &AppSettings) -> Result<(), PortError> {
        if self.reject.load(Ordering::SeqCst) {
            return Err(PortError::Io("durable write denied".into()));
        }
        *self.settings.lock().unwrap() = settings.clone();
        self.writes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

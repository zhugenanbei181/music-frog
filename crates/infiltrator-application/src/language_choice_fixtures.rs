//! Isolated capture persistence: real settings commands, no operator files or processes.
use async_trait::async_trait;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_domain::settings::AppSettings;
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::error::PortError;
use infiltrator_ports::settings_store::SettingsStore;
use std::sync::Mutex;

#[derive(Default)]
pub struct LanguageCaptureStore(Mutex<AppSettings>);
impl LanguageCaptureStore {
    pub fn saved(&self) -> AppSettings {
        self.0.lock().expect("isolated capture store").clone()
    }
}
#[async_trait]
impl SettingsStore for LanguageCaptureStore {
    async fn load(&self) -> Result<AppSettings, PortError> {
        Ok(self.saved())
    }
    async fn load_hydrated(&self) -> Result<AppSettings, PortError> {
        self.load().await
    }
    async fn save(&self, settings: &AppSettings) -> Result<(), PortError> {
        *self
            .0
            .lock()
            .map_err(|_| PortError::Io("capture store poisoned".into()))? = settings.clone();
        Ok(())
    }
}
pub struct LanguageCaptureProcess;
#[async_trait]
impl CoreProcess for LanguageCaptureProcess {
    async fn start(&self) -> Result<(), PortError> {
        Err(PortError::Failed(
            "language capture refuses lifecycle effects".into(),
        ))
    }
    async fn stop(&self) -> Result<(), PortError> {
        Err(PortError::Failed(
            "language capture refuses lifecycle effects".into(),
        ))
    }
    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(CoreLifecycle::Stopped)
    }
    fn controller_endpoint(&self) -> Option<String> {
        None
    }
}
#[async_trait]
impl CoreReadiness for LanguageCaptureProcess {
    async fn probe(&self) -> Result<String, PortError> {
        Err(PortError::Failed(
            "language capture refuses network effects".into(),
        ))
    }
}

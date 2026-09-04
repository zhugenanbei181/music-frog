//! Settings use-cases over the runtime-neutral settings store port.

use infiltrator_contract::error::Failure;
use infiltrator_domain::settings::AppSettings;
use infiltrator_ports::settings_store::SettingsStore;
use std::sync::Arc;

#[derive(Clone)]
pub struct SettingsApplication {
    store: Arc<dyn SettingsStore>,
}

impl SettingsApplication {
    pub fn new(store: Arc<dyn SettingsStore>) -> Self {
        Self { store }
    }

    pub async fn load(&self) -> Result<AppSettings, Failure> {
        self.store.load().await.map_err(Failure::from)
    }

    pub async fn load_hydrated(&self) -> Result<AppSettings, Failure> {
        self.store.load_hydrated().await.map_err(Failure::from)
    }

    pub async fn save(&self, settings: &AppSettings) -> Result<(), Failure> {
        self.store.save(settings).await.map_err(Failure::from)
    }

    pub async fn update<F>(&self, update: F) -> Result<(), Failure>
    where
        F: FnOnce(&mut AppSettings),
    {
        let mut settings = self.load().await?;
        update(&mut settings);
        self.save(&settings).await
    }
}

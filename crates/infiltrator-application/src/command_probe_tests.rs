//! Behavior tests for retained runtime probe preferences.
use super::*;
use crate::command_application::CommandApplication;
use crate::command_application::handler::ValidatedCoreCommandScope;
use crate::core_capability_test_support::{core_scope, with_test_capabilities};
use crate::settings_application::SettingsApplication;
use async_trait::async_trait;
use infiltrator_contract::runtime_settings::RuntimeProbeSettings;
use infiltrator_domain::settings::AppSettings;
use infiltrator_ports::settings_store::SettingsStore;

#[derive(Default)]
struct ProbeSettings(Mutex<AppSettings>);

#[async_trait]
impl SettingsStore for ProbeSettings {
    async fn load(&self) -> Result<AppSettings, PortError> {
        Ok(self.0.lock().unwrap().clone())
    }

    async fn load_hydrated(&self) -> Result<AppSettings, PortError> {
        self.load().await
    }

    async fn save(&self, settings: &AppSettings) -> Result<(), PortError> {
        *self.0.lock().unwrap() = settings.clone();
        Ok(())
    }
}

#[tokio::test]
async fn set_runtime_probe_settings_persists_typed_values() {
    let settings = Arc::new(ProbeSettings::default());
    let app = with_test_capabilities(
        CommandApplication::new().with_settings(SettingsApplication::new(settings.clone())),
    );
    let probe_settings =
        RuntimeProbeSettings::new("https://probe.example.test/selected", 60_000).unwrap();

    app.handle_scoped(
        ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
        CommandIntent::SetRuntimeProbeSettings {
            settings: probe_settings.clone(),
        },
    )
    .await
    .unwrap();

    let stored = settings.load().await.unwrap();
    assert_eq!(stored.runtime_panel.probe_settings, probe_settings);

    assert!(RuntimeProbeSettings::new("file:///etc/passwd", 1000).is_err());
    assert_eq!(settings.load().await.unwrap(), stored);
}

//! Persist the shared runtime probe settings before publishing an applied fact.
use super::CommandApplication;
use infiltrator_contract::error::Failure;
use infiltrator_contract::runtime_settings::RuntimeProbeSettings;

impl CommandApplication {
    pub(super) async fn save_runtime_probe_settings(
        &self,
        probe_settings: RuntimeProbeSettings,
    ) -> Result<(), Failure> {
        self.settings()?
            .update(|settings| {
                settings.runtime_panel.probe_settings = probe_settings;
            })
            .await
    }
}

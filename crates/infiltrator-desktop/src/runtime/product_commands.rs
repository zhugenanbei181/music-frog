//! The complete desktop product command facade, using the same host engines as its reader.
use super::MihomoRuntime;
use crate::composition::{DesktopCommandServices, desktop_command_application};
use crate::log_export::DesktopLogExportPort;
use crate::storage::{app_routing_store, settings_store, subscription_source, sync};
use infiltrator_application::log_export_application::LogExportApplication;
use infiltrator_application::routing_application::RoutingApplication;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_application::sync_application::SyncApplication;
use infiltrator_ports::endpoint::EndpointSource;
use std::sync::Arc;

impl MihomoRuntime {
    pub async fn install_product_commands(self: &Arc<Self>) -> anyhow::Result<()> {
        let endpoint = self
            .endpoints
            .resolve()
            .await
            .map_err(|error| anyhow::anyhow!(error))?;
        let exports = LogExportApplication::new(
            self.application.log_application(),
            Some(Arc::new(DesktopLogExportPort::new(
                self.config_manager.config_dir(),
            ))),
            endpoint.secret.into_iter().collect(),
        )
        .with_endpoint_source(self.endpoints.clone());
        self.application
            .install_log_gateway(Arc::new(self.client.clone()))
            .map_err(|failure| anyhow::anyhow!(failure.message))?;
        let commands = desktop_command_application(DesktopCommandServices {
            binary_path: self.binary_path.clone(),
            client: self.client.clone(),
            engines: self.surface_engines(),
            profile_store: self.config_manager.clone(),
            subscription_source: Arc::new(subscription_source()),
        })?
        .with_logs(self.application.log_application())
        .with_log_export(exports)
        .with_managed_runtime(self.clone())
        .with_settings(SettingsApplication::new(settings_store().await?))
        .with_doctor(self.doctor.clone())
        .with_routing(RoutingApplication::new(Arc::new(app_routing_store()?)))
        .with_sync(SyncApplication::new(Arc::new(sync()?)));
        self.application.install_command_handler(Arc::new(commands));
        Ok(())
    }
}

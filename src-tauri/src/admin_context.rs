use async_trait::async_trait;
use anyhow::anyhow;
use mihomo_api::MihomoClient;
use mihomo_version::{Channel, channel::fetch_latest};
use tauri::{AppHandle, async_runtime};

use crate::{app_state::AppState, platform, runtime::rebuild_runtime};
use infiltrator_admin::AdminApiContext;
use infiltrator_core::AppSettings;
use infiltrator_desktop::editor;

#[derive(Clone)]
pub(crate) struct TauriAdminContext {
    pub(crate) app: AppHandle,
    pub(crate) app_state: AppState,
}

#[async_trait]
impl AdminApiContext for TauriAdminContext {
    async fn rebuild_runtime(&self) -> anyhow::Result<()> {
        rebuild_runtime(&self.app, &self.app_state).await
    }

    async fn set_use_bundled_core(&self, enabled: bool) {
        self.app_state.set_use_bundled_core(enabled).await;
    }

    async fn refresh_core_version_info(&self) {
        self.app_state.refresh_core_version_info().await;
    }

    async fn latest_stable_core(&self) -> anyhow::Result<(String, String)> {
        let latest = fetch_latest(Channel::Stable).await?;
        Ok((latest.version, latest.release_date))
    }

    async fn notify_subscription_update(
        &self,
        profile: String,
        success: bool,
        message: Option<String>,
    ) {
        self.app_state
            .notify_subscription_update(&profile, success, message)
            .await;
    }

    async fn editor_path(&self) -> Option<String> {
        self.app_state.editor_path().await
    }

    async fn set_editor_path(&self, path: Option<String>) {
        self.app_state.set_editor_path(path).await;
    }

    async fn pick_editor_path(&self) -> Option<String> {
        async_runtime::spawn_blocking(platform::pick_editor_path)
            .await
            .unwrap_or(None)
    }

    async fn open_profile_in_editor(&self, profile_name: &str) -> anyhow::Result<()> {
        let editor_path = self.app_state.editor_path().await;
        editor::open_profile_in_editor(editor_path, profile_name).await
    }

    async fn get_app_settings(&self) -> AppSettings {
        self.app_state.settings.read().await.clone()
    }

    async fn save_app_settings(&self, settings: AppSettings) -> anyhow::Result<()> {
        self.app_state.set_app_settings(settings).await
    }

    async fn runtime_running(&self) -> bool {
        match self.app_state.runtime().await {
            Ok(runtime) => runtime.is_running().await,
            Err(_) => false,
        }
    }

    async fn runtime_controller_url(&self) -> Option<String> {
        self.app_state
            .runtime()
            .await
            .ok()
            .map(|runtime| runtime.controller_url.clone())
    }

    async fn stop_runtime(&self) -> anyhow::Result<()> {
        self.app_state.stop_runtime().await;
        if self.app_state.is_system_proxy_enabled().await {
            self.app_state.disable_system_proxy().await;
        }
        Ok(())
    }

    async fn runtime_client(&self) -> anyhow::Result<MihomoClient> {
        let runtime = self.app_state.runtime().await?;
        Ok(runtime.client())
    }

    async fn system_proxy_enabled(&self) -> bool {
        self.app_state.is_system_proxy_enabled().await
    }

    async fn set_system_proxy_enabled(&self, enabled: bool) -> anyhow::Result<()> {
        if enabled {
            let runtime = self.app_state.runtime().await?;
            let endpoint = runtime
                .http_proxy_endpoint()
                .await?
                .ok_or_else(|| anyhow!("当前配置中未配置代理端口（port/mixed-port）"))?;
            crate::system_proxy::apply_system_proxy(Some(&endpoint))?;
        } else {
            crate::system_proxy::apply_system_proxy(None)?;
        }
        self.app_state.refresh_system_proxy_state().await;
        Ok(())
    }

    async fn autostart_enabled(&self) -> bool {
        crate::autostart::is_autostart_enabled()
    }

    async fn set_autostart_enabled(&self, enabled: bool) -> anyhow::Result<()> {
        crate::autostart::set_autostart_enabled(enabled)?;
        self.app_state.set_autostart_checked(enabled).await;
        Ok(())
    }

    fn supports_system_proxy_control(&self) -> bool {
        cfg!(target_os = "windows")
    }

    fn supports_autostart_control(&self) -> bool {
        cfg!(target_os = "windows")
    }
}

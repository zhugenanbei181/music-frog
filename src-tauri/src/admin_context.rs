use async_trait::async_trait;
use tauri::{async_runtime, AppHandle};

use crate::{app_state::AppState, platform, runtime::rebuild_runtime};
use despicable_infiltrator_core::{admin_api::AdminApiContext, AppSettings};

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
        async_runtime::spawn_blocking(|| platform::pick_editor_path())
            .await
            .unwrap_or(None)
    }

    async fn get_app_settings(&self) -> AppSettings {
        self.app_state.settings.read().await.clone()
    }

    async fn save_app_settings(&self, settings: AppSettings) -> anyhow::Result<()> {
        self.app_state.set_app_settings(settings).await
    }
}
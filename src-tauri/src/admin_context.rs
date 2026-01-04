use async_trait::async_trait;
use tauri::AppHandle;

use crate::{app_state::AppState, runtime::rebuild_runtime};
use despicable_infiltrator_core::admin_api::AdminApiContext;

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

    async fn editor_path(&self) -> Option<String> {
        self.app_state.editor_path().await
    }

    async fn set_editor_path(&self, path: Option<String>) {
        self.app_state.set_editor_path(path).await;
    }
}

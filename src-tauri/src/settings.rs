use std::path::PathBuf;

use anyhow::anyhow;
use despicable_infiltrator_core::settings as core_settings;

use crate::{app_state::AppState, paths::app_data_dir};

pub(crate) async fn load_settings(state: &AppState) -> anyhow::Result<()> {
    let path = settings_path(state).await?;
    let settings = core_settings::load_settings(&path).await?;
    *state.settings.write().await = settings;
    Ok(())
}

pub(crate) async fn save_settings(state: &AppState) -> anyhow::Result<()> {
    let path = settings_path(state).await?;
    let settings = state.settings.read().await;
    core_settings::save_settings(&path, &settings).await?;
    Ok(())
}

async fn settings_path(state: &AppState) -> anyhow::Result<PathBuf> {
    let app_handle = state
        .app_handle
        .read()
        .await
        .clone()
        .ok_or_else(|| anyhow!("app handle is not ready"))?;
    let base = app_data_dir(&app_handle)?;
    core_settings::settings_path(&base)
}

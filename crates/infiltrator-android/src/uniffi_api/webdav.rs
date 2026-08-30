//! WebDAV backup surface: credential/settings storage and validation plus
//! the on-demand sync run driven by the sync engine planner/executor.

use std::path::PathBuf;

use dav_client::DavClient;
use dav_client::client::WebDavClient;
use infiltrator_core::settings::{
    AppSettings, WebDavConfig as CoreWebDavConfig, load_settings, save_settings, settings_path,
};
use mihomo_platform::paths::get_home_dir;
use state_store::StateStore;
use sync_engine::{SyncPlanner, executor::SyncExecutor};

use super::support::{get_runtime, map_anyhow_error, map_mihomo_error};
use crate::ffi::{FfiErrorCode, FfiStatus};

// --- WebDAV API ---

#[derive(Debug, Clone, uniffi::Record)]
pub struct WebDavSettings {
    pub enabled: bool,
    pub url: String,
    pub username: String,
    pub password: String,
    pub sync_interval_mins: u32,
    pub sync_on_startup: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct WebDavSettingsResult {
    pub status: FfiStatus,
    pub settings: Option<WebDavSettings>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct WebDavSyncResult {
    pub status: FfiStatus,
    pub success_count: u32,
    pub failed_count: u32,
    pub total_actions: u32,
}

#[uniffi::export]
pub async fn webdav_settings() -> WebDavSettingsResult {
    get_runtime()
        .spawn(async move {
            match load_webdav_settings().await {
                Ok(settings) => WebDavSettingsResult {
                    status: FfiStatus::ok(),
                    settings: Some(settings),
                },
                Err(status) => WebDavSettingsResult {
                    status,
                    settings: None,
                },
            }
        })
        .await
        .unwrap_or_else(|e| WebDavSettingsResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            settings: None,
        })
}

#[uniffi::export]
pub async fn webdav_settings_save(settings: WebDavSettings) -> WebDavSettingsResult {
    get_runtime()
        .spawn(async move {
            match save_webdav_settings(settings).await {
                Ok(settings) => WebDavSettingsResult {
                    status: FfiStatus::ok(),
                    settings: Some(settings),
                },
                Err(status) => WebDavSettingsResult {
                    status,
                    settings: None,
                },
            }
        })
        .await
        .unwrap_or_else(|e| WebDavSettingsResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            settings: None,
        })
}

#[uniffi::export]
pub async fn webdav_test(settings: WebDavSettings) -> FfiStatus {
    get_runtime()
        .spawn(async move { test_webdav_settings(settings).await })
        .await
        .unwrap_or_else(|e| {
            FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

#[uniffi::export]
pub async fn webdav_sync_now() -> WebDavSyncResult {
    get_runtime()
        .spawn(async move {
            match sync_webdav_now().await {
                Ok(summary) => WebDavSyncResult {
                    status: FfiStatus::ok(),
                    success_count: summary.success_count as u32,
                    failed_count: summary.failed_count as u32,
                    total_actions: summary.total_actions as u32,
                },
                Err(status) => WebDavSyncResult {
                    status,
                    success_count: 0,
                    failed_count: 0,
                    total_actions: 0,
                },
            }
        })
        .await
        .unwrap_or_else(|e| WebDavSyncResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            success_count: 0,
            failed_count: 0,
            total_actions: 0,
        })
}

async fn load_webdav_settings() -> Result<WebDavSettings, FfiStatus> {
    let (settings, _) = load_app_settings().await?;
    Ok(webdav_settings_from_core(&settings.webdav))
}

async fn save_webdav_settings(settings: WebDavSettings) -> Result<WebDavSettings, FfiStatus> {
    let (mut app_settings, path) = load_app_settings().await?;
    app_settings.webdav = webdav_settings_to_core(settings);
    save_settings(&path, &app_settings)
        .await
        .map_err(map_anyhow_error)?;
    Ok(webdav_settings_from_core(&app_settings.webdav))
}

async fn test_webdav_settings(settings: WebDavSettings) -> FfiStatus {
    crate::tls::ensure_rustls_provider();
    let config = webdav_settings_to_core(settings);
    if let Err(status) = validate_webdav_config(&config) {
        return status;
    }
    let dav = match WebDavClient::new(&config.url, &config.username, &config.password) {
        Ok(client) => client,
        Err(err) => {
            return FfiStatus::err(
                FfiErrorCode::InvalidInput,
                format!("invalid WebDAV config: {err}"),
            );
        }
    };
    match dav.list("/").await {
        Ok(_) => FfiStatus::ok(),
        Err(err) => FfiStatus::err(
            FfiErrorCode::Network,
            format!("connection test failed: {err}"),
        ),
    }
}

#[derive(Debug, Default)]
struct WebDavSyncSummary {
    success_count: usize,
    failed_count: usize,
    total_actions: usize,
}

async fn sync_webdav_now() -> Result<WebDavSyncSummary, FfiStatus> {
    crate::tls::ensure_rustls_provider();
    let (settings, _) = load_app_settings().await?;
    if !settings.webdav.enabled {
        return Err(FfiStatus::err(FfiErrorCode::NotReady, "WebDAV is disabled"));
    }
    run_webdav_sync(&settings.webdav).await
}

async fn run_webdav_sync(config: &CoreWebDavConfig) -> Result<WebDavSyncSummary, FfiStatus> {
    validate_webdav_config(config)?;
    let dav =
        WebDavClient::new(&config.url, &config.username, &config.password).map_err(|err| {
            FfiStatus::err(
                FfiErrorCode::InvalidInput,
                format!("invalid WebDAV config: {err}"),
            )
        })?;

    let home = get_home_dir().map_err(map_mihomo_error)?;
    let local_root = home.join("configs");
    if !local_root.exists() {
        tokio::fs::create_dir_all(&local_root)
            .await
            .map_err(|e| FfiStatus::err(FfiErrorCode::Io, e.to_string()))?;
    }
    let db_path = home.join("sync_state.db").to_string_lossy().to_string();
    let store = StateStore::new(&db_path).await.map_err(map_anyhow_error)?;

    let planner = SyncPlanner::new(local_root, "/".to_string(), &dav, &store);
    let actions = planner.build_plan().await.map_err(map_anyhow_error)?;

    if actions.is_empty() {
        return Ok(WebDavSyncSummary::default());
    }

    let executor = SyncExecutor::new(&dav, &store);
    let total_actions = actions.len();
    let mut success_count = 0usize;
    let mut failed_count = 0usize;
    for action in actions {
        match executor.execute(action).await {
            Ok(()) => success_count = success_count.saturating_add(1),
            Err(_) => failed_count = failed_count.saturating_add(1),
        }
    }

    Ok(WebDavSyncSummary {
        success_count,
        failed_count,
        total_actions,
    })
}

async fn load_app_settings() -> Result<(AppSettings, PathBuf), FfiStatus> {
    let base = get_home_dir().map_err(map_mihomo_error)?;
    let path = settings_path(&base)
        .map_err(|err| FfiStatus::err(FfiErrorCode::InvalidState, err.to_string()))?;
    let settings = load_settings(&path).await.map_err(map_anyhow_error)?;
    Ok((settings, path))
}

fn webdav_settings_from_core(config: &CoreWebDavConfig) -> WebDavSettings {
    WebDavSettings {
        enabled: config.enabled,
        url: config.url.clone(),
        username: config.username.clone(),
        password: config.password.clone(),
        sync_interval_mins: config.sync_interval_mins,
        sync_on_startup: config.sync_on_startup,
    }
}

fn webdav_settings_to_core(settings: WebDavSettings) -> CoreWebDavConfig {
    CoreWebDavConfig {
        enabled: settings.enabled,
        url: settings.url.trim().to_string(),
        username: settings.username.trim().to_string(),
        password: settings.password,
        sync_interval_mins: settings.sync_interval_mins,
        sync_on_startup: settings.sync_on_startup,
    }
}

fn validate_webdav_config(config: &CoreWebDavConfig) -> Result<(), FfiStatus> {
    if config.url.trim().is_empty() {
        return Err(FfiStatus::err(
            FfiErrorCode::InvalidInput,
            "WebDAV URL is empty",
        ));
    }
    Ok(())
}

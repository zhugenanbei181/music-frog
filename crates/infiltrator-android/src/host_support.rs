//! Shared plumbing for the FFI surface: the global tokio runtime, generic
//! string/list normalization helpers, error mapping into [`FfiStatus`], and
//! the mihomo controller client used by every live-query passthrough.

use std::sync::OnceLock;

use infiltrator_application::cache_application::CacheApplication;
use infiltrator_application::connection_application::ConnectionApplication;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_application::network_application::NetworkApplication;
use infiltrator_application::proxy_application::ProxyApplication;
use infiltrator_application::runtime_query_application::RuntimeQueryApplication;
use infiltrator_application::routing_application::RoutingApplication;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_application::sync_application::SyncApplication;
use infiltrator_ports::subscription_source::SubscriptionSource;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use mihomo_api::client::MihomoClient;
use mihomo_api::error::MihomoError;
use mihomo_config::manager::ConfigManager;
use mihomo_platform::android_bridge::get_android_bridge;
use mihomo_platform::defaults::DefaultCredentialStore;
use mihomo_platform::paths::get_home_dir;
use tokio::runtime::Runtime;

use crate::host_session::shared_core;
use crate::ffi::{FfiErrorCode, FfiStatus};
use infiltrator_contract::error::{ErrorCode, Failure};
pub(super) fn get_runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| Runtime::new().expect("failed to create tokio runtime"))
}

pub(super) fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

pub(super) fn sanitize_list(value: Option<Vec<String>>) -> Option<Vec<String>> {
    value.map(|items| {
        items
            .into_iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect()
    })
}

/// `settings.configs_dir` redirect target, already trimmed by the manager
/// resolution; `None` keeps the default `<home>/configs` behavior. A settings
/// load failure must not block the default path, so it degrades to `None`.
pub(super) async fn configs_dir_override() -> Option<String> {
    match build_settings_application().await {
        Ok(application) => match application.load().await {
            Ok(settings) => settings.configs_dir,
            Err(failure) => {
                log::warn!(
                    "settings load failed, configs_dir override ignored: {}",
                    failure.message
                );
                None
            }
        },
        Err(status) => {
            log::warn!(
                "settings adapter unavailable, configs_dir override ignored: {:?}",
                status.code
            );
            None
        }
    }
}

pub(super) async fn build_settings_application() -> Result<SettingsApplication, FfiStatus> {
    let store = infiltrator_core::settings_store::for_current_home().map_err(map_anyhow_error)?;
    Ok(SettingsApplication::new(std::sync::Arc::new(store)))
}

pub(super) async fn build_configuration_application() -> Result<ConfigurationApplication, FfiStatus>
{
    let manager = build_config_manager().await?;
    Ok(ConfigurationApplication::new(std::sync::Arc::new(manager)))
}

pub(super) fn subscription_source() -> impl SubscriptionSource {
    infiltrator_core::subscription_io::HttpSubscriptionSource::with_default_clients()
}

pub(crate) async fn save_webdav_password(password: &str) -> Result<(), FfiStatus> {
    let store = DefaultCredentialStore::default();
    infiltrator_core::settings_io::save_webdav_password(&store, password)
        .await
        .map_err(map_anyhow_error)
}

pub(crate) async fn clear_webdav_password() {
    let store = DefaultCredentialStore::default();
    infiltrator_core::settings_io::clear_webdav_password(&store).await;
}

pub(super) fn doctor_application() -> Result<DoctorApplication, FfiStatus> {
    let doctor = infiltrator_core::doctor_port::MihomoDoctor::detect()
        .map_err(map_anyhow_error)?;
    Ok(DoctorApplication::new(std::sync::Arc::new(doctor)))
}

pub(super) fn cache_application() -> CacheApplication {
    CacheApplication::new(std::sync::Arc::new(
        infiltrator_core::fake_ip_cache_io::FileFakeIpCache::current(),
    ))
}

pub(super) fn build_routing_application() -> Result<RoutingApplication, FfiStatus> {
    let store = infiltrator_core::app_routing_io::FileAppRoutingStore::current()
        .map_err(map_anyhow_error)?;
    Ok(RoutingApplication::new(std::sync::Arc::new(store)))
}

/// ConfigManager wired for the configs-dir redirect. The `INFILTRATOR_CONFIGS_DIR`
/// env keeps priority over the settings field inside `resolve_configs_dir_in`;
/// with no override anywhere this uses `<home>/configs`.
pub(crate) async fn build_config_manager()
-> Result<ConfigManager<DefaultCredentialStore>, FfiStatus> {
    let home = get_home_dir().map_err(map_mihomo_error)?;
    let override_dir = configs_dir_override().await;
    ConfigManager::with_home_configs_dir_and_store(
        home,
        override_dir.as_deref(),
        DefaultCredentialStore::default(),
    )
    .map_err(map_mihomo_error)
}

pub(super) async fn build_controller_client() -> Result<MihomoClient, FfiStatus> {
    // Prefer the shared endpoint port: URL and secret are re-resolved from
    // the current profile on every call (port rotation and secret aware).
    if let Ok(core) = shared_core().await {
        match core.endpoints.resolve().await {
            Ok(endpoint) => match MihomoClient::new(&endpoint.url, endpoint.secret) {
                Ok(client) => return Ok(client),
                Err(err) => {
                    log::debug!(
                        "application endpoint client unavailable, using legacy resolution: {err}"
                    );
                }
            },
            Err(err) => {
                log::debug!("application endpoint unavailable, using legacy resolution: {err}");
            }
        }
    }
    let manager = build_config_manager().await?;
    let controller_url = match manager.get_external_controller().await {
        Ok(url) => url,
        Err(err) => {
            if let Some(bridge) = get_android_bridge()
                && let Some(url) = bridge.core_controller_url()
            {
                return MihomoClient::new(&url, None).map_err(map_mihomo_error);
            }
            return Err(map_mihomo_error(err));
        }
    };
    MihomoClient::new(&controller_url, None).map_err(map_mihomo_error)
}

/// Build the controller port at the Android composition boundary. FFI
/// modules consume application facades rather than retaining a concrete
/// `MihomoClient` or its Tokio receiver types.
pub(super) async fn build_runtime_gateway() -> Result<std::sync::Arc<dyn RuntimeGateway>, FfiStatus> {
    Ok(std::sync::Arc::new(build_controller_client().await?))
}

pub(super) async fn build_proxy_application() -> Result<ProxyApplication, FfiStatus> {
    Ok(ProxyApplication::new(build_runtime_gateway().await?))
}

pub(super) async fn build_connection_application() -> Result<ConnectionApplication, FfiStatus> {
    Ok(ConnectionApplication::new(build_runtime_gateway().await?))
}

pub(super) async fn build_runtime_query_application() -> Result<RuntimeQueryApplication, FfiStatus> {
    Ok(RuntimeQueryApplication::new(build_runtime_gateway().await?))
}

pub(super) fn network_application() -> NetworkApplication {
    NetworkApplication::new(std::sync::Arc::new(
        infiltrator_core::public_ip_io::HttpPublicIpProbe::with_geolocation_client(),
    ))
}

pub(super) async fn build_sync_application() -> Result<SyncApplication, FfiStatus> {
    let home = get_home_dir().map_err(map_mihomo_error)?;
    let sync = infiltrator_core::sync_port::FileWebDavSync::new(
        home,
        DefaultCredentialStore::default(),
    );
    Ok(SyncApplication::new(std::sync::Arc::new(sync)))
}

pub(super) fn map_anyhow_error(err: anyhow::Error) -> FfiStatus {
    if let Some(source) = err.downcast_ref::<MihomoError>() {
        return map_mihomo_error_ref(source);
    }
    FfiStatus::err(FfiErrorCode::Unknown, err.to_string())
}

pub(super) fn map_application_failure(failure: Failure) -> FfiStatus {
    let code = match failure.code {
        ErrorCode::InvalidInput => FfiErrorCode::InvalidInput,
        ErrorCode::InvalidState => FfiErrorCode::InvalidState,
        ErrorCode::NotReady => FfiErrorCode::NotReady,
        ErrorCode::Unsupported => FfiErrorCode::NotSupported,
        ErrorCode::Network => FfiErrorCode::Network,
        ErrorCode::Authentication => FfiErrorCode::Auth,
        ErrorCode::Configuration => FfiErrorCode::Config,
        ErrorCode::Storage => FfiErrorCode::Io,
        ErrorCode::Permission => FfiErrorCode::InvalidState,
        ErrorCode::Canceled | ErrorCode::Internal => FfiErrorCode::Unknown,
    };
    FfiStatus::err(code, failure.message)
}

pub(crate) fn map_mihomo_error(err: MihomoError) -> FfiStatus {
    map_mihomo_error_ref(&err)
}

fn map_mihomo_error_ref(err: &MihomoError) -> FfiStatus {
    match err {
        MihomoError::Http(_) => FfiStatus::err(FfiErrorCode::Network, err.to_string()),
        MihomoError::Io(_) => FfiStatus::err(FfiErrorCode::Io, err.to_string()),
        MihomoError::Json(_) | MihomoError::Yaml(_) | MihomoError::YamlEmit(_) => {
            FfiStatus::err(FfiErrorCode::InvalidState, err.to_string())
        }
        MihomoError::UrlParse(_) => FfiStatus::err(FfiErrorCode::InvalidInput, err.to_string()),
        MihomoError::WebSocket(_) => FfiStatus::err(FfiErrorCode::Network, err.to_string()),
        MihomoError::Config(_) | MihomoError::Service(_) | MihomoError::Version(_) => {
            FfiStatus::err(FfiErrorCode::InvalidState, err.to_string())
        }
        MihomoError::Proxy(_) | MihomoError::NotFound(_) => {
            FfiStatus::err(FfiErrorCode::NotReady, err.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_core::settings_io::save_settings;
    use infiltrator_domain::settings::AppSettings;
    use mihomo_platform::TEST_LOCK;
    use mihomo_platform::paths::{clear_home_dir_override, set_home_dir_override};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    const CONFIGS_DIR_ENV: &str = "INFILTRATOR_CONFIGS_DIR";

    async fn test_lock() -> tokio::sync::MutexGuard<'static, ()> {
        TEST_LOCK.lock().await
    }

    fn make_test_home(tag: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("infiltrator-android-cc-{tag}-{unique}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create test home dir");
        path
    }

    async fn write_settings_with_configs_dir(home: &std::path::Path, configs_dir: &str) {
        let settings = AppSettings {
            configs_dir: Some(configs_dir.to_string()),
            ..AppSettings::default()
        };
        let path =
            infiltrator_core::settings_io::settings_path(home).expect("settings path resolves");
        save_settings(&path, &settings)
            .await
            .expect("save settings");
    }

    fn set_env(value: &str) {
        unsafe { std::env::set_var(CONFIGS_DIR_ENV, value) };
    }

    fn clear_env() {
        unsafe { std::env::remove_var(CONFIGS_DIR_ENV) };
    }

    fn restore_env(saved: Option<String>) {
        match saved {
            Some(value) => set_env(&value),
            None => clear_env(),
        }
    }

    async fn configs_parent(manager: &ConfigManager<DefaultCredentialStore>) -> PathBuf {
        manager
            .get_current_path()
            .await
            .expect("current profile path resolves")
            .parent()
            .expect("profile path has a parent")
            .to_path_buf()
    }

    #[tokio::test]
    async fn config_manager_follows_settings_configs_dir() {
        let _guard = test_lock().await;
        let home = make_test_home("settings-redirect");
        set_home_dir_override(home.clone());
        write_settings_with_configs_dir(&home, "cloud/profiles").await;

        let manager = build_config_manager()
            .await
            .expect("manager builds with settings override");
        assert_eq!(
            configs_parent(&manager).await,
            home.join("cloud").join("profiles")
        );

        clear_home_dir_override();
        let _ = fs::remove_dir_all(home);
    }

    #[tokio::test]
    async fn env_configs_dir_wins_over_settings_field() {
        let _guard = test_lock().await;
        let home = make_test_home("env-priority");
        set_home_dir_override(home.clone());
        write_settings_with_configs_dir(&home, "cloud/profiles").await;

        let saved = std::env::var(CONFIGS_DIR_ENV).ok();
        let env_dir = home.join("env-cloud");
        set_env(env_dir.to_str().unwrap());

        let manager = build_config_manager().await.expect("manager builds");
        assert_eq!(configs_parent(&manager).await, env_dir);

        restore_env(saved);
        clear_home_dir_override();
        let _ = fs::remove_dir_all(home);
    }

    #[tokio::test]
    async fn no_override_keeps_default_configs_dir() {
        let _guard = test_lock().await;
        let home = make_test_home("default-dir");
        set_home_dir_override(home.clone());

        let saved = std::env::var(CONFIGS_DIR_ENV).ok();
        clear_env();

        let manager = build_config_manager().await.expect("manager builds");
        assert_eq!(configs_parent(&manager).await, home.join("configs"));

        restore_env(saved);
        clear_home_dir_override();
        let _ = fs::remove_dir_all(home);
    }
}

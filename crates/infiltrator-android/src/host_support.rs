//! Shared plumbing for Android FFI: the global tokio runtime, config and
//! application adapters, and error mapping into [`FfiStatus`].

use crate::ffi::{FfiErrorCode, FfiFailureLocalization, FfiFailureLocalizationKey, FfiStatus};
use infiltrator_application::cache_application::CacheApplication;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_application::failure_projection::{FailureLocalizationKey, project_failure};
use infiltrator_application::network_application::NetworkApplication;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::routing_application::RoutingApplication;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_application::sync_application::SyncApplication;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_core::app_routing_io::FileAppRoutingStore;
use infiltrator_core::doctor_port::MihomoDoctor;
use infiltrator_core::fake_ip_cache_io::FileFakeIpCache;
use infiltrator_core::public_ip_io::HttpPublicIpProbe;
use infiltrator_core::settings_io;
use infiltrator_core::settings_store::for_current_home;
use infiltrator_core::subscription_io::HttpSubscriptionSource;
use infiltrator_core::sync_port::FileWebDavSync;
use infiltrator_platform::defaults::DefaultCredentialStore;
use infiltrator_platform::paths::get_home_dir;
use infiltrator_ports::error::PortError;
use infiltrator_ports::subscription_source::SubscriptionSource;
use mihomo_api::error::MihomoError;
use mihomo_config::manager::ConfigManager;
use mihomo_config::profile_document_codec::MihomoProfileDocumentCodec;
use std::sync::{Arc, OnceLock};
use tokio::runtime::Runtime;
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
    let store = for_current_home().map_err(map_anyhow_error)?;
    Ok(SettingsApplication::new(Arc::new(store)))
}

pub(super) async fn build_configuration_application() -> Result<ConfigurationApplication, FfiStatus>
{
    let manager = build_config_manager().await?;
    let profiles = ProfileApplication::new(Arc::new(manager))
        .with_document_codec(Arc::new(MihomoProfileDocumentCodec));
    Ok(ConfigurationApplication::from_profile_application(profiles))
}

pub(super) fn subscription_source() -> impl SubscriptionSource {
    HttpSubscriptionSource::with_default_clients()
}

pub(crate) async fn save_webdav_password(password: &str) -> Result<(), FfiStatus> {
    let store = DefaultCredentialStore::default();
    settings_io::save_webdav_password(&store, password)
        .await
        .map_err(map_anyhow_error)
}

pub(crate) async fn clear_webdav_password() {
    let store = DefaultCredentialStore::default();
    settings_io::clear_webdav_password(&store).await;
}

pub(super) fn doctor_application() -> Result<DoctorApplication, FfiStatus> {
    let doctor = MihomoDoctor::detect().map_err(map_anyhow_error)?;
    Ok(DoctorApplication::new(Arc::new(doctor)))
}

pub(super) fn cache_application() -> CacheApplication {
    CacheApplication::new(Arc::new(FileFakeIpCache::current()))
}

pub(super) fn build_routing_application() -> Result<RoutingApplication, FfiStatus> {
    let store = FileAppRoutingStore::current().map_err(map_anyhow_error)?;
    Ok(RoutingApplication::new(Arc::new(store)))
}

/// ConfigManager wired for the configs-dir redirect. The `INFILTRATOR_CONFIGS_DIR`
/// env keeps priority over the settings field inside `resolve_configs_dir_in`;
/// with no override anywhere this uses `<home>/configs`.
pub(crate) async fn build_config_manager()
-> Result<ConfigManager<DefaultCredentialStore>, FfiStatus> {
    let home = get_home_dir().map_err(map_port_error)?;
    let override_dir = configs_dir_override().await;
    ConfigManager::with_home_configs_dir_and_store(
        home,
        override_dir.as_deref(),
        DefaultCredentialStore::default(),
    )
    .map_err(map_mihomo_error)
}

pub(super) fn network_application() -> NetworkApplication {
    NetworkApplication::new(Arc::new(HttpPublicIpProbe::with_geolocation_client()))
}

pub(super) async fn build_sync_application() -> Result<SyncApplication, FfiStatus> {
    let home = get_home_dir().map_err(map_port_error)?;
    let sync = FileWebDavSync::new(home, DefaultCredentialStore::default());
    Ok(SyncApplication::new(Arc::new(sync)))
}

pub(super) fn map_anyhow_error(err: anyhow::Error) -> FfiStatus {
    if let Some(source) = err.downcast_ref::<MihomoError>() {
        return map_mihomo_error_ref(source);
    }
    FfiStatus::err(FfiErrorCode::Unknown, err.to_string())
}

pub(super) fn map_application_failure(failure: Failure) -> FfiStatus {
    let code = match &failure.code {
        ErrorCode::InvalidInput => FfiErrorCode::InvalidInput,
        ErrorCode::InvalidState => FfiErrorCode::InvalidState,
        ErrorCode::NotReady => FfiErrorCode::NotReady,
        ErrorCode::Unsupported => FfiErrorCode::NotSupported,
        ErrorCode::Network => FfiErrorCode::Network,
        ErrorCode::Authentication => FfiErrorCode::Auth,
        ErrorCode::Configuration => FfiErrorCode::Config,
        ErrorCode::Storage => FfiErrorCode::Io,
        ErrorCode::Permission => FfiErrorCode::InvalidState,
        ErrorCode::Canceled => FfiErrorCode::Canceled,
        ErrorCode::Internal => FfiErrorCode::Unknown,
    };
    let projection = project_failure(&failure);
    let localization = FfiFailureLocalization {
        key: match projection.key {
            FailureLocalizationKey::InvalidInput => FfiFailureLocalizationKey::InvalidInput,
            FailureLocalizationKey::InvalidState => FfiFailureLocalizationKey::InvalidState,
            FailureLocalizationKey::NotReady => FfiFailureLocalizationKey::NotReady,
            FailureLocalizationKey::Unsupported => FfiFailureLocalizationKey::Unsupported,
            FailureLocalizationKey::Network => FfiFailureLocalizationKey::Network,
            FailureLocalizationKey::Authentication => FfiFailureLocalizationKey::Authentication,
            FailureLocalizationKey::Configuration => FfiFailureLocalizationKey::Configuration,
            FailureLocalizationKey::Storage => FfiFailureLocalizationKey::Storage,
            FailureLocalizationKey::Permission => FfiFailureLocalizationKey::Permission,
            FailureLocalizationKey::Canceled => FfiFailureLocalizationKey::Canceled,
            FailureLocalizationKey::Internal => FfiFailureLocalizationKey::Internal,
            FailureLocalizationKey::YamlSyntax => FfiFailureLocalizationKey::YamlSyntax,
            FailureLocalizationKey::MixinYaml => FfiFailureLocalizationKey::MixinYaml,
            FailureLocalizationKey::ActiveProfileInconsistent => {
                FfiFailureLocalizationKey::ActiveProfileInconsistent
            }
            FailureLocalizationKey::QuotaSourceChanged => {
                FfiFailureLocalizationKey::QuotaSourceChanged
            }
            FailureLocalizationKey::CurrentTimeUnavailable => {
                FfiFailureLocalizationKey::CurrentTimeUnavailable
            }
            FailureLocalizationKey::DownloadCanceled => FfiFailureLocalizationKey::DownloadCanceled,
        },
        line: projection.line.map(|line| line as u64),
        column: projection.column.map(|column| column as u64),
    };
    FfiStatus::err_with_localization(code, failure.message, localization)
}

pub(crate) fn map_mihomo_error(err: MihomoError) -> FfiStatus {
    map_mihomo_error_ref(&err)
}

pub(crate) fn map_port_error(error: PortError) -> FfiStatus {
    map_application_failure(Failure::from(error))
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
        MihomoError::Canceled => FfiStatus::err(FfiErrorCode::Canceled, err.to_string()),
    }
}

#[cfg(test)]
mod failure_localization_tests {
    use super::*;
    use infiltrator_contract::error::FailureReason;

    #[test]
    fn application_failure_keeps_machine_code_diagnostic_and_typed_reason() {
        let status = map_application_failure(
            Failure::new(ErrorCode::Configuration, "parser detail", false)
                .with_reason(FailureReason::YamlSyntax { line: 7, column: 3 }),
        );
        assert_eq!(status.code, FfiErrorCode::Config);
        assert_eq!(status.message.as_deref(), Some("parser detail"));
        assert_eq!(
            status.localization,
            Some(FfiFailureLocalization {
                key: FfiFailureLocalizationKey::YamlSyntax,
                line: Some(7),
                column: Some(3),
            })
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_core::settings_io::save_settings;
    use infiltrator_domain::settings::AppSettings;
    use infiltrator_platform::TEST_LOCK;
    use infiltrator_platform::paths::{clear_home_dir_override, set_home_dir_override};
    #[cfg(test)]
    use std::env::remove_var;
    #[cfg(test)]
    use std::env::set_var;
    #[cfg(test)]
    use std::env::temp_dir;
    #[cfg(test)]
    use std::env::var;
    use std::fs;
    #[cfg(test)]
    use std::path;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    #[cfg(test)]
    use tokio::sync::MutexGuard;

    const CONFIGS_DIR_ENV: &str = "INFILTRATOR_CONFIGS_DIR";

    async fn test_lock() -> MutexGuard<'static, ()> {
        TEST_LOCK.lock().await
    }

    fn make_test_home(tag: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = temp_dir().join(format!("infiltrator-android-cc-{tag}-{unique}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create test home dir");
        path
    }

    async fn write_settings_with_configs_dir(home: &path::Path, configs_dir: &str) {
        let settings = AppSettings {
            configs_dir: Some(configs_dir.to_string()),
            ..AppSettings::default()
        };
        let path = settings_io::settings_path(home).expect("settings path resolves");
        save_settings(&path, &settings)
            .await
            .expect("save settings");
    }

    fn set_env(value: &str) {
        unsafe { set_var(CONFIGS_DIR_ENV, value) };
    }

    fn clear_env() {
        unsafe { remove_var(CONFIGS_DIR_ENV) };
    }

    fn restore_env(saved: Option<String>) {
        match saved {
            Some(value) => set_env(&value),
            None => clear_env(),
        }
    }

    async fn mihomo_namespace(manager: &ConfigManager<DefaultCredentialStore>) -> PathBuf {
        manager.config_dir().to_path_buf()
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
            mihomo_namespace(&manager).await,
            home.join("cloud")
                .join("profiles")
                .join("engines")
                .join("mihomo")
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

        let saved = var(CONFIGS_DIR_ENV).ok();
        let env_dir = home.join("env-cloud");
        set_env(env_dir.to_str().unwrap());

        let manager = build_config_manager().await.expect("manager builds");
        assert_eq!(
            mihomo_namespace(&manager).await,
            env_dir.join("engines").join("mihomo")
        );

        restore_env(saved);
        clear_home_dir_override();
        let _ = fs::remove_dir_all(home);
    }

    #[tokio::test]
    async fn no_override_keeps_default_configs_dir() {
        let _guard = test_lock().await;
        let home = make_test_home("default-dir");
        set_home_dir_override(home.clone());

        let saved = var(CONFIGS_DIR_ENV).ok();
        clear_env();

        let manager = build_config_manager().await.expect("manager builds");
        assert_eq!(
            mihomo_namespace(&manager).await,
            home.join("configs").join("engines").join("mihomo")
        );

        restore_env(saved);
        clear_home_dir_override();
        let _ = fs::remove_dir_all(home);
    }
}

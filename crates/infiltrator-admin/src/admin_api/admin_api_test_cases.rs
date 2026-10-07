use crate::admin_api::models::{ImportProfilePayload, SaveProfilePayload, SwitchProfilePayload};
use crate::admin_api::state::{AdminApiContext, AdminApiState};
use crate::admin_api::*;
use crate::support::{
    app_config_manager, cache_application, configuration_application, doctor_application,
    profile_application, profile_reset_application, subscription_source, sync_application,
    version_application,
};
use anyhow::anyhow;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use infiltrator_application::cache_application::CacheApplication;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::profile_reset_application::ProfileResetApplication;
use infiltrator_application::sync_application::SyncApplication;
use infiltrator_application::version_application::VersionApplication;
use infiltrator_contract::version::{
    CoreRelease, CoreReleaseChannel, CoreReleaseSummary, InstalledCoreVersion,
};
use infiltrator_core::settings_io::{WEBDAV_CREDENTIAL_SERVICE, WEBDAV_PASSWORD_KEY};
use infiltrator_domain::settings::AppSettings;
use infiltrator_ports::error::PortError;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use infiltrator_ports::subscription_source::SubscriptionSource;
use infiltrator_ports::version::{VersionPort, VersionProgressSink};
use mihomo_api::client::MihomoClient;
use mihomo_platform::TEST_LOCK;
use mihomo_platform::defaults::DefaultCredentialStore;
use std::collections::HashMap;
#[cfg(unix)]
use std::fs::Permissions;
#[cfg(unix)]
use std::fs::create_dir_all;
#[cfg(unix)]
use std::fs::set_permissions;
#[cfg(unix)]
use std::fs::write;
#[cfg(unix)]
use std::path;
use std::sync::{Arc, Mutex};
use tower::ServiceExt; // for `oneshot`, `ready`, and `call`

/// set_default smoke-checks the candidate binary; these route tests only
/// exercise the HTTP plumbing, so a tiny runnable stand-in suffices.
#[cfg(unix)]
fn plant_runnable_fake_binary(home: &path::Path, version: &str) {
    use std::os::unix::fs::PermissionsExt;
    let dir = home.join("versions").join(version);
    create_dir_all(&dir).unwrap();
    let bin = dir.join("mihomo");
    write(&bin, "#!/bin/sh\necho \"Mihomo Meta v1.19.18 test\"\n").unwrap();
    set_permissions(&bin, Permissions::from_mode(0o755)).unwrap();
}

#[derive(Clone)]
struct MockContext {
    rebuild_count: Arc<Mutex<usize>>,
    runtime_url: Option<String>,
    settings: Arc<Mutex<AppSettings>>,
    /// 内存版 WebDAV 密码库（替代真实 OS keyring，测试零外部依赖）。
    secrets: Arc<Mutex<HashMap<String, String>>>,
    version: Option<VersionApplication>,
}

/// 内存密码库的键：service/key 拼接，语义与真实 keyring 一致。
fn secrets_key() -> String {
    format!("{}/{}", WEBDAV_CREDENTIAL_SERVICE, WEBDAV_PASSWORD_KEY)
}

type SharedSecrets = Arc<Mutex<HashMap<String, String>>>;

#[async_trait::async_trait]
impl AdminApiContext for MockContext {
    async fn profile_application(&self) -> anyhow::Result<ProfileApplication> {
        profile_application().await
    }

    async fn configuration_application(&self) -> anyhow::Result<ConfigurationApplication> {
        configuration_application().await
    }

    async fn doctor_application(&self) -> anyhow::Result<DoctorApplication> {
        doctor_application()
    }

    async fn profile_reset_application(&self) -> anyhow::Result<ProfileResetApplication> {
        Ok(profile_reset_application())
    }

    async fn cache_application(&self) -> anyhow::Result<CacheApplication> {
        Ok(cache_application())
    }

    async fn subscription_source(&self) -> anyhow::Result<Arc<dyn SubscriptionSource>> {
        Ok(subscription_source())
    }

    async fn sync_application(&self) -> anyhow::Result<SyncApplication> {
        sync_application()
    }

    async fn version_application(&self) -> anyhow::Result<VersionApplication> {
        self.version
            .clone()
            .map(Ok)
            .unwrap_or_else(version_application)
    }

    async fn profile_controller_url(&self) -> anyhow::Result<Option<String>> {
        Ok(app_config_manager()
            .await?
            .get_external_controller()
            .await
            .ok())
    }

    async fn rebuild_runtime(&self) -> anyhow::Result<()> {
        let mut count = self.rebuild_count.lock().unwrap_or_else(|e| e.into_inner());
        *count += 1;
        Ok(())
    }
    async fn set_use_bundled_core(&self, _enabled: bool) {}
    async fn refresh_core_version_info(&self) {}
    async fn notify_subscription_update(&self, _p: String, _s: bool, _m: Option<String>) {}
    async fn editor_path(&self) -> Option<String> {
        None
    }
    async fn set_editor_path(&self, _path: Option<String>) {}
    async fn pick_editor_path(&self) -> Option<String> {
        None
    }
    async fn open_profile_in_editor(&self, _name: &str) -> anyhow::Result<()> {
        Ok(())
    }
    async fn get_app_settings(&self) -> AppSettings {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    async fn save_app_settings(&self, s: AppSettings) -> anyhow::Result<()> {
        *self.settings.lock().unwrap_or_else(|e| e.into_inner()) = s;
        Ok(())
    }
    async fn webdav_password(&self) -> Option<String> {
        self.secrets
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&secrets_key())
            .cloned()
    }
    async fn set_webdav_password(&self, password: &str) -> anyhow::Result<()> {
        self.secrets
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(secrets_key(), password.to_string());
        Ok(())
    }
    async fn runtime_running(&self) -> bool {
        self.runtime_url.is_some()
    }
    async fn runtime_controller_url(&self) -> Option<String> {
        self.runtime_url.clone()
    }
    async fn stop_runtime(&self) -> anyhow::Result<()> {
        Ok(())
    }
    async fn runtime_gateway(&self) -> anyhow::Result<Arc<dyn RuntimeGateway>> {
        let runtime_url = self
            .runtime_url
            .as_deref()
            .ok_or_else(|| anyhow!("runtime url is not configured"))?;
        let client = MihomoClient::new(runtime_url, None).map_err(|e| anyhow!(e.to_string()))?;
        Ok(Arc::new(client))
    }
    async fn system_proxy_enabled(&self) -> bool {
        false
    }
    async fn set_system_proxy_enabled(&self, _enabled: bool) -> anyhow::Result<()> {
        Ok(())
    }
    async fn autostart_enabled(&self) -> bool {
        false
    }
    async fn set_autostart_enabled(&self, _enabled: bool) -> anyhow::Result<()> {
        Ok(())
    }
    fn supports_system_proxy_control(&self) -> bool {
        false
    }
    fn supports_autostart_control(&self) -> bool {
        false
    }
}

fn setup_app() -> axum::Router {
    setup_app_with_runtime(None)
}

fn setup_app_with_runtime(runtime_url: Option<String>) -> axum::Router {
    let (app, _) = setup_app_with_runtime_and_secrets(runtime_url);
    app
}

fn setup_app_with_runtime_and_secrets(
    runtime_url: Option<String>,
) -> (axum::Router, SharedSecrets) {
    let ctx = MockContext {
        rebuild_count: Arc::new(Mutex::new(0)),
        runtime_url,
        settings: Arc::new(Mutex::new(AppSettings::default())),
        secrets: Arc::new(Mutex::new(HashMap::new())),
        version: None,
    };
    let secrets = ctx.secrets.clone();
    let bus = events::AdminEventBus::new();
    let state = AdminApiState::new(ctx, bus);
    (router(state), secrets)
}

fn setup_app_with_auth(token: Option<String>) -> axum::Router {
    let ctx = MockContext {
        rebuild_count: Arc::new(Mutex::new(0)),
        runtime_url: None,
        settings: Arc::new(Mutex::new(AppSettings::default())),
        secrets: Arc::new(Mutex::new(HashMap::new())),
        version: None,
    };
    let bus = events::AdminEventBus::new();
    let state = AdminApiState::with_auth_token(ctx, bus, token);
    router(state)
}

struct StaticVersionPort {
    home: path::PathBuf,
}

#[async_trait::async_trait]
impl VersionPort for StaticVersionPort {
    async fn list_installed(&self) -> Result<Vec<InstalledCoreVersion>, PortError> {
        Ok(vec![InstalledCoreVersion {
            version: "v1.20.0".to_string(),
            path: self
                .home
                .join("versions/v1.20.0/mihomo")
                .to_string_lossy()
                .into_owned(),
            is_default: true,
        }])
    }

    async fn latest(&self, _channel: CoreReleaseChannel) -> Result<CoreRelease, PortError> {
        Ok(CoreRelease {
            version: "v1.20.0".to_string(),
            release_date: "2026-01-01T00:00:00Z".to_string(),
        })
    }

    async fn list_releases(&self, _limit: usize) -> Result<Vec<CoreReleaseSummary>, PortError> {
        Ok(Vec::new())
    }

    async fn install(
        &self,
        _version: String,
        _progress: Arc<dyn VersionProgressSink>,
    ) -> Result<(), PortError> {
        Ok(())
    }

    async fn activate(&self, version: &str) -> Result<(), PortError> {
        create_dir_all(&self.home).map_err(|error| PortError::Io(error.to_string()))?;
        write(
            self.home.join("config.toml"),
            format!("version = \"{version}\"\n"),
        )
        .map_err(|error| PortError::Io(error.to_string()))?;
        Ok(())
    }

    async fn uninstall(&self, _version: &str) -> Result<(), PortError> {
        Ok(())
    }
}

fn setup_app_with_static_version(home: &path::Path) -> axum::Router {
    let ctx = MockContext {
        rebuild_count: Arc::new(Mutex::new(0)),
        runtime_url: None,
        settings: Arc::new(Mutex::new(AppSettings::default())),
        secrets: Arc::new(Mutex::new(HashMap::new())),
        version: Some(VersionApplication::new(Arc::new(StaticVersionPort {
            home: home.to_path_buf(),
        }))),
    };
    let bus = events::AdminEventBus::new();
    router(AdminApiState::new(ctx, bus))
}

#[path = "admin_api_test_cases/activate.rs"]
mod activate;
#[path = "admin_api_test_cases/admin.rs"]
mod admin;
#[path = "admin_api_test_cases/bootstrap.rs"]
mod bootstrap;
#[path = "admin_api_test_cases/delete.rs"]
mod delete;
#[path = "admin_api_test_cases/doctor.rs"]
mod doctor;
#[path = "admin_api_test_cases/download.rs"]
mod download;
#[path = "admin_api_test_cases/extension.rs"]
mod extension;
#[path = "admin_api_test_cases/flush.rs"]
mod flush;
#[path = "admin_api_test_cases/get.rs"]
mod get;
#[path = "admin_api_test_cases/import.rs"]
mod import;
#[path = "admin_api_test_cases/list.rs"]
mod list;
#[path = "admin_api_test_cases/runtime.rs"]
mod runtime;
#[path = "admin_api_test_cases/save.rs"]
mod save;
#[path = "admin_api_test_cases/script.rs"]
mod script;
#[path = "admin_api_test_cases/settings.rs"]
mod settings;
#[path = "admin_api_test_cases/switch.rs"]
mod switch;
#[path = "admin_api_test_cases/update.rs"]
mod update;

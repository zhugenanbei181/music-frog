use crate::admin_api::models::{ImportProfilePayload, SaveProfilePayload, SwitchProfilePayload};
use crate::admin_api::state::{
    AdminApiContext, AdminApiState, AdminOutboundOwner, CoreOwnerAction,
};
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
use infiltrator_application::resource_application::ResourceApplication;
use infiltrator_application::sync_application::SyncApplication;
use infiltrator_application::version_application::VersionApplication;
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::error::Failure;
use infiltrator_contract::routing_intent::{
    RoutingIntent, RoutingIntentSnapshot, RoutingIntentStatus,
};
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::version::{
    CoreRelease, CoreReleaseSummary, EngineReleaseChannel, InstalledCoreVersion,
};
use infiltrator_core::settings_io::{WEBDAV_CREDENTIAL_SERVICE, WEBDAV_PASSWORD_KEY};
use infiltrator_domain::settings::AppSettings;
use infiltrator_platform::TEST_LOCK;
use infiltrator_platform::defaults::DefaultCredentialStore;
use infiltrator_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use infiltrator_ports::error::PortError;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use infiltrator_ports::subscription_source::SubscriptionSource;
use infiltrator_ports::version::{
    VersionActivationRequest, VersionInstallRequest, VersionPort, VersionUninstallRequest,
};
use mihomo_api::client::MihomoClient;
use mihomo_api::outbound::MihomoOutboundAdapter;
use mihomo_api::runtime_configuration::RuntimeScopeResolver;
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
    let dir = home.join("engines/mihomo/versions").join(version);
    create_dir_all(&dir).unwrap();
    let bin = dir.join("mihomo");
    write(&bin, "#!/bin/sh\necho \"Mihomo Meta v1.19.18 test\"\n").unwrap();
    set_permissions(&bin, Permissions::from_mode(0o755)).unwrap();
}

#[derive(Clone)]
struct MockContext {
    core_owner_actions: Arc<Mutex<Vec<CoreOwnerAction>>>,
    runtime_url: Option<String>,
    settings: Arc<Mutex<AppSettings>>,
    /// 内存版 WebDAV 密码库（替代真实 OS keyring，测试零外部依赖）。
    secrets: Arc<Mutex<HashMap<String, String>>>,
    version: Option<VersionApplication>,
}

#[derive(Clone)]
struct TestCoreScopeSource {
    scope: CoreScope,
}

impl CoreScopeSource for TestCoreScopeSource {
    fn scope_observation(&self) -> CoreScopeObservation {
        CoreScopeObservation::Observed(self.scope.clone())
    }
}

fn test_outbound_scope() -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("mihomo").expect("valid engine ID"),
            CoreInstanceId::new("admin-test-instance").expect("valid instance ID"),
            Some(CoreVersion::new("test-version").expect("valid core version")),
        ),
        generation: CoreGeneration::new(1),
        session_token: Some(SessionToken::new(7)),
    }
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

    async fn apply_core_owner_action(&self, action: CoreOwnerAction) -> anyhow::Result<()> {
        self.core_owner_actions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(action);
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
    async fn runtime_gateway(&self) -> anyhow::Result<Arc<dyn RuntimeGateway>> {
        let runtime_url = self
            .runtime_url
            .as_deref()
            .ok_or_else(|| anyhow!("runtime url is not configured"))?;
        let scope = test_outbound_scope();
        let scope_resolver: RuntimeScopeResolver = Arc::new(move || Ok(scope.clone()));
        let client = MihomoClient::new(runtime_url, None)
            .map_err(|e| anyhow!(e.to_string()))?
            .with_configuration_scope_resolver(scope_resolver);
        Ok(Arc::new(client))
    }
    async fn resource_application(&self) -> anyhow::Result<ResourceApplication> {
        let gateway = self.runtime_gateway().await?;
        Ok(ResourceApplication::new(
            Arc::new(TestCoreScopeSource {
                scope: test_outbound_scope(),
            }),
            gateway.resource_port(),
        ))
    }
    async fn outbound_owner(&self) -> anyhow::Result<AdminOutboundOwner> {
        let runtime_url = self
            .runtime_url
            .as_deref()
            .ok_or_else(|| anyhow!("runtime url is not configured"))?;
        let client = MihomoClient::new(runtime_url, None).map_err(|e| anyhow!(e.to_string()))?;
        let scope_source: Arc<dyn CoreScopeSource> = Arc::new(TestCoreScopeSource {
            scope: test_outbound_scope(),
        });
        let port = Arc::new(MihomoOutboundAdapter::new(
            Arc::new(client),
            scope_source.clone(),
        ));
        Ok(AdminOutboundOwner::new(port, scope_source))
    }
    async fn routing_intent_snapshot(
        &self,
    ) -> Result<(Option<CoreScope>, RoutingIntentSnapshot), Failure> {
        Ok((
            Some(test_outbound_scope()),
            RoutingIntentSnapshot::demo_fixture(),
        ))
    }
    async fn apply_routing_intent(
        &self,
        scope: CoreScope,
        intent: RoutingIntent,
    ) -> Result<(Option<CoreScope>, RoutingIntentSnapshot), Failure> {
        if scope != test_outbound_scope() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "stale test scope",
                false,
            ));
        }
        Ok((
            Some(scope),
            RoutingIntentSnapshot {
                current: Some(intent),
                status: RoutingIntentStatus::Ready,
                failure: None,
            },
        ))
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
        core_owner_actions: Arc::new(Mutex::new(Vec::new())),
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

fn setup_app_with_owner_actions() -> (axum::Router, Arc<Mutex<Vec<CoreOwnerAction>>>) {
    let actions = Arc::new(Mutex::new(Vec::new()));
    let ctx = MockContext {
        core_owner_actions: Arc::clone(&actions),
        runtime_url: None,
        settings: Arc::new(Mutex::new(AppSettings::default())),
        secrets: Arc::new(Mutex::new(HashMap::new())),
        version: None,
    };
    let bus = events::AdminEventBus::new();
    let state = AdminApiState::new(ctx, bus);
    (router(state), actions)
}

fn setup_app_with_auth(token: Option<String>) -> axum::Router {
    let ctx = MockContext {
        core_owner_actions: Arc::new(Mutex::new(Vec::new())),
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
    fn engine_id(&self) -> EngineId {
        EngineId::new("mihomo").unwrap()
    }

    async fn list_installed(
        &self,
        _engine_id: &EngineId,
    ) -> Result<Vec<InstalledCoreVersion>, PortError> {
        Ok(vec![InstalledCoreVersion {
            engine_id: self.engine_id(),
            version: "v1.20.0".to_string(),
            path: self
                .home
                .join("engines/mihomo/versions/v1.20.0/mihomo")
                .to_string_lossy()
                .into_owned(),
            is_default: true,
        }])
    }

    async fn latest(&self, channel: EngineReleaseChannel) -> Result<CoreRelease, PortError> {
        if channel.engine_id != self.engine_id() {
            return Err(PortError::Failed("wrong engine context".to_owned()));
        }
        Ok(CoreRelease {
            engine_id: self.engine_id(),
            version: "v1.20.0".to_string(),
            release_date: "2026-01-01T00:00:00Z".to_string(),
        })
    }

    async fn list_releases(
        &self,
        _engine_id: &EngineId,
        _limit: usize,
    ) -> Result<Vec<CoreReleaseSummary>, PortError> {
        Ok(Vec::new())
    }

    async fn install(&self, _request: VersionInstallRequest) -> Result<(), PortError> {
        Ok(())
    }

    async fn activate(&self, request: VersionActivationRequest) -> Result<(), PortError> {
        create_dir_all(&self.home).map_err(|error| PortError::Io(error.to_string()))?;
        write(
            self.home.join("config.toml"),
            format!("version = \"{}\"\n", request.version),
        )
        .map_err(|error| PortError::Io(error.to_string()))?;
        Ok(())
    }

    async fn uninstall(&self, _request: VersionUninstallRequest) -> Result<(), PortError> {
        Ok(())
    }
}

fn setup_app_with_static_version(home: &path::Path) -> axum::Router {
    let ctx = MockContext {
        core_owner_actions: Arc::new(Mutex::new(Vec::new())),
        runtime_url: None,
        settings: Arc::new(Mutex::new(AppSettings::default())),
        secrets: Arc::new(Mutex::new(HashMap::new())),
        version: Some(
            VersionApplication::new(
                EngineId::new("mihomo").unwrap(),
                Arc::new(StaticVersionPort {
                    home: home.to_path_buf(),
                }),
            )
            .unwrap(),
        ),
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
#[path = "admin_api_test_cases/webhook.rs"]
mod webhook;

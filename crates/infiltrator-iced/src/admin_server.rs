//! Admin Web UI server integration (mirrors the legacy Tauri embedding).
//!
//! The server itself lives in `infiltrator-admin` ([`infiltrator_admin::start_admin_server`])
//! and always binds `127.0.0.1`. This module owns the iced-side glue:
//!
//! * [`resolve_admin_dir`] — WebUI asset path resolution (env override, dev
//!   checkout fallback, exe-relative packaged candidates).
//! * [`plan_admin_server_action`] — the pure start/stop/restart state machine,
//!   unit-tested headlessly.
//! * [`AdminServerManager`] — the live [`AdminServerHandle`] bookkeeping shared
//!   between the main thread (update loop) and the tokio tasks that start the
//!   server.
//! * [`AdminSharedRuntime`] — thread-safe snapshot of the mihomo runtime plus a
//!   command channel back into the iced update loop (same pattern as the tray
//!   event channel), so [`IcedAdminContext`] can serve the admin REST API from
//!   any tokio worker without ever touching the `!Send`-adjacent app state.
//!
//! Red lines: the server never binds anything but loopback, and unit tests
//! never start it (`apply_admin_server_lifecycle` is inert under `cfg(test)`).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use infiltrator_admin::admin_api::state::AdminApiContext;
use infiltrator_admin::servers::AdminServerHandle;
use infiltrator_core::settings::AdminServerConfig;
use iced::advanced::subscription::{EventStream, Hasher, Recipe, from_recipe};
use iced::futures::stream::BoxStream;
use iced::{Subscription, Task, stream};
use mihomo_api::client::MihomoClient;
use mihomo_version::manager::VersionManager;

use crate::locales::Localizer;
use crate::state::AppState;
use crate::types::{InfiltratorError, Message, ToastStatus};

/// Same default the legacy Tauri client passes to `start_admin_server`.
pub const ADMIN_DEFAULT_PORT: u16 = 25210;

/// Env override shared with the Tauri client so dev workflows stay identical.
pub const ADMIN_DIR_ENV: &str = "METACUBEXD_ADMIN_DIR";

/// Resolve the config-manager-ui static assets directory.
///
/// Order mirrors `src-tauri/src/paths.rs::resolve_admin_dir`:
/// 1. `METACUBEXD_ADMIN_DIR` env override,
/// 2. development checkout `webui/config-manager-ui/dist` (then the root, for
///    un-built checkouts),
/// 3. packaged resources next to the executable.
pub fn resolve_admin_dir() -> anyhow::Result<PathBuf> {
    // The crate lives one level deeper than src-tauri, hence `../..`.
    let dev_admin =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../webui/config-manager-ui");
    let dev_admin_dist = dev_admin.join("dist");

    if let Ok(custom) = std::env::var(ADMIN_DIR_ENV) {
        let path = PathBuf::from(custom);
        if path.exists() {
            return Ok(path);
        }
    }
    if dev_admin_dist.exists() {
        return Ok(dev_admin_dist);
    }
    if dev_admin.exists() {
        return Ok(dev_admin);
    }

    // Packaged layout: cargo-packager copies the resource entry (basename kept)
    // next to the binary; probe the plausible spellings.
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        for candidate in [
            dir.join("config-manager-ui").join("dist"),
            dir.join("config-manager-ui"),
            dir.join("dist"),
        ] {
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    Err(anyhow!(
        "未找到配置管理静态资源，请构建 webui/config-manager-ui/ 目录"
    ))
}

/// Whether the web admin feature is switched on in the settings snapshot.
pub fn admin_enabled(config: &AdminServerConfig) -> bool {
    config.enabled
}

/// The transition the lifecycle glue must execute right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminServerIntent {
    /// Desired state already matches reality.
    None,
    /// No server is running and one should be started.
    Start,
    /// A running server must be stopped (feature disabled).
    Stop,
    /// A running server must be stopped and restarted for the new config.
    Restart,
}

/// Pure decision function. `running` says whether a server is live *or* a
/// start attempt is already pending (the spawned start task has not finished
/// yet), and `started_for` is the config snapshot that produced it (used to
/// detect a no-op even when the OS-assigned port drifted from the configured
/// one).
pub fn plan_admin_server_action(
    running: bool,
    started_for: Option<AdminServerConfig>,
    desired: AdminServerConfig,
) -> AdminServerIntent {
    match (running, desired.enabled) {
        (false, false) => AdminServerIntent::None,
        (true, false) => AdminServerIntent::Stop,
        (false, true) => AdminServerIntent::Start,
        (true, true) => {
            if started_for == Some(desired) {
                AdminServerIntent::None
            } else {
                AdminServerIntent::Restart
            }
        }
    }
}

/// Commands sent from the admin REST context (tokio workers) back into the
/// iced update loop. Drained by a subscription, exactly like tray events.
///
/// `Clone` mirrors [`Message`] (the loop may replay messages); the one-shot
/// dialog reply channel is therefore shared behind an `Arc` — exactly one
/// clone ever sends on it. `Debug` is hand-written because the runtime handle
/// is not `Debug` and only its presence matters in logs.
#[derive(Clone)]
pub enum AdminHostCommand {
    /// A context-driven `rebuild_runtime` finished; resync the UI runtime.
    RuntimeResynced(Result<Arc<infiltrator_desktop::runtime::MihomoRuntime>, String>),
    /// A context-driven `stop_runtime` finished; clear the UI runtime.
    RuntimeStopped,
    /// Surface a toast (e.g. subscription update notifications).
    Toast(String, ToastStatus),
    /// The WebUI saved settings; reload them from disk.
    SettingsSavedExternally,
    /// Core version data changed; refresh the kernel list.
    CoreVersionsChanged,
    /// Open a native file dialog on the main thread and report the pick back.
    PickEditorPath(Arc<tokio::sync::oneshot::Sender<Option<String>>>),
}

impl std::fmt::Debug for AdminHostCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RuntimeResynced(result) => f
                .debug_tuple("RuntimeResynced")
                .field(&result.is_ok())
                .finish(),
            Self::RuntimeStopped => f.write_str("RuntimeStopped"),
            Self::Toast(content, status) => f
                .debug_tuple("Toast")
                .field(content)
                .field(status)
                .finish(),
            Self::SettingsSavedExternally => f.write_str("SettingsSavedExternally"),
            Self::CoreVersionsChanged => f.write_str("CoreVersionsChanged"),
            Self::PickEditorPath(_) => f.debug_tuple("PickEditorPath").field(&"reply").finish(),
        }
    }
}

/// Receiver side of the admin command channel, shared with the subscription.
pub type SharedAdminCommandReceiver = Arc<Mutex<std::sync::mpsc::Receiver<AdminHostCommand>>>;

/// Shared, `Send + Sync` snapshot of what the admin REST context needs from
/// the app: the live mihomo runtime, the event bus and the command channel.
#[derive(Clone)]
pub struct AdminSharedRuntime {
    inner: Arc<SharedInner>,
}

struct SharedInner {
    runtime: Mutex<Option<Arc<infiltrator_desktop::runtime::MihomoRuntime>>>,
    commands: std::sync::mpsc::Sender<AdminHostCommand>,
    events: infiltrator_admin::admin_api::events::AdminEventBus,
}

impl AdminSharedRuntime {
    pub fn new(
        events: infiltrator_admin::admin_api::events::AdminEventBus,
        commands: std::sync::mpsc::Sender<AdminHostCommand>,
    ) -> Self {
        Self {
            inner: Arc::new(SharedInner {
                runtime: Mutex::new(None),
                commands,
                events,
            }),
        }
    }

    pub fn event_bus(&self) -> infiltrator_admin::admin_api::events::AdminEventBus {
        self.inner.events.clone()
    }

    pub fn set_runtime(&self, runtime: Option<Arc<infiltrator_desktop::runtime::MihomoRuntime>>) {
        *self
            .inner
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = runtime;
    }

    pub fn take_runtime(&self) -> Option<Arc<infiltrator_desktop::runtime::MihomoRuntime>> {
        self.inner
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }

    pub fn runtime(&self) -> Option<Arc<infiltrator_desktop::runtime::MihomoRuntime>> {
        self.inner
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn send(&self, command: AdminHostCommand) {
        // A closed receiver just means the app is shutting down.
        let _ = self.inner.commands.send(command);
    }
}

/// Bookkeeping for the live [`AdminServerHandle`]. Cloned freely; all state
/// sits behind one mutex so the main thread and start tasks agree.
#[derive(Clone)]
pub struct AdminServerManager {
    inner: Arc<ManagerInner>,
}

struct ManagerInner {
    handle: Mutex<Option<AdminServerHandle>>,
    started_for: Mutex<Option<AdminServerConfig>>,
    events: infiltrator_admin::admin_api::events::AdminEventBus,
}

impl Default for AdminServerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AdminServerManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ManagerInner {
                handle: Mutex::new(None),
                started_for: Mutex::new(None),
                events: infiltrator_admin::admin_api::events::AdminEventBus::new(),
            }),
        }
    }

    /// The event bus feeding the server's SSE stream; stable across restarts.
    pub fn event_bus(&self) -> infiltrator_admin::admin_api::events::AdminEventBus {
        self.inner.events.clone()
    }

    /// The bound URL (e.g. `http://127.0.0.1:25210/admin/`) when running.
    pub fn url(&self) -> Option<String> {
        self.inner
            .handle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .map(|handle| handle.url.clone())
    }

    pub fn is_running(&self) -> bool {
        self.inner
            .handle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_some()
    }

    /// The config snapshot the current (or pending) start was requested for.
    pub fn started_config(&self) -> Option<AdminServerConfig> {
        self.inner
            .started_for
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Decide the transition for `desired` and record it. Must be called on
    /// the main thread before executing the returned intent.
    pub fn begin_transition(&self, desired: AdminServerConfig) -> AdminServerIntent {
        let mut handle = self
            .inner
            .handle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut started_for = self
            .inner
            .started_for
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // A pending start counts as live: toggling off while the start task
        // is still in flight must still produce a Stop (and clear the
        // bookkeeping so the late-completing start loses the race).
        let intent = plan_admin_server_action(
            handle.is_some() || started_for.is_some(),
            started_for.clone(),
            desired.clone(),
        );
        match intent {
            AdminServerIntent::None => {}
            AdminServerIntent::Stop => {
                if let Some(server) = handle.take() {
                    server.stop();
                }
                *started_for = None;
            }
            AdminServerIntent::Start | AdminServerIntent::Restart => {
                if let Some(server) = handle.take() {
                    server.stop();
                }
                *started_for = Some(desired);
            }
        }
        intent
    }

    /// Complete a start spawned after [`Self::begin_transition`]. A start that
    /// lost a race against a newer transition is stopped immediately instead
    /// of being adopted.
    pub fn finish_transition(
        &self,
        desired: AdminServerConfig,
        result: anyhow::Result<AdminServerHandle>,
    ) {
        let mut handle = self
            .inner
            .handle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut started_for = self
            .inner
            .started_for
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match result {
            Ok(server) => {
                if *started_for == Some(desired) {
                    *handle = Some(server);
                } else {
                    server.stop();
                }
            }
            Err(_) => {
                if *started_for == Some(desired) {
                    *started_for = None;
                }
            }
        }
    }

    /// Best-effort shutdown (app exit). Safe to call multiple times.
    pub fn shutdown(&self) {
        if let Some(server) = self
            .inner
            .handle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
        {
            server.stop();
        }
        *self
            .inner
            .started_for
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    }
}

/// The [`AdminApiContext`] implementation backing the admin REST API with the
/// iced app's runtime. Like `TauriAdminContext`, but everything it touches is
/// shared state (`Arc`/mutex/channel) instead of the Tauri app handle.
#[derive(Clone)]
pub struct IcedAdminContext {
    shared: AdminSharedRuntime,
}

impl IcedAdminContext {
    pub fn new(shared: AdminSharedRuntime) -> Self {
        Self { shared }
    }

    async fn load_settings(&self) -> infiltrator_core::settings::AppSettings {
        load_settings_from_disk()
            .await
            .unwrap_or_default()
    }

    async fn update_settings(
        &self,
        apply: impl FnOnce(&mut infiltrator_core::settings::AppSettings),
    ) -> anyhow::Result<()> {
        let mut settings = load_settings_from_disk().await?;
        apply(&mut settings);
        save_settings_to_disk(&settings).await?;
        Ok(())
    }
}

async fn load_settings_from_disk()
-> anyhow::Result<infiltrator_core::settings::AppSettings> {
    let base_dir = mihomo_platform::paths::get_home_dir().map_err(|e| anyhow!(e.to_string()))?;
    let path = infiltrator_core::settings::settings_path(&base_dir)?;
    infiltrator_core::settings::load_settings(&path).await
}

async fn save_settings_to_disk(
    settings: &infiltrator_core::settings::AppSettings,
) -> anyhow::Result<()> {
    let base_dir = mihomo_platform::paths::get_home_dir().map_err(|e| anyhow!(e.to_string()))?;
    let path = infiltrator_core::settings::settings_path(&base_dir)?;
    infiltrator_core::settings::save_settings(&path, settings)
            .await
}

#[async_trait::async_trait]
impl AdminApiContext for IcedAdminContext {
    async fn rebuild_runtime(&self) -> anyhow::Result<()> {
        if let Some(runtime) = self.shared.take_runtime() {
            let _ = runtime.shutdown().await;
        }
        let vm = VersionManager::new().map_err(|e| anyhow!(e.to_string()))?;
        let data_dir = mihomo_platform::paths::get_home_dir().map_err(|e| anyhow!(e.to_string()))?;
        let rebuilt = Arc::new(
            infiltrator_desktop::runtime::MihomoRuntime::bootstrap(&vm, true, &[], &data_dir).await?,
        );
        self.shared.set_runtime(Some(rebuilt.clone()));
        self.shared.send(AdminHostCommand::RuntimeResynced(Ok(rebuilt)));
        Ok(())
    }

    /// Session-level restart: same core process manager, new generation, and
    /// a readiness wait — no shutdown/bootstrap cycle. Falls back to a full
    /// rebuild when no live runtime exists.
    async fn restart_core(&self) -> anyhow::Result<()> {
        let Some(runtime) = self.shared.runtime() else {
            return self.rebuild_runtime().await;
        };
        let session = runtime.session();
        let generation = session.restart().await.map_err(|e| anyhow!(e.to_string()))?;
        session
            .wait_for_ready(generation, infiltrator_core::session::READINESS_TIMEOUT)
            .await
            .map_err(|e| anyhow!("core restart did not become ready: {e}"))?;
        self.shared.send(AdminHostCommand::RuntimeResynced(Ok(runtime)));
        Ok(())
    }

    async fn set_use_bundled_core(&self, enabled: bool) {
        if let Err(err) = self
            .update_settings(|settings| settings.use_bundled_core = enabled)
            .await
        {
            log::warn!("failed to persist use_bundled_core: {err:#}");
        }
    }

    async fn refresh_core_version_info(&self) {
        self.shared.send(AdminHostCommand::CoreVersionsChanged);
    }

    async fn latest_stable_core(&self) -> anyhow::Result<(String, String)> {
        let latest =
            mihomo_version::channel::fetch_latest(mihomo_version::channel::Channel::Stable).await?;
        Ok((latest.version, latest.release_date))
    }

    async fn notify_subscription_update(
        &self,
        profile: String,
        success: bool,
        message: Option<String>,
    ) {
        let status = if success {
            ToastStatus::Success
        } else {
            ToastStatus::Error
        };
        let text = match message {
            Some(detail) => format!("订阅更新 {profile}: {detail}"),
            None => format!("订阅更新 {profile}"),
        };
        self.shared.send(AdminHostCommand::Toast(text, status));
    }

    async fn editor_path(&self) -> Option<String> {
        self.load_settings().await.editor_path
    }

    async fn set_editor_path(&self, path: Option<String>) {
        if let Err(err) = self
            .update_settings(|settings| settings.editor_path = path)
            .await
        {
            log::warn!("failed to persist editor path: {err:#}");
        }
    }

    async fn pick_editor_path(&self) -> Option<String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.shared.send(AdminHostCommand::PickEditorPath(Arc::new(tx)));
        // The dialog runs on the main thread; give the user ample time.
        tokio::time::timeout(std::time::Duration::from_secs(300), rx)
            .await
            .ok()?
            .ok()?
    }

    async fn open_profile_in_editor(&self, profile_name: &str) -> anyhow::Result<()> {
        let editor_path = self.editor_path().await;
        infiltrator_desktop::editor::open_profile_in_editor(editor_path, profile_name).await
    }

    async fn get_app_settings(&self) -> infiltrator_core::settings::AppSettings {
        self.load_settings().await
    }

    async fn save_app_settings(
        &self,
        settings: infiltrator_core::settings::AppSettings,
    ) -> anyhow::Result<()> {
        save_settings_to_disk(&settings).await?;
        self.shared
            .event_bus()
            .publish(infiltrator_admin::admin_api::events::AdminEvent::new(
                infiltrator_admin::admin_api::events::EVENT_SETTINGS_CHANGED,
            ));
        self.shared.send(AdminHostCommand::SettingsSavedExternally);
        Ok(())
    }

    async fn runtime_running(&self) -> bool {
        match self.shared.runtime() {
            Some(runtime) => runtime.is_running().await,
            None => false,
        }
    }

    async fn runtime_controller_url(&self) -> Option<String> {
        self.shared
            .runtime()
            .map(|runtime| runtime.controller_url.clone())
    }

    async fn stop_runtime(&self) -> anyhow::Result<()> {
        if let Some(runtime) = self.shared.take_runtime() {
            let _ = runtime.shutdown().await;
        }
        if infiltrator_desktop::proxy::read_system_proxy_state()
            .map(|state| state.enabled)
            .unwrap_or(false)
        {
            let _ = infiltrator_desktop::proxy::apply_system_proxy(None);
        }
        self.shared.send(AdminHostCommand::RuntimeStopped);
        Ok(())
    }

    async fn runtime_client(&self) -> anyhow::Result<MihomoClient> {
        let runtime = self
            .shared
            .runtime()
            .ok_or_else(|| anyhow!("内核未在运行"))?;
        Ok(runtime.client())
    }

    async fn system_proxy_enabled(&self) -> bool {
        infiltrator_desktop::proxy::read_system_proxy_state()
            .map(|state| state.enabled)
            .unwrap_or(false)
    }

    async fn set_system_proxy_enabled(&self, enabled: bool) -> anyhow::Result<()> {
        if enabled {
            let runtime = self
                .shared
                .runtime()
                .ok_or_else(|| anyhow!("内核未在运行"))?;
            let endpoint = runtime
                .http_proxy_endpoint()
                .await?
                .ok_or_else(|| anyhow!("当前配置中未配置代理端口（port/mixed-port）"))?;
            infiltrator_desktop::proxy::apply_system_proxy(Some(&endpoint))?;
        } else {
            infiltrator_desktop::proxy::apply_system_proxy(None)?;
        }
        Ok(())
    }

    async fn autostart_enabled(&self) -> bool {
        crate::autostart::is_autostart_enabled(crate::AUTOSTART_REG_NAME)
    }

    async fn set_autostart_enabled(&self, enabled: bool) -> anyhow::Result<()> {
        Ok(crate::autostart::set_autostart_enabled(
            crate::AUTOSTART_REG_NAME,
            enabled,
        )?)
    }

    fn supports_system_proxy_control(&self) -> bool {
        cfg!(target_os = "windows")
    }

    fn supports_autostart_control(&self) -> bool {
        cfg!(target_os = "windows")
    }
}

/// Spawn a fresh admin server for `desired`. Returns a task whose completion
/// message records the handle (or the failure) on the manager.
#[cfg_attr(test, allow(dead_code))] // tests never start the real server
fn spawn_admin_server_start(
    manager: AdminServerManager,
    ctx: IcedAdminContext,
    desired: AdminServerConfig,
) -> Task<Message> {
    Task::perform(
        async move {
            let attempt = async {
                let dir = resolve_admin_dir()
                    .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                // preferred_port=None mirrors src-tauri: scan upward from the
                // configured port when it is occupied instead of failing hard.
                let handle = infiltrator_admin::servers::start_admin_server(
                    dir,
                    ctx,
                    None,
                    desired.port,
                    manager.event_bus(),
                )
                .await
                .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                Ok(handle)
            }
            .await;

            match attempt {
                Ok(handle) => {
                    let url = handle.url.clone();
                    manager.finish_transition(desired, Ok(handle));
                    Ok(url)
                }
                Err(e) => {
                    manager.finish_transition(desired, Err(anyhow!("start failed")));
                    Err(e)
                }
            }
        },
        Message::AdminServerStarted,
    )
}

/// Subscription draining the admin command channel into the update loop.
pub(crate) fn admin_commands_subscription(
    receiver: &SharedAdminCommandReceiver,
) -> Subscription<Message> {
    from_recipe(AdminCommandsRecipe {
        receiver: Arc::clone(receiver),
    })
}

struct AdminCommandsRecipe {
    receiver: SharedAdminCommandReceiver,
}

impl Recipe for AdminCommandsRecipe {
    type Output = Message;

    fn hash(&self, state: &mut Hasher) {
        use std::hash::Hash;
        "infiltrator-admin-commands".hash(state);
    }

    fn stream(self: Box<Self>, _input: EventStream) -> BoxStream<'static, Message> {
        let receiver = self.receiver;
        let channel = stream::channel(
            100,
            move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                loop {
                    let commands = {
                        let rx = receiver
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        let mut batch = Vec::new();
                        while let Ok(command) = rx.try_recv() {
                            batch.push(command);
                        }
                        batch
                    };
                    for command in commands {
                        let _ = output.try_send(Message::AdminHostCommand(command));
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
            },
        );
        Box::pin(channel)
    }
}

impl AppState {
    /// Keep `AppState.runtime` and the admin context's shared snapshot in
    /// sync. Every runtime mutation on the main thread must go through here
    /// (or [`Self::take_app_runtime`]) so the REST context sees the live one.
    pub(crate) fn sync_runtime_slot(
        &mut self,
        runtime: Option<std::sync::Arc<infiltrator_desktop::runtime::MihomoRuntime>>,
    ) {
        self.runtime = runtime;
        self.admin_shared.set_runtime(self.runtime.clone());
    }

    /// Take the runtime for shutdown/teardown, clearing the shared snapshot.
    pub(crate) fn take_app_runtime(
        &mut self,
    ) -> Option<std::sync::Arc<infiltrator_desktop::runtime::MihomoRuntime>> {
        let taken = self.runtime.take();
        self.admin_shared.set_runtime(None);
        taken
    }

    /// Re-evaluate the admin server lifecycle against the desired settings
    /// snapshot. Called after settings load and after every admin settings
    /// change. Inert under `cfg(test)` (tests never start a real server).
    pub fn apply_admin_server_lifecycle(&mut self) -> Task<Message> {
        let desired = AdminServerConfig {
            enabled: self.admin_enabled,
            port: self.admin_port,
        };
        match self.admin_server.begin_transition(desired.clone()) {
            AdminServerIntent::None => Task::none(),
            AdminServerIntent::Stop => {
                self.refresh_tray();
                Task::none()
            }
            AdminServerIntent::Start | AdminServerIntent::Restart => {
                #[cfg(test)]
                {
                    let _ = &desired;
                    Task::none()
                }
                #[cfg(not(test))]
                {
                    self.refresh_tray();
                    let ctx = IcedAdminContext::new(self.admin_shared.clone());
                    spawn_admin_server_start(self.admin_server.clone(), ctx, desired)
                }
            }
        }
    }

    /// Handle a command that travelled from the admin REST context back into
    /// the update loop.
    pub fn handle_admin_host_command(&mut self, command: AdminHostCommand) -> Task<Message> {
        match command {
            AdminHostCommand::RuntimeResynced(result) => match result {
                Ok(runtime) => {
                    self.status = crate::types::RuntimeStatus::Running;
                    self.sync_runtime_slot(Some(runtime));
                    self.refresh_tray();
                    Task::batch(vec![
                        Task::done(Message::FetchRuntimeConfig),
                        Task::done(Message::LoadProxies),
                        Task::done(Message::RefreshRuntimeNow),
                    ])
                }
                Err(e) => {
                    self.status = crate::types::RuntimeStatus::Error(InfiltratorError::Config(
                        e.clone(),
                    ));
                    self.set_error(InfiltratorError::Config(e));
                    Task::none()
                }
            },
            AdminHostCommand::RuntimeStopped => {
                self.sync_runtime_slot(None);
                Task::done(Message::ProxyStopped)
            }
            AdminHostCommand::Toast(content, status) => {
                Task::done(Message::ShowToast(content, status))
            }
            AdminHostCommand::SettingsSavedExternally => {
                // Reload from disk and re-apply, but skip the WebDAV
                // sync-on-startup side effect the startup path performs.
                Task::perform(
                    async {
                        load_settings_from_disk()
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))
                    },
                    Message::ExternalSettingsLoaded,
                )
            }
            AdminHostCommand::CoreVersionsChanged => Task::done(Message::LoadKernels),
            AdminHostCommand::PickEditorPath(tx) => Task::perform(
                async {
                    tokio::task::spawn_blocking(|| {
                        rfd::FileDialog::new().pick_file()
                    })
                    .await
                    .ok()
                    .flatten()
                },
                move |path: Option<PathBuf>| {
                    // Exactly one clone of the command is ever handled; the
                    // unwraps failure path (a stale duplicate) just drops the
                    // reply and the requester times out.
                    if let Ok(tx) = Arc::try_unwrap(tx) {
                        let _ = tx.send(path.map(|p| p.to_string_lossy().into_owned()));
                    }
                    Message::Noop
                },
            ),
        }
    }

    /// Open the admin WebUI in the system browser (tray entry / settings
    /// button). Graceful failure when the server is off.
    pub fn open_web_admin(&self) -> Task<Message> {
        match self.admin_server.url() {
            Some(url) => Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || webbrowser::open(&url))
                        .await
                        .map_err(|e| InfiltratorError::Internal(e.to_string()))?
                        .map_err(|e| InfiltratorError::Internal(e.to_string()))
                },
                |result| match result {
                    Ok(()) => Message::Noop,
                    Err(e) => Message::ShowToast(e.to_string(), ToastStatus::Error),
                },
            ),
            None => Task::done(Message::ShowToast(
                crate::locales::Lang(&self.lang).tr("settings_admin_not_running").into_owned(),
                ToastStatus::Warning,
            )),
        }
    }
}

#[cfg(test)]
#[path = "../tests/gui/admin_server_tests.rs"]
mod tests;

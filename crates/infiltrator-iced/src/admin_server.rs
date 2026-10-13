//! Admin Web UI server integration (mirrors the legacy Tauri embedding).
//!
//! The server itself lives in `infiltrator-admin` ([`infiltrator_admin::start_admin_server`])
//! and always binds `127.0.0.1`. This module owns the iced-side glue:
//!
//! * (0.20) WebUI static hosting retired; the server is API-only — former
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

use crate::host::desktop::{read_system_proxy_state, system_proxy_port};
use crate::notify::NotifyUrgency;
use crate::state::AppState;
use crate::state::runtime::TunServiceInstallState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use crate::version_application::application;
use anyhow::anyhow;
use iced::advanced::subscription::{EventStream, Hasher, Recipe, from_recipe};
use iced::futures::channel::mpsc::Sender;
use iced::futures::channel::oneshot;
use iced::futures::stream::BoxStream;
use iced::{Subscription, Task, stream};
use infiltrator_admin::admin_api::events::{AdminEvent, AdminEventBus, EVENT_SETTINGS_CHANGED};
use infiltrator_admin::admin_api::state::{AdminApiContext, AdminOutboundOwner, CoreOwnerAction};
use infiltrator_admin::servers::{AdminServerHandle, start_admin_server};
use infiltrator_application::cache_application::CacheApplication;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::connection_application::ConnectionApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_application::network_application::NetworkApplication;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::profile_reset_application::ProfileResetApplication;
use infiltrator_application::resource_application::ResourceApplication;
use infiltrator_application::runtime_query_application::RuntimeQueryApplication;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_application::sync_application::SyncApplication;
use infiltrator_application::system_proxy_application::SystemProxyApplication;
use infiltrator_application::version_application::VersionApplication;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_identity::{CoreCommandRequest, CoreScope};
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::routing_intent::{
    RoutingIntent, RoutingIntentSnapshot, RoutingIntentStatus,
};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_desktop::editor::open_profile_in_editor;
use infiltrator_desktop::product::{DesktopProductSessionAction, DesktopProductSessionHandle};
use infiltrator_desktop::storage::{
    doctor, fake_ip_cache, profile_controller_url, profile_reset, profile_store, public_ip_probe,
    save_webdav_password, settings_store, subscription_source, sync, webdav_password,
};
use infiltrator_domain::settings::{AdminServerConfig, AppSettings};
use infiltrator_ports::host_runtime::HostRuntime;
use infiltrator_ports::runtime_http_proxy_endpoint::RuntimeHttpProxyEndpoint;
use infiltrator_ports::subscription_source::SubscriptionSource;
use infiltrator_shared::autostart::{is_autostart_enabled, set_autostart_enabled};
use std::fmt::{Debug, Formatter};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::{fmt, time};
use tokio::task::spawn_blocking;
use tokio::time::{sleep, timeout};

mod context;

/// Same default the legacy Tauri client passes to `start_admin_server`.
pub const ADMIN_DEFAULT_PORT: u16 = 25210;

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
    /// A Core owner action finished; reread all handles from the one product session.
    ProductSessionChanged,
    /// Surface a toast (e.g. subscription update notifications).
    Toast(String, ToastStatus),
    /// Surface an OS system notification (0.20): the scheduler can fire while
    /// the window is hidden, so toasts alone are not enough. `title` carries a
    /// `notify_*` locale key (the consumer resolves it via `Lang::tr`, which
    /// passes unknown keys through unchanged).
    SystemNotify {
        title: String,
        body: String,
        urgency: NotifyUrgency,
    },
    /// The WebUI saved settings; reload them from disk.
    SettingsSavedExternally,
    /// Core version data changed; refresh the kernel list.
    CoreVersionsChanged,
    /// Open a native file dialog on the main thread and report the pick back.
    PickEditorPath(Arc<oneshot::Sender<Option<String>>>),
}

impl Debug for AdminHostCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProductSessionChanged => f.write_str("ProductSessionChanged"),
            Self::Toast(content, status) => {
                f.debug_tuple("Toast").field(content).field(status).finish()
            }
            Self::SystemNotify {
                title,
                body,
                urgency,
            } => f
                .debug_struct("SystemNotify")
                .field("title", title)
                .field("body", body)
                .field("urgency", urgency)
                .finish(),
            Self::SettingsSavedExternally => f.write_str("SettingsSavedExternally"),
            Self::CoreVersionsChanged => f.write_str("CoreVersionsChanged"),
            Self::PickEditorPath(_) => f.debug_tuple("PickEditorPath").field(&"reply").finish(),
        }
    }
}

/// Receiver side of the admin command channel, shared with the subscription.
pub type SharedAdminCommandReceiver = Arc<Mutex<mpsc::Receiver<AdminHostCommand>>>;

/// Shared, `Send + Sync` snapshot of what the admin REST context needs from
/// the app: the live mihomo runtime, the event bus and the command channel.
#[derive(Clone)]
pub struct AdminSharedRuntime {
    inner: Arc<SharedInner>,
}

enum AdminProductSessionState {
    Unavailable,
    Bound(DesktopProductSessionHandle),
}

struct SharedInner {
    product_session: Mutex<AdminProductSessionState>,
    commands: mpsc::Sender<AdminHostCommand>,
    events: AdminEventBus,
}

impl AdminSharedRuntime {
    pub fn new(events: AdminEventBus, commands: mpsc::Sender<AdminHostCommand>) -> Self {
        Self {
            inner: Arc::new(SharedInner {
                product_session: Mutex::new(AdminProductSessionState::Unavailable),
                commands,
                events,
            }),
        }
    }

    pub fn event_bus(&self) -> AdminEventBus {
        self.inner.events.clone()
    }

    pub fn set_product_session(&self, session: DesktopProductSessionHandle) {
        *self
            .inner
            .product_session
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) =
            AdminProductSessionState::Bound(session);
    }

    fn product_session(&self) -> Option<DesktopProductSessionHandle> {
        match &*self
            .inner
            .product_session
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
        {
            AdminProductSessionState::Unavailable => None,
            AdminProductSessionState::Bound(session) => Some(session.clone()),
        }
    }

    fn application(&self) -> Option<CoreApplication> {
        self.product_session()?
            .binding()
            .ok()
            .map(|binding| binding.application.as_ref().clone())
    }

    pub fn runtime(&self) -> Option<Arc<dyn HostRuntime>> {
        self.product_session()?
            .binding()
            .ok()
            .map(|binding| binding.host_runtime)
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
    events: AdminEventBus,
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
                events: AdminEventBus::new(),
            }),
        }
    }

    /// The event bus feeding the server's SSE stream; stable across restarts.
    pub fn event_bus(&self) -> AdminEventBus {
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

async fn settings_application() -> anyhow::Result<SettingsApplication> {
    let store = settings_store().await?;
    Ok(SettingsApplication::new(store))
}

async fn load_settings_from_disk() -> anyhow::Result<AppSettings> {
    settings_application()
        .await?
        .load()
        .await
        .map_err(|failure| anyhow!(failure.message))
}

/// 同 [`load_settings_from_disk`]，但把 keyring 里的 WebDAV 密码水合进
/// `webdav.password` 内存镜像，供 UI 域（`webdav_pass`）回填。仅 UI 展示/
/// 重新应用路径使用；REST 读取路径保持不触碰 keyring。
async fn load_settings_hydrated_from_disk() -> anyhow::Result<AppSettings> {
    settings_application()
        .await?
        .load_hydrated()
        .await
        .map_err(|failure| anyhow!(failure.message))
}

async fn save_settings_to_disk(settings: &AppSettings) -> anyhow::Result<()> {
    settings_application()
        .await?
        .save(settings)
        .await
        .map_err(|failure| anyhow!(failure.message))
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
                // WebUI retired in 0.20: the embedded admin server runs
                // API-only (Doctor loopback client + future embedders).
                let dir = None;
                // preferred_port=None: scan upward from the configured port
                // when it is occupied instead of failing hard.
                let handle = start_admin_server(dir, ctx, None, desired.port, manager.event_bus())
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
        let channel = stream::channel(100, move |mut output: Sender<Message>| async move {
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
                sleep(time::Duration::from_millis(20)).await;
            }
        });
        Box::pin(channel)
    }
}

impl AppState {
    /// Keep `AppState.runtime` and the admin context's shared snapshot in
    /// sync. Every runtime mutation on the main thread must go through here
    /// (or [`Self::take_app_runtime`]) so the REST context sees the live one.
    pub(crate) fn sync_runtime_slot(&mut self, runtime: Option<Arc<dyn HostRuntime>>) {
        let runtime_changed = match (&self.runtime.runtime, &runtime) {
            (Some(previous), Some(next)) => !Arc::ptr_eq(previous, next),
            (None, None) => false,
            _ => true,
        };
        if runtime_changed {
            // Invalidate every response started against the previous core;
            // this also protects RuntimePatchResult from a late controller
            // response after stop/restart.
            self.runtime.runtime_patch_token = self.runtime.runtime_patch_token.wrapping_add(1);
            self.runtime.pending_runtime_patch = None;
            self.runtime.system_proxy_pending = false;
        }
        self.runtime.runtime_generation = runtime
            .as_ref()
            .map(|runtime| runtime.lifecycle_port().generation())
            .unwrap_or_else(|| self.runtime.runtime_generation.saturating_add(1));
        self.runtime.tun_service_status =
            runtime.as_ref().map(|runtime| runtime.tun_service_status());
        if runtime_changed
            && let Some(runtime) = runtime.as_ref()
            && let Some(proxy) = runtime.system_proxy_port()
        {
            self.runtime.system_proxy_port = Some(proxy.clone());
            self.runtime.system_proxy_application = Some(SystemProxyApplication::new(proxy));
        }
        if runtime_changed {
            // DUAL-11-06/07: keep the kernel's local rule-provider files
            // reachable for exactly as long as the runtime that owns them.
            self.runtime.rule_provider_cache_port = runtime
                .as_ref()
                .and_then(|runtime| runtime.rule_provider_cache_port());
        }
        self.runtime.runtime = runtime;
    }

    /// Take the runtime for shutdown/teardown, clearing the shared snapshot.
    pub(crate) fn take_app_runtime(&mut self) -> Option<Arc<dyn HostRuntime>> {
        let taken = self.runtime.runtime.take();
        self.runtime.runtime_generation = self.runtime.runtime_generation.saturating_add(1);
        self.runtime.runtime_patch_token = self.runtime.runtime_patch_token.wrapping_add(1);
        self.runtime.pending_runtime_patch = None;
        self.runtime.system_proxy_pending = false;
        self.runtime.tun_service_status = None;
        self.runtime.tun_service_install_state = TunServiceInstallState::Idle;
        taken
    }

    /// Re-evaluate the admin server lifecycle against the desired settings
    /// snapshot. Called after settings load and after every admin settings
    /// change. Inert under `cfg(test)` (tests never start a real server).
    pub fn apply_admin_server_lifecycle(&mut self) -> Task<Message> {
        let desired = AdminServerConfig {
            enabled: self.shell.admin_enabled,
            port: self.shell.admin_port,
        };
        match self.shell.admin_server.begin_transition(desired.clone()) {
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
                    let ctx = IcedAdminContext::new(self.shell.admin_shared.clone());
                    spawn_admin_server_start(self.shell.admin_server.clone(), ctx, desired)
                }
            }
        }
    }

    /// Handle a command that travelled from the admin REST context back into
    /// the update loop.
    pub fn handle_admin_host_command(&mut self, command: AdminHostCommand) -> Task<Message> {
        match command {
            AdminHostCommand::ProductSessionChanged => match self.refresh_desktop_product_session()
            {
                Ok(()) => {
                    self.refresh_tray();
                    Task::batch(vec![
                        Task::done(Message::FetchRuntimeConfig),
                        Task::done(Message::RefreshOutboundInventory),
                        Task::done(Message::RefreshRuntimeNow),
                    ])
                }
                Err(failure) => {
                    self.set_error(&failure);
                    self.system_notify(
                        "notify_kernel_error",
                        &failure.message,
                        NotifyUrgency::Critical,
                    )
                }
            },
            AdminHostCommand::Toast(content, status) => {
                Task::done(Message::ShowToast(content, status))
            }
            AdminHostCommand::SystemNotify {
                title,
                body,
                urgency,
            } => {
                // `title` carries a `notify_*` locale key (see
                // `IcedAdminContext::notify_subscription_update`); `Lang::tr`
                // passes unknown keys through unchanged.
                self.system_notify(&title, &body, urgency)
            }
            AdminHostCommand::SettingsSavedExternally => {
                // Reload from disk and re-apply, but skip the WebDAV
                // sync-on-startup side effect the startup path performs.
                // Hydrated: re-fill the webdav_pass mirror from the keyring.
                Task::perform(
                    async {
                        load_settings_hydrated_from_disk()
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))
                    },
                    Message::ExternalSettingsLoaded,
                )
            }
            AdminHostCommand::CoreVersionsChanged => Task::done(Message::LoadKernels),
            AdminHostCommand::PickEditorPath(tx) => Task::perform(
                async {
                    spawn_blocking(|| rfd::FileDialog::new().pick_file())
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
}

#[cfg(test)]
#[path = "../tests/gui/admin_server_tests.rs"]
mod tests;

//! Slint adapter for the shared MusicFrog product contract.
//!
//! The imported Lumen component tree owns presentation and transient widget
//! state. This crate projects owner-authored snapshots and turns user actions
//! into the same typed commands used by the other surfaces. It does not own a
//! Core, profile store, or a second Android runtime.

#![allow(
    clippy::collapsible_if,
    clippy::needless_borrows_for_generic_args,
    clippy::map_clone,
    clippy::items_after_test_module,
    clippy::large_enum_variant,
    clippy::manual_pattern_char_comparison
)]

pub mod age_action;
mod callbacks;
pub mod capture_scenario;
#[cfg(feature = "capture-tools")]
pub mod demo;
#[cfg(not(target_os = "android"))]
mod desktop_window_chrome;
mod flag_assets;
mod lumen_projection;
pub mod mascot_driver;
mod mobile_dock_driver;
pub mod shell_receipt_host;
pub mod surface;
pub mod theme_util;
pub mod widget_vocabulary;

use crate::surface::workbenches::Workbenches;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::language::LanguagePreference;
use infiltrator_contract::named_override::NamedOverrideId;
use infiltrator_contract::profile_age_credential::{
    AgePrivateKey, ProfileAgeCredentialMutation, ProfileAgeCredentialObservation,
    ProfileAgeCredentialOwnerReadback, ProfileAgeCredentialOwnerRequest,
    ProfileAgeCredentialReadOutcome, ProfileAgeCredentialState, ProfileAgeCredentialTarget,
};
use infiltrator_contract::shell_navigation::destination_for_route;
use infiltrator_contract::subscription_import::{
    SubscriptionImportChannel, SubscriptionScheduleDraft,
};
use infiltrator_contract::surface::SurfaceKind;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::system_proxy::SystemProxyStatus;
use infiltrator_contract::tray_menu::{TrayMenuCapabilities, TraySpec};
use infiltrator_contract::tray_status::{
    TrayEvent, TrayEventContext, TrayIntent, TraySupport, resolve_tray_event_in,
};
use infiltrator_presentation::tray_projection::{build_tray_spec, tray_context_from_snapshot};
use infiltrator_shared::locales::{Lang, Localizer, resolve_language_code};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::profile_document_identity::ProfileId;
use lurker_core_contract::profile_protection::ProfileWriteProtection;
use slint::Model;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// Slint 1.18.1 的 codegen 对每个嵌套子组件都生成 `todo!()` 嵌入占位（永不执行，
// 只存在于 build script 产物里）。工作区对 `clippy::todo` 的零容忍针对手写
// 生产代码；内层 allow 只豁免生成模块自身，src/ 与 tests/ 依旧零违例。
#[allow(clippy::todo)]
mod lumen_ui {
    slint::include_modules!();
}
pub use lumen_ui::*;

pub(crate) fn slint_model<T: Clone + 'static>(items: Vec<T>) -> slint::ModelRc<T> {
    slint::ModelRc::new(slint::VecModel::from(items))
}

/// Publish the active route and the typed navigation destination it belongs
/// to in one call. The "which sidebar row is active" fact is resolved by the
/// shared contract, never re-derived by the surface's own string chain.
pub(crate) fn set_page(window: &AppWindow, page: &str) {
    window.set_page(page.into());
    let destination = destination_for_route(page)
        .map(|target| target.id())
        .unwrap_or("");
    window.set_active_destination(destination.into());
}

/// Refresh a long-list property in place (EFFECTS-040 #18): when the window
/// still carries a `VecModel` of the same row type, the rows are rewritten
/// where they overlap and only the tail grows or shrinks, so the model object
/// and every binding that reached it survive the snapshot; a fresh `ModelRc`
/// is mounted only when there is nothing to refresh yet. The per-snapshot
/// destroy-and-rebuild of the old `slint_model` call is the thing this helper
/// retires.
///
/// The update never passes through a zero-row model. `VecModel::set_vec` does
/// exactly that (one reset notification for the whole range), and a `GridLayout`
/// repeater that reads `index` to place its cells can then be re-laid out
/// against the jump table of the previous row count — which is the
/// `i-slint-core` `index out of bounds: the len is 4 but the index is 4` panic
/// the proxies node grid hit while a search filter narrowed the rows.
pub(crate) fn refresh_model<T: Clone + 'static>(
    current: &slint::ModelRc<T>,
    rows: Vec<T>,
) -> slint::ModelRc<T> {
    match current.as_any().downcast_ref::<slint::VecModel<T>>() {
        Some(vec_model) => {
            let common = vec_model.row_count().min(rows.len());
            for (index, row) in rows.iter().take(common).enumerate() {
                vec_model.set_row_data(index, row.clone());
            }
            for (index, row) in rows.iter().enumerate().skip(vec_model.row_count()) {
                vec_model.insert(index, row.clone());
            }
            while vec_model.row_count() > rows.len() {
                vec_model.remove(vec_model.row_count() - 1);
            }
            current.clone()
        }
        None => slint_model(rows),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VpnPermissionOutcome {
    Granted,
    Denied,
    Canceled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationPermissionOutcome {
    Granted,
    Denied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatteryOptimizationOutcome {
    Supported { enabled: bool },
    Unsupported,
    Canceled,
    PermissionDenied,
}

/// Work handed to the platform composition. Sequences are reserved for one
/// user action that must move through several shared commands (for example,
/// switching the desktop network mode while preserving the running state).
#[derive(Debug)]
pub enum SlintAction {
    Command(CommandIntent),
    Sequence(Vec<CommandIntent>),
    StartRuntime,
    SetAutostart(bool),
    OpenAutostartSettings,
    RequestBatteryOptimizationExemption,
    SetHideBackgroundCard(bool),
    SetReleaseUiWhenBackground(bool),
    SetShowTrafficNotification(bool),
    CacheBackPreview,
    CommitBackPreview,
    ScanQrCode,
    ImportEncryptedSubscription {
        name: String,
        url: String,
        private_key: Option<AgePrivateKey>,
    },
    SetProfileAgeCredential {
        profile_id: String,
        private_key: AgePrivateKey,
    },
    ClearProfileAgeCredential {
        profile_id: String,
    },
    SaveProfileSettings {
        profile_id: String,
        name: Option<String>,
        url: String,
        private_key: Option<AgePrivateKey>,
    },
    ImportProfileFile {
        name: String,
        path: String,
        private_key: Option<AgePrivateKey>,
    },
    ImportOverrideFile {
        path: String,
    },
    ExportProfile {
        profile_id: String,
        destination: PathBuf,
    },
    ExportOverride {
        override_id: String,
        destination: PathBuf,
    },
    ExportLogs {
        destination: PathBuf,
    },
}

/// The only seam between this frontend and the shared application owner.
pub trait SlintProductHost: Send + Sync + 'static {
    /// Which product surface this host is mounting.
    fn surface_kind(&self) -> SurfaceKind;
    fn read_snapshot(&self) -> Result<SurfaceSnapshot, Failure>;
    fn execute(
        &self,
        snapshot: Option<&SurfaceSnapshot>,
        action: SlintAction,
    ) -> Result<(), Failure>;

    /// Executes one owner action and reports its typed readback.
    ///
    /// The default runs [`SlintProductHost::execute`] and reports that the
    /// host published no typed readback, so a host that cannot carry one
    /// keeps working unchanged. A host that does carry one lets the surface
    /// correlate the readbacks that carry an owner identity, for example a
    /// reviewed export or a restoration review.
    fn execute_typed(
        &self,
        snapshot: Option<&SurfaceSnapshot>,
        action: SlintAction,
    ) -> Result<CommandOutput, Failure> {
        self.execute(snapshot, action).map(|_| CommandOutput::Unit)
    }

    /// Host-owned preference, deliberately absent on hosts without an
    /// autostart adapter. The shared application snapshot remains the source
    /// of every application setting.
    fn autostart_enabled(&self) -> Option<bool> {
        None
    }

    fn execute_age_credential_request(
        &self,
        _request: ProfileAgeCredentialOwnerRequest,
    ) -> Result<ProfileAgeCredentialOwnerReadback, Failure> {
        Err(Failure::unsupported(
            "Age credential operations are not supported by this host",
        ))
    }

    fn request_vpn_permission(&self) -> Result<VpnPermissionOutcome, Failure> {
        Ok(VpnPermissionOutcome::Granted)
    }

    fn request_notification_permission(&self) -> Result<NotificationPermissionOutcome, Failure> {
        Ok(NotificationPermissionOutcome::Granted)
    }

    fn scan_qr_code(&self) -> Result<String, Failure> {
        Err(Failure::unsupported(
            "QR code scanning is not supported on this platform",
        ))
    }

    fn request_battery_optimization_exemption(
        &self,
    ) -> Result<BatteryOptimizationOutcome, Failure> {
        Ok(BatteryOptimizationOutcome::Unsupported)
    }

    fn open_autostart_settings(&self) -> Result<(), Failure> {
        Err(Failure::unsupported(
            "Autostart settings not available on this platform",
        ))
    }

    fn set_hide_background_card(&self, _enabled: bool) -> Result<(), Failure> {
        Ok(())
    }

    fn set_release_ui_when_background(&self, _enabled: bool) -> Result<(), Failure> {
        Ok(())
    }

    fn set_show_traffic_notification(&self, _enabled: bool) -> Result<(), Failure> {
        Ok(())
    }

    fn cache_back_preview(&self) -> Result<(), Failure> {
        Ok(())
    }

    fn commit_back_preview(&self) -> Result<(), Failure> {
        Ok(())
    }
}

pub(crate) enum WorkerRequest {
    Read,
    Action(SlintAction),
    ReadAgeCredential { profile_id: String },
}

pub(crate) enum WorkerEvent {
    Snapshot(Result<SurfaceSnapshot, Failure>, Option<bool>),
    ActionFinished(Result<CommandOutput, Failure>),
    AgeCredentialRead {
        profile_id: String,
        result: Result<ProfileAgeCredentialObservation, Failure>,
    },
}

/// Launch the same Slint component on a native host supplied by the product
/// composition root.
/// Host-provided system tray port for the Slint surface.
pub trait SlintTrayHost {
    fn push(&self, spec: TraySpec);
    fn drain(&self) -> Vec<TrayEvent>;
    fn support(&self) -> TraySupport;
}

pub fn run_with_host(host: Arc<dyn SlintProductHost>) -> Result<(), slint::PlatformError> {
    run_with_host_prepared(host, |_, _| {})
}

pub fn run_with_host_prepared_and_tray(
    host: Arc<dyn SlintProductHost>,
    prepare: impl FnOnce(&AppWindow, &Arc<Workbenches>),
    tray: Option<Box<dyn SlintTrayHost>>,
) -> Result<(), slint::PlatformError> {
    run_with_host_prepared_impl(host, prepare, tray)
}

/// Select the desktop GPU backend (EFFECTS-036 #146~#149, = SLINTUI-001 #1~#4).
///
/// The offline-build default channel is `winit` + `femtovg`; `renderer-skia`
/// is opt-in through `--features gpu-skia` because the Skia bindings fetch a
/// prebuilt binary at build time, which network-isolated builds must not
/// require. When `gpu-skia` is compiled in, the selector requests `winit` +
/// `skia`, so a Skia that fails to load surfaces as a startup error instead
/// of a silent fallback. An explicit `SLINT_BACKEND` from the environment
/// wins either way, keeping the femtovg channel and the software fallback
/// operator-selectable without a rebuild. VSync rides the winit present mode
/// of the selected GPU backend. The backend-qt bridge stays a reserved seam
/// (nothing in this tree links Qt).
#[cfg(not(target_os = "android"))]
fn select_desktop_gpu_backend() -> Result<(), slint::PlatformError> {
    if env::var_os("SLINT_BACKEND").is_some() {
        return Ok(());
    }
    let renderer = if cfg!(feature = "gpu-skia") {
        "skia"
    } else {
        "femtovg"
    };
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name(renderer.into())
        .select()
}

/// Launch the Slint component and let the caller prepare the window first.
///
/// The preparation runs before the owner worker starts, so a host that owns a
/// fixture can project it once and let the worker keep it fresh. The
/// preparation receives the window and the workbench state the surface owns,
/// so the prepared state is the state the product runs with.
pub fn run_with_host_prepared(
    host: Arc<dyn SlintProductHost>,
    prepare: impl FnOnce(&AppWindow, &Arc<Workbenches>),
) -> Result<(), slint::PlatformError> {
    run_with_host_prepared_impl(host, prepare, None)
}

/// Launch the Slint component with the window preparation *and* the host's
/// system tray seam.
///
/// The tray is a host capability: the surface never talks to the OS tray
/// itself. The host hands over the seam, the surface publishes the shared
/// menu from the owner's snapshot and forwards the user's activations back
/// as typed intents.
fn run_with_host_prepared_impl(
    host: Arc<dyn SlintProductHost>,
    prepare: impl FnOnce(&AppWindow, &Arc<Workbenches>),
    tray: Option<Box<dyn SlintTrayHost>>,
) -> Result<(), slint::PlatformError> {
    #[cfg(not(target_os = "android"))]
    select_desktop_gpu_backend()?;
    let window = AppWindow::new()?;
    window.set_android_platform(cfg!(target_os = "android"));
    #[cfg(not(target_os = "android"))]
    desktop_window_chrome::install(&window);

    let initial_language = env::var("INFILTRATOR_LANG")
        .map(|value| resolve_language_code(&value))
        .unwrap_or_else(|_| resolve_language_code("system"));
    select_translation(&window, &initial_language);
    lumen_projection::project_initial(
        &window,
        &initial_language,
        host.autostart_enabled(),
        host.surface_kind(),
    );
    mobile_dock_driver::install(&window);

    let (request_tx, request_rx) = channel::<WorkerRequest>();
    let (event_tx, event_rx) = channel::<WorkerEvent>();
    let poll_pending = Arc::new(AtomicBool::new(false));
    let latest_snapshot = Arc::new(Mutex::new(None));
    let workbenches = Arc::new(Workbenches::default());
    prepare(&window, &workbenches);
    spawn_host_worker(
        Arc::clone(&host),
        request_rx,
        event_tx,
        Arc::clone(&poll_pending),
        Arc::clone(&latest_snapshot),
    );
    callbacks::register_callbacks(
        &window,
        request_tx.clone(),
        Arc::clone(&latest_snapshot),
        Arc::clone(&workbenches),
    );
    let mobile_dock_weak = window.as_weak();
    window.on_mobile_dock_band_state_changed(move || {
        if let Some(window) = mobile_dock_weak.upgrade() {
            mobile_dock_driver::project(&window);
        }
    });
    // The mascot is a passive consumer: its clock and state machine live in
    // this driver, not in the .slint tree. A capture run publishes still frames
    // so the frog cannot jitter between two receipts.
    mascot_driver::install(
        &window,
        Arc::clone(&latest_snapshot),
        env::var_os("INFILTRATOR_CAPTURE_MARKER").is_some(),
    );
    // The surface asks for its first snapshot right away, so the owner's
    // terminal is projected with a snapshot the surface already observes.
    let _ = request_tx.send(WorkerRequest::Read);

    let update_window = window.as_weak();
    let poll_tx = request_tx.clone();
    // The event loop closure owns a handle to the shared workbench state, so
    // the surface's reducers stay reachable from every projection.
    let workbenches = Arc::clone(&workbenches);
    // The owner's terminal is projected with the owner's last snapshot, so
    // the panel that waits for the terminal shows the answer at once.
    let latest_for_terminal = Arc::clone(&latest_snapshot);
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(700),
        move || {
            if poll_pending
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
                && poll_tx.send(WorkerRequest::Read).is_err()
            {
                poll_pending.store(false, Ordering::Release);
            }
            while let Ok(event) = event_rx.try_recv() {
                let weak = update_window.clone();
                // Each event closure takes its own handle to the shared
                // workbench state; the outer loop keeps its own.
                let workbenches = Arc::clone(&workbenches);
                let latest_for_terminal = Arc::clone(&latest_for_terminal);
                if let WorkerEvent::Snapshot(Ok(ref snapshot), _) = event {
                    if let Some(tray) = tray.as_ref() {
                        let color_mode = snapshot
                            .pages
                            .settings
                            .data
                            .as_ref()
                            .map(|s| s.tray_icon_color_mode)
                            .unwrap_or_default();
                        let lang = lumen_projection::snapshot_language(snapshot);
                        let mut ctx = tray_context_from_snapshot(snapshot, &lang);
                        ctx.capabilities = TrayMenuCapabilities::shared_desktop();
                        let spec = build_tray_spec(&ctx, color_mode);
                        tray.push(spec);
                    }
                }
                let _ = weak.upgrade_in_event_loop(move |window| match event {
                    WorkerEvent::Snapshot(Ok(snapshot), autostart) => {
                        let language = lumen_projection::snapshot_language(&snapshot);
                        select_translation(&window, &language);
                        lumen_projection::project_snapshot(&window, &snapshot, autostart);
                        lumen_projection::project_workbench(&window, &snapshot, &workbenches);
                        mobile_dock_driver::project(&window);
                    }
                    WorkerEvent::Snapshot(Err(failure), _) => {
                        window.set_notice(failure.message.into());
                    }
                    WorkerEvent::ActionFinished(result) => {
                        // The reducer that started the command correlates the
                        // owner's terminal, so the business that is waiting
                        // for it learns the outcome.
                        workbenches.settle(result.clone());
                        // The terminal moved the reducer's phase, so the
                        // panel that waits for it is projected again: the
                        // business shows the owner's answer instead of the
                        // state it had before the command.
                        let terminal_snapshot = latest_for_terminal
                            .lock()
                            .ok()
                            .and_then(|guard| guard.clone());
                        if let Some(snapshot) = terminal_snapshot {
                            lumen_projection::project_workbench(&window, &snapshot, &workbenches);
                        }
                        match result {
                            Ok(_) => {
                                // The next owner snapshot is the only source of readback.
                                let close_profile_settings = window.get_profile_settings_saving();
                                let close_user_agent = window.get_user_agent_saving();
                                clear_pending_ui(&window);
                                window.set_user_agent_error("".into());
                                window.set_profile_settings_age_key("".into());
                                if close_profile_settings {
                                    window.set_profile_settings_id("".into());
                                }
                                if close_user_agent {
                                    window.set_user_agent_dialog_open(false);
                                }
                                window.set_notice("".into());
                            }
                            Err(failure) => {
                                project_action_failure(&window, &failure);
                            }
                        }
                    }
                    WorkerEvent::AgeCredentialRead { profile_id, result } => {
                        if window.get_profile_settings_id().as_str() == profile_id {
                            match result {
                                Ok(observation) => {
                                    window.set_profile_settings_age_configured(
                                        observation.state == ProfileAgeCredentialState::Configured,
                                    );
                                }
                                Err(failure) => {
                                    window.set_notice(failure.message.into());
                                }
                            }
                        }
                    }
                });
            }
            if let Some(tray) = tray.as_ref() {
                let events = tray.drain();
                if !events.is_empty() {
                    let snap_guard = latest_for_terminal.lock().ok().and_then(|g| g.clone());
                    let context = tray_event_context(snap_guard.as_ref());
                    for event in events {
                        if let Some(intent) = resolve_tray_event_in(&event, &context) {
                            match intent {
                                TrayIntent::ShowWindow => {
                                    if let Some(w) = update_window.upgrade() {
                                        let _ = w.show();
                                        w.window().set_minimized(false);
                                    }
                                }
                                TrayIntent::Exit => {
                                    let _ = slint::quit_event_loop();
                                }
                                TrayIntent::ToggleIconColorMode => {
                                    let next = snap_guard
                                        .as_ref()
                                        .and_then(|s| s.pages.settings.data.as_ref())
                                        .map(|s| s.tray_icon_color_mode)
                                        .unwrap_or_default()
                                        .toggle();
                                    let _ = poll_tx.send(WorkerRequest::Action(
                                        SlintAction::Command(CommandIntent::UpdateSetting {
                                            key: "tray_icon_color_mode".into(),
                                            value: next.as_str().to_string(),
                                        }),
                                    ));
                                }
                                TrayIntent::SetRoutingIntent(intent) => {
                                    let _ =
                                        poll_tx.send(WorkerRequest::Action(SlintAction::Command(
                                            CommandIntent::SetRoutingIntent { intent },
                                        )));
                                }
                                TrayIntent::SetSystemProxy(enabled) => {
                                    let _ =
                                        poll_tx.send(WorkerRequest::Action(SlintAction::Command(
                                            CommandIntent::SetSystemProxy { enabled },
                                        )));
                                }
                                TrayIntent::SetTunEnabled(enabled) => {
                                    let _ = poll_tx.send(WorkerRequest::Action(
                                        SlintAction::Command(CommandIntent::ToggleTun { enabled }),
                                    ));
                                }
                                TrayIntent::ActivateProfile(name) => {
                                    let _ =
                                        poll_tx.send(WorkerRequest::Action(SlintAction::Command(
                                            CommandIntent::SwitchProfile { profile_id: name },
                                        )));
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        },
    );

    let result = window.run();
    timer.stop();
    drop(timer);
    drop(window);
    drop(request_tx);
    result
}

fn tray_event_context(snapshot: Option<&SurfaceSnapshot>) -> TrayEventContext<'static> {
    let Some(snapshot) = snapshot else {
        return TrayEventContext {
            system_proxy: false,
            tun: false,
            autostart: false,
            profiles: &[],
        };
    };
    TrayEventContext {
        system_proxy: matches!(snapshot.system_proxy.status, SystemProxyStatus::Enabled),
        tun: snapshot.runtime_control.tun_enabled.unwrap_or(false),
        autostart: false,
        profiles: &[],
    }
}

fn select_translation(window: &AppWindow, language: &str) {
    let catalog_language = if language.starts_with("en") {
        "en"
    } else {
        "zh-CN"
    };
    if window.get_current_translated_language().as_str() == catalog_language {
        return;
    }
    if let Err(error) = slint::select_bundled_translation(catalog_language) {
        window.set_notice(error.to_string().into());
        return;
    }
    window.set_current_translated_language(catalog_language.into());
}

fn spawn_host_worker(
    host: Arc<dyn SlintProductHost>,
    request_rx: Receiver<WorkerRequest>,
    event_tx: Sender<WorkerEvent>,
    poll_pending: Arc<AtomicBool>,
    latest_snapshot: Arc<Mutex<Option<SurfaceSnapshot>>>,
) {
    thread::Builder::new()
        .name("musicfrog-slint-owner".to_owned())
        .spawn(move || {
            let mut latest: Option<SurfaceSnapshot> = None;
            while let Ok(request) = request_rx.recv() {
                let event = match request {
                    WorkerRequest::Read => match host.read_snapshot() {
                        Ok(snapshot) => {
                            latest = Some(snapshot.clone());
                            if let Ok(mut shared) = latest_snapshot.lock() {
                                *shared = Some(snapshot.clone());
                            }
                            WorkerEvent::Snapshot(Ok(snapshot), host.autostart_enabled())
                        }
                        Err(failure) => {
                            latest = None;
                            if let Ok(mut shared) = latest_snapshot.lock() {
                                *shared = None;
                            }
                            WorkerEvent::Snapshot(Err(failure), None)
                        }
                    },
                    WorkerRequest::Action(action) => {
                        WorkerEvent::ActionFinished(host.execute_typed(latest.as_ref(), action))
                    }
                    WorkerRequest::ReadAgeCredential { profile_id } => {
                        let snapshot = latest_snapshot.lock().ok().and_then(|guard| guard.clone());
                        let result = match snapshot.as_ref().and_then(|s| s.core.scope()) {
                            Some(scope) => {
                                match ProfileId::new(profile_id.clone()) {
                                    Ok(pid) => {
                                        host.execute_age_credential_request(ProfileAgeCredentialOwnerRequest::Read {
                                            scope,
                                            profile_id: pid,
                                        })
                                        .and_then(|readback| match readback {
                                            ProfileAgeCredentialOwnerReadback::Read(outcome) => match outcome {
                                                ProfileAgeCredentialReadOutcome::Observed { current, .. } => Ok(current),
                                                ProfileAgeCredentialReadOutcome::StaleSelection { .. } => {
                                                    Err(Failure::new(ErrorCode::NotReady, "Age credential read selection was stale", true))
                                                }
                                            },
                                            _ => Err(Failure::new(ErrorCode::Internal, "Unexpected Age credential readback", true)),
                                        })
                                    }
                                    Err(error) => Err(Failure::new(ErrorCode::InvalidInput, error.to_string(), false)),
                                }
                            }
                            None => Err(Failure::new(ErrorCode::NotReady, "No active Core scope for Age credential reading", true)),
                        };
                        WorkerEvent::AgeCredentialRead { profile_id, result }
                    }
                };
                if matches!(&event, WorkerEvent::Snapshot(_, _)) {
                    poll_pending.store(false, Ordering::Release);
                }
                let _ = event_tx.send(event);
            }
        })
        .expect("Slint owner worker must start");
}

fn queue_intent(tx: &Sender<WorkerRequest>, weak: &slint::Weak<AppWindow>, intent: CommandIntent) {
    queue_action(tx, weak, Ok(SlintAction::Command(intent)));
}

fn queue_action(
    tx: &Sender<WorkerRequest>,
    weak: &slint::Weak<AppWindow>,
    action: Result<SlintAction, Failure>,
) {
    match action {
        Ok(action) => {
            if tx.send(WorkerRequest::Action(action)).is_err() {
                show_failure(
                    weak,
                    Failure::new(
                        ErrorCode::NotReady,
                        "The shared owner worker is unavailable",
                        true,
                    ),
                );
            }
        }
        Err(failure) => show_failure(weak, failure),
    }
}

fn show_unsupported(weak: &slint::Weak<AppWindow>, reason: &str) {
    show_failure(weak, Failure::unsupported(reason));
}

fn show_failure(weak: &slint::Weak<AppWindow>, failure: Failure) {
    surface::report_failure(&failure);
    let _ = weak.clone().upgrade_in_event_loop(move |window| {
        clear_pending_ui(&window);
        window.set_notice(failure.message.into());
    });
}

fn clear_pending_ui(window: &AppWindow) {
    window.set_user_agent_saving(false);
    window.set_user_agent_result_revision(window.get_user_agent_save_revision());
    window.set_theme_color_saving(false);
    window.set_profile_settings_saving(false);
    window.set_proxy_testing_group("".into());
    window.set_proxy_testing_node("".into());
    window.set_rule_pending_index("".into());
    window.set_connection_breaking_id("".into());
    window.set_override_importing(false);
}

/// Projects the typed failure of a refused owner action onto the surface.
///
/// The owner worker and the headless surface harness share this projection,
/// so a refused command is presented the same way in the product and in a
/// driven surface test.
pub(crate) fn project_action_failure(window: &AppWindow, failure: &Failure) {
    let user_agent_save_failed = window.get_user_agent_saving();
    clear_pending_ui(window);
    window.set_profile_settings_age_key("".into());
    if user_agent_save_failed {
        window.set_user_agent_error(failure.message.clone().into());
    }
    window.set_notice(failure.message.clone().into());
}

fn refresh_projection(weak: &slint::Weak<AppWindow>, latest: &Arc<Mutex<Option<SurfaceSnapshot>>>) {
    if let Some(snapshot) = clone_latest(latest) {
        let _ = weak.clone().upgrade_in_event_loop(move |window| {
            lumen_projection::project_snapshot(&window, &snapshot, None);
        });
    }
}

fn clone_latest(latest: &Arc<Mutex<Option<SurfaceSnapshot>>>) -> Option<SurfaceSnapshot> {
    latest.lock().ok()?.clone()
}

fn require_snapshot(
    latest: &Arc<Mutex<Option<SurfaceSnapshot>>>,
) -> Result<SurfaceSnapshot, Failure> {
    clone_latest(latest).ok_or_else(|| {
        Failure::new(
            ErrorCode::NotReady,
            "The shared owner has not published its first snapshot",
            true,
        )
    })
}

fn open_profile_settings(
    weak: &slint::Weak<AppWindow>,
    latest: &Arc<Mutex<Option<SurfaceSnapshot>>>,
    id: String,
    requests: &Sender<WorkerRequest>,
) {
    let Some(snapshot) = clone_latest(latest) else {
        show_unsupported(
            weak,
            "Wait for the shared profile snapshot before editing a profile",
        );
        return;
    };
    let Some(profile) = snapshot
        .pages
        .profiles
        .data
        .as_ref()
        .and_then(|page| page.profiles.iter().find(|profile| profile.id == id))
    else {
        show_unsupported(
            weak,
            "The selected profile is not present in the latest shared snapshot",
        );
        return;
    };
    let pid = id.clone();
    let id_shared = id.into();
    let name = profile.name.clone().into();
    let url = profile.url.clone().into();
    let location = if profile.url.is_empty() {
        profile.name.clone()
    } else {
        safe_profile_source_location(
            &profile.url,
            &lumen_projection::snapshot_language(&snapshot),
        )
    }
    .into();
    let kind = if profile.write_protection.is_protected() {
        "remote_subscription"
    } else {
        "local_yaml"
    };
    let kind = kind.into();
    let _ = weak.clone().upgrade_in_event_loop(move |window| {
        window.set_profile_settings_id(id_shared);
        window.set_profile_settings_name(name);
        window.set_profile_settings_url(url);
        window.set_profile_settings_location(location);
        window.set_profile_settings_kind(kind);
        // The shared profile snapshot intentionally never returns subscription
        // credentials such as the Age private key.
        window.set_profile_settings_age_key("".into());
        window.set_profile_settings_age_configured(false);
    });
    let _ = requests.send(WorkerRequest::ReadAgeCredential { profile_id: pid });
}

fn safe_profile_source_location(raw_url: &str, language: &str) -> String {
    let Some((scheme, authority_and_path)) = raw_url.split_once("://") else {
        return Lang(language).tr("已配置订阅来源").into_owned();
    };
    let authority = authority_and_path
        .split(|character| matches!(character, '/' | '?' | '#'))
        .next()
        .unwrap_or_default();
    let host = authority.rsplit('@').next().unwrap_or_default();
    if host.is_empty() {
        Lang(language).tr("已配置订阅来源").into_owned()
    } else {
        format!("{scheme}://{host}/…")
    }
}

#[cfg(feature = "desktop-host")]
#[path = "desktop.rs"]
mod desktop;

#[cfg(feature = "desktop-host")]
pub fn run_desktop() -> anyhow::Result<()> {
    desktop::run()
}

#[cfg(feature = "capture-tools")]
pub fn run_demo() -> anyhow::Result<()> {
    let settings = demo::DemoSettings::from_env();
    demo::run_demo(settings).map_err(|e| anyhow::anyhow!(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    fn rows<T: Clone + 'static>(model: &slint::ModelRc<T>) -> Vec<T> {
        (0..model.row_count())
            .map(|index| model.row_data(index).expect("row within range"))
            .collect()
    }

    /// The proxies grid places every card with `index`, so a refresh that
    /// briefly empties the model lets Slint recompute the grid against the
    /// previous row count and panic. A refresh therefore has to keep the same
    /// model object and land exactly the requested rows.
    #[test]
    fn refresh_model_keeps_the_model_and_lands_the_rows() {
        let first: slint::ModelRc<String> = slint_model(vec!["a".into(), "b".into(), "c".into()]);
        let refreshed = refresh_model(&first, vec!["x".into(), "y".into()]);
        assert!(
            ptr::eq(
                first
                    .as_any()
                    .downcast_ref::<slint::VecModel<String>>()
                    .unwrap(),
                refreshed
                    .as_any()
                    .downcast_ref::<slint::VecModel<String>>()
                    .unwrap(),
            ),
            "the same VecModel must survive the refresh"
        );
        assert_eq!(rows(&refreshed), vec!["x".to_string(), "y".to_string()]);

        let grown = refresh_model(
            &refreshed,
            vec!["p".into(), "q".into(), "r".into(), "s".into()],
        );
        assert_eq!(rows(&grown), vec!["p", "q", "r", "s"]);

        let emptied = refresh_model(&grown, Vec::new());
        assert_eq!(emptied.row_count(), 0);

        let mounted = refresh_model(&emptied, vec!["only".into()]);
        assert_eq!(rows(&mounted), vec!["only"]);
    }
}

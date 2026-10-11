//! Slint adapter for the shared MusicFrog product contract.
//!
//! The imported Lumen component tree owns presentation and transient widget
//! state. This crate projects owner-authored snapshots and turns user actions
//! into the same typed commands used by the other surfaces. It does not own a
//! Core, profile store, or a second Android runtime.

mod callbacks;
mod flag_assets;
mod lumen_projection;

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::language::LanguagePreference;
use infiltrator_contract::outbound::identity::OutboundContractError;
use infiltrator_contract::outbound::inventory::OutboundInventoryState;
use infiltrator_contract::outbound::preferences::{
    OutboundSortOrder, OutboundUiPreferenceIntent, OutboundUiPreferenceRequest,
};
use infiltrator_contract::outbound::probe::OutboundSelectionRequest;
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_contract::routing_intent::RoutingIntent;
use infiltrator_contract::subscription_import::{
    SubscriptionImportChannel, SubscriptionScheduleDraft,
};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::system_proxy::SystemProxyStatus;
use infiltrator_contract::vpn::VpnSurfaceObservation;
use infiltrator_shared::locales::{Lang, Localizer, resolve_language_code};
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

slint::include_modules!();

pub(crate) fn slint_model<T: Clone + 'static>(items: Vec<T>) -> slint::ModelRc<T> {
    slint::ModelRc::new(slint::VecModel::from(items))
}

/// Work handed to the platform composition. Sequences are reserved for one
/// user action that must move through several shared commands (for example,
/// switching the desktop network mode while preserving the running state).
#[derive(Clone, Debug)]
pub enum SlintAction {
    Command(CommandIntent),
    Sequence(Vec<CommandIntent>),
    SetAutostart(bool),
}

/// The only seam between this frontend and the shared application owner.
pub trait SlintProductHost: Send + Sync + 'static {
    fn read_snapshot(&self) -> Result<SurfaceSnapshot, Failure>;
    fn execute(
        &self,
        snapshot: Option<&SurfaceSnapshot>,
        action: SlintAction,
    ) -> Result<(), Failure>;

    /// Host-owned preference, deliberately absent on hosts without an
    /// autostart adapter. The shared application snapshot remains the source
    /// of every application setting.
    fn autostart_enabled(&self) -> Option<bool> {
        None
    }
}

enum WorkerRequest {
    Read,
    Action(SlintAction),
}

enum WorkerEvent {
    Snapshot(Result<SurfaceSnapshot, Failure>, Option<bool>),
    ActionFinished(Result<(), Failure>),
}

/// Launch the same Slint component on a native host supplied by the product
/// composition root.
pub fn run_with_host(host: Arc<dyn SlintProductHost>) -> Result<(), slint::PlatformError> {
    let window = AppWindow::new()?;
    window.set_android_platform(cfg!(target_os = "android"));

    let initial_language = env::var("INFILTRATOR_LANG")
        .map(|value| resolve_language_code(&value))
        .unwrap_or_else(|_| resolve_language_code("system"));
    select_translation(&window, &initial_language);
    lumen_projection::project_initial(&window, &initial_language, host.autostart_enabled());

    let (request_tx, request_rx) = channel::<WorkerRequest>();
    let (event_tx, event_rx) = channel::<WorkerEvent>();
    let poll_pending = Arc::new(AtomicBool::new(false));
    let latest_snapshot = Arc::new(Mutex::new(None));
    spawn_host_worker(
        Arc::clone(&host),
        request_rx,
        event_tx,
        Arc::clone(&poll_pending),
        Arc::clone(&latest_snapshot),
    );
    callbacks::register_callbacks(&window, request_tx.clone(), Arc::clone(&latest_snapshot));

    let update_window = window.as_weak();
    let poll_tx = request_tx.clone();
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
                let _ = weak.upgrade_in_event_loop(move |window| match event {
                    WorkerEvent::Snapshot(Ok(snapshot), autostart) => {
                        let language = lumen_projection::snapshot_language(&snapshot);
                        select_translation(&window, &language);
                        lumen_projection::project_snapshot(&window, &snapshot, autostart);
                    }
                    WorkerEvent::Snapshot(Err(failure), _) => {
                        window.set_notice(failure.message.into());
                    }
                    WorkerEvent::ActionFinished(Ok(())) => {
                        // The next owner snapshot is the only source of readback.
                        let close_profile_settings = window.get_profile_settings_saving();
                        let close_user_agent = window.get_user_agent_saving();
                        clear_pending_ui(&window);
                        window.set_user_agent_error("".into());
                        if close_profile_settings {
                            window.set_profile_settings_id("".into());
                        }
                        if close_user_agent {
                            window.set_user_agent_dialog_open(false);
                        }
                        window.set_notice("".into());
                    }
                    WorkerEvent::ActionFinished(Err(failure)) => {
                        let user_agent_save_failed = window.get_user_agent_saving();
                        clear_pending_ui(&window);
                        if user_agent_save_failed {
                            window.set_user_agent_error(failure.message.clone().into());
                        }
                        window.set_notice(failure.message.into());
                    }
                });
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
                        WorkerEvent::ActionFinished(host.execute(latest.as_ref(), action))
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
    let id = id.into();
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
        window.set_profile_settings_id(id);
        window.set_profile_settings_name(name);
        window.set_profile_settings_url(url);
        window.set_profile_settings_location(location);
        window.set_profile_settings_kind(kind);
        // The shared profile snapshot intentionally never returns subscription
        // credentials such as the Age private key.
        window.set_profile_settings_age_key("".into());
    });
}

fn network_mode_action(
    snapshot: &SurfaceSnapshot,
    requested: &str,
) -> Result<SlintAction, Failure> {
    use infiltrator_contract::snapshot::CoreLifecycle;
    if cfg!(target_os = "android") {
        return if requested != "VPN" {
            Err(Failure::unsupported(
                "Android Slint supports the owner-managed VPN mode only",
            ))
        } else {
            match &snapshot.vpn {
                VpnSurfaceObservation::AndroidOwner(owner) if owner.is_running() => {
                    Ok(SlintAction::Command(CommandIntent::StopVpn))
                }
                VpnSurfaceObservation::AndroidOwner(owner)
                    if owner.readiness
                        == infiltrator_contract::android_control::VpnOwnerReadiness::CleanupPending
                        || owner.resources.iter().any(|resource| {
                            matches!(
                                &resource.state,
                                infiltrator_contract::android_control::VpnOwnerResourceState::CleanupPending { .. }
                            )
                        }) =>
                {
                    Ok(SlintAction::Command(CommandIntent::StopVpn))
                }
                VpnSurfaceObservation::AndroidOwner(owner) => match &owner.vpn_state {
                    infiltrator_contract::android_control::VpnOwnerLifecycle::PermissionRequired
                    | infiltrator_contract::android_control::VpnOwnerLifecycle::Stopped
                    | infiltrator_contract::android_control::VpnOwnerLifecycle::Revoked
                    | infiltrator_contract::android_control::VpnOwnerLifecycle::Failed { .. } => {
                        Ok(SlintAction::Command(CommandIntent::StartVpn))
                    }
                    infiltrator_contract::android_control::VpnOwnerLifecycle::Unavailable {
                        failure_message,
                    } => Err(Failure::unsupported(failure_message.clone())),
                    infiltrator_contract::android_control::VpnOwnerLifecycle::StartRequested
                    | infiltrator_contract::android_control::VpnOwnerLifecycle::Starting
                    | infiltrator_contract::android_control::VpnOwnerLifecycle::Foreground
                    | infiltrator_contract::android_control::VpnOwnerLifecycle::TunnelEstablished
                    | infiltrator_contract::android_control::VpnOwnerLifecycle::Running
                    | infiltrator_contract::android_control::VpnOwnerLifecycle::Stopping => Err(
                        Failure::new(
                            ErrorCode::NotReady,
                            "VPN owner is in a transition and cannot accept a duplicate mode change",
                            true,
                        ),
                    ),
                },
                VpnSurfaceObservation::Unsupported { reason } => {
                    Err(Failure::unsupported(reason.clone()))
                }
                VpnSurfaceObservation::NotApplicable => {
                    Err(Failure::unsupported("VPN is not applicable to this surface"))
                }
                VpnSurfaceObservation::Unavailable { failure } => Err(failure.clone()),
            }
        };
    }

    let settings = snapshot.pages.settings.data.as_ref();
    let tun_enabled = settings
        .and_then(|settings| settings.tun_enabled)
        .unwrap_or(false);
    let proxy_enabled = matches!(&snapshot.system_proxy.status, SystemProxyStatus::Enabled);
    let running = matches!(
        &snapshot.core.lifecycle,
        CoreLifecycle::Ready | CoreLifecycle::Running
    );
    let mut commands = Vec::new();
    if running {
        commands.push(CommandIntent::StopProxy);
    }
    match requested {
        "System Proxy" => {
            if tun_enabled {
                commands.push(CommandIntent::ToggleTun { enabled: false });
            }
            commands.push(CommandIntent::SetSystemProxy { enabled: true });
        }
        "TUN" => {
            if proxy_enabled {
                commands.push(CommandIntent::SetSystemProxy { enabled: false });
            }
            commands.push(CommandIntent::ToggleTun { enabled: true });
        }
        "Core only" => {
            if proxy_enabled {
                commands.push(CommandIntent::SetSystemProxy { enabled: false });
            }
            if tun_enabled {
                commands.push(CommandIntent::ToggleTun { enabled: false });
            }
        }
        _ => {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "Unknown network mode",
                false,
            ));
        }
    }
    if running {
        commands.push(CommandIntent::StartProxy);
    }
    Ok(if commands.len() == 1 {
        SlintAction::Command(commands.remove(0))
    } else {
        SlintAction::Sequence(commands)
    })
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

fn selection_action(
    latest: &Arc<Mutex<Option<SurfaceSnapshot>>>,
    group: &str,
    node: &str,
) -> Result<SlintAction, Failure> {
    let snapshot = require_snapshot(latest)?;
    let inventory = snapshot.outbound_inventory.data.as_ref().ok_or_else(|| {
        Failure::new(
            ErrorCode::NotReady,
            "Outbound inventory is unavailable",
            true,
        )
    })?;
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = inventory.inventory()
    else {
        return Err(Failure::new(
            ErrorCode::NotReady,
            "Outbound inventory is not ready",
            true,
        ));
    };
    let collection = collections
        .iter()
        .find(|collection| collection.id.value() == group)
        .ok_or_else(|| Failure::new(ErrorCode::InvalidInput, "Unknown proxy group", false))?;
    let outbound = entries
        .iter()
        .find(|entry| entry.id.value() == node)
        .map(|entry| entry.id.clone())
        .ok_or_else(|| Failure::new(ErrorCode::InvalidInput, "Unknown proxy node", false))?;
    let request =
        OutboundSelectionRequest::new(collection.id.clone(), outbound, inventory.revision())
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
    Ok(SlintAction::Command(CommandIntent::SelectOutbound {
        request,
    }))
}

fn outbound_preference_action(
    latest: &Arc<Mutex<Option<SurfaceSnapshot>>>,
    intent: OutboundUiPreferenceIntent,
) -> Result<SlintAction, Failure> {
    let snapshot = require_snapshot(latest)?;
    let preferences = snapshot
        .outbound_ui_preferences
        .data
        .as_ref()
        .ok_or_else(|| {
            Failure::new(
                ErrorCode::NotReady,
                "Outbound presentation preferences are not available",
                true,
            )
        })?;
    let request = OutboundUiPreferenceRequest::new(preferences.revision(), intent)
        .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
    Ok(SlintAction::Command(
        CommandIntent::ApplyOutboundUiPreference { request },
    ))
}

fn probe_action(
    latest: &Arc<Mutex<Option<SurfaceSnapshot>>>,
    group: &str,
    node: &str,
) -> Result<SlintAction, Failure> {
    use infiltrator_contract::outbound::probe::{
        OutboundProbeBatchScope, OutboundProbeConcurrency, ProbeOutboundBatchRequest,
        ProbeOutboundRequest,
    };
    use infiltrator_contract::runtime_settings::RuntimeProbeSettingsState;

    let snapshot = require_snapshot(latest)?;
    let inventory = snapshot.outbound_inventory.data.as_ref().ok_or_else(|| {
        Failure::new(
            ErrorCode::NotReady,
            "Outbound inventory is unavailable",
            true,
        )
    })?;
    let settings = match &snapshot.probe_settings {
        RuntimeProbeSettingsState::Available { settings } => settings,
        RuntimeProbeSettingsState::Unavailable { failure } => return Err(failure.clone()),
    };
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = inventory.inventory()
    else {
        return Err(Failure::new(
            ErrorCode::NotReady,
            "Outbound inventory is not ready",
            true,
        ));
    };
    let map_error = |error: OutboundContractError| {
        Failure::new(ErrorCode::InvalidInput, error.to_string(), false)
    };
    if node.is_empty() {
        let collection = collections
            .iter()
            .find(|collection| collection.id.value() == group)
            .ok_or_else(|| Failure::new(ErrorCode::InvalidInput, "Unknown proxy group", false))?;
        let concurrency = OutboundProbeConcurrency::new(8).map_err(map_error)?;
        let request = ProbeOutboundBatchRequest::new(
            OutboundProbeBatchScope::Collection(collection.id.clone()),
            inventory.revision(),
            settings.url(),
            settings.timeout_ms() as u64,
            concurrency,
        )
        .map_err(map_error)?;
        return Ok(SlintAction::Command(CommandIntent::ProbeOutboundBatch {
            request,
        }));
    }
    let outbound = entries
        .iter()
        .find(|entry| entry.id.value() == node)
        .map(|entry| entry.id.clone())
        .ok_or_else(|| Failure::new(ErrorCode::InvalidInput, "Unknown proxy node", false))?;
    let request = ProbeOutboundRequest::new(
        outbound,
        inventory.revision(),
        settings.url(),
        settings.timeout_ms() as u64,
    )
    .map_err(map_error)?;
    Ok(SlintAction::Command(CommandIntent::ProbeOutbound {
        request,
    }))
}

fn parse_color(value: &str) -> Option<slint::Color> {
    let value = value.trim().trim_start_matches('#');
    if value.len() != 6 {
        return None;
    }
    let red = u8::from_str_radix(&value[0..2], 16).ok()?;
    let green = u8::from_str_radix(&value[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&value[4..6], 16).ok()?;
    Some(slint::Color::from_rgb_u8(red, green, blue))
}

fn apply_theme_color(weak: &slint::Weak<AppWindow>, hex: &str) {
    let Some(color) = parse_color(hex) else {
        return;
    };
    let hex = hex.to_owned();
    let _ = weak.clone().upgrade_in_event_loop(move |window| {
        window.set_theme_color(color);
        window.set_theme_color_input_code(hex.clone().into());
        window.set_theme_color_code_draft(hex.clone().into());
        window.set_theme_color_draft(hex.clone().into());
        window.set_theme_accent_light_foreground(contrast_is_light(&hex));
    });
}

fn contrast_is_light(hex: &str) -> bool {
    let Some(color) = parse_color(hex) else {
        return true;
    };
    let rgba = color.to_argb_u8();
    u32::from(rgba.red) * 299 + u32::from(rgba.green) * 587 + u32::from(rgba.blue) * 114 >= 128_000
}

fn hsv_hex(hue: f64, saturation: f64, value: f64) -> String {
    let hue = hue.rem_euclid(360.0) / 60.0;
    let saturation = saturation.clamp(0.0, 1.0);
    let value = value.clamp(0.0, 1.0);
    let chroma = value * saturation;
    let secondary = chroma * (1.0 - (hue.rem_euclid(2.0) - 1.0).abs());
    let offset = value - chroma;
    let (red, green, blue) = match hue as u8 {
        0 => (chroma, secondary, 0.0),
        1 => (secondary, chroma, 0.0),
        2 => (0.0, chroma, secondary),
        3 => (0.0, secondary, chroma),
        4 => (secondary, 0.0, chroma),
        _ => (chroma, 0.0, secondary),
    };
    format!(
        "#{:02X}{:02X}{:02X}",
        ((red + offset) * 255.0).round() as u8,
        ((green + offset) * 255.0).round() as u8,
        ((blue + offset) * 255.0).round() as u8,
    )
}

#[cfg(feature = "desktop-host")]
mod desktop {
    use super::{SlintAction, SlintProductHost, run_with_host};
    use infiltrator_application::core_application::CoreApplication;
    use infiltrator_application::surface_application::SurfacePump;
    use infiltrator_contract::command::{CommandIntent, CommandResult};
    use infiltrator_contract::core_identity::{CoreCommandRequest, CoreScope};
    use infiltrator_contract::error::{ErrorCode, Failure};
    use infiltrator_contract::surface::SurfaceKind;
    use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
    use infiltrator_desktop::product::DesktopProductSession;
    use infiltrator_shared::autostart::{is_autostart_enabled, set_autostart_enabled};
    use std::sync::Arc;

    struct DesktopProductHost {
        _session: Arc<DesktopProductSession>,
        application: Arc<CoreApplication>,
        pump: SurfacePump,
    }

    impl SlintProductHost for DesktopProductHost {
        fn read_snapshot(&self) -> Result<SurfaceSnapshot, Failure> {
            Ok(self.pump.current())
        }

        fn execute(
            &self,
            snapshot: Option<&SurfaceSnapshot>,
            action: SlintAction,
        ) -> Result<(), Failure> {
            match action {
                SlintAction::SetAutostart(enabled) => {
                    set_autostart_enabled("MusicFrogInfiltrator", enabled)
                        .map_err(|error| Failure::new(ErrorCode::Internal, error.to_string(), true))
                }
                SlintAction::Command(intent) => {
                    let snapshot = snapshot.ok_or_else(no_snapshot)?;
                    self.execute_commands(snapshot.core.scope(), [intent])
                }
                SlintAction::Sequence(intents) => {
                    let snapshot = snapshot.ok_or_else(no_snapshot)?;
                    self.execute_commands(snapshot.core.scope(), intents)
                }
            }
        }

        fn autostart_enabled(&self) -> Option<bool> {
            Some(is_autostart_enabled("MusicFrogInfiltrator"))
        }
    }

    impl DesktopProductHost {
        fn execute_commands(
            &self,
            scope: Option<CoreScope>,
            intents: impl IntoIterator<Item = CommandIntent>,
        ) -> Result<(), Failure> {
            let mut scope = scope.ok_or_else(no_snapshot)?;
            for intent in intents {
                let (request_id, receiver) =
                    self.application
                        .dispatch_scoped_tracked(CoreCommandRequest {
                            scope: scope.clone(),
                            intent,
                        });
                let outcome = receiver.recv().map_err(|_| {
                    Failure::new(
                        ErrorCode::Internal,
                        format!("Core command {request_id:?} ended without a result"),
                        true,
                    )
                })?;
                if let Some(next_scope) = outcome.scope {
                    scope = next_scope;
                }
                match outcome.result {
                    CommandResult::Completed { .. }
                    | CommandResult::Produced { .. }
                    | CommandResult::Accepted { .. } => {}
                    CommandResult::Rejected { failure, .. } => return Err(failure),
                }
            }
            Ok(())
        }
    }

    fn no_snapshot() -> Failure {
        Failure::new(
            ErrorCode::NotReady,
            "The shared owner has not published an active Core identity",
            true,
        )
    }

    pub fn run() -> anyhow::Result<()> {
        let session = Arc::new(DesktopProductSession::open(SurfaceKind::SlintDesktop)?);
        let host = Arc::new(DesktopProductHost {
            application: session.application(),
            pump: session.surface_pump(),
            _session: session,
        });
        run_with_host(host).map_err(|error| anyhow::anyhow!(error.to_string()))
    }
}

#[cfg(feature = "desktop-host")]
pub fn run_desktop() -> anyhow::Result<()> {
    desktop::run()
}

//! System tray, platform-split behind one neutral abstraction:
//!
//! * [`spec`] — the headlessly-testable seam: [`TraySpec`] (menu with stable
//!   action ids), [`TrayEvent`], [`TrayController`], [`TrayStartup`].
//! * [`menu`] — the pure localized spec builder over a [`TraySpecContext`]
//!   snapshot (0.20 full-feature menu).
//! * ksni backend (Linux default) — pure StatusNotifierItem/D-Bus, no GTK.
//! * native backend (Windows/macOS, or `native-tray-backend` on Linux) — the
//!   existing muda/tray-icon implementation wrapped behind the same seam.
//!
//! The tray is a pure enhancement: any startup failure degrades to a
//! window-only app with a warning and never fails or panics the process.

use iced::futures::channel::mpsc;
use std::time::Instant;
use tokio::time::sleep;
pub mod spec;

mod menu;

#[cfg(all(unix, not(target_os = "macos"), not(feature = "native-tray-backend")))]
#[cfg_attr(test, allow(dead_code))] // spawn path never runs under test
mod ksni_backend;
#[cfg(any(windows, target_os = "macos", feature = "native-tray-backend"))]
#[cfg_attr(test, allow(dead_code))] // spawn path never runs under test
mod native;

#[cfg(test)]
#[path = "../tests/gui/tray_tests.rs"]
mod tests;

use self::menu::build_tray_spec;
use self::spec::{
    TRAY_MAX_GROUPS, TrayCoreStatus, TrayEvent, TrayEventReceiver, TrayIntent, TrayProxyGroup,
    TrayProxyNode, TraySpec, TraySpecContext, TrayStartup, resolve_tray_event_in,
};
use crate::state::AppState;
use crate::types::app::{ConfirmAction, Route, ToastStatus};
use crate::types::message::Message;
use crate::types::runtime::RuntimeStatus;
use iced::advanced::subscription::{EventStream, Hasher, Recipe, from_recipe};
use iced::futures::stream::BoxStream;
use iced::{Subscription, Task, stream};
use infiltrator_contract::tray_status;
use lurker_core_contract::outbound::inventory::{
    OutboundInventoryState, OutboundSelectionState, SelectionAvailability,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The shared rate-badge push cadence (DUAL-15-02), in the host's time type.
pub(crate) const TRAY_RATE_REFRESH_INTERVAL: Duration =
    Duration::from_millis(tray_status::TRAY_RATE_REFRESH_INTERVAL_MS);

/// Spawn the platform tray with the given spec. Returns the typed startup
/// result; never panics and never fails the app.
#[cfg_attr(test, allow(dead_code))] // never called under test by design
pub(crate) fn spawn(spec: TraySpec) -> TrayStartup {
    #[cfg(all(unix, not(target_os = "macos"), not(feature = "native-tray-backend")))]
    return ksni_backend::spawn_ksni(spec);

    #[cfg(any(windows, target_os = "macos", feature = "native-tray-backend"))]
    return native::spawn_native(spec);
}

/// Shared receiver side of the tray event channel. The iced subscription
/// drains it; tests inject their own instance into the app state.
pub type SharedTrayEventReceiver = Arc<Mutex<TrayEventReceiver>>;

/// Drain every pending tray event without blocking.
pub(crate) fn drain_tray_events(rx: &SharedTrayEventReceiver) -> Vec<TrayEvent> {
    let rx = rx.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    events
}

/// Subscription recipe over the neutral channel (needed because iced 0.14's
/// `run`/`run_with` builders cannot capture the shared receiver).
struct TrayEventsRecipe {
    receiver: SharedTrayEventReceiver,
}

impl Recipe for TrayEventsRecipe {
    type Output = Message;

    fn hash(&self, state: &mut Hasher) {
        use std::hash::Hash;
        "infiltrator-tray-events".hash(state);
    }

    fn stream(self: Box<Self>, _input: EventStream) -> BoxStream<'static, Message> {
        let receiver = self.receiver;
        let channel = stream::channel(100, move |mut output: mpsc::Sender<Message>| async move {
            loop {
                for event in drain_tray_events(&receiver) {
                    let _ = output.try_send(Message::TrayEvent(event));
                }
                sleep(Duration::from_millis(20)).await;
            }
        });
        Box::pin(channel)
    }
}

/// Subscribe to tray activations from whichever backend is compiled in.
pub(crate) fn tray_events_subscription(
    receiver: &SharedTrayEventReceiver,
) -> Subscription<Message> {
    from_recipe(TrayEventsRecipe {
        receiver: Arc::clone(receiver),
    })
}

impl AppState {
    /// Map a tray activation onto the intent-driven action set: window,
    /// theme, routing intent, system proxy/TUN/autostart toggles,
    /// per-group node switching, profile activation/auto-update, kernel
    /// version management, core update, WebDAV sync and the destructive
    /// confirmations. Optimistic state changes are applied here so the
    /// tray/UI reflect the user's click immediately; the emitted follow-up
    /// messages run the same handlers as the in-app controls.
    pub fn handle_tray_event(&mut self, event: TrayEvent) -> Task<Message> {
        let ctx = self.tray_event_context();
        let Some(intent) = resolve_tray_event_in(&event, &ctx) else {
            return Task::none();
        };
        match intent {
            TrayIntent::ShowWindow => Task::done(Message::ShowWindow),
            TrayIntent::Exit => Task::done(Message::Exit),
            TrayIntent::ToggleTheme => Task::done(Message::ToggleTheme),
            TrayIntent::SetRoutingIntent(intent) => Task::done(Message::SetRoutingIntent(intent)),
            TrayIntent::SetSystemProxy(enabled) => {
                self.runtime.system_proxy_enabled = enabled;
                self.refresh_tray();
                Task::done(Message::SetSystemProxy(enabled))
            }
            TrayIntent::SetTunEnabled(enabled) => {
                self.runtime.tun_enabled = Some(enabled);
                self.refresh_tray();
                Task::done(Message::SetTunEnabled(enabled))
            }
            TrayIntent::SetAutostart(enabled) => {
                self.runtime.autostart_enabled = enabled;
                self.refresh_tray();
                Task::done(Message::SetAutostart(enabled))
            }
            TrayIntent::SelectGlobalOutbound(node) => self
                .outbound_selection_message("GLOBAL", &node)
                .map(Task::done)
                .unwrap_or_else(|| {
                    Task::done(Message::ShowToast(
                        "Outbound inventory is unavailable for this tray selection".into(),
                        ToastStatus::Info,
                    ))
                }),
            TrayIntent::SelectOutbound {
                collection,
                outbound,
            } => self
                .outbound_selection_message(&collection, &outbound)
                .map(Task::done)
                .unwrap_or_else(|| {
                    Task::done(Message::ShowToast(
                        "Outbound inventory is unavailable for this tray selection".into(),
                        ToastStatus::Info,
                    ))
                }),
            TrayIntent::ActivateProfile(name) => Task::done(Message::SetActiveProfile(name)),
            TrayIntent::UpdateAllProfilesNow => Task::done(Message::UpdateAllSubscriptionsNow),
            TrayIntent::SetProfileAutoUpdate { name, enabled } => {
                Task::done(Message::SetProfileAutoUpdate { name, enabled })
            }
            TrayIntent::SetDefaultKernel(version) => Task::done(Message::SetDefaultKernel(version)),
            TrayIntent::UninstallKernel(version) => Task::done(Message::RequestConfirmation(
                ConfirmAction::DeleteKernel(version),
            )),
            TrayIntent::UpdateCoreToLatest => Task::done(Message::CheckCoreUpdate),
            TrayIntent::CancelCoreDownload => Task::done(Message::CancelCoreDownload),
            TrayIntent::FlushFakeIp => Task::done(Message::FlushFakeIpCache),
            TrayIntent::SyncUpload => Task::done(Message::SyncUpload),
            TrayIntent::SyncDownload => Task::done(Message::SyncDownload),
            TrayIntent::CancelSync => Task::done(Message::CancelWebDavSync),
            TrayIntent::NavigateSync => Task::done(Message::Navigate(Route::Sync)),
            TrayIntent::RequestFactoryReset => {
                Task::done(Message::RequestConfirmation(ConfirmAction::FactoryReset))
            }
        }
    }

    /// Rebuild the [`TraySpec`] from current app state and push it to the
    /// tray backend. No-op when the tray is unavailable.
    pub fn refresh_tray(&self) {
        if let Some(controller) = &self.shell.tray_controller {
            controller.update_spec(self.current_tray_spec());
        }
    }

    /// Stream-driven refresh (core download / sync progress) at most once
    /// per second so the D-Bus menu never floods. Returns whether a spec was
    /// actually pushed.
    pub fn refresh_tray_throttled(&mut self) -> bool {
        let now = Instant::now();
        if self
            .shell
            .tray_refresh_cooldown
            .is_some_and(|at| now.duration_since(at) < TRAY_RATE_REFRESH_INTERVAL)
        {
            return false;
        }
        self.shell.tray_refresh_cooldown = Some(now);
        self.refresh_tray();
        true
    }

    /// DUAL-15-02: push the shared live rate badge when the newest real
    /// traffic sample changed its display text. The shared
    /// [`TRAY_RATE_REFRESH_INTERVAL_MS`] cadence and the text dedup keep an
    /// idle shell from pushing anything at all.
    pub fn refresh_tray_rates(&mut self) {
        let Some(badge) = tray_status::TrayRateBadge::from_waveform(&self.runtime.traffic_waveform)
        else {
            return;
        };
        let text = badge.badge_text();
        if self.shell.tray_last_rate_text.as_deref() == Some(text.as_str()) {
            return;
        }
        if self.refresh_tray_throttled() {
            self.shell.tray_last_rate_text = Some(text);
        }
    }

    /// The spec describing what the tray should show right now: assemble the
    /// full snapshot from the five app-state domains and hand it to the pure
    /// builder.
    pub fn current_tray_spec(&self) -> TraySpec {
        let groups = self.tray_proxy_groups();
        let sync_step = self
            .profile
            .sync_progress
            .as_ref()
            .map(|progress| (progress.current, progress.total));
        let core_download_percent = self.runtime.core_download.progress().and_then(|stats| {
            let total = stats.total?;
            (total > 0).then(|| ((stats.downloaded.min(total) * 100) / total) as u8)
        });
        let controller = self
            .runtime
            .runtime
            .as_ref()
            .map(|runtime| runtime.controller_url());
        let ctx = TraySpecContext {
            lang: &self.shell.lang,
            routing_intent: self.runtime.routing_intent.as_ref(),
            system_proxy: self.runtime.system_proxy_enabled,
            tun: self.runtime.tun_enabled.unwrap_or(false),
            groups: &groups,
            profiles: &self.profile.profiles,
            kernels: &self.runtime.installed_kernels,
            status: self.tray_core_status(),
            core_checking: self.runtime.core_update_check.is_checking(),
            core_downloading: self.runtime.core_download.is_downloading(),
            core_download_percent,
            webdav_enabled: self.profile.webdav_enabled,
            syncing: self.profile.webdav_sync_phase.is_syncing(),
            sync_step,
            autostart: self.runtime.autostart_enabled,
            controller: controller.as_deref(),
            admin_enabled: self.shell.admin_enabled,
            admin_port: self.shell.admin_port,
            rate_badge: tray_status::TrayRateBadge::from_waveform(&self.runtime.traffic_waveform),
        };
        build_tray_spec(&ctx)
    }

    /// App-side states the event resolver needs for its toggles.
    fn tray_event_context(&self) -> spec::TrayEventContext<'_> {
        spec::TrayEventContext {
            system_proxy: self.runtime.system_proxy_enabled,
            tun: self.runtime.tun_enabled.unwrap_or(false),
            autostart: self.runtime.autostart_enabled,
            profiles: &self.profile.profiles,
        }
    }

    fn tray_core_status(&self) -> TrayCoreStatus {
        match self.runtime.status {
            RuntimeStatus::Stopped => TrayCoreStatus::Stopped,
            RuntimeStatus::Starting => TrayCoreStatus::Starting,
            RuntimeStatus::Running => TrayCoreStatus::Running,
            RuntimeStatus::Error(_) => TrayCoreStatus::Error,
        }
    }

    /// Typed outbound collections for the node-selection submenu. Only
    /// collections explicitly observed as selectable become tray actions.
    fn tray_proxy_groups(&self) -> Vec<TrayProxyGroup> {
        let Some(snapshot) = self.runtime.outbound_inventory.data.as_ref() else {
            return Vec::new();
        };
        let OutboundInventoryState::Ready {
            entries,
            collections,
        } = snapshot.inventory()
        else {
            return Vec::new();
        };
        collections
            .iter()
            .filter(|collection| collection.availability == SelectionAvailability::Selectable)
            .take(TRAY_MAX_GROUPS)
            .map(|collection| TrayProxyGroup {
                name: collection.id.value().to_owned(),
                current: match &collection.selection {
                    OutboundSelectionState::Ready { selected } => selected.value().to_owned(),
                    OutboundSelectionState::Unknown
                    | OutboundSelectionState::Empty
                    | OutboundSelectionState::Unsupported
                    | OutboundSelectionState::Failed { .. } => String::new(),
                },
                nodes: collection
                    .member_ids
                    .iter()
                    .filter(|member_id| entries.iter().any(|entry| &entry.id == *member_id))
                    .map(|member_id| TrayProxyNode {
                        name: member_id.value().to_owned(),
                        delay_ms: None,
                    })
                    .collect(),
            })
            .collect()
    }
}

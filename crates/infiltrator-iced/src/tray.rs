//! System tray, platform-split behind one neutral abstraction:
//!
//! * [`spec`] — the headlessly-testable seam: [`TraySpec`] (menu with stable
//!   action ids), [`TrayEvent`], [`TrayController`], [`TrayStartup`].
//! * ksni backend (Linux default) — pure StatusNotifierItem/D-Bus, no GTK.
//! * native backend (Windows/macOS, or `native-tray-backend` on Linux) — the
//!   existing muda/tray-icon implementation wrapped behind the same seam.
//!
//! The tray is a pure enhancement: any startup failure degrades to a
//! window-only app with a warning and never fails or panics the process.

pub mod spec;

#[cfg(all(unix, not(target_os = "macos"), not(feature = "native-tray-backend")))]
#[cfg_attr(test, allow(dead_code))] // spawn path never runs under test
mod ksni_backend;
#[cfg(any(windows, target_os = "macos", feature = "native-tray-backend"))]
#[cfg_attr(test, allow(dead_code))] // spawn path never runs under test
mod native;

#[cfg(test)]
#[path = "../tests/gui/tray_tests.rs"]
mod tests;

pub use spec::{
    GlobalProxyMenu, TRAY_ACTION_MODE_DIRECT, TRAY_ACTION_MODE_GLOBAL, TRAY_ACTION_MODE_RULE,
    TRAY_ACTION_NO_PROXIES, TRAY_ACTION_OPEN_WEB_ADMIN, TRAY_ACTION_QUIT,
    TRAY_ACTION_SELECT_GLOBAL_PROXY, TRAY_ACTION_SHOW, TRAY_ACTION_TOGGLE_SYSTEM_PROXY,
    TRAY_ACTION_TOGGLE_THEME, TRAY_ACTION_TOGGLE_TUN, TRAY_SUBMENU_GLOBAL, TRAY_SUBMENU_MODE,
    TrayActionId, TrayController, TrayEvent, TrayEventReceiver, TrayIconData, TrayIntent,
    TrayMenuItem, TrayMenuSpec, TraySpec, TrayStartup, WebAdminMenu, build_tray_spec,
    load_icon_rgba, resolve_tray_event,
};

use crate::state::AppState;
use crate::types::Message;
use iced::advanced::subscription::{EventStream, Hasher, Recipe, from_recipe};
use iced::futures::stream::BoxStream;
use iced::{Subscription, Task, stream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
        let channel = stream::channel(
            100,
            move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                loop {
                    for event in drain_tray_events(&receiver) {
                        let _ = output.try_send(Message::TrayEvent(event));
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            },
        );
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
    /// Map a tray activation onto the exact same actions the old muda menu
    /// ids drove (mode switch, GLOBAL quick-switch, system proxy/TUN toggle,
    /// theme, show window, quit). Optimistic state changes are applied here
    /// so the tray/UI reflect the user's click immediately; the emitted
    /// follow-up messages run the same handlers as before (and re-set the
    /// same fields before doing their network work).
    pub fn handle_tray_event(&mut self, event: TrayEvent) -> Task<Message> {
        let tun = self.runtime.tun_enabled.unwrap_or(false);
        let intent = resolve_tray_event(&event, self.runtime.system_proxy_enabled, tun);
        match intent {
            Some(TrayIntent::ShowWindow) => Task::done(Message::ShowWindow),
            Some(TrayIntent::Exit) => Task::done(Message::Exit),
            Some(TrayIntent::ToggleTheme) => Task::done(Message::ToggleTheme),
            Some(TrayIntent::SetMode(mode)) => {
                self.runtime.proxy_mode = Some(mode.clone());
                self.refresh_tray();
                Task::done(Message::SetProxyMode(mode))
            }
            Some(TrayIntent::SetSystemProxy(enabled)) => {
                self.runtime.system_proxy_enabled = enabled;
                self.refresh_tray();
                Task::done(Message::SetSystemProxy(enabled))
            }
            Some(TrayIntent::SetTunEnabled(enabled)) => {
                self.runtime.tun_enabled = Some(enabled);
                self.refresh_tray();
                Task::done(Message::SetTunEnabled(enabled))
            }
            Some(TrayIntent::SelectGlobalProxy(node)) => {
                Task::done(Message::SelectProxy("GLOBAL".to_string(), node))
            }
            Some(TrayIntent::OpenWebAdmin) => self.open_web_admin(),
            None => Task::none(),
        }
    }

    /// Rebuild the [`TraySpec`] from current app state and push it to the
    /// tray backend. No-op when the tray is unavailable.
    pub fn refresh_tray(&self) {
        if let Some(controller) = &self.shell.tray_controller {
            controller.update_spec(self.current_tray_spec());
        }
    }

    /// The spec describing what the tray should show right now.
    pub fn current_tray_spec(&self) -> TraySpec {
        let global = self.global_proxy_menu();
        let web_admin = if self.shell.admin_enabled {
            Some(crate::tray::WebAdminMenu {
                running: self.shell.admin_server.is_running(),
            })
        } else {
            None
        };
        build_tray_spec(
            self.runtime.proxy_mode.as_deref(),
            self.runtime.system_proxy_enabled,
            self.runtime.tun_enabled.unwrap_or(false),
            global,
            web_admin,
        )
    }

    fn global_proxy_menu(&self) -> Option<GlobalProxyMenu<'_>> {
        let global = self.runtime.proxies.get("GLOBAL")?;
        let nodes = global.all()?;
        Some(GlobalProxyMenu {
            current: global.now().unwrap_or_default(),
            nodes,
        })
    }
}

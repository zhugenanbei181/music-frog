//! Linux StatusNotifierItem (D-Bus) tray backend, pure safe Rust through
//! `ksni`: no GTK, no libappindicator, no additional executor thread beyond
//! ksni's own service thread. The menu is re-derived from the neutral
//! [`TraySpec`] on every SNI `GetLayout`-style read; activations travel over
//! the same `std::sync::mpsc` channel the native backend uses.
//!
//! When no session bus or no StatusNotifierWatcher exists (e.g. GNOME without
//! the AppIndicator extension, headless sessions), [`spawn_ksni`] returns
//! [`TrayStartup::Unavailable`] and the app degrades to window-only.

use std::collections::HashMap;
use std::sync::mpsc::Sender;

use ksni::menu::{CheckmarkItem, StandardItem, SubMenu};
use ksni::{Icon, MenuItem, ToolTip};

use super::spec::{
    TrayActionId, TrayController, TrayEvent, TrayIconData, TrayMenuItem, TraySpec, TrayStartup,
};

/// Stable StatusNotifierItem id; consistent across sessions.
const TRAY_ID: &str = "MusicFrogInfiltrator";
const TRAY_TITLE: &str = "MusicFrog Infiltrator";

/// The ksni tray service. `menu()` re-derives the ksni menu from the pushed
/// spec on every read, so the app-side state stays the single source of truth.
pub(super) struct KsniTray {
    pub(super) spec: TraySpec,
    /// Checkmark overrides applied at click time (some shell toggle their own
    /// checkbox and the new spec only arrives one event-loop round later).
    pub(super) checked_overrides: HashMap<TrayActionId, bool>,
    pub(super) events: Sender<TrayEvent>,
}

impl KsniTray {
    fn send(&self, event: TrayEvent) {
        let _ = self.events.send(event);
    }
}

impl ksni::Tray for KsniTray {
    fn id(&self) -> String {
        TRAY_ID.to_owned()
    }

    fn title(&self) -> String {
        TRAY_TITLE.to_owned()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        self.spec
            .icon
            .as_ref()
            .map(to_ksni_icon)
            .into_iter()
            .collect()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            icon_pixmap: self.icon_pixmap(),
            title: TRAY_TITLE.to_owned(),
            description: self.spec.tooltip.clone(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayEvent::IconActivated);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        map_items(&self.spec.menu.items, &self.checked_overrides, &self.events)
    }
}

/// Map the neutral menu tree onto ksni menu items. Pure apart from the
/// cloned channel sender; unit-tested headlessly.
pub(super) fn map_items(
    items: &[TrayMenuItem],
    overrides: &HashMap<TrayActionId, bool>,
    events: &Sender<TrayEvent>,
) -> Vec<MenuItem<KsniTray>> {
    items
        .iter()
        .map(|item| match item {
            TrayMenuItem::Separator => MenuItem::Separator,
            TrayMenuItem::Action {
                id,
                label,
                enabled,
                payload,
            } => {
                let events = events.clone();
                let id = *id;
                let payload = payload.clone();
                StandardItem {
                    label: label.clone(),
                    enabled: *enabled,
                    activate: Box::new(move |_| {
                        let _ = events.send(TrayEvent::MenuActivated {
                            id,
                            payload: payload.clone(),
                        });
                    }),
                    ..Default::default()
                }
                .into()
            }
            TrayMenuItem::Checkmark {
                id,
                label,
                checked,
                enabled,
            } => {
                let initial = *checked;
                let events = events.clone();
                let id = *id;
                CheckmarkItem {
                    label: label.clone(),
                    checked: overrides.get(&id).copied().unwrap_or(initial),
                    enabled: *enabled,
                    activate: Box::new(move |tray: &mut KsniTray| {
                        let current = tray.checked_overrides.get(&id).copied().unwrap_or(initial);
                        tray.checked_overrides.insert(id, !current);
                        let _ = events.send(TrayEvent::MenuActivated { id, payload: None });
                    }),
                    ..Default::default()
                }
                .into()
            }
            TrayMenuItem::Submenu {
                label,
                enabled,
                items,
                ..
            } => SubMenu {
                label: label.clone(),
                enabled: *enabled,
                submenu: map_items(items, overrides, events),
                ..Default::default()
            }
            .into(),
        })
        .collect()
}

/// RGBA8 to ksni ARGB32 network byte order (rotate each `[r, g, b, a]`
/// pixel right by one byte to get `[a, r, g, b]`).
pub(super) fn to_ksni_icon(icon: &TrayIconData) -> Icon {
    let mut data = icon.rgba.clone();
    for pixel in data.as_chunks_mut::<4>().0 {
        pixel.rotate_right(1);
    }
    Icon {
        width: icon.width as i32,
        height: icon.height as i32,
        data,
    }
}

/// The [`TrayController`] for Linux; every mutation routes through the ksni
/// blocking handle to the service thread. Called from iced's main-thread
/// update paths only, never from inside a tokio worker, so the handle's
/// dedicated runtime never nests.
struct KsniController {
    handle: ksni::blocking::Handle<KsniTray>,
}

impl TrayController for KsniController {
    fn update_spec(&self, spec: TraySpec) {
        let _ = self.handle.update(move |tray: &mut KsniTray| {
            tray.spec = spec;
            // The pushed spec carries the authoritative checkmark states.
            tray.checked_overrides.clear();
        });
    }

    fn shutdown(&mut self) {
        // Dropping the handle stops the service thread and unregisters the
        // StatusNotifierItem; nothing further to do here.
    }
}

/// Spawn the ksni tray. Blocks briefly on D-Bus setup (fast-fail when no
/// session bus or no StatusNotifierWatcher exists). Never panics; a typed
/// [`TrayStartup::Unavailable`] lets the app continue window-only.
pub(super) fn spawn_ksni(spec: TraySpec) -> TrayStartup {
    use ksni::blocking::TrayMethods;

    let (events, receiver) = std::sync::mpsc::channel();
    let tray = KsniTray {
        spec,
        checked_overrides: HashMap::new(),
        events,
    };
    match tray.spawn() {
        Ok(handle) => TrayStartup::Ready {
            controller: Box::new(KsniController { handle }),
            events: receiver,
        },
        Err(error) => TrayStartup::Unavailable {
            reason: format!("StatusNotifierItem tray spawn failed: {error}"),
        },
    }
}

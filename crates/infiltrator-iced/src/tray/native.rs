//! Windows/macOS tray backend: the existing muda + tray-icon implementation,
//! wrapped behind the neutral [`TrayController`]/[`TraySpec`] seam. Since 0.20
//! the menu is fully generic: every [`update_spec`] push clears the root menu
//! and re-renders the whole [`TrayMenuItem`] tree from the spec (same source
//! of truth as the ksni backend), and the tooltip is refreshed too.
//!
//! Compiles on Linux too (muda/tray-icon keep the gtk feature at the
//! workspace root), which is how this backend stays checkable on this OS.

use muda::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use std::sync::mpsc::Sender;
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use super::spec::{
    TRAY_ACTION_NO_PROXIES, TrayActionId, TrayController, TrayEvent, TrayIconData, TrayMenuItem,
    TraySpec, TrayStartup,
};

pub(super) struct TrayManager {
    tray_icon: Option<TrayIcon>,
    menu: Menu,
}

impl TrayManager {
    pub fn new(icon: Option<&TrayIconData>) -> Self {
        // The menu content is driven entirely by apply_spec; spawn starts
        // with an empty menu so there is exactly one renderer of state.
        let menu = Menu::new();
        let mut builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu.clone()))
            .with_tooltip("MusicFrog Infiltrator");

        if let Some(spec_icon) = icon
            && let Ok(i) =
                Icon::from_rgba(spec_icon.rgba.clone(), spec_icon.width, spec_icon.height)
        {
            builder = builder.with_icon(i);
        }

        // 这里的 build() 可能因为系统不支持或资源冲突失败，包装在 Option 中
        let tray_icon = builder.build().ok();

        Self { tray_icon, menu }
    }

    /// Re-render the imperative muda menu from the neutral spec tree and
    /// refresh the tooltip. The spec is the single source of truth: labels
    /// (including `● ` markers and disabled placeholders), checkmark states,
    /// enabled flags and payloads all come straight from the spec builder.
    pub fn apply_spec(&self, spec: &TraySpec) {
        while self.menu.remove_at(0).is_some() {}
        for item in &spec.menu.items {
            if let Some(entry) = build_entry(item) {
                let _ = append_entry_to_menu(&self.menu, &entry);
            }
        }
        if let Some(icon) = &self.tray_icon {
            let _ = icon.set_tooltip(Some(spec.tooltip.as_str()));
        }
    }
}

/// A concrete muda widget built from one neutral spec entry. Kept as an enum
/// because `Menu` and `Submenu` share no append-capable trait.
enum MudaEntry {
    Command(MenuItem),
    Check(CheckMenuItem),
    Submenu(Submenu),
    Separator(PredefinedMenuItem),
}

fn build_entry(item: &TrayMenuItem) -> Option<MudaEntry> {
    match item {
        TrayMenuItem::Action {
            id,
            label,
            enabled,
            payload,
        } => Some(MudaEntry::Command(MenuItem::with_id(
            muda_id_for(*id, payload.as_deref()),
            label,
            *enabled,
            None,
        ))),
        TrayMenuItem::Checkmark {
            id,
            label,
            checked,
            enabled,
            payload,
        } => Some(MudaEntry::Check(CheckMenuItem::with_id(
            muda_id_for(*id, payload.as_deref()),
            label,
            *checked,
            *enabled,
            None,
        ))),
        TrayMenuItem::Submenu {
            id,
            label,
            enabled,
            items,
        } => {
            let submenu = Submenu::with_id(muda_id_for(*id, None), label, *enabled);
            append_entries(&submenu, items);
            Some(MudaEntry::Submenu(submenu))
        }
        TrayMenuItem::Separator => Some(MudaEntry::Separator(PredefinedMenuItem::separator())),
    }
}

fn append_entries(target: &Submenu, items: &[TrayMenuItem]) {
    for item in items {
        if let Some(entry) = build_entry(item) {
            let _ = append_entry_to_submenu(target, &entry);
        }
    }
}

fn append_entry_to_menu(target: &Menu, entry: &MudaEntry) -> muda::Result<()> {
    match entry {
        MudaEntry::Command(item) => target.append(item),
        MudaEntry::Check(item) => target.append(item),
        MudaEntry::Submenu(item) => target.append(item),
        MudaEntry::Separator(item) => target.append(item),
    }
}

fn append_entry_to_submenu(target: &Submenu, entry: &MudaEntry) -> muda::Result<()> {
    match entry {
        MudaEntry::Command(item) => target.append(item),
        MudaEntry::Check(item) => target.append(item),
        MudaEntry::Submenu(item) => target.append(item),
        MudaEntry::Separator(item) => target.append(item),
    }
}

/// muda menu ids are strings. The wire format mirrors the neutral id/payload
/// pair: `tray-action-<id>` for payload-less entries and
/// `tray-action-<id>:<payload>` otherwise (payloads may themselves contain
/// `:`, only the first separator splits). Submenu ids use the same encoding
/// for traceability; they are never emitted as activations.
fn muda_id_for(id: TrayActionId, payload: Option<&str>) -> String {
    match payload {
        Some(payload) => format!("tray-action-{id}:{payload}"),
        None => format!("tray-action-{id}"),
    }
}

/// Translate a muda menu id back into the neutral event. Unparseable ids
/// (foreign entries, races with menu teardown) fall back to the disabled
/// placeholder id, which resolves to no intent.
fn translate_menu_id(id: &str) -> TrayEvent {
    let parsed = id.strip_prefix("tray-action-").and_then(|rest| {
        let (action, payload) = match rest.split_once(':') {
            Some((action, payload)) => (action, Some(payload.to_string())),
            None => (rest, None),
        };
        action
            .parse::<TrayActionId>()
            .ok()
            .map(|action| (action, payload))
    });
    match parsed {
        Some((action, payload)) => TrayEvent::MenuActivated {
            id: action,
            payload,
        },
        None => TrayEvent::MenuActivated {
            id: TRAY_ACTION_NO_PROXIES,
            payload: None,
        },
    }
}

struct NativeTrayController {
    manager: TrayManager,
}

impl TrayController for NativeTrayController {
    fn update_spec(&self, spec: TraySpec) {
        self.manager.apply_spec(&spec);
    }

    fn shutdown(&mut self) {
        if let Some(icon) = &mut self.manager.tray_icon {
            let _ = icon.set_visible(false);
        }
    }
}

/// Spawn the muda/tray-icon tray and forward its native event streams into
/// the neutral channel. `builder.build()` failure (unsupported system,
/// resource conflict) degrades to a typed [`TrayStartup::Unavailable`].
pub(super) fn spawn_native(spec: TraySpec) -> TrayStartup {
    let (events, receiver) = std::sync::mpsc::channel();
    let manager = TrayManager::new(spec.icon.as_ref());

    if manager.tray_icon.is_none() {
        return TrayStartup::Unavailable {
            reason: "tray_icon build failed (unsupported system or resource conflict)".to_string(),
        };
    }

    manager.apply_spec(&spec);

    let menu_tx: Sender<TrayEvent> = events.clone();
    let _ = muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
        let _ = menu_tx.send(translate_menu_id(event.id.as_ref()));
    }));
    let _ = tray_icon::TrayIconEvent::set_event_handler(Some(move |event| {
        // Same behavior as before: any icon Click shows the main window.
        if matches!(event, tray_icon::TrayIconEvent::Click { .. })
            && events.send(TrayEvent::IconActivated).is_err()
        {
            // receiver dropped; stop caring
        }
    }));

    TrayStartup::Ready {
        controller: Box::new(NativeTrayController { manager }),
        events: receiver,
    }
}

#[cfg(test)]
mod id_codec {
    use super::*;

    #[test]
    fn muda_id_round_trips_action_and_payload() {
        // Payload-less actions round-trip bare.
        assert_eq!(muda_id_for(7, None), "tray-action-7");
        let TrayEvent::MenuActivated { id, payload } = translate_menu_id("tray-action-7") else {
            panic!("must parse into a menu activation");
        };
        assert_eq!((id, payload.as_deref()), (7, None));

        // Payload-carrying actions keep the payload verbatim, including
        // `:` and the `\u{1}` pair separator.
        let payload = format!("GLOBAL\u{1}node:1");
        let wire = muda_id_for(31, Some(&payload));
        assert_eq!(wire, format!("tray-action-31:{payload}"));
        let TrayEvent::MenuActivated { id, payload: back } = translate_menu_id(&wire) else {
            panic!("must parse into a menu activation");
        };
        assert_eq!(id, 31);
        assert_eq!(back.as_deref(), Some(payload.as_str()));
    }

    #[test]
    fn translate_menu_id_rejects_foreign_ids_via_placeholder() {
        let TrayEvent::MenuActivated { id, payload } = translate_menu_id("not-ours") else {
            panic!("fallback must still be a menu activation");
        };
        assert_eq!(id, TRAY_ACTION_NO_PROXIES);
        assert!(payload.is_none());

        // Numeric-looking garbage with a bad action part also falls back.
        let TrayEvent::MenuActivated { id, .. } = translate_menu_id("tray-action-99999:big") else {
            panic!("fallback must still be a menu activation");
        };
        assert_eq!(id, TRAY_ACTION_NO_PROXIES);
    }
}

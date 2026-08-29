//! Windows/macOS tray backend: the existing muda + tray-icon implementation,
//! wrapped behind the neutral [`TrayController`]/[`TraySpec`] seam. The menu
//! structure built in [`TrayManager::new`] is unchanged; what the spec drives
//! is the dynamic content (checkmarks + GLOBAL quick-switch entries) and the
//! translation of muda/tray-icon events into neutral [`TrayEvent`]s.
//!
//! Compiles on Linux too (muda/tray-icon keep the gtk feature at the
//! workspace root), which is how this backend stays checkable on this OS.

use muda::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use std::sync::mpsc::Sender;
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use super::spec::{
    TRAY_ACTION_MODE_DIRECT, TRAY_ACTION_MODE_GLOBAL, TRAY_ACTION_MODE_RULE,
    TRAY_ACTION_NO_PROXIES, TRAY_ACTION_QUIT, TRAY_ACTION_SELECT_GLOBAL_PROXY, TRAY_ACTION_SHOW,
    TRAY_ACTION_TOGGLE_SYSTEM_PROXY, TRAY_ACTION_TOGGLE_THEME, TRAY_ACTION_TOGGLE_TUN,
    TRAY_SUBMENU_GLOBAL, TRAY_SUBMENU_MODE, TrayActionId, TrayController, TrayEvent, TrayIconData,
    TrayMenuItem, TraySpec, TrayStartup,
};

pub(super) struct TrayManager {
    tray_icon: Option<TrayIcon>,
    mode_menu: Submenu,
    groups_menu: Submenu,
    system_proxy_item: CheckMenuItem,
    tun_mode_item: CheckMenuItem,
}

impl TrayManager {
    pub fn new(icon: Option<&TrayIconData>) -> Self {
        let menu = Menu::new();
        let show_item = MenuItem::with_id(
            muda_id_for(TRAY_ACTION_SHOW, None),
            "显示主界面",
            true,
            None,
        );

        let mode_menu = Submenu::new("代理模式", true);
        let mode_rule = MenuItem::with_id(
            muda_id_for(TRAY_ACTION_MODE_RULE, None),
            "规则模式",
            true,
            None,
        );
        let mode_global = MenuItem::with_id(
            muda_id_for(TRAY_ACTION_MODE_GLOBAL, None),
            "全局模式",
            true,
            None,
        );
        let mode_direct = MenuItem::with_id(
            muda_id_for(TRAY_ACTION_MODE_DIRECT, None),
            "直连模式",
            true,
            None,
        );
        let _ = mode_menu.append_items(&[&mode_rule, &mode_global, &mode_direct]);

        let groups_menu = Submenu::new("快速切换 (GLOBAL)", true);

        let system_proxy_item = CheckMenuItem::with_id(
            muda_id_for(TRAY_ACTION_TOGGLE_SYSTEM_PROXY, None),
            "系统代理",
            true,
            false,
            None,
        );
        let tun_mode_item = CheckMenuItem::with_id(
            muda_id_for(TRAY_ACTION_TOGGLE_TUN, None),
            "TUN 模式",
            true,
            false,
            None,
        );
        let theme_item = MenuItem::with_id(
            muda_id_for(TRAY_ACTION_TOGGLE_THEME, None),
            "切换深/浅色模式",
            true,
            None,
        );

        let quit_item =
            MenuItem::with_id(muda_id_for(TRAY_ACTION_QUIT, None), "退出应用", true, None);

        let _ = menu.append_items(&[
            &show_item,
            &PredefinedMenuItem::separator(),
            &mode_menu,
            &groups_menu,
            &PredefinedMenuItem::separator(),
            &system_proxy_item,
            &tun_mode_item,
            &theme_item,
            &PredefinedMenuItem::separator(),
            &quit_item,
        ]);

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

        Self {
            tray_icon,
            mode_menu,
            groups_menu,
            system_proxy_item,
            tun_mode_item,
        }
    }

    /// Translate the neutral spec onto the imperative muda items: checkmark
    /// states land on the stored `CheckMenuItem`s, the mode and GLOBAL
    /// quick-switch submenus are re-rendered from the spec entries.
    pub fn apply_spec(&self, spec: &TraySpec) {
        for item in &spec.menu.items {
            match item {
                TrayMenuItem::Checkmark {
                    id,
                    checked,
                    enabled: _,
                    label: _,
                } => match *id {
                    TRAY_ACTION_TOGGLE_SYSTEM_PROXY => {
                        let _ = self.system_proxy_item.set_checked(*checked);
                    }
                    TRAY_ACTION_TOGGLE_TUN => {
                        let _ = self.tun_mode_item.set_checked(*checked);
                    }
                    _ => {}
                },
                TrayMenuItem::Submenu {
                    id: TRAY_SUBMENU_MODE,
                    items,
                    ..
                } => self.apply_menu_entries(&self.mode_menu, items),
                TrayMenuItem::Submenu {
                    id: TRAY_SUBMENU_GLOBAL,
                    items,
                    ..
                } => self.apply_menu_entries(&self.groups_menu, items),
                _ => {}
            }
        }
    }

    /// Render spec action entries into a muda submenu. Entry labels
    /// (including the `● ` active markers and the disabled placeholder) come
    /// straight from the spec builder.
    fn apply_menu_entries(&self, target: &Submenu, entries: &[TrayMenuItem]) {
        while !target.items().is_empty() {
            let _ = target.remove_at(0);
        }

        for entry in entries {
            let TrayMenuItem::Action {
                id,
                label,
                enabled,
                payload,
            } = entry
            else {
                continue;
            };
            let item =
                MenuItem::with_id(muda_id_for(*id, payload.as_deref()), label, *enabled, None);
            let _ = target.append(&item);
        }
    }
}

/// muda menu ids are strings. Proxy entries keep their historical
/// `proxy_GLOBAL_<node>` form, the disabled placeholder keeps `no_proxies`,
/// and every fixed action uses its numeric [`TrayActionId`] so the event
/// translator can map ids back without a lookup table.
fn muda_id_for(id: TrayActionId, payload: Option<&str>) -> String {
    match id {
        TRAY_ACTION_SELECT_GLOBAL_PROXY => {
            format!("proxy_GLOBAL_{}", payload.unwrap_or_default())
        }
        TRAY_ACTION_NO_PROXIES => "no_proxies".to_string(),
        other => format!("tray-action-{other}"),
    }
}

/// Translate a muda menu id back into the neutral event.
fn translate_menu_id(id: &str) -> TrayEvent {
    if let Some(node) = id.strip_prefix("proxy_GLOBAL_") {
        TrayEvent::MenuActivated {
            id: TRAY_ACTION_SELECT_GLOBAL_PROXY,
            payload: Some(node.to_string()),
        }
    } else if let Some(rest) = id.strip_prefix("tray-action-")
        && let Ok(action) = rest.parse::<TrayActionId>()
    {
        TrayEvent::MenuActivated {
            id: action,
            payload: None,
        }
    } else {
        TrayEvent::MenuActivated {
            id: TRAY_ACTION_NO_PROXIES,
            payload: None,
        }
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

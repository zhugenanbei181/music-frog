//! Neutral, headlessly-testable tray abstraction.
//!
//! [`TraySpec`] describes the menu (stable action ids, checkable entries,
//! separators, submenus); [`TrayEvent`] reports user activations; the
//! [`TrayController`] trait pushes spec updates to whichever backend is
//! compiled in (ksni on Linux by default, muda/tray-icon on Windows/macOS or
//! with the `native-tray-backend` feature). Neither backend is referenced
//! here, so the spec, event resolution and the spec builder can all be unit
//! tested without a display server or a D-Bus session.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

/// Stable menu action id, shared by the spec builder, both backends and the
/// update handlers. Never reuse a number; the mapping is part of the contract.
pub type TrayActionId = u16;

pub const TRAY_ACTION_SHOW: TrayActionId = 1;
pub const TRAY_ACTION_QUIT: TrayActionId = 2;
pub const TRAY_ACTION_TOGGLE_THEME: TrayActionId = 3;
pub const TRAY_ACTION_MODE_RULE: TrayActionId = 4;
pub const TRAY_ACTION_MODE_GLOBAL: TrayActionId = 5;
pub const TRAY_ACTION_MODE_DIRECT: TrayActionId = 6;
pub const TRAY_ACTION_TOGGLE_SYSTEM_PROXY: TrayActionId = 7;
pub const TRAY_ACTION_TOGGLE_TUN: TrayActionId = 8;
pub const TRAY_ACTION_SELECT_GLOBAL_PROXY: TrayActionId = 9;
/// Disabled placeholder shown when the GLOBAL group has no nodes yet.
pub const TRAY_ACTION_NO_PROXIES: TrayActionId = 10;
/// Opens the admin WebUI in the system browser (only rendered when the
/// admin server feature is enabled in settings).
pub const TRAY_ACTION_OPEN_WEB_ADMIN: TrayActionId = 11;

/// Submenu identities (used by backends to find what to re-render; the id is
/// never reported as an activation).
pub const TRAY_SUBMENU_MODE: TrayActionId = 20;
pub const TRAY_SUBMENU_GLOBAL: TrayActionId = 21;

/// One entry of the neutral tray menu tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayMenuItem {
    /// Plain clickable entry. `payload` is opaque data (e.g. a proxy node
    /// name) echoed back verbatim in the [`TrayEvent::MenuActivated`] event.
    Action {
        id: TrayActionId,
        label: String,
        enabled: bool,
        payload: Option<String>,
    },
    /// Checkable entry; `checked` is the authoritative app-side state.
    Checkmark {
        id: TrayActionId,
        label: String,
        checked: bool,
        enabled: bool,
    },
    Separator,
    Submenu {
        id: TrayActionId,
        label: String,
        enabled: bool,
        items: Vec<TrayMenuItem>,
    },
}

impl TrayMenuItem {
    /// Convenience constructor for enabled, payload-less actions.
    pub fn action(id: TrayActionId, label: impl Into<String>) -> Self {
        TrayMenuItem::Action {
            id,
            label: label.into(),
            enabled: true,
            payload: None,
        }
    }

    /// Convenience constructor for enabled checkmarks.
    pub fn checkmark(id: TrayActionId, label: impl Into<String>, checked: bool) -> Self {
        TrayMenuItem::Checkmark {
            id,
            label: label.into(),
            checked,
            enabled: true,
        }
    }

    /// The action id of an `Action` entry (test/diagnostic convenience).
    pub fn action_id(&self) -> Option<TrayActionId> {
        match self {
            TrayMenuItem::Action { id, .. } => Some(*id),
            _ => None,
        }
    }

    /// The label of an `Action` entry (test/diagnostic convenience).
    pub fn action_label(&self) -> Option<&str> {
        match self {
            TrayMenuItem::Action { label, .. } => Some(label),
            _ => None,
        }
    }

    /// The payload of an `Action` entry (test/diagnostic convenience).
    pub fn action_payload(&self) -> Option<&str> {
        match self {
            TrayMenuItem::Action { payload, .. } => payload.as_deref(),
            _ => None,
        }
    }
}

/// The full neutral menu tree.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayMenuSpec {
    pub items: Vec<TrayMenuItem>,
}

/// RGBA8 (non-premultiplied) icon pixels; backend-neutral.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayIconData {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Everything a backend needs to render the tray.
#[derive(Clone, Debug)]
pub struct TraySpec {
    pub icon: Option<TrayIconData>,
    pub tooltip: String,
    pub menu: TrayMenuSpec,
}

/// A user activation coming from any backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayEvent {
    /// A menu entry was activated; `payload` is the [`TrayMenuItem::Action`]
    /// payload echoed back.
    MenuActivated {
        id: TrayActionId,
        payload: Option<String>,
    },
    /// The tray icon itself was activated (left click).
    IconActivated,
}

/// Receiver side of the backend event channel.
pub type TrayEventReceiver = Receiver<TrayEvent>;

/// Frontend intent behind a tray activation, fully resolved against the
/// current app state (checkmarks arrive already flipped).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayIntent {
    ShowWindow,
    Exit,
    ToggleTheme,
    SetMode(String),
    SetSystemProxy(bool),
    SetTunEnabled(bool),
    SelectGlobalProxy(String),
    OpenWebAdmin,
}

/// Pure mapping from a neutral event to an intent. `system_proxy`/`tun` are
/// the current app-side states used to resolve the toggle targets.
pub fn resolve_tray_event(event: &TrayEvent, system_proxy: bool, tun: bool) -> Option<TrayIntent> {
    let TrayEvent::MenuActivated { id, payload } = event else {
        return Some(TrayIntent::ShowWindow);
    };
    let payload = payload.as_deref();
    let intent = match *id {
        TRAY_ACTION_SHOW => TrayIntent::ShowWindow,
        TRAY_ACTION_QUIT => TrayIntent::Exit,
        TRAY_ACTION_TOGGLE_THEME => TrayIntent::ToggleTheme,
        TRAY_ACTION_MODE_RULE => TrayIntent::SetMode("rule".to_string()),
        TRAY_ACTION_MODE_GLOBAL => TrayIntent::SetMode("global".to_string()),
        TRAY_ACTION_MODE_DIRECT => TrayIntent::SetMode("direct".to_string()),
        TRAY_ACTION_TOGGLE_SYSTEM_PROXY => TrayIntent::SetSystemProxy(!system_proxy),
        TRAY_ACTION_TOGGLE_TUN => TrayIntent::SetTunEnabled(!tun),
        TRAY_ACTION_SELECT_GLOBAL_PROXY => TrayIntent::SelectGlobalProxy(payload?.to_string()),
        TRAY_ACTION_OPEN_WEB_ADMIN => TrayIntent::OpenWebAdmin,
        _ => return None,
    };
    Some(intent)
}

/// The controller handle handed to the app on a successful tray startup.
/// Both backends implement it; the app only ever sees this trait.
///
/// Deliberately `!Send`: controllers are used from iced's main thread only
/// (muda's Linux menu is `Rc`-based and neither `Send` nor `Sync`; the ksni
/// blocking handle is `Send + Sync` but is confined all the same).
pub trait TrayController {
    /// Push a full spec refresh (checkmarks, mode, GLOBAL quick-switch
    /// entries). Best-effort: errors are swallowed, as they were before.
    fn update_spec(&self, spec: TraySpec);

    /// Best-effort resource release. Dropping the controller also cleans up;
    /// this exists for explicit teardown (e.g. before process exit).
    fn shutdown(&mut self);
}

/// Typed startup result. A tray is a pure enhancement: any failure degrades
/// to a window-only app and must never fail or panic the process.
pub enum TrayStartup {
    Ready {
        controller: Box<dyn TrayController>,
        events: Receiver<TrayEvent>,
    },
    Unavailable {
        reason: String,
    },
}

/// Snapshot of the mihomo GLOBAL group used for the quick-switch submenu.
#[derive(Clone, Copy, Debug)]
pub struct GlobalProxyMenu<'a> {
    pub current: &'a str,
    pub nodes: &'a [String],
}

/// Tray presence of the admin WebUI entry. `None` hides the entry entirely
/// (feature disabled in settings); `running` enables the click.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WebAdminMenu {
    pub running: bool,
}

const TOOLTIP: &str = "MusicFrog Infiltrator";

/// Build the localized tray spec from current app state. Mirrors the menu the
/// muda backend used to build imperatively: 显示主界面 / 打开 Web 管理端 /
/// 代理模式 (rule/global/direct, active mode marked with `●`) / 快速切换
/// (GLOBAL) / 系统代理 + TUN 模式 checkmarks / theme / quit.
pub fn build_tray_spec(
    mode: Option<&str>,
    system_proxy: bool,
    tun: bool,
    global: Option<GlobalProxyMenu<'_>>,
    web_admin: Option<WebAdminMenu>,
) -> TraySpec {
    let mode_item = |id: TrayActionId, mode_key: &str, label: &str| {
        let is_active = mode.is_some_and(|current| current == mode_key);
        TrayMenuItem::Action {
            id,
            label: if is_active {
                format!("● {label}")
            } else {
                label.to_string()
            },
            enabled: true,
            payload: None,
        }
    };
    let mode_items = vec![
        mode_item(TRAY_ACTION_MODE_RULE, "rule", "规则模式"),
        mode_item(TRAY_ACTION_MODE_GLOBAL, "global", "全局模式"),
        mode_item(TRAY_ACTION_MODE_DIRECT, "direct", "直连模式"),
    ];
    let global_items = match global {
        Some(global) if !global.nodes.is_empty() => global
            .nodes
            .iter()
            .map(|node| {
                let is_active = node == global.current;
                TrayMenuItem::Action {
                    id: TRAY_ACTION_SELECT_GLOBAL_PROXY,
                    label: if is_active {
                        format!("● {node}")
                    } else {
                        node.clone()
                    },
                    enabled: true,
                    payload: Some(node.clone()),
                }
            })
            .collect(),
        _ => vec![TrayMenuItem::Action {
            id: TRAY_ACTION_NO_PROXIES,
            label: "暂无节点 (请先启动)".to_string(),
            enabled: false,
            payload: None,
        }],
    };

    let web_admin_item = web_admin.map(|info| {
        TrayMenuItem::Action {
            id: TRAY_ACTION_OPEN_WEB_ADMIN,
            label: "打开 Web 管理端".to_string(),
            enabled: info.running,
            payload: None,
        }
    });

    let mut items = vec![TrayMenuItem::action(TRAY_ACTION_SHOW, "显示主界面")];
    if let Some(item) = web_admin_item {
        items.push(item);
    }
    items.push(TrayMenuItem::Separator);
    items.extend([
        TrayMenuItem::Submenu {
            id: TRAY_SUBMENU_MODE,
            label: "代理模式".to_string(),
            enabled: true,
            items: mode_items,
        },
        TrayMenuItem::Submenu {
            id: TRAY_SUBMENU_GLOBAL,
            label: "快速切换 (GLOBAL)".to_string(),
            enabled: true,
            items: global_items,
        },
        TrayMenuItem::Separator,
        TrayMenuItem::checkmark(TRAY_ACTION_TOGGLE_SYSTEM_PROXY, "系统代理", system_proxy),
        TrayMenuItem::checkmark(TRAY_ACTION_TOGGLE_TUN, "TUN 模式", tun),
        TrayMenuItem::action(TRAY_ACTION_TOGGLE_THEME, "切换深/浅色模式"),
        TrayMenuItem::Separator,
        TrayMenuItem::action(TRAY_ACTION_QUIT, "退出应用"),
    ]);

    TraySpec {
        icon: load_icon_rgba(),
        tooltip: TOOLTIP.to_string(),
        menu: TrayMenuSpec { items },
    }
}

/// Resolve an RGBA icon shared by both backends. Development builds resolve
/// against the crate's own icons directory so the icon is found regardless of
/// the current working directory; packaged builds fall back to the resource
/// directory shipped next to the binary (cwd-relative or exe-relative).
pub fn load_icon_rgba() -> Option<TrayIconData> {
    let mut icon_dirs = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("icons")];
    if let Ok(cwd) = std::env::current_dir() {
        icon_dirs.push(cwd.join("icons"));
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        icon_dirs.push(dir.join("icons"));
    }
    for dir in icon_dirs {
        let ico = dir.join("icon.ico");
        if ico.exists()
            && let Some(data) = load_rgba_from_path(&ico)
        {
            return Some(data);
        }
        let png = dir.join("icon.png");
        if png.exists()
            && let Some(data) = load_rgba_from_path(&png)
        {
            return Some(data);
        }
    }
    None
}

fn load_rgba_from_path(path: &Path) -> Option<TrayIconData> {
    let image = image::open(path).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    Some(TrayIconData {
        width,
        height,
        rgba: image.into_raw(),
    })
}

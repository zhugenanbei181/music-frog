//! Neutral, headlessly-testable tray abstraction.
//!
//! [`TraySpec`] describes the menu (stable action ids, checkable entries,
//! separators, submenus); [`TrayEvent`] reports user activations; the
//! [`TrayController`] trait pushes spec updates to whichever backend is
//! compiled in (ksni on Linux by default, muda/tray-icon on Windows/macOS or
//! with the `native-tray-backend` feature). Neither backend is referenced
//! here, so the spec, event resolution and the spec builder (see [`build_tray_spec`],
//! implemented in `super::menu`) can all be unit tested without a display
//! server or a D-Bus session.
//!
//! Red line: everything in this module is pure data plus pure functions —
//! no app-state access, no I/O, no backend calls.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use infiltrator_domain::profiles::ProfileInfo;
use infiltrator_contract::version::InstalledCoreVersion;

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
/// Legacy GLOBAL quick-switch entry (the dedicated submenu was folded into
/// the per-group 节点切换 submenu in 0.20; the id stays reserved).
pub const TRAY_ACTION_SELECT_GLOBAL_PROXY: TrayActionId = 9;
/// Disabled placeholder shown when no proxy groups are available yet.
pub const TRAY_ACTION_NO_PROXIES: TrayActionId = 10;
/// Script mode; only enabled when the loaded profile carries a script block.
pub const TRAY_ACTION_MODE_SCRIPT: TrayActionId = 30;
/// Generic per-node switch; payload is `group␁node` (see [`encode_pair_payload`]).
pub const TRAY_ACTION_SELECT_PROXY: TrayActionId = 31;
/// Disabled placeholder shown when no profiles exist yet.
pub const TRAY_ACTION_NO_PROFILES: TrayActionId = 32;
/// Activate one profile; payload is the profile name.
pub const TRAY_ACTION_ACTIVATE_PROFILE: TrayActionId = 33;
pub const TRAY_ACTION_UPDATE_ALL_PROFILES: TrayActionId = 34;
/// Per-profile auto-update toggle; payload is the profile name.
pub const TRAY_ACTION_SET_PROFILE_AUTO_UPDATE: TrayActionId = 35;
/// Set the default kernel; payload is the version string.
pub const TRAY_ACTION_SET_DEFAULT_KERNEL: TrayActionId = 36;
/// Uninstall one kernel (staged behind a confirmation); payload is the version.
pub const TRAY_ACTION_UNINSTALL_KERNEL: TrayActionId = 37;
pub const TRAY_ACTION_CHECK_CORE_UPDATE: TrayActionId = 38;
pub const TRAY_ACTION_CANCEL_CORE_DOWNLOAD: TrayActionId = 39;
pub const TRAY_ACTION_FLUSH_FAKEIP: TrayActionId = 40;
pub const TRAY_ACTION_SYNC_UPLOAD: TrayActionId = 41;
pub const TRAY_ACTION_SYNC_DOWNLOAD: TrayActionId = 42;
pub const TRAY_ACTION_CANCEL_SYNC: TrayActionId = 43;
pub const TRAY_ACTION_NAVIGATE_SYNC: TrayActionId = 44;
pub const TRAY_ACTION_TOGGLE_AUTOSTART: TrayActionId = 45;
pub const TRAY_ACTION_FACTORY_RESET: TrayActionId = 46;

/// Informational, always-disabled entries. They exist so the reader sees
/// static state in place; they must never resolve to an intent.
pub const TRAY_ACTION_INFO_MODE: TrayActionId = 80;
pub const TRAY_ACTION_INFO_STATUS: TrayActionId = 81;
pub const TRAY_ACTION_INFO_CONTROLLER: TrayActionId = 82;
pub const TRAY_ACTION_INFO_ADMIN: TrayActionId = 83;
pub const TRAY_ACTION_INFO_KERNEL_VERSION: TrayActionId = 84;
pub const TRAY_ACTION_INFO_SYNC: TrayActionId = 85;
pub const TRAY_ACTION_INFO_KERNEL_DEFAULT: TrayActionId = 86;
pub const TRAY_ACTION_INFO_KERNEL_STATUS: TrayActionId = 87;
pub const TRAY_ACTION_INFO_DOWNLOAD: TrayActionId = 88;

/// Submenu identities (used for tree identity and tests; the id is never
/// reported as an activation).
pub const TRAY_SUBMENU_MODE: TrayActionId = 20;
pub const TRAY_SUBMENU_PROXIES: TrayActionId = 22;
pub const TRAY_SUBMENU_PROFILES: TrayActionId = 23;
pub const TRAY_SUBMENU_KERNEL: TrayActionId = 24;
pub const TRAY_SUBMENU_SYNC: TrayActionId = 25;
pub const TRAY_SUBMENU_INFO: TrayActionId = 26;
/// Per-group submenus under 节点切换: ids 61..=65 (at most 5 groups).
pub const TRAY_SUBMENU_PROXY_GROUP_BASE: TrayActionId = 61;
/// Nested "remaining nodes" overflow submenus: ids 66..=70.
pub const TRAY_SUBMENU_PROXY_MORE_BASE: TrayActionId = 66;
/// Per-version submenus under 内核: ids 90 and up.
pub const TRAY_SUBMENU_KERNEL_VERSION_BASE: TrayActionId = 90;

/// Tray proxy switch caps (aligned with the retired Tauri tray ledger §1.1).
pub const TRAY_MAX_GROUPS: usize = 5;
pub const TRAY_MAX_NODES_PER_GROUP: usize = 20;

/// Field separator inside a multi-value [`TrayMenuItem::Action`] payload.
/// `\u{1}` (SOH) cannot appear in proxy group/node names, profile names or
/// version strings, so the pair encoding is unambiguous.
pub const PAYLOAD_SEPARATOR: char = '\u{1}';

/// Encode a two-field action payload (`group` + `node`).
pub fn encode_pair_payload(first: &str, second: &str) -> String {
    format!("{first}{PAYLOAD_SEPARATOR}{second}")
}

/// Decode a two-field action payload; `None` when the separator is missing.
pub fn decode_pair_payload(payload: &str) -> Option<(&str, &str)> {
    payload.split_once(PAYLOAD_SEPARATOR)
}

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
    /// `payload` identifies the checked thing (e.g. a profile name) and is
    /// echoed back in the activation event.
    Checkmark {
        id: TrayActionId,
        label: String,
        checked: bool,
        enabled: bool,
        payload: Option<String>,
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

    /// Convenience constructor for a disabled, payload-less informational
    /// action (the tray's read-only state lines).
    pub fn info(id: TrayActionId, label: impl Into<String>) -> Self {
        TrayMenuItem::Action {
            id,
            label: label.into(),
            enabled: false,
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
            payload: None,
        }
    }

    /// Enabled checkmark carrying an identifying payload.
    pub fn checkmark_with_payload(
        id: TrayActionId,
        label: impl Into<String>,
        checked: bool,
        payload: impl Into<String>,
    ) -> Self {
        TrayMenuItem::Checkmark {
            id,
            label: label.into(),
            checked,
            enabled: true,
            payload: Some(payload.into()),
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
    /// A menu entry was activated; `payload` is the entry's payload echoed
    /// back (also set for checkmarks that carry one).
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
    /// Legacy GLOBAL quick-switch (id 9, kept for the never-reuse contract);
    /// live menus route node selection through [`TrayIntent::SelectProxy`].
    SelectGlobalProxy(String),
    SelectProxy {
        group: String,
        node: String,
    },
    ActivateProfile(String),
    UpdateAllProfilesNow,
    SetProfileAutoUpdate {
        name: String,
        enabled: bool,
    },
    SetDefaultKernel(String),
    /// Stage a kernel uninstall behind the confirmation dialog.
    UninstallKernel(String),
    UpdateCoreToLatest,
    CancelCoreDownload,
    FlushFakeIp,
    SetAutostart(bool),
    SyncUpload,
    SyncDownload,
    CancelSync,
    NavigateSync,
    RequestFactoryReset,
}

/// App-side state snapshot needed to resolve one activation event.
#[derive(Clone, Copy, Debug)]
pub struct TrayEventContext<'a> {
    pub system_proxy: bool,
    pub tun: bool,
    pub autostart: bool,
    pub profiles: &'a [ProfileInfo],
}

/// Pure mapping from a neutral event to an intent. Toggle entries are
/// resolved against the current app-side states carried in `ctx`
/// (checkmark targets arrive already flipped).
pub fn resolve_tray_event_in(event: &TrayEvent, ctx: &TrayEventContext<'_>) -> Option<TrayIntent> {
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
        TRAY_ACTION_MODE_SCRIPT => TrayIntent::SetMode("script".to_string()),
        TRAY_ACTION_TOGGLE_SYSTEM_PROXY => TrayIntent::SetSystemProxy(!ctx.system_proxy),
        TRAY_ACTION_TOGGLE_TUN => TrayIntent::SetTunEnabled(!ctx.tun),
        TRAY_ACTION_TOGGLE_AUTOSTART => TrayIntent::SetAutostart(!ctx.autostart),
        TRAY_ACTION_SELECT_GLOBAL_PROXY => TrayIntent::SelectGlobalProxy(payload?.to_string()),
        TRAY_ACTION_SELECT_PROXY => {
            let (group, node) = decode_pair_payload(payload?)?;
            TrayIntent::SelectProxy {
                group: group.to_string(),
                node: node.to_string(),
            }
        }
        TRAY_ACTION_ACTIVATE_PROFILE => TrayIntent::ActivateProfile(payload?.to_string()),
        TRAY_ACTION_UPDATE_ALL_PROFILES => TrayIntent::UpdateAllProfilesNow,
        TRAY_ACTION_SET_PROFILE_AUTO_UPDATE => {
            let name = payload?;
            let current = ctx.profiles.iter().find(|p| p.name == name)?;
            TrayIntent::SetProfileAutoUpdate {
                name: name.to_string(),
                enabled: !current.auto_update_enabled,
            }
        }
        TRAY_ACTION_SET_DEFAULT_KERNEL => TrayIntent::SetDefaultKernel(payload?.to_string()),
        TRAY_ACTION_UNINSTALL_KERNEL => TrayIntent::UninstallKernel(payload?.to_string()),
        TRAY_ACTION_CHECK_CORE_UPDATE => TrayIntent::UpdateCoreToLatest,
        TRAY_ACTION_CANCEL_CORE_DOWNLOAD => TrayIntent::CancelCoreDownload,
        TRAY_ACTION_FLUSH_FAKEIP => TrayIntent::FlushFakeIp,
        TRAY_ACTION_SYNC_UPLOAD => TrayIntent::SyncUpload,
        TRAY_ACTION_SYNC_DOWNLOAD => TrayIntent::SyncDownload,
        TRAY_ACTION_CANCEL_SYNC => TrayIntent::CancelSync,
        TRAY_ACTION_NAVIGATE_SYNC => TrayIntent::NavigateSync,
        TRAY_ACTION_FACTORY_RESET => TrayIntent::RequestFactoryReset,
        _ => return None,
    };
    Some(intent)
}

/// Backward-compatible three-argument form of [`resolve_tray_event_in`]:
/// resolves against the system-proxy/TUN states with no autostart and an
/// empty profile list (entries needing them resolve to `None`).
pub fn resolve_tray_event(event: &TrayEvent, system_proxy: bool, tun: bool) -> Option<TrayIntent> {
    resolve_tray_event_in(
        event,
        &TrayEventContext {
            system_proxy,
            tun,
            autostart: false,
            profiles: &[],
        },
    )
}

/// One proxy group snapshot for the 节点切换 submenu (owned: the assembly in
/// `tray.rs` derives it per refresh from the runtime proxy map).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayProxyGroup {
    pub name: String,
    pub current: String,
    pub nodes: Vec<TrayProxyNode>,
}

/// One switchable node plus its latest measured delay, if any.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayProxyNode {
    pub name: String,
    pub delay_ms: Option<u32>,
}

/// Core lifecycle state, mirrored from `RuntimeStatus` without its payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayCoreStatus {
    Stopped,
    Starting,
    Running,
    Error,
}

/// The i18n key describing a [`TrayCoreStatus`].
pub fn tray_status_key(status: TrayCoreStatus) -> &'static str {
    match status {
        TrayCoreStatus::Running => "tray_status_running",
        TrayCoreStatus::Starting => "tray_status_starting",
        TrayCoreStatus::Stopped => "tray_status_stopped",
        TrayCoreStatus::Error => "tray_status_error",
    }
}

/// Full snapshot of everything the localized spec builder needs. Assembled
/// from the five app-state domains by `AppState::current_tray_spec`; the
/// builder stays a pure function of this snapshot.
#[derive(Clone, Debug)]
pub struct TraySpecContext<'a> {
    pub lang: &'a str,
    /// Current proxy mode (`rule` / `global` / `direct` / `script`).
    pub mode: Option<&'a str>,
    /// Whether the running core reports a top-level `script:` block.
    pub script_block_present: bool,
    pub system_proxy: bool,
    pub tun: bool,
    /// Proxy group snapshots (at most [`TRAY_MAX_GROUPS`], GLOBAL first).
    pub groups: &'a [TrayProxyGroup],
    pub profiles: &'a [ProfileInfo],
    pub kernels: &'a [InstalledCoreVersion],
    pub status: TrayCoreStatus,
    pub core_checking: bool,
    pub core_downloading: bool,
    /// Download progress in whole percent, when the total size is known.
    pub core_download_percent: Option<u8>,
    pub webdav_enabled: bool,
    pub syncing: bool,
    /// `(current, total)` step counters while a sync is running.
    pub sync_step: Option<(usize, usize)>,
    pub autostart: bool,
    /// External controller URL of the running core (`http://127.0.0.1:9090`).
    pub controller: Option<&'a str>,
    pub admin_enabled: bool,
    pub admin_port: u16,
}

/// The controller handle handed to the app on a successful tray startup.
/// Both backends implement it; the app only ever sees this trait.
///
/// Deliberately `!Send`: controllers are used from iced's main thread only
/// (muda's Linux menu is `Rc`-based and neither `Send` nor `Sync`; the ksni
/// blocking handle is `Send + Sync` but is confined all the same).
pub trait TrayController {
    /// Push a full spec refresh (checkmarks, submenus, info lines). Best
    /// effort: errors are swallowed, as they were before.
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

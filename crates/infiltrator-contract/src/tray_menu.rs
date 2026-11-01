//! Shared tray menu vocabulary (FLPARITY-006).
//!
//! The system tray is a per-surface host capability, but the *menu shape* is
//! a product fact: the same action ids, the same tree, the same payload
//! encoding. Every desktop surface (Iced, Bevy, Slint) renders the tray from
//! one neutral tree, so a user reads the same "切换路由 / 切换配置 / 系统代理"
//! entries no matter which window they launched.
//!
//! This module owns that vocabulary:
//!
//! * [`TrayActionId`] and the action-id constants — the stable wire ids a
//!   backend echoes back on activation;
//! * [`TrayMenuContext`] — the *owned* snapshot every surface projects its
//!   state into (no borrowed domain types, so the Iced app state, the Bevy
//!   surface snapshot and the Slint surface snapshot all feed the same
//!   builder);
//! * [`TrayMenuItem`] / [`TrayMenuSpec`] — the neutral menu tree;
//! * [`TraySpec`] — icon pixels + tooltip + menu, everything a backend needs.
//!
//! Red line: pure data only. The localized *rendering* of the tree lives in
//! the desktop host layer; no surface builds a second copy of the menu.

use crate::tray_status::TrayRateBadge;
use lurker_core_contract::routing_intent::RoutingIntent;
use serde::{Deserialize, Serialize};

/// Stable menu action id, shared by the spec builder, backends and handlers.
pub type TrayActionId = u16;

pub const TRAY_ACTION_SHOW: TrayActionId = 1;
pub const TRAY_ACTION_QUIT: TrayActionId = 2;
pub const TRAY_ACTION_TOGGLE_THEME: TrayActionId = 3;
pub const TRAY_ACTION_ROUTING_FOLLOW_RULES: TrayActionId = 4;
pub const TRAY_ACTION_ROUTING_ALL_THROUGH_PROXY: TrayActionId = 5;
pub const TRAY_ACTION_ROUTING_BYPASS_PROXY: TrayActionId = 6;
pub const TRAY_ACTION_TOGGLE_SYSTEM_PROXY: TrayActionId = 7;
pub const TRAY_ACTION_TOGGLE_TUN: TrayActionId = 8;
pub const TRAY_ACTION_SELECT_GLOBAL_PROXY: TrayActionId = 9;
pub const TRAY_ACTION_NO_PROXIES: TrayActionId = 10;
pub const TRAY_ACTION_SELECT_PROXY: TrayActionId = 31;
pub const TRAY_ACTION_NO_PROFILES: TrayActionId = 32;
pub const TRAY_ACTION_ACTIVATE_PROFILE: TrayActionId = 33;
pub const TRAY_ACTION_UPDATE_ALL_PROFILES: TrayActionId = 34;
pub const TRAY_ACTION_SET_PROFILE_AUTO_UPDATE: TrayActionId = 35;
pub const TRAY_ACTION_SET_DEFAULT_KERNEL: TrayActionId = 36;
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
pub const TRAY_ACTION_TOGGLE_COLOR_MODE: TrayActionId = 47;

pub const TRAY_ACTION_INFO_ROUTING_INTENT: TrayActionId = 80;
pub const TRAY_ACTION_INFO_STATUS: TrayActionId = 81;
pub const TRAY_ACTION_INFO_CONTROLLER: TrayActionId = 82;
pub const TRAY_ACTION_INFO_ADMIN: TrayActionId = 83;
pub const TRAY_ACTION_INFO_KERNEL_VERSION: TrayActionId = 84;
pub const TRAY_ACTION_INFO_SYNC: TrayActionId = 85;
pub const TRAY_ACTION_INFO_KERNEL_DEFAULT: TrayActionId = 86;
pub const TRAY_ACTION_INFO_KERNEL_STATUS: TrayActionId = 87;
pub const TRAY_ACTION_INFO_DOWNLOAD: TrayActionId = 88;
pub const TRAY_ACTION_INFO_RATE: TrayActionId = 89;

pub const TRAY_SUBMENU_ROUTING_INTENT: TrayActionId = 20;
pub const TRAY_SUBMENU_PROXIES: TrayActionId = 22;
pub const TRAY_SUBMENU_PROFILES: TrayActionId = 23;
pub const TRAY_SUBMENU_KERNEL: TrayActionId = 24;
pub const TRAY_SUBMENU_SYNC: TrayActionId = 25;
pub const TRAY_SUBMENU_INFO: TrayActionId = 26;
pub const TRAY_SUBMENU_PROXY_GROUP_BASE: TrayActionId = 61;
pub const TRAY_SUBMENU_PROXY_MORE_BASE: TrayActionId = 66;
pub const TRAY_SUBMENU_KERNEL_VERSION_BASE: TrayActionId = 90;

pub const TRAY_MAX_GROUPS: usize = 5;
pub const TRAY_MAX_NODES_PER_GROUP: usize = 20;

/// Field separator inside a multi-value menu payload.
///
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

/// One profile as the tray menu needs it: identity, selection state and the
/// per-profile auto-update preference.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayProfileEntry {
    pub name: String,
    pub active: bool,
    pub auto_update_enabled: bool,
}

/// One installed kernel as the tray menu needs it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayKernelEntry {
    pub version: String,
    pub is_default: bool,
}

/// One switchable outbound node plus its latest measured delay, if any.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayOutboundNode {
    pub name: String,
    pub delay_ms: Option<u32>,
}

/// One proxy group as the tray menu needs it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayOutboundGroup {
    pub name: String,
    pub current: String,
    pub nodes: Vec<TrayOutboundNode>,
}

/// Core lifecycle state, mirrored from the runtime status without its payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrayCoreStatus {
    Stopped,
    Starting,
    Running,
    Error,
}

/// The tray action families a hosting surface can honor.
///
/// The menu tree is one product fact, but a surface that cannot execute an
/// action must not offer it: the shared builder drops the families the host
/// reports as unsupported, so no tray entry is ever a silent no-op.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayMenuCapabilities {
    pub routing_intent: bool,
    pub outbound_selection: bool,
    pub profile_activation: bool,
    pub profile_auto_update: bool,
    pub system_proxy: bool,
    pub tun: bool,
    pub theme_toggle: bool,
    pub icon_color_mode: bool,
    pub kernel_management: bool,
    pub sync: bool,
    pub factory_reset: bool,
    pub autostart: bool,
}

impl TrayMenuCapabilities {
    /// Every action family — the reference host (Iced) honours them all.
    pub const fn full() -> Self {
        Self {
            routing_intent: true,
            outbound_selection: true,
            profile_activation: true,
            profile_auto_update: true,
            system_proxy: true,
            tun: true,
            theme_toggle: true,
            icon_color_mode: true,
            kernel_management: true,
            sync: true,
            factory_reset: true,
            autostart: true,
        }
    }

    /// The families every desktop surface shares: the routing choice, the
    /// profile switch, the system proxy, TUN and the tray icon style.
    ///
    /// A host that cannot execute the remaining families reports them as
    /// unsupported, and the shared builder leaves those entries out of the menu
    /// instead of offering a click that would do nothing.
    pub const fn shared_desktop() -> Self {
        Self {
            routing_intent: true,
            outbound_selection: false,
            profile_activation: true,
            profile_auto_update: false,
            system_proxy: true,
            tun: true,
            theme_toggle: false,
            icon_color_mode: true,
            kernel_management: false,
            sync: false,
            factory_reset: false,
            autostart: false,
        }
    }
}

impl Default for TrayMenuCapabilities {
    fn default() -> Self {
        Self::full()
    }
}

/// The owned snapshot every desktop surface projects its state into before
/// the shared builder renders the menu tree.
///
/// It is deliberately owned (no borrowed domain types): the Iced app state,
/// the Bevy desktop host and the Slint desktop host each read their own
/// authoritative facts and hand them over here, so the menu shape stays a
/// single product fact while the values stay owned by their own surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrayMenuContext {
    /// BCP-47 language tag the menu is rendered in.
    pub lang: String,
    /// Current typed routing intent.
    pub routing_intent: Option<RoutingIntent>,
    pub system_proxy: bool,
    pub tun: bool,
    /// Proxy groups (at most [`TRAY_MAX_GROUPS`], GLOBAL first).
    pub groups: Vec<TrayOutboundGroup>,
    pub profiles: Vec<TrayProfileEntry>,
    pub kernels: Vec<TrayKernelEntry>,
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
    pub controller: Option<String>,
    pub admin_enabled: bool,
    pub admin_port: u16,
    /// The shared live rate badge projected from the newest real traffic
    /// sample; `None` until the host delivers a real sample, so no surface
    /// ever fabricates a `0 B/s` badge.
    pub rate_badge: Option<TrayRateBadge>,
    /// The action families this host can honor.
    pub capabilities: TrayMenuCapabilities,
}

impl Default for TrayMenuContext {
    fn default() -> Self {
        Self {
            lang: "zh-CN".to_string(),
            routing_intent: None,
            system_proxy: false,
            tun: false,
            groups: Vec::new(),
            profiles: Vec::new(),
            kernels: Vec::new(),
            status: TrayCoreStatus::Stopped,
            core_checking: false,
            core_downloading: false,
            core_download_percent: None,
            webdav_enabled: false,
            syncing: false,
            sync_step: None,
            autostart: false,
            controller: None,
            admin_enabled: false,
            admin_port: 25210,
            rate_badge: None,
            capabilities: TrayMenuCapabilities::full(),
        }
    }
}

/// One entry of the neutral tray menu tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrayMenuItem {
    Action {
        id: TrayActionId,
        label: String,
        enabled: bool,
        payload: Option<String>,
    },
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
    pub fn action(id: TrayActionId, label: impl Into<String>) -> Self {
        Self::Action {
            id,
            label: label.into(),
            enabled: true,
            payload: None,
        }
    }

    pub fn info(id: TrayActionId, label: impl Into<String>) -> Self {
        Self::Action {
            id,
            label: label.into(),
            enabled: false,
            payload: None,
        }
    }

    pub fn checkmark(id: TrayActionId, label: impl Into<String>, checked: bool) -> Self {
        Self::Checkmark {
            id,
            label: label.into(),
            checked,
            enabled: true,
            payload: None,
        }
    }

    pub fn checkmark_with_payload(
        id: TrayActionId,
        label: impl Into<String>,
        checked: bool,
        payload: impl Into<String>,
    ) -> Self {
        Self::Checkmark {
            id,
            label: label.into(),
            checked,
            enabled: true,
            payload: Some(payload.into()),
        }
    }

    pub fn action_id(&self) -> Option<TrayActionId> {
        match self {
            Self::Action { id, .. } | Self::Checkmark { id, .. } | Self::Submenu { id, .. } => {
                Some(*id)
            }
            Self::Separator => None,
        }
    }

    pub fn action_label(&self) -> Option<&str> {
        match self {
            Self::Action { label, .. }
            | Self::Checkmark { label, .. }
            | Self::Submenu { label, .. } => Some(label.as_str()),
            Self::Separator => None,
        }
    }

    pub fn action_payload(&self) -> Option<&str> {
        match self {
            Self::Action { payload, .. } | Self::Checkmark { payload, .. } => payload.as_deref(),
            Self::Submenu { .. } | Self::Separator => None,
        }
    }
}

/// The full neutral menu tree.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayMenuSpec {
    pub items: Vec<TrayMenuItem>,
}

/// RGBA8 (non-premultiplied) icon pixels; backend-neutral.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayIconData {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Everything a backend needs to render the tray.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraySpec {
    pub icon: Option<TrayIconData>,
    pub tooltip: String,
    pub menu: TrayMenuSpec,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_context_defaults_to_a_stopped_core_without_fabricated_facts() {
        let ctx = TrayMenuContext::default();
        assert_eq!(ctx.status, TrayCoreStatus::Stopped);
        assert!(ctx.groups.is_empty());
        assert!(ctx.profiles.is_empty());
        assert!(ctx.rate_badge.is_none());
        assert!(!ctx.system_proxy);
        assert!(!ctx.tun);
    }

    #[test]
    fn the_menu_item_factories_carry_the_authoritative_state() {
        let action = TrayMenuItem::action(TRAY_ACTION_SHOW, "Show");
        assert_eq!(action.action_id(), Some(TRAY_ACTION_SHOW));
        assert_eq!(action.action_label(), Some("Show"));
        assert_eq!(action.action_payload(), None);

        let info = TrayMenuItem::info(TRAY_ACTION_INFO_STATUS, "Running");
        assert_eq!(
            info,
            TrayMenuItem::Action {
                id: TRAY_ACTION_INFO_STATUS,
                label: "Running".to_string(),
                enabled: false,
                payload: None,
            }
        );

        let check = TrayMenuItem::checkmark_with_payload(
            TRAY_ACTION_ACTIVATE_PROFILE,
            "Profile 1",
            true,
            "prof-1",
        );
        assert_eq!(check.action_id(), Some(TRAY_ACTION_ACTIVATE_PROFILE));
        assert_eq!(check.action_payload(), Some("prof-1"));
    }

    #[test]
    fn the_payload_encoding_round_trips_one_pair() {
        let encoded = encode_pair_payload("ProxyGroup", "NodeA");
        let (group, node) = decode_pair_payload(&encoded).expect("decoded pair");
        assert_eq!(group, "ProxyGroup");
        assert_eq!(node, "NodeA");
        assert!(decode_pair_payload("no-separator").is_none());
    }
}

//! Shared accessibility-semantics grammar for the shell (DUAL-15-10).
//!
//! AccessKit is published by the Bevy surface (winit bridge); Iced 0.14 has no
//! AccessKit integration at all. Both surfaces still have to describe the same
//! shell — to a screen reader where one exists, and to a visible label/tooltip
//! where it does not — so the *grammar* lives here:
//!
//! * one toolkit-neutral [`A11yRole`] per shell node;
//! * one i18n key ([`ShellA11yNode::label_key`]) for the localized surfaces;
//! * one bare-Chinese label ([`ShellA11yNode::label_zh`]) for the Bevy
//!   convention;
//! * [`ShellA11yNode::ALL`] as the coverage inventory a surface can iterate,
//!   so a mounted shell can prove every node is described with the shared
//!   role and label instead of an ad-hoc string.
//!
//! Bevy turns a spec into a real `AccessibilityNode`; Iced resolves the label
//! key through its own localizer for visible text/tooltips. Iced cannot publish
//! roles, and the group guard forbids it from pretending otherwise.

/// Toolkit-neutral semantic role of a shell node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum A11yRole {
    Window,
    Header,
    Region,
    Navigation,
    Button,
    Switch,
    Status,
    Dialog,
    /// A live region that announces its content as it changes (toasts).
    LiveRegion,
    /// A read-only text line.
    Text,
}

/// One node of the shell's accessibility grammar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ShellA11yNode {
    /// The top-level application window.
    Window,
    /// The shell header row (logo, title, status).
    ShellHeader,
    /// The routed content region.
    ContentRegion,
    /// The sidebar navigation region.
    SidebarNav,
    /// The core run-state dot beside the header actions.
    GlobalStatusDot,
    /// The proxy-mode segmented control.
    ModeSegment,
    /// The sidebar system-proxy master switch.
    SystemProxySwitch,
    /// The sidebar TUN master switch.
    TunSwitch,
    /// The header theme toggle.
    ThemeToggle,
    /// The Mini HUD card.
    MiniHudCard,
    /// The Mini HUD system-proxy quick switch.
    MiniHudSystemProxySwitch,
    /// The Mini HUD TUN quick switch.
    MiniHudTunSwitch,
    /// The command palette dialog.
    CommandPaletteDialog,
    /// The toast live region.
    ToastRegion,
    /// The duplex live-rate readout.
    TrafficReadout,
    /// The frameless chrome strip's minimize control.
    ChromeMinimize,
    /// The frameless chrome strip's maximize/restore control.
    ChromeMaximize,
    /// The frameless chrome strip's close control.
    ChromeClose,
}

impl ShellA11yNode {
    /// Every node a complete shell description must cover.
    pub const ALL: [Self; 18] = [
        Self::Window,
        Self::ShellHeader,
        Self::ContentRegion,
        Self::SidebarNav,
        Self::GlobalStatusDot,
        Self::ModeSegment,
        Self::SystemProxySwitch,
        Self::TunSwitch,
        Self::ThemeToggle,
        Self::MiniHudCard,
        Self::MiniHudSystemProxySwitch,
        Self::MiniHudTunSwitch,
        Self::CommandPaletteDialog,
        Self::ToastRegion,
        Self::TrafficReadout,
        Self::ChromeMinimize,
        Self::ChromeMaximize,
        Self::ChromeClose,
    ];

    /// The node's role in the accessibility tree.
    pub const fn role(self) -> A11yRole {
        match self {
            Self::Window => A11yRole::Window,
            Self::ShellHeader => A11yRole::Header,
            Self::ContentRegion => A11yRole::Region,
            Self::SidebarNav => A11yRole::Navigation,
            Self::GlobalStatusDot => A11yRole::Status,
            Self::TrafficReadout => A11yRole::Text,
            Self::ModeSegment | Self::MiniHudCard => A11yRole::Region,
            Self::SystemProxySwitch
            | Self::TunSwitch
            | Self::MiniHudSystemProxySwitch
            | Self::MiniHudTunSwitch => A11yRole::Switch,
            Self::ThemeToggle => A11yRole::Button,
            Self::CommandPaletteDialog => A11yRole::Dialog,
            Self::ToastRegion => A11yRole::LiveRegion,
            Self::ChromeMinimize | Self::ChromeMaximize | Self::ChromeClose => A11yRole::Button,
        }
    }

    /// The Iced localization key for the node's label.
    pub const fn label_key(self) -> &'static str {
        match self {
            Self::Window => "a11y_window",
            Self::ShellHeader => "a11y_shell_header",
            Self::ContentRegion => "a11y_content_region",
            Self::SidebarNav => "a11y_sidebar_nav",
            Self::GlobalStatusDot => "a11y_global_status_dot",
            Self::ModeSegment => "a11y_mode_segment",
            Self::SystemProxySwitch => "a11y_system_proxy_switch",
            Self::TunSwitch => "a11y_tun_switch",
            Self::ThemeToggle => "a11y_theme_toggle",
            Self::MiniHudCard => "a11y_mini_hud_card",
            Self::MiniHudSystemProxySwitch => "a11y_mini_hud_system_proxy",
            Self::MiniHudTunSwitch => "a11y_mini_hud_tun",
            Self::CommandPaletteDialog => "a11y_command_palette",
            Self::ToastRegion => "a11y_toast_region",
            Self::TrafficReadout => "a11y_traffic_readout",
            Self::ChromeMinimize => "chrome_minimize",
            Self::ChromeMaximize => "chrome_maximize",
            Self::ChromeClose => "chrome_close",
        }
    }

    /// The bare-Chinese label the Bevy surface publishes (Bevy strings follow
    /// the bare-Chinese convention; the Iced surface localizes `label_key`).
    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::Window => "应用主窗口",
            Self::ShellHeader => "应用标题栏",
            Self::ContentRegion => "内容区",
            Self::SidebarNav => "页面导航",
            Self::GlobalStatusDot => "内核运行状态",
            Self::ModeSegment => "代理模式选择",
            Self::SystemProxySwitch => "系统代理开关",
            Self::TunSwitch => "TUN 模式开关",
            Self::ThemeToggle => "切换深浅主题",
            Self::MiniHudCard => "网速悬浮窗",
            Self::MiniHudSystemProxySwitch => "悬浮窗系统代理开关",
            Self::MiniHudTunSwitch => "悬浮窗 TUN 开关",
            Self::CommandPaletteDialog => "命令面板",
            Self::ToastRegion => "通知与告警",
            Self::TrafficReadout => "上下行实时速率",
            Self::ChromeMinimize => "最小化",
            Self::ChromeMaximize => "最大化 / 还原",
            Self::ChromeClose => "关闭",
        }
    }

    /// Whether this node is an on/off switch that must carry its state.
    pub const fn is_switch(self) -> bool {
        matches!(self.role(), A11yRole::Switch)
    }

    /// The full spec row for this node.
    pub const fn spec(self) -> A11yNodeSpec {
        A11yNodeSpec {
            node: self,
            role: self.role(),
            label_key: self.label_key(),
            label_zh: self.label_zh(),
        }
    }
}

/// One row of the shell accessibility grammar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct A11yNodeSpec {
    pub node: ShellA11yNode,
    pub role: A11yRole,
    pub label_key: &'static str,
    pub label_zh: &'static str,
}

/// Every grammar row, in inventory order.
pub fn shell_a11y_specs() -> Vec<A11yNodeSpec> {
    ShellA11yNode::ALL.iter().map(|node| node.spec()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_shell_node_carries_a_role_and_both_label_sources() {
        for node in ShellA11yNode::ALL {
            let spec = node.spec();
            assert_eq!(spec.node, node);
            assert_eq!(spec.role, node.role());
            assert!(!spec.label_key.is_empty(), "{node:?} has no label key");
            assert!(
                !spec.label_zh.trim().is_empty(),
                "{node:?} has no bare-Chinese label"
            );
            assert_eq!(spec.label_key, node.label_key());
            assert_eq!(spec.label_zh, node.label_zh());
        }
        assert_eq!(shell_a11y_specs().len(), ShellA11yNode::ALL.len());
    }

    #[test]
    fn the_coverage_inventory_is_unique_and_complete() {
        let unique: BTreeSet<ShellA11yNode> = ShellA11yNode::ALL.iter().copied().collect();
        assert_eq!(unique.len(), ShellA11yNode::ALL.len());
        let keys: BTreeSet<&str> = ShellA11yNode::ALL
            .iter()
            .map(|node| node.label_key())
            .collect();
        assert_eq!(
            keys.len(),
            ShellA11yNode::ALL.len(),
            "label keys must be unique"
        );
    }

    #[test]
    fn the_switch_and_dialog_roles_match_their_interaction() {
        assert!(ShellA11yNode::SystemProxySwitch.is_switch());
        assert!(ShellA11yNode::TunSwitch.is_switch());
        assert!(ShellA11yNode::MiniHudSystemProxySwitch.is_switch());
        assert!(ShellA11yNode::MiniHudTunSwitch.is_switch());
        assert!(!ShellA11yNode::ThemeToggle.is_switch());
        assert_eq!(ShellA11yNode::CommandPaletteDialog.role(), A11yRole::Dialog);
        assert_eq!(ShellA11yNode::ToastRegion.role(), A11yRole::LiveRegion);
    }
}

//! Shared command-palette catalogue.
//!
//! Both surfaces render the palette from this one list: the identifiers, the
//! categories, the i18n key / bare-Chinese copy and the typed
//! [`CommandTarget`] are the contract; each surface only maps a target onto
//! its own dispatch vocabulary (Iced `Message`, Bevy [`UiCommand`]). The
//! global-chord actions reuse [`ShortcutAction`] so a palette row and its
//! `Ctrl+…` chord land in the same handler on both ends.
//!
//! Surface-local navigation that has no counterpart (the Iced config editor)
//! is *not* smuggled into this list; a surface appends its own entry after
//! the shared catalogue and the ledger records the deviation.

use crate::command::ProxyMode;
use crate::shortcuts::ShortcutAction;

/// A page both surfaces can navigate to by a stable, shared id.
///
/// This is not either toolkit's route enum: Iced folds `Logs` into its
/// runtime page (the runtime page hosts the logs section) and Bevy has a
/// dedicated logs page. The id is the contract, the route mapping is
/// surface-local.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ShellPage {
    Overview,
    Proxies,
    Profiles,
    Rules,
    Connections,
    Logs,
    Dns,
    Doctor,
    AppRouting,
    Sync,
    Settings,
}

impl ShellPage {
    /// Every shared page in stable enumeration order.
    pub const ALL: [Self; 11] = [
        Self::Overview,
        Self::Proxies,
        Self::Profiles,
        Self::Rules,
        Self::Connections,
        Self::Logs,
        Self::Dns,
        Self::Doctor,
        Self::AppRouting,
        Self::Sync,
        Self::Settings,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Proxies => "proxies",
            Self::Profiles => "profiles",
            Self::Rules => "rules",
            Self::Connections => "connections",
            Self::Logs => "logs",
            Self::Dns => "dns",
            Self::Doctor => "doctor",
            Self::AppRouting => "app_routing",
            Self::Sync => "sync",
            Self::Settings => "settings",
        }
    }

    /// Iced locale key for the page's navigation copy.
    pub const fn title_key(self) -> &'static str {
        match self {
            Self::Overview => "cmd_nav_overview",
            Self::Proxies => "cmd_nav_proxies",
            Self::Profiles => "cmd_nav_profiles",
            Self::Rules => "cmd_nav_rules",
            Self::Connections => "cmd_nav_connections",
            Self::Logs => "cmd_nav_logs",
            Self::Dns => "cmd_nav_dns",
            Self::Doctor => "cmd_nav_doctor",
            Self::AppRouting => "nav_app_routing",
            Self::Sync => "cmd_nav_sync",
            Self::Settings => "cmd_nav_settings",
        }
    }

    /// Bare-Chinese copy for the Bevy surface (existing convention).
    pub const fn title_zh(self) -> &'static str {
        match self {
            Self::Overview => "前往 核心概览",
            Self::Proxies => "前往 代理策略",
            Self::Profiles => "前往 配置订阅",
            Self::Rules => "前往 分流规则",
            Self::Connections => "前往 连接审计",
            Self::Logs => "前往 运行日志",
            Self::Dns => "前往 域名解析",
            Self::Doctor => "前往 自愈诊断",
            Self::AppRouting => "前往 应用分流",
            Self::Sync => "前往 数据同步",
            Self::Settings => "前往 系统设置",
        }
    }
}

/// The palette group a command row belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CommandCategory {
    Navigation,
    Modes,
    Maintenance,
    Appearance,
    Profiles,
}

impl CommandCategory {
    /// Iced locale key for the category badge.
    pub const fn label_key(self) -> &'static str {
        match self {
            Self::Navigation => "cmd_cat_nav",
            Self::Modes => "cmd_cat_modes",
            Self::Maintenance => "cmd_cat_actions",
            Self::Appearance => "cmd_cat_appearance",
            Self::Profiles => "cmd_cat_profiles",
        }
    }

    /// Bare-Chinese category copy for the Bevy surface (and for the shared
    /// substring matcher, so "代理模式" finds the mode rows on both ends).
    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::Navigation => "页面导航",
            Self::Modes => "代理模式",
            Self::Maintenance => "快捷运维",
            Self::Appearance => "外观偏好",
            Self::Profiles => "切换配置",
        }
    }
}

/// What running a palette row does, in toolkit-neutral terms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandTarget {
    /// Navigate to a shared page.
    Navigate(ShellPage),
    /// Switch the core proxy mode.
    SetProxyMode(ProxyMode),
    /// Activate one stored subscription profile.
    SwitchProfile { id: String, name: String },
    /// Toggle the OS system proxy (also the `Ctrl+Alt+P` binding).
    ToggleSystemProxy,
    /// Toggle the TUN adapter (also the `Ctrl+Alt+T` binding).
    ToggleTun,
    /// Show/hide the Mini HUD (also the `Ctrl+Alt+M` binding).
    ToggleMiniHud,
    /// Cycle the appearance preference (also the `Ctrl+Alt+D` binding).
    CycleTheme,
    /// Flush the Fake-IP table and the OS resolver cache.
    FlushDnsCache,
    /// Run the latency benchmark across every proxy group.
    TestAllProxyGroups,
    /// Run the Doctor diagnostics suite.
    RunDoctor,
    /// Close every live connection.
    CloseAllConnections,
    /// Restart the core lifecycle.
    RestartKernel,
}

impl CommandTarget {
    /// The global chord that also dispatches this target, when one exists.
    /// The surfaces route both entries through their single shortcut
    /// handler instead of duplicating the toggle logic.
    pub const fn shortcut_action(&self) -> Option<ShortcutAction> {
        match self {
            Self::ToggleSystemProxy => Some(ShortcutAction::ToggleSystemProxy),
            Self::ToggleTun => Some(ShortcutAction::ToggleTun),
            Self::ToggleMiniHud => Some(ShortcutAction::ToggleMiniHud),
            Self::CycleTheme => Some(ShortcutAction::CycleTheme),
            _ => None,
        }
    }
}

/// One row of the palette.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandEntry {
    pub id: String,
    pub category: CommandCategory,
    /// Iced locale key (profile rows reuse the category key).
    pub title_key: &'static str,
    /// Fully resolved bare-Chinese copy for the Bevy surface.
    pub title_zh: String,
    pub target: CommandTarget,
}

impl CommandEntry {
    pub fn new(
        id: &'static str,
        category: CommandCategory,
        title_key: &'static str,
        title_zh: &'static str,
        target: CommandTarget,
    ) -> Self {
        Self {
            id: id.to_owned(),
            category,
            title_key,
            title_zh: title_zh.to_owned(),
            target,
        }
    }

    /// A profile-switch row built from a real stored profile.
    pub fn profile(id: &str, name: &str) -> Self {
        Self {
            id: format!("profile.{id}"),
            category: CommandCategory::Profiles,
            title_key: "cmd_cat_profiles",
            title_zh: format!("切换配置：{name}"),
            target: CommandTarget::SwitchProfile {
                id: id.to_owned(),
                name: name.to_owned(),
            },
        }
    }

    /// The profile name of a profile-switch row.
    pub fn profile_name(&self) -> Option<&str> {
        match &self.target {
            CommandTarget::SwitchProfile { name, .. } => Some(name.as_str()),
            _ => None,
        }
    }

    /// Case-insensitive substring match used by both surfaces. The Iced
    /// surface additionally applies its pinyin matcher on top; the shared
    /// substring rule is what keeps the plain-language results identical.
    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return true;
        }
        self.id.to_lowercase().contains(&query)
            || self.title_zh.to_lowercase().contains(&query)
            || self.title_key.to_lowercase().contains(&query)
            || self.category.label_key().contains(&query)
            || self.category.label_zh().contains(&query)
    }
}

/// A stored subscription offered as a palette row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileChoice {
    pub id: String,
    pub name: String,
}

impl ProfileChoice {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
}

/// The shared palette list: product commands plus the caller's profiles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandCatalogue {
    entries: Vec<CommandEntry>,
}

impl Default for CommandCatalogue {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandCatalogue {
    /// The product command set, without any stored profile rows.
    pub fn new() -> Self {
        let mut entries = Vec::new();
        for page in ShellPage::ALL {
            entries.push(CommandEntry::new(
                match page {
                    ShellPage::Overview => "nav.overview",
                    ShellPage::Proxies => "nav.proxies",
                    ShellPage::Profiles => "nav.profiles",
                    ShellPage::Rules => "nav.rules",
                    ShellPage::Connections => "nav.connections",
                    ShellPage::Logs => "nav.logs",
                    ShellPage::Dns => "nav.dns",
                    ShellPage::Doctor => "nav.doctor",
                    ShellPage::AppRouting => "nav.app_routing",
                    ShellPage::Sync => "nav.sync",
                    ShellPage::Settings => "nav.settings",
                },
                CommandCategory::Navigation,
                page.title_key(),
                page.title_zh(),
                CommandTarget::Navigate(page),
            ));
        }
        entries.extend([
            CommandEntry::new(
                "mode.rule",
                CommandCategory::Modes,
                "cmd_mode_rule",
                "切换至规则分流模式",
                CommandTarget::SetProxyMode(ProxyMode::Rule),
            ),
            CommandEntry::new(
                "mode.global",
                CommandCategory::Modes,
                "cmd_mode_global",
                "切换至全局代理模式",
                CommandTarget::SetProxyMode(ProxyMode::Global),
            ),
            CommandEntry::new(
                "mode.direct",
                CommandCategory::Modes,
                "cmd_mode_direct",
                "切换至直接连接模式",
                CommandTarget::SetProxyMode(ProxyMode::Direct),
            ),
            CommandEntry::new(
                "action.toggle_system_proxy",
                CommandCategory::Maintenance,
                "cmd_action_toggle_sysproxy",
                "切换 系统代理 开关",
                CommandTarget::ToggleSystemProxy,
            ),
            CommandEntry::new(
                "action.toggle_tun",
                CommandCategory::Maintenance,
                "cmd_action_toggle_tun",
                "切换 TUN 模式 开关",
                CommandTarget::ToggleTun,
            ),
            CommandEntry::new(
                "action.toggle_mini_hud",
                CommandCategory::Maintenance,
                "command_mini_hud",
                "切换迷你网速悬浮窗",
                CommandTarget::ToggleMiniHud,
            ),
            CommandEntry::new(
                "action.flush_dns_cache",
                CommandCategory::Maintenance,
                "cmd_action_flush_fakeip",
                "清理 Fake-IP 与系统 DNS 缓存",
                CommandTarget::FlushDnsCache,
            ),
            CommandEntry::new(
                "action.test_all_proxy_groups",
                CommandCategory::Maintenance,
                "cmd_action_speed_test_all",
                "对全部策略组执行测速",
                CommandTarget::TestAllProxyGroups,
            ),
            CommandEntry::new(
                "action.run_doctor",
                CommandCategory::Maintenance,
                "cmd_action_run_doctor",
                "运行系统全景自愈体检",
                CommandTarget::RunDoctor,
            ),
            CommandEntry::new(
                "action.close_connections",
                CommandCategory::Maintenance,
                "cmd_action_close_all_conns",
                "断开全部实时活动连接",
                CommandTarget::CloseAllConnections,
            ),
            CommandEntry::new(
                "action.restart_kernel",
                CommandCategory::Maintenance,
                "cmd_action_restart_kernel",
                "重启核心服务",
                CommandTarget::RestartKernel,
            ),
            CommandEntry::new(
                "theme.toggle",
                CommandCategory::Appearance,
                "hotkey_cycle_theme",
                "切换界面外观明暗主题",
                CommandTarget::CycleTheme,
            ),
        ]);
        Self { entries }
    }

    /// The product set plus one row per stored profile, in list order.
    pub fn with_profiles(profiles: &[ProfileChoice]) -> Self {
        let mut catalogue = Self::new();
        for profile in profiles {
            catalogue
                .entries
                .push(CommandEntry::profile(&profile.id, &profile.name));
        }
        catalogue
    }

    pub fn entries(&self) -> &[CommandEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entry(&self, index: usize) -> Option<&CommandEntry> {
        self.entries.get(index)
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.entries.iter().position(|entry| entry.id == id)
    }

    /// Indices of the entries a query keeps, in catalogue order. Both
    /// surfaces derive their filtered list from exactly this vector, so the
    /// arrow-key order is identical on both ends.
    pub fn filtered_indices(&self, query: &str) -> Vec<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.matches(query))
            .map(|(index, _)| index)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_covers_every_shared_page_and_action_once() {
        let catalogue = CommandCatalogue::new();
        assert_eq!(
            catalogue.len(),
            ShellPage::ALL.len() + 12,
            "11 pages + 3 modes + 8 actions + 1 appearance"
        );
        for page in ShellPage::ALL {
            let id = format!("nav.{}", page.id());
            let index = catalogue.index_of(&id).unwrap_or_else(|| panic!("{id}"));
            assert_eq!(
                catalogue.entry(index).unwrap().target,
                CommandTarget::Navigate(page)
            );
        }
        for id in [
            "action.toggle_system_proxy",
            "action.toggle_tun",
            "action.toggle_mini_hud",
            "action.flush_dns_cache",
            "action.test_all_proxy_groups",
            "action.run_doctor",
            "action.close_connections",
            "action.restart_kernel",
            "theme.toggle",
        ] {
            assert!(catalogue.index_of(id).is_some(), "{id} missing");
        }
        let ids: Vec<&str> = catalogue
            .entries()
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), ids.len(), "ids are unique");
    }

    #[test]
    fn chord_targets_reuse_the_shared_shortcut_vocabulary() {
        assert_eq!(
            CommandTarget::ToggleMiniHud.shortcut_action(),
            Some(ShortcutAction::ToggleMiniHud)
        );
        assert_eq!(
            CommandTarget::CycleTheme.shortcut_action(),
            Some(ShortcutAction::CycleTheme)
        );
        assert_eq!(CommandTarget::RunDoctor.shortcut_action(), None);
        // Both the palette row and the global chord name the same action.
        let catalogue = CommandCatalogue::new();
        let index = catalogue
            .index_of("action.toggle_mini_hud")
            .expect("mini hud row");
        assert_eq!(
            catalogue.entry(index).unwrap().target.shortcut_action(),
            Some(ShortcutAction::ToggleMiniHud)
        );
    }

    #[test]
    fn filtering_is_shared_and_profiles_append_in_order() {
        let catalogue = CommandCatalogue::with_profiles(&[
            ProfileChoice::new("sub-1", "主力高速订阅"),
            ProfileChoice::new("sub-2", "备用线路"),
        ]);
        assert_eq!(catalogue.len(), CommandCatalogue::new().len() + 2);
        assert_eq!(
            catalogue.entries().last().unwrap().profile_name(),
            Some("备用线路")
        );

        let ids: Vec<String> = catalogue
            .filtered_indices("dns")
            .into_iter()
            .map(|index| catalogue.entry(index).unwrap().id.clone())
            .collect();
        assert_eq!(
            ids,
            vec!["nav.dns".to_owned(), "action.flush_dns_cache".to_owned()]
        );

        let profile_hits = catalogue.filtered_indices("备用");
        assert_eq!(profile_hits.len(), 1);
        assert_eq!(
            catalogue.entry(profile_hits[0]).unwrap().profile_name(),
            Some("备用线路")
        );

        // An empty query keeps every row, in catalogue order.
        assert_eq!(catalogue.filtered_indices("   ").len(), catalogue.len());
    }
}

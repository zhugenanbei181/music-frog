//! Typed navigation capability and visible destination authority.
//!
//! FlClash's `NavigationPort.getItems()` publishes destinations with typed
//! `modes`, then runtime facts (`openLogs`, `hasProxies`) decide which row is
//! visible. This module is the Core-side equivalent: capability facts are
//! published once, and every surface asks one owner which destinations and
//! in-page entries are reachable. No surface may filter a raw registry by a
//! local boolean or guess an active-page fallback.

use crate::command_catalogue::ShellPage;
use crate::shell_navigation::{NavigationItem, NavigationSurface, NavigationTarget, desktop_items};
use crate::surface::SurfaceKind;
use serde::{Deserialize, Serialize};

/// Where one navigation item is allowed to appear.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NavigationMode {
    /// The compact/mobile bar.
    Mobile,
    /// The expanded desktop sidebar.
    Desktop,
    /// The Tools in-page index.
    Tools,
    /// A Tools entry that opens a full-width detail view.
    ToolsFull,
}

/// One destination plus the typed modes that own its presentation surfaces.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NavigationEntry {
    pub target: String,
    pub modes: Vec<NavigationMode>,
}

impl NavigationEntry {
    pub fn new(target: &str, modes: &[NavigationMode]) -> Self {
        Self {
            target: target.to_owned(),
            modes: modes.to_vec(),
        }
    }

    pub fn contains(&self, mode: NavigationMode) -> bool {
        let mut index = 0;
        while index < self.modes.len() {
            if self.modes[index] == mode {
                return true;
            }
            index += 1;
        }
        false
    }
}

/// The two runtime facts FlClash uses to gate capability-dependent rows.
///
/// Each field is one independent binary fact, not a stage encoded by a pair of
/// flags. The surface consumes the answer only through [`NavigationCapabilitySnapshot`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NavigationFeatureState {
    pub open_logs: bool,
    pub has_proxies: bool,
}

impl NavigationFeatureState {
    pub const fn new(open_logs: bool, has_proxies: bool) -> Self {
        Self {
            open_logs,
            has_proxies,
        }
    }

    pub fn allows(&self, entry: &NavigationEntry) -> bool {
        if entry.target == "logs" && !self.open_logs {
            return false;
        }
        if entry.target == "proxies" && !self.has_proxies {
            return false;
        }
        true
    }
}

impl Default for NavigationFeatureState {
    fn default() -> Self {
        Self::new(true, true)
    }
}

/// The authoritative answer for one surface's visible navigation destinations,
/// in-page Tools entries and the active destination after capability gating.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationCapabilitySnapshot {
    pub surface: SurfaceKind,
    pub features: NavigationFeatureState,
    pub desktop_rows: Vec<NavigationEntry>,
    pub mobile_rows: Vec<NavigationEntry>,
    pub tool_entries: Vec<NavigationEntry>,
    pub active_destination: String,
}

/// One visible destination after capability gating.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisibleNavigationRow {
    pub target: String,
    pub active: bool,
    pub full_width_tools_entry: bool,
}

impl NavigationCapabilitySnapshot {
    pub fn resolve(
        surface: SurfaceKind,
        features: NavigationFeatureState,
        requested_destination: &str,
    ) -> Self {
        let desktop_rows = desktop_items(surface)
            .iter()
            .map(|item| navigation_entry(*item))
            .filter(|entry| entry.contains(NavigationMode::Desktop))
            .filter(|entry| features.allows(entry))
            .collect::<Vec<_>>();
        let mobile_rows = NavigationItem::items(NavigationSurface::Dock)
            .iter()
            .map(|item| navigation_entry(*item))
            .filter(|entry| entry.contains(NavigationMode::Mobile))
            .filter(|entry| features.allows(entry))
            .collect::<Vec<_>>();
        let tool_entries = ShellPage::ALL
            .iter()
            .map(|page| navigation_entry_for_target(NavigationTarget::Shared(*page)))
            .filter(|entry| {
                entry.contains(NavigationMode::Tools) || entry.contains(NavigationMode::ToolsFull)
            })
            .filter(|entry| features.allows(entry))
            .collect::<Vec<_>>();
        let visible_rows = rows_for_surface(&desktop_rows, &mobile_rows, surface);
        let active_destination = visible_rows
            .iter()
            .find(|entry| entry.target == requested_destination)
            .or_else(|| visible_rows.first())
            .map(|entry| entry.target.clone())
            .unwrap_or_else(|| "overview".to_owned());
        Self {
            surface,
            features,
            desktop_rows,
            mobile_rows,
            tool_entries,
            active_destination,
        }
    }

    pub fn visible_rows(&self) -> &[NavigationEntry] {
        rows_for_surface(&self.desktop_rows, &self.mobile_rows, self.surface)
    }

    pub fn projected_rows(&self) -> Vec<VisibleNavigationRow> {
        self.visible_rows()
            .iter()
            .map(|entry| VisibleNavigationRow {
                target: entry.target.clone(),
                active: entry.target == self.active_destination,
                full_width_tools_entry: entry.contains(NavigationMode::ToolsFull),
            })
            .collect()
    }
}

/// The navigation mode a surface presents. Admin and CLI surfaces have no
/// visual navigation column, so they publish no mode rather than borrowing the
/// desktop answer.
pub fn mode_for_surface(surface: SurfaceKind) -> Option<NavigationMode> {
    match surface {
        SurfaceKind::IcedDesktop | SurfaceKind::BevyDesktop | SurfaceKind::SlintDesktop => {
            Some(NavigationMode::Desktop)
        }
        SurfaceKind::BevyAndroid
        | SurfaceKind::SlintAndroid
        | SurfaceKind::AndroidCompose
        | SurfaceKind::IosCompose => Some(NavigationMode::Mobile),
        SurfaceKind::AdminRest | SurfaceKind::Cli => None,
    }
}

fn rows_for_surface<'a>(
    desktop_rows: &'a [NavigationEntry],
    mobile_rows: &'a [NavigationEntry],
    surface: SurfaceKind,
) -> &'a [NavigationEntry] {
    match mode_for_surface(surface) {
        Some(NavigationMode::Desktop) => desktop_rows,
        Some(NavigationMode::Mobile) => mobile_rows,
        _ => &[],
    }
}

impl Default for NavigationCapabilitySnapshot {
    fn default() -> Self {
        Self::resolve(
            SurfaceKind::BevyDesktop,
            NavigationFeatureState::default(),
            "overview",
        )
    }
}

/// Map a registry item onto the typed modes that own it.
///
/// The default mirrors the upstream `NavigationItem` default (`mobile` +
/// `desktop`). Capability-dependent rows and `more`/`moreFull` entries state
/// their own modes so runtime filtering and the Tools index share one table.
pub fn navigation_entry(item: NavigationItem) -> NavigationEntry {
    navigation_entry_for_target(item.target)
}

fn navigation_entry_for_target(target: NavigationTarget) -> NavigationEntry {
    use NavigationMode::{Desktop, Mobile, Tools, ToolsFull};

    const MOBILE_DESKTOP: &[NavigationMode] = &[Mobile, Desktop];
    const DESKTOP_TOOLS_FULL: &[NavigationMode] = &[Desktop, ToolsFull];
    const DESKTOP_MOBILE_TOOLS: &[NavigationMode] = &[Desktop, Mobile, Tools];
    const TOOLS_ONLY: &[NavigationMode] = &[Tools];
    const TOOLS_FULL_ONLY: &[NavigationMode] = &[ToolsFull];
    const SURFACE_LOCAL_DESKTOP: &[NavigationMode] = &[Desktop];

    match target {
        NavigationTarget::Shared(ShellPage::Proxies) => {
            NavigationEntry::new(target.id(), MOBILE_DESKTOP)
        }
        NavigationTarget::Shared(ShellPage::Logs) => {
            NavigationEntry::new(target.id(), DESKTOP_MOBILE_TOOLS)
        }
        NavigationTarget::Shared(ShellPage::Connections) => {
            NavigationEntry::new(target.id(), DESKTOP_TOOLS_FULL)
        }
        NavigationTarget::Shared(ShellPage::Rules) => {
            NavigationEntry::new(target.id(), DESKTOP_MOBILE_TOOLS)
        }
        NavigationTarget::Shared(ShellPage::Dns) => {
            NavigationEntry::new(target.id(), TOOLS_FULL_ONLY)
        }
        NavigationTarget::Shared(ShellPage::Doctor)
        | NavigationTarget::Shared(ShellPage::AppRouting)
        | NavigationTarget::Shared(ShellPage::Sync)
        | NavigationTarget::Shared(ShellPage::About)
        | NavigationTarget::Shared(ShellPage::Disclaimer)
        | NavigationTarget::Shared(ShellPage::Licenses)
        | NavigationTarget::Shared(ShellPage::Tools) => {
            NavigationEntry::new(target.id(), TOOLS_ONLY)
        }
        NavigationTarget::SurfaceLocal("traffic") => {
            NavigationEntry::new(target.id(), SURFACE_LOCAL_DESKTOP)
        }
        NavigationTarget::SurfaceLocal(id) => NavigationEntry::new(id, MOBILE_DESKTOP),
        NavigationTarget::Shared(_) => NavigationEntry::new(target.id(), MOBILE_DESKTOP),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design_resources::IconSemantic;
    use crate::shell_navigation::NavigationGroup;

    #[test]
    fn capability_facts_gate_only_the_rows_they_own() {
        let all = NavigationFeatureState::new(true, true);
        let none = NavigationFeatureState::new(false, false);
        let proxies = NavigationEntry::new("proxies", &[NavigationMode::Desktop]);
        let logs = NavigationEntry::new("logs", &[NavigationMode::Desktop]);
        let overview = NavigationEntry::new("overview", &[NavigationMode::Desktop]);
        assert!(all.allows(&proxies));
        assert!(all.allows(&logs));
        assert!(all.allows(&overview));
        assert!(!none.allows(&proxies));
        assert!(!none.allows(&logs));
        assert!(none.allows(&overview));
    }

    #[test]
    fn a_hidden_requested_destination_falls_back_to_the_first_visible_row() {
        let snapshot = NavigationCapabilitySnapshot::resolve(
            SurfaceKind::SlintDesktop,
            NavigationFeatureState::new(false, false),
            "logs",
        );
        assert_eq!(snapshot.active_destination, "overview");
        assert!(!snapshot.desktop_rows.iter().any(|row| row.target == "logs"));
        assert!(
            !snapshot
                .desktop_rows
                .iter()
                .any(|row| row.target == "proxies")
        );
    }

    #[test]
    fn a_visible_requested_destination_is_preserved() {
        let snapshot = NavigationCapabilitySnapshot::resolve(
            SurfaceKind::SlintDesktop,
            NavigationFeatureState::default(),
            "logs",
        );
        assert_eq!(snapshot.active_destination, "logs");
    }

    #[test]
    fn tool_entries_are_selected_by_typed_modes() {
        let snapshot = NavigationCapabilitySnapshot::resolve(
            SurfaceKind::SlintDesktop,
            NavigationFeatureState::default(),
            "overview",
        );
        assert!(
            snapshot
                .tool_entries
                .iter()
                .any(|entry| entry.target == "connections")
        );
        assert!(
            snapshot
                .tool_entries
                .iter()
                .any(|entry| entry.target == "dns")
        );
        assert!(
            !snapshot
                .tool_entries
                .iter()
                .any(|entry| entry.target == "overview")
        );
    }

    #[test]
    fn the_surface_local_traffic_row_stays_a_desktop_destination() {
        let traffic = NavigationItem::new(
            NavigationTarget::SurfaceLocal("traffic"),
            NavigationGroup::Observation,
            IconSemantic::Traffic,
        );
        assert!(
            navigation_entry(traffic).contains(NavigationMode::Desktop),
            "a surface that owns traffic must keep the row reachable"
        );
    }

    #[test]
    fn projected_rows_publish_one_active_destination_and_tools_layout() {
        let snapshot = NavigationCapabilitySnapshot::resolve(
            SurfaceKind::SlintDesktop,
            NavigationFeatureState::default(),
            "connections",
        );
        let rows = snapshot.projected_rows();
        assert_eq!(
            rows.iter().filter(|row| row.active).count(),
            1,
            "one visible destination owns the active state"
        );
        let connections = rows
            .iter()
            .find(|row| row.target == "connections")
            .expect("connections is visible");
        assert!(connections.active);
        assert!(connections.full_width_tools_entry);
    }
}

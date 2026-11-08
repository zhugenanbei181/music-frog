//! The typed shell navigation registry.
//!
//! One owner answers "which destinations exist, in what order, in which
//! group, with which semantic icon, and on which surface they appear". A
//! surface renders this list; it never re-declares the destinations, their
//! labels or their grouping as literals. This is the cross-surface equivalent
//! of an upstream `NavigationPort.getItems()` with typed `modes`.

use crate::command_catalogue::ShellPage;
use crate::design_resources::IconSemantic;

/// A destination's identity. Shared pages use the cross-surface id; a
/// surface-local page is named explicitly instead of being disguised as a
/// shared page, so no surface can accidentally claim a second truth source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NavigationTarget {
    Shared(ShellPage),
    /// The Slint desktop traffic-statistics page. The other surfaces fold
    /// traffic into the overview, so this id stays surface-local by design.
    SurfaceLocal(&'static str),
}

impl NavigationTarget {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Shared(page) => page.id(),
            Self::SurfaceLocal(id) => id,
        }
    }

    pub const fn label_key(self) -> Option<&'static str> {
        match self {
            Self::Shared(page) => Some(page.sidebar_key()),
            Self::SurfaceLocal(_) => None,
        }
    }
}

/// A navigation group is a title-only separator; it owns no second visual
/// signal such as a hairline, so one boundary is stated exactly once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NavigationGroup {
    Primary,
    Observation,
}

impl NavigationGroup {
    pub const ALL: [Self; 2] = [Self::Primary, Self::Observation];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Observation => "observation",
        }
    }

    /// The locale key of the group heading. There is no bare-Chinese fallback:
    /// the copy table is the single source of user-visible text.
    pub const fn title_key(self) -> &'static str {
        match self {
            Self::Primary => "nav_group_primary",
            Self::Observation => "nav_group_observation",
        }
    }
}

/// One destination in the shell navigation. The page supplies the label via
/// [`ShellPage::sidebar_key`]; the icon is a semantic id, never a file path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NavigationItem {
    pub target: NavigationTarget,
    pub group: NavigationGroup,
    pub icon: IconSemantic,
}

/// Where a navigation item is allowed to appear.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NavigationSurface {
    /// The desktop sidebar (Slint rail / Iced / Bevy).
    Desktop,
    /// The compact floating dock.
    Dock,
}

impl NavigationItem {
    /// Cross-surface destinations plus the one documented surface-local row.
    pub const DESKTOP: [Self; 8] = [
        Self::shared(ShellPage::Overview, NavigationGroup::Primary, IconSemantic::Overview),
        Self::shared(ShellPage::Proxies, NavigationGroup::Primary, IconSemantic::Proxy),
        Self::shared(ShellPage::Profiles, NavigationGroup::Primary, IconSemantic::Config),
        Self::shared(ShellPage::Settings, NavigationGroup::Primary, IconSemantic::Settings),
        Self::shared(ShellPage::Connections, NavigationGroup::Observation, IconSemantic::Connections),
        Self::shared(ShellPage::Logs, NavigationGroup::Observation, IconSemantic::Logs),
        Self::shared(ShellPage::Rules, NavigationGroup::Observation, IconSemantic::Rules),
        Self::new(
            NavigationTarget::SurfaceLocal("traffic"),
            NavigationGroup::Observation,
            IconSemantic::Traffic,
        ),
    ];

    /// The dock is a four-item surface; it does not carry group headings.
    pub const DOCK: [Self; 4] = [
        Self::shared(ShellPage::Overview, NavigationGroup::Primary, IconSemantic::Overview),
        Self::shared(ShellPage::Proxies, NavigationGroup::Primary, IconSemantic::Proxy),
        Self::shared(ShellPage::Profiles, NavigationGroup::Primary, IconSemantic::Config),
        Self::shared(ShellPage::Settings, NavigationGroup::Primary, IconSemantic::Settings),
    ];

    pub const fn new(target: NavigationTarget, group: NavigationGroup, icon: IconSemantic) -> Self {
        Self { target, group, icon }
    }

    pub const fn shared(page: ShellPage, group: NavigationGroup, icon: IconSemantic) -> Self {
        Self::new(NavigationTarget::Shared(page), group, icon)
    }

    pub const fn items(surface: NavigationSurface) -> &'static [Self] {
        match surface {
            NavigationSurface::Desktop => &Self::DESKTOP,
            NavigationSurface::Dock => &Self::DOCK,
        }
    }

    /// The typed id a surface uses to report which destination is active.
    pub const fn id(self) -> &'static str {
        self.target.id()
    }
}

/// Resolve the authoritative destination for a page that may be nested under
/// another destination. A leaf page never invents its own sidebar row.
pub const fn destination_for(page: ShellPage) -> ShellPage {
    match page {
        ShellPage::Overview
        | ShellPage::Proxies
        | ShellPage::Profiles
        | ShellPage::Settings
        | ShellPage::Connections
        | ShellPage::Logs
        | ShellPage::Rules => page,
        ShellPage::Dns
        | ShellPage::Doctor
        | ShellPage::AppRouting
        | ShellPage::Sync
        | ShellPage::About
        | ShellPage::Disclaimer
        | ShellPage::Licenses
        | ShellPage::Tools => ShellPage::Settings,
    }
}

/// Resolve a surface route id onto the typed destination that owns it.
///
/// Route ids are presentation vocabulary (Slint `page`, Iced `Route`, Bevy
/// `PageId`), but "which sidebar row is active" is a business fact. It belongs
/// here, once, instead of being re-derived by every surface with its own
/// string comparison chain. An unknown route has no owner and stays empty.
pub fn destination_for_route(route: &str) -> Option<NavigationTarget> {
    let target = match route {
        "Home" | "Overview" | "OverviewWorkbench" => NavigationTarget::Shared(ShellPage::Overview),
        "Proxy" | "Proxies" | "ProxiesWorkbench" => {
            NavigationTarget::Shared(ShellPage::Proxies)
        }
        "Config" | "Profiles" | "ProfileSettings" | "Preview" => {
            NavigationTarget::Shared(ShellPage::Profiles)
        }
        "Settings" | "Network" | "AppSettings" | "Overrides" | "About" | "Licenses"
        | "LicenseDetail" | "Dns" | "Editor" | "Filters" | "Doctor" | "Sync"
        | "AppRouting" | "Meta" | "SettingsWorkbench" | "ShellWorkbench" | "Tools"
        | "Disclaimer" => NavigationTarget::Shared(ShellPage::Settings),
        "Connections" | "ConnectionDetail" | "ConnectionsWorkbench" => {
            NavigationTarget::Shared(ShellPage::Connections)
        }
        "Logs" | "LogsWorkbench" => NavigationTarget::Shared(ShellPage::Logs),
        "Rules" | "Providers" | "RulesWorkbench" => {
            NavigationTarget::Shared(ShellPage::Rules)
        }
        "Traffic" | "Runtime" | "RuntimeWorkbench" => {
            NavigationTarget::SurfaceLocal("traffic")
        }
        _ => return None,
    };
    Some(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_desktop_destination_is_unique_and_uses_a_distinct_icon() {
        let items = NavigationItem::items(NavigationSurface::Desktop);
        let targets: HashSet<NavigationTarget> = items.iter().map(|item| item.target).collect();
        let icons: HashSet<IconSemantic> = items.iter().map(|item| item.icon).collect();
        assert_eq!(targets.len(), items.len());
        assert_eq!(icons.len(), items.len());
    }

    #[test]
    fn groups_are_contiguous_and_in_declared_order() {
        let items = NavigationItem::items(NavigationSurface::Desktop);
        let order: Vec<usize> = items
            .iter()
            .map(|item| {
                NavigationGroup::ALL
                    .iter()
                    .position(|group| *group == item.group)
                    .unwrap()
            })
            .collect();
        assert!(order.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(items.iter().any(|item| item.group == NavigationGroup::Observation));
    }

    #[test]
    fn every_shared_page_maps_to_a_desktop_destination() {
        for page in ShellPage::ALL {
            let destination = destination_for(page);
            assert!(
                NavigationItem::items(NavigationSurface::Desktop)
                    .iter()
                    .any(|item| item.target == NavigationTarget::Shared(destination)),
                "{page:?} -> {destination:?}"
            );
        }
    }

    #[test]
    fn the_dock_is_a_subset_of_the_primary_group() {
        for item in NavigationItem::items(NavigationSurface::Dock) {
            assert_eq!(item.group, NavigationGroup::Primary);
            assert!(
                NavigationItem::items(NavigationSurface::Desktop)
                    .iter()
                    .any(|desktop| desktop.target == item.target)
            );
        }
    }

    #[test]
    fn a_destination_owns_its_own_label_and_icon_semantics() {
        for item in NavigationItem::items(NavigationSurface::Desktop) {
            assert!(!item.id().is_empty());
            assert!(!item.icon.as_str().is_empty());
        }
    }

    #[test]
    fn the_surface_local_row_is_explicit_and_never_claims_a_shared_label() {
        let local: Vec<_> = NavigationItem::items(NavigationSurface::Desktop)
            .iter()
            .filter(|item| matches!(item.target, NavigationTarget::SurfaceLocal(_)))
            .collect();
        assert_eq!(local.len(), 1);
        assert_eq!(local[0].id(), "traffic");
        assert_eq!(local[0].target.label_key(), None);
    }

    #[test]
    fn every_workbench_route_resolves_to_the_destination_that_owns_it() {
        let cases = [
            ("ProxiesWorkbench", NavigationTarget::Shared(ShellPage::Proxies)),
            ("ConnectionsWorkbench", NavigationTarget::Shared(ShellPage::Connections)),
            ("LogsWorkbench", NavigationTarget::Shared(ShellPage::Logs)),
            ("RulesWorkbench", NavigationTarget::Shared(ShellPage::Rules)),
            ("RuntimeWorkbench", NavigationTarget::SurfaceLocal("traffic")),
            ("SettingsWorkbench", NavigationTarget::Shared(ShellPage::Settings)),
            ("OverviewWorkbench", NavigationTarget::Shared(ShellPage::Overview)),
            ("ShellWorkbench", NavigationTarget::Shared(ShellPage::Settings)),
        ];
        for (route, expected) in cases {
            assert_eq!(destination_for_route(route), Some(expected), "{route}");
        }
    }

    #[test]
    fn an_unknown_route_has_no_destination_instead_of_a_guessed_one() {
        assert_eq!(destination_for_route("NotAPage"), None);
    }

    #[test]
    fn every_resolved_destination_exists_in_the_desktop_registry() {
        for route in [
            "Home", "Proxy", "Config", "Settings", "Connections", "Logs", "Rules",
            "Traffic", "Dns", "About", "AppRouting", "Editor", "Sync", "Tools",
        ] {
            let target = destination_for_route(route).unwrap();
            assert!(
                NavigationItem::items(NavigationSurface::Desktop)
                    .iter()
                    .any(|item| item.target == target),
                "{route} -> {target:?}"
            );
        }
    }
}

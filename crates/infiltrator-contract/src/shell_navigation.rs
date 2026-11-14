//! The typed shell navigation registry.
//!
//! One owner answers "which destinations exist, in what order, in which
//! group, with which semantic icon, and on which surface they appear". A
//! surface renders this list; it never re-declares the destinations, their
//! labels or their grouping as literals. This is the cross-surface equivalent
//! of an upstream `NavigationPort.getItems()` with typed `modes`.

use crate::command_catalogue::ShellPage;
use crate::design_resources::IconSemantic;
use crate::surface::SurfaceKind;

/// A destination's identity. Shared pages use the cross-surface id; a
/// surface-local page is named explicitly instead of being disguised as a
/// shared page, so no surface can accidentally claim a second truth source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NavigationTarget {
    Shared(ShellPage),
    /// The standalone traffic-statistics page on Slint and Iced. Bevy folds
    /// traffic into the overview, so this id stays surface-local by design.
    SurfaceLocal(&'static str),
}

/// The surface-local destination ids the registry knows.
///
/// A surface-local id is a page some surfaces own and others fold elsewhere;
/// naming the closed set here is what lets [`owns_surface_local`] answer
/// "who mounts it" in one place, instead of every surface keeping its own
/// list of which local rows it happens to have today.
pub const SURFACE_LOCAL_TRAFFIC: &str = "traffic";

/// The recorded-request page (FlClash's 请求 desktop row).
///
/// The reference ships this as a first-class desktop destination
/// (`lib/views/navigation.dart`: `NavigationItemMode.desktop`), and the
/// tracker's facts are already published to every surface through
/// `SurfaceSnapshot::requests`. A surface that renders its own request page
/// owns the row; a surface that folds the records into another page does not,
/// and must not be handed a row that navigates nowhere.
pub const SURFACE_LOCAL_REQUESTS: &str = "requests";

pub const SURFACE_LOCAL_IDS: [&str; 2] = [SURFACE_LOCAL_TRAFFIC, SURFACE_LOCAL_REQUESTS];

/// The shared copy-table key that names one surface-local row.
///
/// A local row has no [`ShellPage`], so it cannot borrow a page's
/// `sidebar_key`; before this, every surface kept its own chain naming the
/// traffic row. The row's *word* is still one shared entry, and which row
/// owns which word is decided here once.
/// The registry answers this for every id it can publish: [`DESKTOP`] holds
/// only ids from [`SURFACE_LOCAL_IDS`], and [`owns_surface_local`] refuses an
/// id outside that set, so a surface can never be handed a row whose word is
/// undecided. An unknown id therefore names itself, which is what the row's
/// identity already is; the registry's own test is what keeps that arm dead.
/// The id is `&'static str` because every row the registry publishes is a
/// const, so a caller can never hand this a borrowed temporary.
pub fn surface_local_nav_key(id: &'static str) -> &'static str {
    match id {
        SURFACE_LOCAL_TRAFFIC => "nav_traffic",
        SURFACE_LOCAL_REQUESTS => "nav_requests",
        other => {
            debug_assert!(
                SURFACE_LOCAL_IDS.contains(&other),
                "`{other}` is not a registry surface-local id"
            );
            other
        }
    }
}

impl NavigationTarget {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Shared(page) => page.id(),
            Self::SurfaceLocal(id) => id,
        }
    }

    /// The shared copy-table entry that names this destination, always one.
    ///
    /// Every row the registry publishes has one, including the surface-local
    /// rows: a local page owns no `ShellPage`, so its word comes from
    /// [`surface_local_nav_key`] instead. A surface that cannot resolve this
    /// key has a row the registry never named, which is a bug the registry's
    /// own test refuses to publish.
    pub fn label_key(self) -> &'static str {
        match self {
            Self::Shared(page) => page.sidebar_key(),
            Self::SurfaceLocal(id) => surface_local_nav_key(id),
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

/// Whether one product surface owns a surface-local destination.
///
/// A `SurfaceLocal` row is a page that exists on some surfaces and is folded
/// into the overview on others, so a surface that does not own the page must
/// not be handed the row: a nav row that navigates nowhere is a lie the shell
/// tells the user. This is the one place the pairing is stated, and it is
/// read off [`destination_for_route`], which resolves the Slint and Iced
/// `Traffic` routes onto this id. The Bevy desktop
/// surface owns no traffic route — its `runtime-*` capture scenarios render
/// its overview page — so it mounts the shared destinations alone.
pub fn owns_surface_local(surface: SurfaceKind, id: &str) -> bool {
    match id {
        SURFACE_LOCAL_TRAFFIC => matches!(
            surface,
            SurfaceKind::SlintDesktop | SurfaceKind::IcedDesktop
        ),
        SURFACE_LOCAL_REQUESTS => matches!(surface, SurfaceKind::BevyDesktop),
        // An id outside the closed set owns nothing: a row that navigates
        // nowhere is a lie the shell tells the user, so the answer is "no".
        other => {
            debug_assert!(
                false,
                "`{other}` is not a registry surface-local id ({SURFACE_LOCAL_IDS:?})"
            );
            false
        }
    }
}

/// The destinations one desktop surface mounts as its sidebar rows.
///
/// The order and grouping come from [`NavigationItem::items`]; only the
/// surface-local rows are filtered, and only by [`owns_surface_local`]. A
/// surface renders this list instead of declaring its own: the Bevy surface
/// used to paint every route it has (including DNS, Doctor, Sync, Tools,
/// Disclaimer and Licenses, which the registry folds under `Settings`), which
/// pushed the navigation stack below the fold and left the shell's own
/// controls to occupy the space above it.
///
/// [`NavigationItem::items`] stays the unfiltered registry: a surface mounts
/// what this function publishes, never the raw list, so a surface-local row
/// can only reach a shell that owns the page behind it.
pub fn desktop_items(surface: SurfaceKind) -> Vec<NavigationItem> {
    NavigationItem::items(NavigationSurface::Desktop)
        .iter()
        .copied()
        .filter(|item| match item.target {
            NavigationTarget::Shared(_) => true,
            NavigationTarget::SurfaceLocal(id) => owns_surface_local(surface, id),
        })
        .collect()
}

impl NavigationItem {
    /// Cross-surface destinations plus the documented surface-local rows.
    ///
    /// The shared rows come first, in registry order, and the surface-local
    /// rows follow: a surface mounts the shared set and then only the local
    /// rows it owns, so the order of the rows every surface shares never
    /// depends on which local page a surface happens to have.
    pub const DESKTOP: [Self; 9] = [
        Self::shared(
            ShellPage::Overview,
            NavigationGroup::Primary,
            IconSemantic::Overview,
        ),
        Self::shared(
            ShellPage::Proxies,
            NavigationGroup::Primary,
            IconSemantic::Proxy,
        ),
        Self::shared(
            ShellPage::Profiles,
            NavigationGroup::Primary,
            IconSemantic::Config,
        ),
        Self::shared(
            ShellPage::Settings,
            NavigationGroup::Primary,
            IconSemantic::Settings,
        ),
        Self::shared(
            ShellPage::Connections,
            NavigationGroup::Observation,
            IconSemantic::Connections,
        ),
        Self::shared(
            ShellPage::Logs,
            NavigationGroup::Observation,
            IconSemantic::Logs,
        ),
        Self::shared(
            ShellPage::Rules,
            NavigationGroup::Observation,
            IconSemantic::Rules,
        ),
        Self::new(
            NavigationTarget::SurfaceLocal(SURFACE_LOCAL_TRAFFIC),
            NavigationGroup::Observation,
            IconSemantic::Traffic,
        ),
        Self::new(
            NavigationTarget::SurfaceLocal(SURFACE_LOCAL_REQUESTS),
            NavigationGroup::Observation,
            IconSemantic::Activity,
        ),
    ];

    /// The dock is a four-item surface; it does not carry group headings.
    pub const DOCK: [Self; 4] = [
        Self::shared(
            ShellPage::Overview,
            NavigationGroup::Primary,
            IconSemantic::Overview,
        ),
        Self::shared(
            ShellPage::Proxies,
            NavigationGroup::Primary,
            IconSemantic::Proxy,
        ),
        Self::shared(
            ShellPage::Profiles,
            NavigationGroup::Primary,
            IconSemantic::Config,
        ),
        Self::shared(
            ShellPage::Settings,
            NavigationGroup::Primary,
            IconSemantic::Settings,
        ),
    ];

    pub const fn new(target: NavigationTarget, group: NavigationGroup, icon: IconSemantic) -> Self {
        Self {
            target,
            group,
            icon,
        }
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

/// The shared pages that fold *under* one destination and therefore have no
/// sidebar row of their own.
///
/// [`destination_for`] answers "which row is active for this page"; this is
/// its inverse, and it is what a surface needs to build the in-page index
/// that reaches those pages. A surface renders this list rather than keeping
/// a literal table of nested links: the Bevy settings index used to be
/// absent altogether, which is why its sidebar mounted all fifteen routes —
/// the rows were the only way to reach DNS, Doctor, Sync, Tools, About,
/// Disclaimer and Licenses. Order follows [`ShellPage::ALL`], so the index is
/// stable across surfaces.
pub fn pages_folded_under(destination: ShellPage) -> Vec<ShellPage> {
    ShellPage::ALL
        .into_iter()
        .filter(|page| destination_for(*page) == destination && *page != destination)
        .collect()
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
        "Proxy" | "Proxies" | "ProxiesWorkbench" => NavigationTarget::Shared(ShellPage::Proxies),
        "Config" | "Profiles" | "ProfileSettings" | "Preview" => {
            NavigationTarget::Shared(ShellPage::Profiles)
        }
        "Settings" | "Network" | "Runtime" | "AppSettings" | "Overrides" | "About" | "Licenses"
        | "LicenseDetail" | "Dns" | "Editor" | "Filters" | "Doctor" | "Sync" | "AppRouting"
        | "Meta" | "SettingsWorkbench" | "ShellWorkbench" | "Tools" | "Disclaimer" => {
            NavigationTarget::Shared(ShellPage::Settings)
        }
        "Connections" | "ConnectionDetail" | "ConnectionsWorkbench" => {
            NavigationTarget::Shared(ShellPage::Connections)
        }
        "Logs" | "LogsWorkbench" => NavigationTarget::Shared(ShellPage::Logs),
        "Rules" | "Providers" | "RulesWorkbench" => NavigationTarget::Shared(ShellPage::Rules),
        "Traffic" | "RuntimeWorkbench" => NavigationTarget::SurfaceLocal(SURFACE_LOCAL_TRAFFIC),
        "Requests" => NavigationTarget::SurfaceLocal(SURFACE_LOCAL_REQUESTS),
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
        assert!(
            items
                .iter()
                .any(|item| item.group == NavigationGroup::Observation)
        );
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

    /// A surface mounts the registry's rows, filtered only by which
    /// surface-local page it owns. The Bevy desktop surface has no traffic
    /// page — its `runtime-*` capture scenarios render its overview — so it
    /// gets the seven shared destinations, while Slint and Iced, which do own
    /// one, get eight. A surface never re-declares the set: this is the only
    /// answer to "which rows does my sidebar mount".
    #[test]
    fn each_desktop_surface_mounts_the_shared_rows_plus_the_local_row_it_owns() {
        let shared: Vec<_> = NavigationItem::items(NavigationSurface::Desktop)
            .iter()
            .filter(|item| matches!(item.target, NavigationTarget::Shared(_)))
            .map(|item| item.id())
            .collect();
        for surface in [
            SurfaceKind::BevyDesktop,
            SurfaceKind::SlintDesktop,
            SurfaceKind::IcedDesktop,
        ] {
            let ids: Vec<_> = desktop_items(surface)
                .iter()
                .map(|item| item.id())
                .collect();
            assert!(
                ids.starts_with(&shared),
                "{surface:?} must keep the shared rows in registry order"
            );
        }
        // Bevy owns the request page and folds traffic into its overview;
        // Slint and Iced own traffic and have no request page. Each surface
        // therefore mounts the shared rows plus exactly one local row.
        assert_eq!(
            desktop_items(SurfaceKind::BevyDesktop).len(),
            shared.len() + 1
        );
        assert!(
            desktop_items(SurfaceKind::BevyDesktop)
                .iter()
                .any(|item| item.id() == SURFACE_LOCAL_REQUESTS),
            "Bevy owns the request page, so the registry must hand it the row"
        );
        assert!(
            !desktop_items(SurfaceKind::BevyDesktop)
                .iter()
                .any(|item| item.id() == SURFACE_LOCAL_TRAFFIC),
            "Bevy folds traffic into its overview and must not be handed a row that navigates nowhere"
        );
        for surface in [SurfaceKind::SlintDesktop, SurfaceKind::IcedDesktop] {
            assert!(
                !desktop_items(surface)
                    .iter()
                    .any(|item| item.id() == SURFACE_LOCAL_REQUESTS),
                "{surface:?} publishes no request page, so it must not mount its row"
            );
        }
        assert_eq!(
            desktop_items(SurfaceKind::SlintDesktop).len(),
            shared.len() + 1
        );
        assert_eq!(
            desktop_items(SurfaceKind::IcedDesktop).len(),
            shared.len() + 1
        );
        assert!(owns_surface_local(
            SurfaceKind::BevyDesktop,
            SURFACE_LOCAL_REQUESTS
        ));
        assert!(!owns_surface_local(
            SurfaceKind::SlintDesktop,
            SURFACE_LOCAL_REQUESTS
        ));
        assert!(!owns_surface_local(
            SurfaceKind::IcedDesktop,
            SURFACE_LOCAL_REQUESTS
        ));
    }

    /// Every row a surface mounts *is* a destination: a shared row's page is
    /// its own destination under [`destination_for`], so a shell never offers
    /// a row that folds under some other row, and a surface-local row only
    /// reaches a surface that owns that page.
    #[test]
    fn every_mounted_row_is_a_destination_the_surface_can_reach() {
        for surface in [
            SurfaceKind::BevyDesktop,
            SurfaceKind::SlintDesktop,
            SurfaceKind::IcedDesktop,
        ] {
            for item in desktop_items(surface) {
                match item.target {
                    NavigationTarget::Shared(page) => {
                        assert_eq!(
                            destination_for(page),
                            page,
                            "{surface:?} mounts {page:?}, which the registry folds under another row"
                        );
                    }
                    NavigationTarget::SurfaceLocal(local) => {
                        assert!(
                            owns_surface_local(surface, local),
                            "{surface:?} does not own the local page {local}"
                        );
                    }
                }
            }
        }
    }

    /// The destinations and the pages that fold under them cover every shared
    /// page exactly once: a surface mounts the destinations as rows and the
    /// folded pages as in-page links, so no page is reachable only by a row
    /// the registry refuses to publish, and none is published twice.
    #[test]
    fn the_rows_and_the_folded_pages_cover_every_shared_page_once() {
        let rows: Vec<_> = NavigationItem::items(NavigationSurface::Desktop)
            .iter()
            .filter_map(|item| match item.target {
                NavigationTarget::Shared(page) => Some(page),
                NavigationTarget::SurfaceLocal(_) => None,
            })
            .collect();
        let mut folded: Vec<ShellPage> = Vec::new();
        for row in &rows {
            folded.extend(pages_folded_under(*row));
        }
        let mut covered: Vec<ShellPage> = rows.clone();
        covered.extend(folded.iter().copied());
        covered.sort_by_key(|page| ShellPage::ALL.iter().position(|p| p == page).unwrap());
        assert_eq!(covered, ShellPage::ALL.to_vec());
        assert_eq!(
            folded.len(),
            ShellPage::ALL.len() - rows.len(),
            "the folded pages are exactly the pages with no row of their own"
        );
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
        // The closed set of local rows, in registry order. A local row names
        // a page some surfaces own and others fold elsewhere, so it carries
        // no shared label key: the surface that owns the page names the row.
        assert_eq!(
            local.iter().map(|item| item.id()).collect::<Vec<_>>(),
            SURFACE_LOCAL_IDS.to_vec()
        );
        for item in local {
            // A local row never borrows a `ShellPage`'s sidebar key - it owns
            // no shared page - but its word is still one shared entry the
            // registry names, so no surface keeps a private copy of it.
            let NavigationTarget::SurfaceLocal(id) = item.target else {
                unreachable!("filtered for surface-local rows");
            };
            assert_eq!(item.target.label_key(), surface_local_nav_key(id));
            let shared_keys: Vec<_> = ShellPage::ALL
                .into_iter()
                .map(|page| page.sidebar_key())
                .collect();
            assert!(
                !shared_keys.contains(&item.target.label_key()),
                "{} claims the label of a shared page",
                item.id()
            );
        }
    }

    #[test]
    fn every_workbench_route_resolves_to_the_destination_that_owns_it() {
        let cases = [
            (
                "ProxiesWorkbench",
                NavigationTarget::Shared(ShellPage::Proxies),
            ),
            (
                "ConnectionsWorkbench",
                NavigationTarget::Shared(ShellPage::Connections),
            ),
            ("LogsWorkbench", NavigationTarget::Shared(ShellPage::Logs)),
            ("RulesWorkbench", NavigationTarget::Shared(ShellPage::Rules)),
            (
                "RuntimeWorkbench",
                NavigationTarget::SurfaceLocal(SURFACE_LOCAL_TRAFFIC),
            ),
            ("Runtime", NavigationTarget::Shared(ShellPage::Settings)),
            (
                "SettingsWorkbench",
                NavigationTarget::Shared(ShellPage::Settings),
            ),
            (
                "OverviewWorkbench",
                NavigationTarget::Shared(ShellPage::Overview),
            ),
            (
                "ShellWorkbench",
                NavigationTarget::Shared(ShellPage::Settings),
            ),
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
            "Home",
            "Proxy",
            "Config",
            "Settings",
            "Connections",
            "Logs",
            "Rules",
            "Traffic",
            "Runtime",
            "Dns",
            "About",
            "AppRouting",
            "Editor",
            "Sync",
            "Tools",
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

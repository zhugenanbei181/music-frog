//! The typed content structure of every shared shell page.
//!
//! A page's *shape* decides how much empty ground is legitimate. A page
//! whose body is one frame (a list whose rows scroll inside the frame) has
//! no room for empty ground at all: the frame fills the page body. A page
//! that lays its blocks out by content leaves ground below the last block,
//! and how much is legitimate depends on the peer product's behaviour for
//! the same shape, not on one global number.
//!
//! This registry is the single owner of that shape. The space-utilization
//! judge reads it to pick a per-page limit; a surface never decides for
//! itself how empty a page is allowed to be.
//!
//! The same exporter also publishes the viewport ladder, because the judge
//! needs both facts about one frame and must not keep a private copy of
//! either: the boundaries themselves belong to [`crate::responsive_viewport`]
//! and are projected here, tier by tier, as `TierFact`.

use crate::command_catalogue::ShellPage;
use crate::responsive_viewport::{
    DASHBOARD_DESKTOP_MIN_PX, DASHBOARD_FOUR_COLUMN_MIN_PX, DASHBOARD_MOBILE_MAX_PX,
    DASHBOARD_STANDARD_FOUR_COLUMN_MIN_PX, DASHBOARD_STANDARD_HEIGHT_MIN_PX, DashboardColumns,
    DashboardDensity, SidebarForm, TIERS, ViewportTier,
};
use crate::shell_receipt::CHROME_KEYS;
use crate::shell_sidebar_preference::{
    SidebarExpansionCapability, SidebarExpansionPreference, resolve_sidebar_expansion,
};
use crate::shell_sidebar_presentation::{NavigationSeparator, separator_for_form};
use serde::Serialize;

/// The page shape a capture slug renders.
///
/// The slug is the capture matrix's vocabulary; the shape is the contract's.
/// A workbench slug is a DIFFERENT shape from its mother page: a list page
/// frames its body and scrolls inside the frame, while a workbench lays out
/// its panels by content and ends where its last panel ends. Judging a
/// workbench by its mother page's frame rule would flag the reference
/// product's own layout as broken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PageShape {
    pub page: ShellPage,
    pub structure: PageStructure,
}

/// Resolve a capture page slug onto the page and the shape it renders.
pub fn shape_for_slug(slug: &str) -> Option<PageShape> {
    let (page, structure) = match slug {
        "overview" => (ShellPage::Overview, PageStructure::Dense),
        "overview_workbench" => (ShellPage::Overview, PageStructure::Content),
        "proxies" => (ShellPage::Proxies, PageStructure::Content),
        "proxies_workbench" => (ShellPage::Proxies, PageStructure::Content),
        "profiles" => (ShellPage::Profiles, PageStructure::Content),
        "rules" => (ShellPage::Rules, PageStructure::Framed),
        "rules_workbench" => (ShellPage::Rules, PageStructure::Content),
        "connections" => (ShellPage::Connections, PageStructure::Framed),
        "connections_workbench" => (ShellPage::Connections, PageStructure::Content),
        "logs" => (ShellPage::Logs, PageStructure::Framed),
        "logs_workbench" => (ShellPage::Logs, PageStructure::Content),
        "dns" => (ShellPage::Dns, PageStructure::Content),
        "doctor" => (ShellPage::Doctor, PageStructure::Content),
        "app_routing" => (ShellPage::AppRouting, PageStructure::Content),
        "sync" => (ShellPage::Sync, PageStructure::Content),
        "settings" => (ShellPage::Settings, PageStructure::Content),
        "settings_workbench" | "shell_workbench" => (ShellPage::Settings, PageStructure::Content),
        "about" => (ShellPage::About, PageStructure::Content),
        "editor" => (ShellPage::Settings, PageStructure::Content),
        "filters" => (ShellPage::Settings, PageStructure::Content),
        "runtime" => (ShellPage::Overview, PageStructure::Content),
        "runtime_workbench" => (ShellPage::Overview, PageStructure::Content),
        _ => return None,
    };
    Some(PageShape { page, structure })
}

/// Emit the page-shape table for the cross-language space-utilization judge.
///
/// The judge is a Python script; it must not keep its own copy of the
/// limits. This JSON is the exported truth, and the judge reads it instead
/// of hard-coding a number.
///
/// Version 11 exports the navigation-column caps next to the page-shape caps,
/// so the judge no longer keeps a private copy of either limit. Version 10
/// added the closed `sidebar_variants` table, so a user-owned
/// collapsed preference is never re-derived by a surface or the judge. Version
/// 5 added `nav_stack_bottom_px`; version 4 published seven chrome keys and
/// left the judge to guess where the nav stack ended from a fraction of the
/// window height; version 1-3 carried no ladder at all.
pub fn page_structure_json() -> String {
    let pages: Vec<String> = ShellPage::ALL
        .iter()
        .map(|page| {
            let structure = page.structure();
            format!(
                "{{\"id\":\"{}\",\"structure\":\"{}\",\"ground_limit_pct\":{}}}",
                page.id(),
                structure.id(),
                ground_limit(structure)
            )
        })
        .collect();
    let slugs: Vec<String> = [
        "overview",
        "overview_workbench",
        "proxies",
        "proxies_workbench",
        "profiles",
        "rules",
        "rules_workbench",
        "connections",
        "connections_workbench",
        "logs",
        "logs_workbench",
        "dns",
        "doctor",
        "app_routing",
        "sync",
        "settings",
        "settings_workbench",
        "shell_workbench",
        "about",
        "editor",
        "filters",
        "runtime",
        "runtime_workbench",
    ]
    .iter()
    .filter_map(|slug| {
        shape_for_slug(slug).map(|shape| {
            format!(
                "{{\"slug\":\"{}\",\"page\":\"{}\",\"structure\":\"{}\",\"ground_limit_pct\":{}}}",
                slug,
                shape.page.id(),
                shape.structure.id(),
                ground_limit(shape.structure)
            )
        })
    })
    .collect();
    format!(
        "{{\"version\":11,\"dashboard_columns\":{},\"dashboard_density\":{},\"navigation_limits\":{},\"pages\":[{}],\"slugs\":[{}],\"tiers\":{},\"sidebar_variants\":{},\"navigation_separators\":{},\"chrome_keys\":{}}}\n",
        dashboard_columns_json(),
        dashboard_density_json(),
        navigation_limits_json(),
        pages.join(","),
        slugs.join(","),
        tiers_json(),
        sidebar_variants_json(),
        navigation_separators_json(),
        chrome_keys_json()
    )
}

/// The vocabulary of a frame's chrome line, exported so no side keeps a copy.
///
/// The capture host writes these keys, the judge requires them, and the guard
/// that pins the Slint property reads the same list: three consumers of one
/// declaration. See `crate::shell_receipt`.
pub fn chrome_marker_keys() -> Vec<&'static str> {
    CHROME_KEYS.to_vec()
}

fn chrome_keys_json() -> String {
    serde_json::to_string(&chrome_marker_keys()).expect("the chrome keys are plain strings")
}

/// The navigation-column caps, exported so the judge does not keep a private
/// copy of the limits it enforces. Both numbers belong to the same contract
/// that owns the page-shape caps.
fn navigation_limits_json() -> String {
    format!(
        "{{\"chrome_band_limit_pct\":{},\"void_limit_pct\":{}}}",
        NAV_CHROME_BAND_LIMIT_PCT, NAV_VOID_LIMIT_PCT
    )
}

/// The dashboard's two column semantics and the one width rule that splits
/// the share grid, exported so a surface never has to guess which `usize` a
/// ladder entry means (see `SPACEUTIL-008`).
fn dashboard_columns_json() -> String {
    let variants = [DashboardColumns::WrappedCards, DashboardColumns::GridShares];
    let entries: Vec<String> = variants
        .iter()
        .map(|semantics| {
            format!(
                "{{\"id\":\"{}\",\"four_column_min_px\":{}}}",
                semantics.id(),
                DASHBOARD_FOUR_COLUMN_MIN_PX
            )
        })
        .collect();
    format!(
        "{{\"variants\":[{}],\"four_column_min_px\":{}}}",
        entries.join(","),
        DASHBOARD_FOUR_COLUMN_MIN_PX
    )
}

/// The peer dashboard's three typed densities and the four room thresholds.
///
/// `DashboardDensity::of(width, height)` owns the decision; the JSON only
/// publishes its vocabulary and the numbers the Slint adapter needs to spell
/// the same lookup. A surface must not compare its own window against these
/// constants: it asks the generated adapter for the typed answer.
fn dashboard_density_json() -> String {
    let variants: Vec<String> = [
        DashboardDensity::Mobile,
        DashboardDensity::Compact,
        DashboardDensity::Standard,
    ]
    .iter()
    .map(|density| format!("{{\"id\":\"{}\"}}", density.id()))
    .collect();
    format!(
        "{{\"variants\":[{}],\"desktop_min_px\":{},\"four_column_min_px\":{},\"standard_four_column_min_px\":{},\"standard_height_min_px\":{},\"mobile_max_px\":{}}}",
        variants.join(","),
        DASHBOARD_DESKTOP_MIN_PX,
        DASHBOARD_FOUR_COLUMN_MIN_PX,
        DASHBOARD_STANDARD_FOUR_COLUMN_MIN_PX,
        DASHBOARD_STANDARD_HEIGHT_MIN_PX,
        DASHBOARD_MOBILE_MAX_PX,
    )
}

/// The navigation separator vocabulary and the decision for every exported
/// form. A surface reads this instead of deciding from a group label, a width
/// or its own `labelled` boolean that a heading would look useful here.
fn navigation_separators_json() -> String {
    let variants: Vec<String> = NavigationSeparator::ALL
        .iter()
        .map(|separator| {
            format!(
                "{{\"id\":\"{}\",\"paints_heading\":{}}}",
                separator.id(),
                separator.paints_heading()
            )
        })
        .collect();
    let forms: Vec<String> = [
        SidebarForm::BottomNav,
        SidebarForm::Rail,
        SidebarForm::Standard,
        SidebarForm::Wide,
    ]
    .iter()
    .map(|form| {
        format!(
            "{{\"id\":\"{}\",\"separator\":\"{}\"}}",
            form.id(),
            separator_for_form(*form).id()
        )
    })
    .collect();
    format!(
        "{{\"variants\":[{}],\"forms\":[{}]}}",
        variants.join(","),
        forms.join(",")
    )
}

/// How a page fills its body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PageStructure {
    /// One card frames the body and its content scrolls inside; the frame
    /// reaches the bottom of the page body, so no ground may show below it.
    Framed,
    /// Blocks are laid out by content and the page ends where its last
    /// block ends.
    Content,
    /// Blocks are laid out by content, and the page carries a dense set of
    /// small modules that fill the body by design.
    Dense,
}

impl PageStructure {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Framed => "framed",
            Self::Content => "content",
            Self::Dense => "dense",
        }
    }
}

impl ShellPage {
    /// The content shape of this page.
    ///
    /// The judgement follows the peer product's measured behaviour for the
    /// same shape: a framed list page is filled to the bottom, a content
    /// page ends with its last block (the reference product's own agent and
    /// about pages leave ground below their last row), and the overview is a
    /// dense card grid.
    pub const fn structure(self) -> PageStructure {
        match self {
            Self::Overview => PageStructure::Dense,
            Self::Connections | Self::Logs | Self::Rules => PageStructure::Framed,
            Self::Proxies
            | Self::Profiles
            | Self::Dns
            | Self::Doctor
            | Self::AppRouting
            | Self::Sync
            | Self::Settings
            | Self::Tools
            | Self::About
            | Self::Disclaimer
            | Self::Licenses => PageStructure::Content,
        }
    }
}

/// The measured ground a peer product leaves below the last content row of
/// a content-shaped page, as a percentage of the page body.
///
/// Measured on the FlClash desktop reference (v0.9.3): its proxies page
/// ends at y357 of a 1849px body (80.6% empty) and its tools page lists its
/// sections by content. A content page may therefore leave ground below its
/// last block; the limit is the reference's own behaviour plus a margin, so
/// a page that empties more than the reference does still fails.
pub const CONTENT_PAGE_GROUND_LIMIT_PCT: f64 = 82.0;

/// A framed page may not leave measurable ground: the frame reaches the
/// bottom of the page body.
pub const FRAMED_PAGE_GROUND_LIMIT_PCT: f64 = 4.0;

/// A dense page is a card grid; only a card's own trailing air counts.
pub const DENSE_PAGE_GROUND_LIMIT_PCT: f64 = 12.0;

/// The navigation column is navigation only: no status card, brand tile or
/// version line may live below the destination stack. The FlClash desktop
/// reference paints only destination rows, so the cap is zero.
pub const NAV_CHROME_BAND_LIMIT_PCT: f64 = 0.0;

/// The largest dead band allowed between destination rows, as a share of the
/// window height. Stretch air belongs at the end of the stack, not inside it.
///
/// The FlClash desktop reference's own largest inter-row gap measures 3.06%
/// of its 1665px window. Our shortest tier is 480px tall and paints eight
/// destinations instead of five, so the same absolute gap is a larger share:
/// 8% is a bounded 2.6x scale headroom over the reference, not a flat copy.
/// `flclash-sidebar-void-guard.py` measures the reference and pins this ratio.
pub const NAV_VOID_LIMIT_PCT: f64 = 8.0;

/// The ground limit for one page shape.
pub const fn ground_limit(structure: PageStructure) -> f64 {
    match structure {
        PageStructure::Framed => FRAMED_PAGE_GROUND_LIMIT_PCT,
        PageStructure::Dense => DENSE_PAGE_GROUND_LIMIT_PCT,
        PageStructure::Content => CONTENT_PAGE_GROUND_LIMIT_PCT,
    }
}

/// The share of the page body a frame without a known shape may leave empty.
///
/// A page only falls back to a tier limit when its shape is missing from the
/// table above, which is itself a bug the `--check-export` leg reports. Below
/// the labelled-sidebar boundary the fallback is tighter than the dense-page
/// limit: a narrow window has proportionally less room, so the same dead band
/// is a larger share of its body.
pub const NARROW_TIER_GROUND_LIMIT_PCT: f64 = 8.0;

/// The tier-level fallback limit for one viewport tier.
///
/// The judge may only reach for this when a frame's page is missing from the
/// shape table, which is itself a bug the `--check-export` leg reports. It is
/// exported so the judge holds no second copy of the number.
pub const fn tier_ground_fallback(tier: ViewportTier) -> f64 {
    if tier.has_labelled_sidebar() {
        DENSE_PAGE_GROUND_LIMIT_PCT
    } else {
        NARROW_TIER_GROUND_LIMIT_PCT
    }
}

/// One tier of the shared viewport ladder, exported.
///
/// The boundary and every chrome fact derived from it are emitted here so a
/// surface's layout language and the Python judge read the same answer to
/// "which viewport tier is this". A surface that writes its own boundary is a
/// second idea of the same window.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TierFact {
    pub id: &'static str,
    pub min_width_px: u16,
    pub max_width_px: Option<u16>,
    pub nav_form: &'static str,
    /// The form a touch surface gets at this width. Exported, not recomputed by
    /// a shell: Slint and Android read the same table Iced and the judge read,
    /// so "phone or desktop navigation" cannot fork per surface.
    pub touch_nav_form: &'static str,
    pub nav_column_px: u16,
    /// The widest column this tier may reserve - the shell's own share limit.
    /// The judge and the Slint guard read it so the ratio standard lives once.
    pub max_nav_column_px: u16,
    /// The widest leading the tier's own column may keep, or 0 when the form
    /// centres its glyphs instead of indenting them. Exported as a limit rather
    /// than a share percentage so the judge compares two numbers instead of
    /// re-deriving the same arithmetic a second time.
    pub nav_leading_limit_px: u16,
    pub labelled_sidebar: bool,
    pub proxy_node_columns: usize,
    pub overview_card_columns: usize,
    pub fallback_ground_limit_pct: f64,
}

impl TierFact {
    const fn of(tier: ViewportTier) -> Self {
        Self {
            id: tier.id(),
            min_width_px: tier.lower_bound_px(),
            max_width_px: tier.upper_bound_px(),
            nav_form: tier.sidebar_form().id(),
            touch_nav_form: tier.shell_navigation(true).id(),
            nav_column_px: tier.nav_column_px(),
            max_nav_column_px: tier.max_nav_column_px(),
            nav_leading_limit_px: tier.nav_leading_limit_px(),
            labelled_sidebar: tier.has_labelled_sidebar(),
            proxy_node_columns: tier.proxy_grid_columns(false),
            overview_card_columns: tier.overview_card_columns(),
            fallback_ground_limit_pct: tier_ground_fallback(tier),
        }
    }
}

/// Every tier of the ladder, in width order.
pub const fn tier_facts() -> [TierFact; TIERS.len()] {
    [
        TierFact::of(TIERS[0]),
        TierFact::of(TIERS[1]),
        TierFact::of(TIERS[2]),
        TierFact::of(TIERS[3]),
    ]
}

/// One legal navigation chrome after a user preference, viewport tier and
/// surface kind meet the shared resolver.
///
/// FlClash derives `expanded = canExpandSidebar(desktop) && sidebarExpanded`
/// once and persists only the preference. The exported table below is that
/// single decision for every closed combination, so Slint and the space judge
/// never write a second `if expanded` or reinterpret a rail as a labelled pane.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SidebarVariantFact {
    pub preference: &'static str,
    pub tier: &'static str,
    pub touch_surface: bool,
    pub nav_form: &'static str,
    pub nav_column_px: u16,
    pub max_nav_column_px: u16,
    pub nav_leading_limit_px: u16,
    pub labelled_sidebar: bool,
    pub toggle_available: bool,
    pub fallback_ground_limit_pct: f64,
}

impl SidebarVariantFact {
    const fn of(
        preference: SidebarExpansionPreference,
        tier: ViewportTier,
        touch_surface: bool,
    ) -> Self {
        let capability_form = tier.shell_navigation(touch_surface);
        let capability = SidebarExpansionCapability::from_form(capability_form);
        let presentation = resolve_sidebar_expansion(preference, capability);
        let form = presentation.form();
        let labelled = presentation.shows_labels();
        Self {
            preference: preference.id(),
            tier: tier.id(),
            touch_surface,
            nav_form: form.id(),
            nav_column_px: match form.width_px() {
                Some(width) => width,
                None => 0,
            },
            max_nav_column_px: tier.max_nav_column_px(),
            nav_leading_limit_px: if labelled {
                tier.nav_leading_limit_px()
            } else {
                0
            },
            labelled_sidebar: labelled,
            toggle_available: capability.can_expand(),
            fallback_ground_limit_pct: tier_ground_fallback(tier),
        }
    }
}

/// The complete preference × tier × surface-kind chrome table.
pub fn sidebar_variant_facts() -> Vec<SidebarVariantFact> {
    let mut facts = Vec::new();
    for preference in SidebarExpansionPreference::ALL {
        for tier in TIERS {
            for touch_surface in [false, true] {
                facts.push(SidebarVariantFact::of(preference, tier, touch_surface));
            }
        }
    }
    facts
}

fn sidebar_variants_json() -> String {
    serde_json::to_string(&sidebar_variant_facts())
        .expect("the sidebar chrome table is a closed list of scalars")
}

/// The exported ladder as a JSON array, for the document below.
fn tiers_json() -> String {
    serde_json::to_string(&tier_facts()).expect("the viewport ladder is a closed table of scalars")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shared_page_declares_a_structure() {
        for page in ShellPage::ALL {
            let structure = page.structure();
            assert!(!structure.id().is_empty());
            assert!(ground_limit(structure) > 0.0);
        }
    }

    #[test]
    fn a_framed_page_is_judged_stricter_than_a_content_page() {
        assert!(ground_limit(PageStructure::Framed) < ground_limit(PageStructure::Content));
    }

    #[test]
    fn the_list_pages_are_framed_and_the_about_page_is_content() {
        assert_eq!(ShellPage::Logs.structure(), PageStructure::Framed);
        assert_eq!(ShellPage::Connections.structure(), PageStructure::Framed);
        assert_eq!(ShellPage::Rules.structure(), PageStructure::Framed);
        assert_eq!(ShellPage::About.structure(), PageStructure::Content);
        assert_eq!(ShellPage::Proxies.structure(), PageStructure::Content);
        assert_eq!(ShellPage::Overview.structure(), PageStructure::Dense);
    }

    #[test]
    fn the_ladder_tiles_the_width_axis_without_gaps() {
        let facts = tier_facts();
        assert_eq!(facts[0].min_width_px, 0);
        for pair in facts.windows(2) {
            // Half-open tiers: the upper bound of one is the lower bound of
            // the next, so no width is unclaimed and none is claimed twice.
            assert_eq!(pair[0].max_width_px, Some(pair[1].min_width_px));
        }
        assert_eq!(facts.last().expect("a non-empty ladder").max_width_px, None);
    }

    #[test]
    fn the_ladder_is_the_only_answer_to_which_tier_this_width_is() {
        for fact in tier_facts() {
            let middle = match fact.max_width_px {
                Some(max) => {
                    f32::from(fact.min_width_px)
                        + (f32::from(max) - f32::from(fact.min_width_px)) / 2.0
                }
                None => f32::from(fact.min_width_px) + 400.0,
            };
            assert_eq!(ViewportTier::from_width(middle).id(), fact.id);
        }
    }

    #[test]
    fn only_the_forms_with_a_column_reserve_one() {
        let facts = tier_facts();
        for fact in facts.iter() {
            assert_eq!(
                fact.nav_column_px > 0,
                fact.max_nav_column_px > 0,
                "{} either owns a column or owns none",
                fact.id
            );
            assert!(
                fact.nav_column_px <= fact.max_nav_column_px,
                "{} reserves {}px but the standard allows {}px",
                fact.id,
                fact.nav_column_px,
                fact.max_nav_column_px
            );
            assert_eq!(
                fact.nav_leading_limit_px > 0,
                fact.labelled_sidebar,
                "{} budgets a leading exactly when it labels its rows",
                fact.id
            );
        }
        // The bottom-navigation tier has no vertical column at all. The rail
        // tier does reserve one - 64px of icon-only navigation - and it is not
        // a labelled sidebar, because a rail labels nothing: the page shape
        // stays narrow while the navigation still owns its column.
        assert_eq!(facts[0].nav_column_px, 0);
        assert_eq!(facts[0].max_nav_column_px, 0);
        assert_eq!(facts[1].nav_column_px, 64);
        assert_eq!(facts[1].nav_leading_limit_px, 0);
        assert!(!facts[1].labelled_sidebar);
        // A labelled column indents its rows, so it budgets 8% of the column
        // it already reserved: 240 -> 19, 280 -> 22.
        assert_eq!(facts[2].nav_leading_limit_px, 19);
        assert_eq!(facts[3].nav_leading_limit_px, 22);
        // A touch surface never gets the rail, at any tier.
        for fact in facts.iter() {
            assert_eq!(fact.touch_nav_form, "bottom_nav");
        }
    }

    #[test]
    fn the_exported_document_carries_the_ladder() {
        let json = page_structure_json();
        let document: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(document["version"], 11);
        // The chrome vocabulary travels with the ladder it describes, so a
        // reader that has the document already has the key list.
        assert_eq!(
            document["chrome_keys"]
                .as_array()
                .expect("the chrome keys")
                .len(),
            CHROME_KEYS.len()
        );
        assert_eq!(document["chrome_keys"][0], "nav_form");
        assert_eq!(
            document["chrome_keys"][CHROME_KEYS.len() - 1],
            "sidebar_toggle_px"
        );
        assert_eq!(document["chrome_keys"][2], "nav_stack_bottom_px");
        assert_eq!(document["chrome_keys"][3], "nav_content_offset_px");
        let dashboard = &document["dashboard_columns"];
        assert_eq!(dashboard["four_column_min_px"], 860);
        assert_eq!(dashboard["variants"][0]["id"], "wrapped_cards");
        assert_eq!(dashboard["variants"][1]["id"], "grid_shares");
        let density = &document["dashboard_density"];
        assert_eq!(density["variants"][0]["id"], "mobile");
        assert_eq!(density["variants"][1]["id"], "compact");
        assert_eq!(density["variants"][2]["id"], "standard");
        assert_eq!(density["desktop_min_px"], 640);
        assert_eq!(density["four_column_min_px"], 860);
        assert_eq!(density["standard_four_column_min_px"], 1040);
        assert_eq!(density["standard_height_min_px"], 824);
        assert_eq!(density["mobile_max_px"], 560);
        let tiers = document["tiers"].as_array().expect("the exported ladder");
        assert_eq!(tiers.len(), TIERS.len());
        assert_eq!(tiers[2]["id"], "expanded");
        assert_eq!(tiers[2]["max_width_px"], 1200);
        assert_eq!(tiers[2]["proxy_node_columns"], 3);
        assert_eq!(tiers[1]["nav_form"], "rail");
        assert_eq!(tiers[1]["nav_column_px"], 64);
        assert_eq!(tiers[1]["max_nav_column_px"], 72);
        assert_eq!(tiers[1]["nav_leading_limit_px"], 0);
        assert_eq!(tiers[2]["nav_leading_limit_px"], 19);
        assert_eq!(tiers[1]["touch_nav_form"], "bottom_nav");
        assert_eq!(tiers[3]["max_width_px"], serde_json::Value::Null);
        assert_eq!(tiers[0]["labelled_sidebar"], false);
        assert_eq!(
            tiers[0]["fallback_ground_limit_pct"],
            NARROW_TIER_GROUND_LIMIT_PCT
        );
        let variants = document["sidebar_variants"]
            .as_array()
            .expect("the exported sidebar chrome table");
        assert_eq!(variants.len(), 2 * TIERS.len() * 2);
        let collapsed_desktop = variants
            .iter()
            .find(|entry| {
                entry["tier"] == "expanded"
                    && entry["preference"] == "collapsed"
                    && entry["touch_surface"] == false
            })
            .expect("the desktop collapsed variant");
        assert_eq!(collapsed_desktop["nav_form"], "rail");
        assert_eq!(collapsed_desktop["nav_column_px"], 64);
        assert_eq!(collapsed_desktop["nav_leading_limit_px"], 0);
        assert_eq!(collapsed_desktop["labelled_sidebar"], false);
        assert_eq!(collapsed_desktop["toggle_available"], true);
        let expanded_desktop = variants
            .iter()
            .find(|entry| {
                entry["tier"] == "expanded"
                    && entry["preference"] == "expanded"
                    && entry["touch_surface"] == false
            })
            .expect("the desktop expanded variant");
        assert_eq!(expanded_desktop["nav_form"], "standard");
        assert_eq!(expanded_desktop["nav_column_px"], 240);
        assert_eq!(expanded_desktop["toggle_available"], true);
        let touch_desktop = variants
            .iter()
            .find(|entry| {
                entry["tier"] == "expanded"
                    && entry["preference"] == "expanded"
                    && entry["touch_surface"] == true
            })
            .expect("the desktop touch variant");
        assert_eq!(touch_desktop["nav_form"], "bottom_nav");
        assert_eq!(touch_desktop["toggle_available"], false);
    }

    #[test]
    fn every_sidebar_variant_comes_from_the_shared_resolver() {
        for preference in SidebarExpansionPreference::ALL {
            for tier in TIERS {
                for touch_surface in [false, true] {
                    let expected = SidebarVariantFact::of(preference, tier, touch_surface);
                    assert!(sidebar_variant_facts().contains(&expected));
                    if touch_surface {
                        assert_eq!(expected.nav_form, "bottom_nav");
                        assert!(!expected.toggle_available);
                    }
                    if expected.toggle_available {
                        assert!(expected.tier == "expanded" || expected.tier == "ultra");
                    }
                }
            }
        }
    }
}

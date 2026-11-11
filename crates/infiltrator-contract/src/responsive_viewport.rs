//! Shared contracts for 4-tier responsive viewport breakpoints across Iced and Bevy.
//!
//! This module is the **single authoritative source for breakpoint thresholds**
//! (`600 / 840 / 1200`). `infiltrator-bevy-widgets` cannot depend on this crate
//! (it is business-agnostic by charter), so it mirrors the same numbers and
//! `crates/infiltrator-bevy-ui/tests/headless/responsive_ui_tests.rs` fails if
//! the two drift.
//! See `crates/infiltrator-contract/docs/UI_PARITY_AUDIT.md` for the shared breakpoints.

use serde::{Deserialize, Serialize};

/// Shortest/longest widths each tier spans, used by tests and guards.
pub const COMPACT_MAX_PX: f32 = 600.0;
/// Medium tier lower/upper boundary.
pub const MEDIUM_MAX_PX: f32 = 840.0;
/// Expanded tier lower/upper boundary; `>=` this value is [`ViewportTier::Ultra`].
pub const EXPANDED_MAX_PX: f32 = 1200.0;

/// The whole ladder in width order.
///
/// A consumer that walks the tiers - the exporter, a guard, or a surface that
/// publishes the ladder into its own layout language - iterates this list, so a
/// new tier can only appear by adding a variant to [`ViewportTier`] and an
/// entry here.
pub const TIERS: [ViewportTier; 4] = [
    ViewportTier::Compact,
    ViewportTier::Medium,
    ViewportTier::Expanded,
    ViewportTier::Ultra,
];

/// The share of a tier's narrowest width an icon rail may reserve.
///
/// FlClash's rail is 48 of 600 (8.0%); this contract's is 64 of 600 (10.7%),
/// inside the same ceiling while keeping our 24px icon and its touch target
/// readable. Beyond this the rail stops being navigation and becomes a second
/// sidebar paid for twice.
pub const MAX_RAIL_COLUMN_SHARE_PCT: f64 = 12.0;

/// The share of a tier's narrowest width a labelled column may reserve.
///
/// FlClash expands to 220px and only above its 840 boundary (26.2% at the
/// boundary); 30% is that allowance stated once for every labelled form. The
/// content floor below usually binds first, which is the point: the ratio is
/// not what licenses a wide column, the content it leaves behind is.
pub const MAX_LABELLED_COLUMN_SHARE_PCT: f64 = 30.0;

/// The narrowest page body a labelled column may leave behind.
///
/// This is [`COMPACT_MAX_PX`]: the widest window a phone-shaped page ever gets.
/// A column that leaves less than that makes this tier's content narrower than
/// the tier below it, which is exactly the defect FlClash refuses to ship.
pub const LABELLED_CONTENT_FLOOR_PX: u16 = COMPACT_MAX_PX as u16;

/// The share of a labelled column that may be spent on the nav content's own
/// leading (the ground between the column's left edge and the first glyph).
///
/// FlClash answers this twice and both answers fit: its shipped scene measures
/// an icon box at 7.2dp and a label ink at 44.1dp inside a 220dp column (this
/// shell copies that pair, 8px and 44px), while its source derives the roomier
/// 12dp / 48dp from `min(12, (48 - 40) / 2)` plus `(40 - 24) / 2` - 5.5%. An
/// 8% ceiling therefore admits either reading and still rejects a leading that
/// spends more of the column than the reference ever does. It is deliberately a
/// share rather than a fixed number of pixels: a wider column earns a wider
/// leading, and a leading that outgrows the share is ground the column paid for
/// twice.
pub const MAX_NAV_LEADING_SHARE_PCT: f64 = 8.0;

/// The page width at which the reference dashboard switches from two grid
/// shares to four.
///
/// FlClash states this once in `lib/views/dashboard/layout.dart`:
/// `dashboardColumnsOf(width) => width < _minFourColumnWidth ? 2 : 4` with
/// `_minFourColumnWidth = 860`. It is a property of the **page body's width**,
/// not of a window tier, so it cannot be derived from `ViewportTier` without
/// pretending a tier boundary is a dashboard boundary.
pub const DASHBOARD_FOUR_COLUMN_MIN_PX: u16 = 860;

/// The narrowest page body that may carry the four-share desktop dashboard.
///
/// FlClash's `dashboard/layout.dart` uses 1040 as the lower edge of a
/// standard-density four-column page. A page between 860 and 1040 still has
/// four shares, but shrinks its type, tiles and subtitles.
pub const DASHBOARD_STANDARD_FOUR_COLUMN_MIN_PX: u16 = 1040;

/// The shortest page body that may carry standard dashboard density.
pub const DASHBOARD_STANDARD_HEIGHT_MIN_PX: u16 = 824;

/// The narrowest page body that owns a desktop dashboard at all.
pub const DASHBOARD_DESKTOP_MIN_PX: u16 = 640;

/// The widest page body that keeps one centred mobile column.
pub const DASHBOARD_MOBILE_MAX_PX: u16 = 560;

/// The narrowest profile card the reference will draw side by side.
///
/// FlClash states this once as `profileItemMinWidth = 300.0`
/// (`lib/common/layout.dart`) and feeds it to `getProfilesColumns`, which the
/// profiles page calls with the width the grid was given and the row gap it is
/// already paying. A surface that stacks profile cards one per row therefore
/// spends the whole page body on a single card and shows fewer of them than
/// the reference at the same window - the horizontal twin of the vertical
/// dead ground the space-utilization standard already refuses.
pub const PROFILE_MIN_ITEM_WIDTH_PX: u16 = 300;

/// The most profile columns the reference will ever draw.
///
/// FlClash clamps `getProfilesColumns` to `_profilesMaxColumns = 3`, so a very
/// wide window keeps cards readable instead of thinning them without end.
pub const PROFILE_MAX_COLUMNS: u16 = 3;

/// The narrowest proxy node card the reference will draw side by side.
///
/// FlClash states this once as `250` in `getProxiesColumns`
/// (`lib/common/layout.dart`) and the desktop scene's 371px card at 1.5x
/// scaling measures back to ~248dp. The same rule keeps a 940px page body
/// from drawing only three oversized cards.
pub const PROXY_MIN_ITEM_WIDTH_PX: u16 = 250;

/// The page-body inset the reference removes before counting proxy cards.
///
/// `lib/views/proxies/tab.dart` passes `max(width - 32, 0)` to
/// `getProxiesColumns`; the grid then pays its own horizontal seam.
pub const PROXY_GRID_INSET_PX: u16 = 32;

/// The fewest proxy node columns the reference draws on a desktop page.
pub const PROXY_MIN_COLUMNS: u16 = 2;

/// Which proxy grid rule owns a page at this shell form.
///
/// FlClash's desktop tab view calls `getProxiesColumns` with a desktop page
/// width and keeps a two-column floor; its mobile view keeps one readable
/// column. This enum is the typed border between those two decisions, so a
/// surface cannot smuggle the distinction in as two booleans.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProxyGridForm {
    Stacked,
    Desktop,
}

/// Which profile grid rule owns a page at this shell form.
///
/// A labelled desktop page has room to ask the 300dp width rule; a bottom-nav
/// or icon-rail page keeps one readable column even when its body would
/// numerically fit two narrow cards.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProfileGridForm {
    Stacked,
    Desktop,
}

impl ProfileGridForm {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Stacked => "stacked",
            Self::Desktop => "desktop",
        }
    }
}

impl ProxyGridForm {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Stacked => "stacked",
            Self::Desktop => "desktop",
        }
    }
}

/// How many profile columns fit in `page_width_px` beside a `gap_px` seam.
///
/// This is FlClash's `getProfilesColumns` arithmetic: a row of `n` cards needs
/// `n * min + (n - 1) * gap`, so `n = floor((width + gap) / (min + gap))`,
/// clamped into `1..=PROFILE_MAX_COLUMNS`. The gap is the caller's own - it is
/// the seam the grid is already paying, not a second layout constant - which
/// is why the count is a pure function of room rather than of a viewport tier.
pub const fn profile_columns(form: ProfileGridForm, page_width_px: u16, gap_px: u16) -> u16 {
    if matches!(form, ProfileGridForm::Stacked) {
        return 1;
    }
    let min = PROFILE_MIN_ITEM_WIDTH_PX;
    let columns = (page_width_px + gap_px) / (min + gap_px);
    if columns < 1 {
        1
    } else if columns > PROFILE_MAX_COLUMNS {
        PROFILE_MAX_COLUMNS
    } else {
        columns
    }
}

/// How many proxy node cards fit in `page_width_px`.
///
/// FlClash's `getProxiesColumns` uses `max(ceil(viewWidth / 250), 2)` for its
/// standard layout; `viewWidth` is the page body after its 32dp inset. This
/// contract keeps that width rule without adding the reference's three user
/// preferences (`tight/standard/loose`) that our outbound model does not own.
/// A zero-width page still gets the reference's two-column floor rather than a
/// private fallback in the surface.
pub const fn proxy_node_columns(form: ProxyGridForm, page_width_px: u16) -> u16 {
    if matches!(form, ProxyGridForm::Stacked) {
        return 1;
    }
    let room = page_width_px.saturating_sub(PROXY_GRID_INSET_PX);
    let columns = room.div_ceil(PROXY_MIN_ITEM_WIDTH_PX);
    if columns < PROXY_MIN_COLUMNS {
        PROXY_MIN_COLUMNS
    } else {
        columns
    }
}

/// The three dashboard densities published by the peer product's layout.
///
/// FlClash declares this enum once beside `dashboardColumnsOf` and injects it
/// through `DashboardDensityScope`; every card asks the scope instead of
/// measuring the window itself. This contract keeps the same typed vocabulary
/// so a surface cannot turn a width and a height comparison into a second,
/// hidden state table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DashboardDensity {
    /// Phone-shaped pages: one centred column and mobile card metrics.
    Mobile,
    /// Desktop pages with narrow four-share rows or too little height.
    #[default]
    Compact,
    /// Desktop pages with room for the full four-share card metrics.
    Standard,
}

impl DashboardDensity {
    /// Resolve the density from the page body's own width and height.
    ///
    /// `size` is the layout room left for the dashboard, not the outer window:
    /// upstream calls `DashboardDensity.of(constraints.biggest / scale)` inside
    /// a `LayoutBuilder` beneath the scaffold, so the navigation column and
    /// title bar are already paid for. A page narrower than 640 is mobile; a
    /// four-share page below 1040 or a short page below 824 is compact.
    pub const fn of(width_px: f32, height_px: f32) -> Self {
        if width_px < DASHBOARD_DESKTOP_MIN_PX as f32 {
            Self::Mobile
        } else if (width_px >= DASHBOARD_FOUR_COLUMN_MIN_PX as f32
            && width_px < DASHBOARD_STANDARD_FOUR_COLUMN_MIN_PX as f32)
            || height_px < DASHBOARD_STANDARD_HEIGHT_MIN_PX as f32
        {
            Self::Compact
        } else {
            Self::Standard
        }
    }

    /// Whether this density paints the desktop grid rather than the phone flow.
    pub const fn is_desktop(self) -> bool {
        !matches!(self, Self::Mobile)
    }

    /// Whether this density tightens the desktop card metrics.
    pub const fn is_compact(self) -> bool {
        matches!(self, Self::Compact)
    }

    /// The stable id shared by the exporter, the Slint adapter and guards.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Mobile => "mobile",
            Self::Compact => "compact",
            Self::Standard => "standard",
        }
    }
}

/// The two dashboard column semantics this product actually ships.
///
/// They answer different questions and must not be collapsed into one number:
/// `WrappedCards` is how many equal-width cards a wrapping flow puts in one
/// row (the tier's `overview_card_columns`, 1/2/2/3, consumed by the Bevy
/// overview list), while `GridShares` is how many equal shares the dashboard's
/// span grid owns at this page width (2 or 4, consumed by Slint's Hero 2-span
/// plus half-share upload/download cards). `SPACEUTIL-008` existed precisely
/// because one untyped `usize` was read as if it answered both questions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DashboardColumns {
    /// Cards per row in a wrapping flow, by viewport tier.
    #[default]
    WrappedCards,
    /// Equal spans in the dashboard grid, by page width.
    GridShares,
}

impl DashboardColumns {
    /// The count this semantics assigns at a given tier and page width.
    ///
    /// `page_width_px` is the width left for the page body after the shell took
    /// its navigation column. A rail/phone form is held to two shares whatever
    /// the width - a four-share row in a 720px rail window clipped its last
    /// card off the right edge - while a labelled column uses the reference's
    /// 860 rule on the body that remains (1180 − 240 = 940 → four shares).
    pub fn count(self, tier: ViewportTier, page_width_px: u16) -> u16 {
        match self {
            Self::WrappedCards => tier.overview_card_columns() as u16,
            Self::GridShares => {
                if !tier.has_labelled_sidebar() || page_width_px < DASHBOARD_FOUR_COLUMN_MIN_PX {
                    2
                } else {
                    4
                }
            }
        }
    }

    /// The stable identity of this semantics, for the exported table.
    pub const fn id(self) -> &'static str {
        match self {
            Self::WrappedCards => "wrapped_cards",
            Self::GridShares => "grid_shares",
        }
    }
}

/// The widest leading a labelled column of `column_px` may keep.
///
/// The judge reads this through the exported share limit rather than keeping a
/// copy of the number, the same way it reads the column ceiling.
pub const fn column_leading_limit_px(column_px: u16) -> u16 {
    share_limit(column_px, MAX_NAV_LEADING_SHARE_PCT)
}

/// `share_pct` of `lower_px`, truncated to whole pixels.
const fn share_limit(lower_px: u16, share_pct: f64) -> u16 {
    (lower_px as f64 * share_pct / 100.0) as u16
}

/// Four canonical responsive viewport tiers shared across desktop, tablet, and mobile.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ViewportTier {
    /// Mobile portrait / compact display (< 600px width).
    Compact,
    /// Tablet portrait or foldables (600px <= width < 840px).
    Medium,
    /// Standard desktop / tablet landscape (840px <= width < 1200px).
    #[default]
    Expanded,
    /// Ultra-wide desktop (>= 1200px width).
    Ultra,
}

impl ViewportTier {
    pub fn from_width(width_px: f32) -> Self {
        if width_px < COMPACT_MAX_PX {
            Self::Compact
        } else if width_px < MEDIUM_MAX_PX {
            Self::Medium
        } else if width_px < EXPANDED_MAX_PX {
            Self::Expanded
        } else {
            Self::Ultra
        }
    }

    /// Navigation form the shell should present in this tier.
    pub const fn sidebar_form(self) -> SidebarForm {
        match self {
            Self::Compact => SidebarForm::BottomNav,
            Self::Medium => SidebarForm::Rail,
            Self::Expanded => SidebarForm::Standard,
            Self::Ultra => SidebarForm::Wide,
        }
    }

    /// The navigation form a window of this tier presents on a given surface.
    ///
    /// A touch surface has no pointing device to hover a narrow rail with, so it
    /// keeps the thumb-height bottom bar at every width; a desktop window reads
    /// the tier's own form. This is the one place that choice is made: a surface
    /// that compares its own width to decide which chrome to mount would be a
    /// second answer to the same question, which is what `SPACEUTIL-006` removes.
    pub const fn shell_navigation(self, touch_surface: bool) -> SidebarForm {
        if touch_surface {
            SidebarForm::BottomNav
        } else {
            self.sidebar_form()
        }
    }

    /// Outer content-region padding in pixels for this tier.
    pub const fn content_padding_px(self) -> u16 {
        match self {
            Self::Compact => 16,
            Self::Medium => 24,
            Self::Expanded => 40,
            Self::Ultra => 48,
        }
    }

    /// The page-body width a shell of this tier leaves for its pages.
    ///
    /// A wide shell reserves its navigation column beside the page; a compact
    /// shell stacks the bottom bar under it, so only the page's own padding
    /// narrows the body. The width-rule lookups (`profile_columns`,
    /// `proxy_node_columns`) and the surfaces must all measure the same room,
    /// so the subtraction lives here once instead of being re-derived per
    /// view (and drifting by one surface's private margin).
    pub const fn page_body_width_px(self, window_width_px: u16) -> u16 {
        let padded = window_width_px.saturating_sub(2 * self.content_padding_px());
        padded.saturating_sub(self.nav_column_px())
    }

    /// Whether this tier is narrow enough that the shell must shed its full sidebar.
    ///
    /// This is page density, not navigation: the narrow tiers stack their cards
    /// and shed the page's own breathing room. Reading it as "this window has no
    /// navigation column" is the conflation `SPACEUTIL-006` fixes - the medium
    /// tier is a narrow page that still owns an icon rail.
    pub const fn is_narrow(self) -> bool {
        matches!(self, Self::Compact | Self::Medium)
    }

    /// The vocabulary id of this tier, shared by every consumer of the ladder.
    ///
    /// A surface that names the same tier differently cannot be checked against
    /// the export, so this id is the one name the JSON, the generated layout
    /// file and the space-utilization judge agree on.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Medium => "medium",
            Self::Expanded => "expanded",
            Self::Ultra => "ultra",
        }
    }

    /// The lowest window width this tier owns, in logical pixels.
    pub const fn lower_bound_px(self) -> u16 {
        match self {
            Self::Compact => 0,
            Self::Medium => COMPACT_MAX_PX as u16,
            Self::Expanded => MEDIUM_MAX_PX as u16,
            Self::Ultra => EXPANDED_MAX_PX as u16,
        }
    }

    /// The exclusive upper bound of this tier, or `None` for the widest tier.
    pub const fn upper_bound_px(self) -> Option<u16> {
        match self {
            Self::Compact => Some(COMPACT_MAX_PX as u16),
            Self::Medium => Some(MEDIUM_MAX_PX as u16),
            Self::Expanded => Some(EXPANDED_MAX_PX as u16),
            Self::Ultra => None,
        }
    }

    /// Whether the shell shows a labelled sidebar column in this tier.
    ///
    /// One projection of [`ViewportTier::sidebar_form`], not a second decision:
    /// the forms that carry labels in a column are exactly the forms that own a
    /// column. A surface choosing between a side column and a bottom bar reads
    /// this instead of comparing the window width itself.
    pub const fn has_labelled_sidebar(self) -> bool {
        self.sidebar_form().shows_labels()
    }

    /// The width of the navigation column this tier owns, or `0` when the shell
    /// has no column at all.
    ///
    /// The judge starts a frame's page body after this column, so the box it
    /// measures is the room the surfaces were told to reserve.
    pub const fn nav_column_px(self) -> u16 {
        match self.sidebar_form().width_px() {
            Some(width) => width,
            None => 0,
        }
    }

    /// The widest leading this tier's own column may keep.
    ///
    /// A rail centres its glyphs in the column, so it has no leading to budget
    /// and states 0; a labelled column indents its rows and is held to
    /// [`MAX_NAV_LEADING_SHARE_PCT`] of the column it already reserved.
    pub const fn nav_leading_limit_px(self) -> u16 {
        if self.has_labelled_sidebar() {
            column_leading_limit_px(self.nav_column_px())
        } else {
            0
        }
    }

    /// The widest navigation column this tier is allowed to reserve.
    ///
    /// This is the space-utilization standard for the shell's own column, copied
    /// from how FlClash reasons rather than invented here. Their
    /// `canExpandSidebar` states the rule outright - "an expanded sidebar in the
    /// laptop range would leave the content narrower than the mobile layout one
    /// breakpoint below, so labels wait for desktop" (`lib/common/layout.dart`) -
    /// and their rail is 48px inside a 600..840 window
    /// (`plugins/prism_ui/lib/sidebar.dart`), on the same 600/840 boundaries this
    /// contract owns. Two limits follow from that:
    ///
    /// * an icon rail is a cheap column, so only its share of the tier's
    ///   narrowest width bounds it ([`MAX_RAIL_COLUMN_SHARE_PCT`]);
    /// * a labelled column is a luxury, so it must additionally leave the page
    ///   body at least [`LABELLED_CONTENT_FLOOR_PX`] wide - the widest window the
    ///   tier below can be - or widening the chrome would shrink the content
    ///   under what a narrower window already gets.
    ///
    /// The exported ladder carries this ceiling, so a surface that reserves more
    /// than its tier allows fails a gate instead of winning an argument.
    pub const fn max_nav_column_px(self) -> u16 {
        match self.sidebar_form() {
            SidebarForm::BottomNav => 0,
            SidebarForm::Rail => share_limit(self.lower_bound_px(), MAX_RAIL_COLUMN_SHARE_PCT),
            SidebarForm::Standard | SidebarForm::Wide => {
                let lower = self.lower_bound_px();
                let share_cap = share_limit(lower, MAX_LABELLED_COLUMN_SHARE_PCT);
                let floor_room = lower.saturating_sub(LABELLED_CONTENT_FLOOR_PX);
                if share_cap < floor_room {
                    share_cap
                } else {
                    floor_room
                }
            }
        }
    }

    /// Whether this tier is Compact (< 600px width).
    pub const fn is_compact(self) -> bool {
        matches!(self, Self::Compact)
    }

    /// Number of grid columns recommended for Overview cards in this tier.
    pub const fn overview_card_columns(self) -> usize {
        match self {
            Self::Compact => 1,
            Self::Medium => 2,
            Self::Expanded => 2,
            Self::Ultra => 3,
        }
    }

    /// Number of columns recommended for the Overview metrics chip grid.
    pub const fn metrics_grid_columns(self) -> usize {
        match self {
            Self::Compact => 2,
            Self::Medium => 3,
            Self::Expanded => 6,
            Self::Ultra => 6,
        }
    }

    /// Number of columns for the proxy node card grid. `compact_view` forces a
    /// single dense column at every tier (the user's explicit high-density mode).
    pub const fn proxy_grid_columns(self, compact_view: bool) -> usize {
        if compact_view {
            return 1;
        }
        match self {
            Self::Compact => 1,
            Self::Medium => 2,
            Self::Expanded => 3,
            Self::Ultra => 4,
        }
    }

    /// Rows per page for a paginated list, scaled from the desktop budget.
    ///
    /// A short/narrow window has less vertical room, so it builds fewer heavy
    /// row widgets; the desktop and ultra tiers keep the historical budget.
    /// This is the paginated-surface analogue of the virtual list's
    /// `viewport_height` and keeps list density tied to the window.
    pub const fn list_page_rows(self, desktop_rows: usize) -> usize {
        let rows = match self {
            Self::Compact => desktop_rows / 3,
            Self::Medium => desktop_rows / 2,
            Self::Expanded | Self::Ultra => desktop_rows,
        };
        let floor = match self {
            Self::Compact => 20,
            Self::Medium | Self::Expanded | Self::Ultra => 30,
        };
        if rows < floor { floor } else { rows }
    }
}

/// Shell navigation form shared by both surfaces. Mirrors
/// `infiltrator_bevy_widgets::responsive::SidebarMode` and is enforced by the
/// responsive parity guard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidebarForm {
    /// Compact tier: horizontal bottom navigation bar, no side rail.
    BottomNav,
    /// Medium tier: slim icon rail.
    Rail,
    /// Expanded tier: standard labelled sidebar.
    #[default]
    Standard,
    /// Ultra tier: roomy sidebar with rich widgets.
    Wide,
}

impl SidebarForm {
    /// Fixed width in pixels, or `None` when the form uses no vertical rail.
    pub const fn width_px(self) -> Option<u16> {
        match self {
            Self::BottomNav => None,
            Self::Rail => Some(64),
            Self::Standard => Some(240),
            Self::Wide => Some(280),
        }
    }

    /// Whether this form reserves a vertical column beside the page body.
    ///
    /// The mount decision for a shell: a form with a column is drawn as a side
    /// surface, one without is drawn across the bottom. It is the same fact as
    /// [`SidebarForm::width_px`], named once so no surface tests `is_some()` and
    /// calls the result "compact layout".
    pub const fn has_column(self) -> bool {
        match self {
            Self::BottomNav => false,
            Self::Rail | Self::Standard | Self::Wide => true,
        }
    }

    /// Whether this form's rows carry a text label beside the icon.
    ///
    /// A rail is navigation by glyph alone: labels in 64px would elide on every
    /// destination and pay the width of a sidebar for the privilege. This is the
    /// fact the sidebar component reads to pick its row shape, and the fact
    /// [`ViewportTier::has_labelled_sidebar`] is built from.
    pub const fn shows_labels(self) -> bool {
        match self {
            Self::BottomNav | Self::Rail => false,
            Self::Standard | Self::Wide => true,
        }
    }

    /// The vocabulary id of this navigation form.
    pub const fn id(self) -> &'static str {
        match self {
            Self::BottomNav => "bottom_nav",
            Self::Rail => "rail",
            Self::Standard => "standard",
            Self::Wide => "wide",
        }
    }
}

/// Shared snapshot reflecting the observed viewport dimensions and derived layout tier.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponsiveViewportSnapshot {
    pub tier: ViewportTier,
    pub width_px: f32,
    pub height_px: f32,
    pub card_columns: usize,
    pub metrics_columns: usize,
}

impl Default for ResponsiveViewportSnapshot {
    fn default() -> Self {
        Self::from_dimensions(1180.0, 780.0)
    }
}

impl ResponsiveViewportSnapshot {
    pub fn from_dimensions(width_px: f32, height_px: f32) -> Self {
        let tier = ViewportTier::from_width(width_px);
        Self {
            tier,
            width_px,
            height_px,
            card_columns: tier.overview_card_columns(),
            metrics_columns: tier.metrics_grid_columns(),
        }
    }

    pub fn demo_fixture() -> Self {
        Self::default()
    }

    /// Clamp a modal/dialog's preferred pixel width so it never exceeds the
    /// observed viewport (with a small margin). Narrow windows therefore shrink
    /// centred dialogs instead of letting them overflow horizontally.
    pub fn clamped_modal_width(&self, preferred_px: f32) -> f32 {
        let available = self.width_px * 0.92;
        preferred_px.min(available.max(280.0))
    }

    /// Width for a side detail panel/drawer. Narrow tiers go full-bleed (so the
    /// panel stays readable and never overflows); wider tiers keep the
    /// preferred width.
    pub fn detail_panel_width_px(&self, preferred_px: f32) -> f32 {
        if self.tier.is_narrow() {
            (self.width_px * 0.96).max(280.0).min(self.width_px)
        } else {
            preferred_px
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewport_tier_thresholds_and_columns() {
        let mobile = ResponsiveViewportSnapshot::from_dimensions(390.0, 844.0);
        assert_eq!(mobile.tier, ViewportTier::Compact);
        assert_eq!(mobile.card_columns, 1);
        assert_eq!(mobile.metrics_columns, 2);

        let tablet = ResponsiveViewportSnapshot::from_dimensions(768.0, 1024.0);
        assert_eq!(tablet.tier, ViewportTier::Medium);
        assert_eq!(tablet.card_columns, 2);
        assert_eq!(tablet.metrics_columns, 3);

        let desktop = ResponsiveViewportSnapshot::from_dimensions(1180.0, 780.0);
        assert_eq!(desktop.tier, ViewportTier::Expanded);
        assert_eq!(desktop.card_columns, 2);
        assert_eq!(desktop.metrics_columns, 6);

        let wide = ResponsiveViewportSnapshot::from_dimensions(1920.0, 1080.0);
        assert_eq!(wide.tier, ViewportTier::Ultra);
        assert_eq!(wide.card_columns, 3);
        assert_eq!(wide.metrics_columns, 6);
    }

    #[test]
    fn test_authoritative_thresholds_are_stable() {
        // These three numbers are the single source of truth mirrored by
        // `infiltrator-bevy-widgets::theme::breakpoint`; the responsive parity
        // guard fails closed if either side drifts.
        assert_eq!(COMPACT_MAX_PX, 600.0);
        assert_eq!(MEDIUM_MAX_PX, 840.0);
        assert_eq!(EXPANDED_MAX_PX, 1200.0);

        // Boundaries: each tier is half-open `[lower, upper)`.
        assert_eq!(ViewportTier::from_width(599.9), ViewportTier::Compact);
        assert_eq!(ViewportTier::from_width(600.0), ViewportTier::Medium);
        assert_eq!(ViewportTier::from_width(839.9), ViewportTier::Medium);
        assert_eq!(ViewportTier::from_width(840.0), ViewportTier::Expanded);
        assert_eq!(ViewportTier::from_width(1199.9), ViewportTier::Expanded);
        assert_eq!(ViewportTier::from_width(1200.0), ViewportTier::Ultra);
    }

    #[test]
    fn test_tier_layout_operators() {
        assert_eq!(ViewportTier::Compact.sidebar_form(), SidebarForm::BottomNav);
        assert_eq!(ViewportTier::Medium.sidebar_form(), SidebarForm::Rail);
        assert_eq!(ViewportTier::Expanded.sidebar_form(), SidebarForm::Standard);
        assert_eq!(ViewportTier::Ultra.sidebar_form(), SidebarForm::Wide);

        assert_eq!(SidebarForm::BottomNav.width_px(), None);
        assert_eq!(SidebarForm::Rail.width_px(), Some(64));
        assert_eq!(SidebarForm::Standard.width_px(), Some(240));
        assert_eq!(SidebarForm::Wide.width_px(), Some(280));

        assert_eq!(ViewportTier::Compact.content_padding_px(), 16);
        assert_eq!(ViewportTier::Ultra.content_padding_px(), 48);

        assert!(ViewportTier::Compact.is_narrow());
        assert!(ViewportTier::Medium.is_narrow());
        assert!(!ViewportTier::Expanded.is_narrow());
        assert!(!ViewportTier::Ultra.is_narrow());

        assert!(ViewportTier::Compact.is_compact());
        assert!(!ViewportTier::Medium.is_compact());
        assert!(!ViewportTier::Expanded.is_compact());
        assert!(!ViewportTier::Ultra.is_compact());
    }

    #[test]
    fn a_touch_surface_never_gets_a_column_it_cannot_hover() {
        // The bottom bar is the touch answer at every tier; only a desktop
        // window reads the tier's own form. This is the one place that splits,
        // so `SPACEUTIL-006` can hold it in the shell without forking the rule.
        for tier in TIERS {
            assert_eq!(tier.shell_navigation(true), SidebarForm::BottomNav);
            assert_eq!(tier.shell_navigation(false), tier.sidebar_form());
        }
        assert!(!SidebarForm::BottomNav.has_column());
        assert!(SidebarForm::Rail.has_column());
        assert!(SidebarForm::Standard.has_column());
        assert!(SidebarForm::Wide.has_column());
    }

    #[test]
    fn the_rail_is_a_column_that_never_claims_labels() {
        // The two projections must stay one table: a form either has a column,
        // shows labels, both, or neither - never a fourth combination.
        assert!(!SidebarForm::Rail.shows_labels());
        assert!(!SidebarForm::BottomNav.shows_labels());
        assert!(SidebarForm::Standard.shows_labels());
        assert!(SidebarForm::Wide.shows_labels());
        for tier in TIERS {
            assert_eq!(
                tier.has_labelled_sidebar(),
                tier.sidebar_form().shows_labels()
            );
            assert_eq!(tier.nav_column_px() > 0, tier.sidebar_form().has_column());
        }
    }

    #[test]
    fn every_navigation_column_fits_its_tier_ceiling() {
        // The space-utilization standard for the shell's own column, stated as
        // numbers a gate can fail on: an icon rail stays inside a small share of
        // the narrowest window it appears in, and a labelled column leaves a
        // phone's worth of page body behind. FlClash's 48px rail and its 220px
        // expansion above 840 are the same two rules on the same boundaries.
        for tier in TIERS {
            assert!(
                tier.nav_column_px() <= tier.max_nav_column_px(),
                "{} reserves {}px but its ceiling is {}px",
                tier.id(),
                tier.nav_column_px(),
                tier.max_nav_column_px()
            );
        }

        // The ceiling itself, spelled out so a boundary move has to argue here.
        assert_eq!(ViewportTier::Compact.max_nav_column_px(), 0);
        assert_eq!(ViewportTier::Medium.max_nav_column_px(), 72);
        // 840 - 600: the labelled column takes exactly the room this tier has
        // above the phone boundary, and no more.
        assert_eq!(ViewportTier::Expanded.max_nav_column_px(), 240);
        assert_eq!(ViewportTier::Ultra.max_nav_column_px(), 360);
        assert_eq!(ViewportTier::Medium.nav_column_px(), 64);
        assert_eq!(ViewportTier::Expanded.nav_column_px(), 240);
        assert_eq!(ViewportTier::Ultra.nav_column_px(), 280);
    }

    #[test]
    fn the_two_dashboard_column_semantics_are_not_the_same_number() {
        // SPACEUTIL-008: one untyped `usize` used to answer both "how many
        // cards wrap in a row" (1/2/2/3, by tier) and "how many shares the
        // dashboard span grid owns" (2/4, by page width). Reading one as the
        // other is why a 1180 window looked like two columns in one surface
        // and four in another. The typed enum keeps them separate.
        assert_eq!(
            DashboardColumns::WrappedCards.count(ViewportTier::Expanded, 940),
            2
        );
        assert_eq!(
            DashboardColumns::GridShares.count(ViewportTier::Expanded, 940),
            4
        );
        assert_eq!(DashboardColumns::WrappedCards.id(), "wrapped_cards");
        assert_eq!(DashboardColumns::GridShares.id(), "grid_shares");
        for &tier in TIERS.iter() {
            assert_eq!(
                DashboardColumns::WrappedCards.count(tier, tier.lower_bound_px()),
                tier.overview_card_columns() as u16,
                "{} wraps its shared number of cards",
                tier.id()
            );
        }
    }

    #[test]
    fn the_dashboard_grid_follows_the_reference_page_width_rule() {
        // FlClash: `dashboardColumnsOf(width) => width < 860 ? 2 : 4`, where
        // width is the page body. The shell's navigation column is paid for
        // first, so the same window with a rail gets two shares and with a
        // labelled column gets four.
        assert_eq!(DASHBOARD_FOUR_COLUMN_MIN_PX, 860);
        assert_eq!(
            DashboardColumns::GridShares.count(ViewportTier::Medium, 656),
            2
        );
        assert_eq!(
            DashboardColumns::GridShares.count(ViewportTier::Expanded, 940),
            4
        );
        assert_eq!(
            DashboardColumns::GridShares
                .count(ViewportTier::Expanded, DASHBOARD_FOUR_COLUMN_MIN_PX - 1),
            2
        );
        assert_eq!(
            DashboardColumns::GridShares
                .count(ViewportTier::Expanded, DASHBOARD_FOUR_COLUMN_MIN_PX),
            4
        );
        // A rail form stays at two shares even with a wide page body: the
        // three-share Hero would otherwise leave no room for the half-share
        // cards at 720px. That is a form decision, not a second width rule.
        assert_eq!(
            DashboardColumns::GridShares.count(ViewportTier::Medium, 940),
            2
        );
    }

    #[test]
    fn dashboard_density_follows_the_reference_page_room() {
        // FlClash resolves density from the room inside its scaffold, after
        // the nav column and title bar are paid for: below 640 is mobile,
        // four-share pages below 1040 are compact, and any desktop page below
        // 824 high is compact even with a standard-width body.
        assert_eq!(DASHBOARD_DESKTOP_MIN_PX, 640);
        assert_eq!(DASHBOARD_STANDARD_FOUR_COLUMN_MIN_PX, 1040);
        assert_eq!(DASHBOARD_STANDARD_HEIGHT_MIN_PX, 824);
        assert_eq!(DASHBOARD_MOBILE_MAX_PX, 560);
        assert_eq!(DashboardDensity::of(560.0, 900.0), DashboardDensity::Mobile);
        assert_eq!(
            DashboardDensity::of(DASHBOARD_DESKTOP_MIN_PX as f32 - 1.0, 900.0),
            DashboardDensity::Mobile
        );
        assert_eq!(
            DashboardDensity::of(DASHBOARD_FOUR_COLUMN_MIN_PX as f32, 900.0),
            DashboardDensity::Compact
        );
        assert_eq!(
            DashboardDensity::of(DASHBOARD_STANDARD_FOUR_COLUMN_MIN_PX as f32, 900.0),
            DashboardDensity::Standard
        );
        assert_eq!(
            DashboardDensity::of(
                DASHBOARD_STANDARD_FOUR_COLUMN_MIN_PX as f32,
                DASHBOARD_STANDARD_HEIGHT_MIN_PX as f32 - 1.0,
            ),
            DashboardDensity::Compact
        );
        assert_eq!(
            DashboardDensity::of(
                DASHBOARD_STANDARD_FOUR_COLUMN_MIN_PX as f32,
                DASHBOARD_STANDARD_HEIGHT_MIN_PX as f32,
            ),
            DashboardDensity::Standard
        );
        assert!(!DashboardDensity::Mobile.is_desktop());
        assert!(DashboardDensity::Compact.is_desktop());
        assert!(DashboardDensity::Standard.is_desktop());
        assert_eq!(DashboardDensity::Compact.id(), "compact");
        assert_eq!(DashboardDensity::Standard.id(), "standard");
        assert_eq!(DashboardDensity::Mobile.id(), "mobile");
    }

    #[test]
    fn profile_columns_follow_the_reference_width_rule() {
        // FlClash's profiles grid: `getProfilesColumns(width, spacing) =
        // floor((width + spacing) / (300 + spacing))`, clamped to 1..=3. The
        // same page body at our two capture widths must match the reference
        // instead of stacking every card on its own row.
        assert_eq!(PROFILE_MIN_ITEM_WIDTH_PX, 300);
        assert_eq!(PROFILE_MAX_COLUMNS, 3);
        assert_eq!(profile_columns(ProfileGridForm::Stacked, 940, 12), 1);
        assert_eq!(profile_columns(ProfileGridForm::Desktop, 0, 12), 1);
        assert_eq!(profile_columns(ProfileGridForm::Desktop, 299, 0), 1);
        assert_eq!(profile_columns(ProfileGridForm::Desktop, 300, 0), 1);
        assert_eq!(profile_columns(ProfileGridForm::Desktop, 300 + 12, 12), 1);
        assert_eq!(
            profile_columns(ProfileGridForm::Desktop, 300 * 2 + 12, 12),
            2
        );
        assert_eq!(
            profile_columns(ProfileGridForm::Desktop, 300 * 2 + 11, 12),
            1
        );
        assert_eq!(
            profile_columns(ProfileGridForm::Desktop, 300 * 3 + 24, 12),
            3
        );
        assert_eq!(
            profile_columns(ProfileGridForm::Desktop, 300 * 4 + 48, 12),
            3
        );
        assert_eq!(profile_columns(ProfileGridForm::Desktop, 1, 1), 1);
        assert_eq!(profile_columns(ProfileGridForm::Desktop, u16::MAX, 0), 3);
    }

    #[test]
    fn proxy_node_columns_follow_the_reference_width_rule() {
        assert_eq!(PROXY_MIN_ITEM_WIDTH_PX, 250);
        assert_eq!(PROXY_GRID_INSET_PX, 32);
        assert_eq!(PROXY_MIN_COLUMNS, 2);
        assert_eq!(proxy_node_columns(ProxyGridForm::Stacked, 940), 1);
        assert_eq!(proxy_node_columns(ProxyGridForm::Desktop, 0), 2);
        assert_eq!(proxy_node_columns(ProxyGridForm::Desktop, 32), 2);
        assert_eq!(proxy_node_columns(ProxyGridForm::Desktop, 282), 2);
        assert_eq!(proxy_node_columns(ProxyGridForm::Desktop, 283), 2);
        assert_eq!(proxy_node_columns(ProxyGridForm::Desktop, 532), 2);
        assert_eq!(proxy_node_columns(ProxyGridForm::Desktop, 533), 3);
        assert_eq!(proxy_node_columns(ProxyGridForm::Desktop, 782), 3);
        assert_eq!(proxy_node_columns(ProxyGridForm::Desktop, 783), 4);
        assert_eq!(proxy_node_columns(ProxyGridForm::Desktop, 940), 4);
        assert_eq!(proxy_node_columns(ProxyGridForm::Desktop, u16::MAX), 263);
    }

    #[test]
    fn test_proxy_grid_columns_follow_tier_and_compact_view() {
        assert_eq!(ViewportTier::Compact.proxy_grid_columns(false), 1);
        assert_eq!(ViewportTier::Medium.proxy_grid_columns(false), 2);
        assert_eq!(ViewportTier::Expanded.proxy_grid_columns(false), 3);
        assert_eq!(ViewportTier::Ultra.proxy_grid_columns(false), 4);

        // The user's compact view forces one dense column everywhere.
        for tier in [
            ViewportTier::Compact,
            ViewportTier::Medium,
            ViewportTier::Expanded,
            ViewportTier::Ultra,
        ] {
            assert_eq!(tier.proxy_grid_columns(true), 1);
        }
    }

    #[test]
    fn page_body_width_subtracts_the_chrome_the_tier_reserves() {
        // 1180 desktop: 240 labelled column + 2 x 40 padding.
        assert_eq!(ViewportTier::Expanded.page_body_width_px(1180), 860);
        // 720 rail: 64 icon column + 2 x 24 padding.
        assert_eq!(ViewportTier::Medium.page_body_width_px(720), 608);
        // 400 phone: no column, bottom bar stacks below the page.
        assert_eq!(ViewportTier::Compact.page_body_width_px(400), 368);
        // A window narrower than its own chrome saturates at zero.
        assert_eq!(ViewportTier::Ultra.page_body_width_px(10), 0);
    }

    #[test]
    fn test_list_page_rows_scale_with_tier() {
        // Desktop keeps its historical budget; narrow tiers build fewer rows.
        assert_eq!(ViewportTier::Expanded.list_page_rows(200), 200);
        assert_eq!(ViewportTier::Ultra.list_page_rows(200), 200);
        assert_eq!(ViewportTier::Medium.list_page_rows(200), 100);
        assert_eq!(ViewportTier::Compact.list_page_rows(200), 66);

        // Floors keep the list usable on tiny desktop budgets.
        assert_eq!(ViewportTier::Compact.list_page_rows(30), 20);
        assert_eq!(ViewportTier::Medium.list_page_rows(30), 30);
    }

    #[test]
    fn test_detail_panel_width_is_full_bleed_when_narrow() {
        // Wide desktop keeps the preferred 480px side panel.
        let desktop = ResponsiveViewportSnapshot::from_dimensions(1600.0, 900.0);
        assert_eq!(desktop.detail_panel_width_px(480.0), 480.0);

        // Compact goes near full-bleed and never exceeds the viewport.
        let compact = ResponsiveViewportSnapshot::from_dimensions(420.0, 800.0);
        let width = compact.detail_panel_width_px(480.0);
        assert!(width <= 420.0);
        assert!(width > 300.0);
    }
}

//! Shared contracts for 4-tier responsive viewport breakpoints across Iced and Bevy.
//!
//! This module is the **single authoritative source for breakpoint thresholds**
//! (`600 / 840 / 1200`). `infiltrator-bevy-widgets` cannot depend on this crate
//! (it is business-agnostic by charter), so it mirrors the same numbers and
//! `scripts/quality/responsive-parity-guard.py` fails closed if the two drift.
//! See `docs/RESPONSIVE_PARITY_LEDGER.md`.

use serde::{Deserialize, Serialize};

/// Shortest/longest widths each tier spans, used by tests and guards.
pub const COMPACT_MAX_PX: f32 = 600.0;
/// Medium tier lower/upper boundary.
pub const MEDIUM_MAX_PX: f32 = 840.0;
/// Expanded tier lower/upper boundary; `>=` this value is [`ViewportTier::Ultra`].
pub const EXPANDED_MAX_PX: f32 = 1200.0;

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

    /// Outer content-region padding in pixels for this tier.
    pub const fn content_padding_px(self) -> u16 {
        match self {
            Self::Compact => 16,
            Self::Medium => 24,
            Self::Expanded => 40,
            Self::Ultra => 48,
        }
    }

    /// Whether this tier is narrow enough that the shell must shed its full sidebar.
    pub const fn is_narrow(self) -> bool {
        matches!(self, Self::Compact | Self::Medium)
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

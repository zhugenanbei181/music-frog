//! Shared contracts for 4-tier responsive viewport breakpoints across Iced and Bevy.

use serde::{Deserialize, Serialize};

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
        if width_px < 600.0 {
            Self::Compact
        } else if width_px < 840.0 {
            Self::Medium
        } else if width_px < 1200.0 {
            Self::Expanded
        } else {
            Self::Ultra
        }
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
}

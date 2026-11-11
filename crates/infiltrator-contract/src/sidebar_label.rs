//! The desktop sidebar label's fit is a shared typed fact (FLPARITY-048).
//!
//! Upstream FlClash shows a destination row's tooltip only when the expanded
//! label was cut. A surface must not measure text or infer the cut from the
//! label's length: it consumes the readback this module computes from one
//! geometry and the shared [`TextMeasure`] rule.

use crate::design_tokens::type_scale::LABEL_LARGE;
use crate::text_truncation::{TextMeasure, plan_truncation};

/// The labelled sidebar's measured geometry, in logical pixels at 100% scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SidebarLabelGeometry {
    pub nav_column_px: f32,
    pub leading_px: f32,
    pub trailing_px: f32,
    pub icon_px: f32,
    pub label_gap_px: f32,
    pub scale_percent: u16,
}

impl SidebarLabelGeometry {
    /// The desktop labelled column measured against the FlClash reference:
    /// 240px column, 8px leading, 16px trailing inset, 24px icon, 12px gap.
    pub const DESKTOP: Self = Self {
        nav_column_px: 240.0,
        leading_px: 8.0,
        trailing_px: 16.0,
        icon_px: 24.0,
        label_gap_px: 12.0,
        scale_percent: 100,
    };

    /// The label's own font size at this surface scale.
    pub const fn label_size_px(self) -> f32 {
        LABEL_LARGE * self.scale_percent as f32 / 100.0
    }

    /// The room left for the label inside one destination row.
    pub const fn available_px(self) -> f32 {
        (self.nav_column_px - self.leading_px - self.trailing_px - self.icon_px - self.label_gap_px)
            * self.scale_percent as f32
            / 100.0
    }

    /// Whether the given label needs its full text in a hover tooltip.
    pub fn is_truncated<M>(self, label: &str, measure: &M) -> bool
    where
        M: TextMeasure + ?Sized,
    {
        let available_px = self.available_px();
        if !available_px.is_finite() || available_px <= 0.0 {
            return false;
        }
        plan_truncation(label, available_px, measure).is_truncated()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text_truncation::UniformCharWidth;

    fn measure(em_px: f32) -> UniformCharWidth {
        UniformCharWidth::new(em_px)
    }

    #[test]
    fn the_desktop_label_box_leaves_180_pixels() {
        assert_eq!(SidebarLabelGeometry::DESKTOP.available_px(), 180.0);
        assert_eq!(SidebarLabelGeometry::DESKTOP.label_size_px(), 14.0);
    }

    #[test]
    fn a_short_label_keeps_no_tooltip() {
        let geometry = SidebarLabelGeometry::DESKTOP;
        assert!(!geometry.is_truncated("代理", &measure(14.0)));
    }

    #[test]
    fn an_overlong_cyrillic_label_keeps_its_tooltip() {
        let geometry = SidebarLabelGeometry::DESKTOP;
        let long = "Подключения и активные соединения";
        assert!(geometry.is_truncated(long, &measure(7.0)));
    }

    #[test]
    fn scaling_grows_the_box_and_the_measure_together() {
        let geometry = SidebarLabelGeometry {
            scale_percent: 125,
            ..SidebarLabelGeometry::DESKTOP
        };
        assert_eq!(geometry.available_px(), 225.0);
        assert_eq!(geometry.label_size_px(), 17.5);
        let label = "连接与活动会话";
        let base = SidebarLabelGeometry::DESKTOP.is_truncated(label, &measure(14.0));
        let scaled = geometry.is_truncated(label, &measure(17.5));
        assert_eq!(base, scaled);
    }

    #[test]
    fn an_impossible_box_never_invents_a_cut() {
        let geometry = SidebarLabelGeometry {
            nav_column_px: 0.0,
            ..SidebarLabelGeometry::DESKTOP
        };
        assert!(geometry.available_px() < 0.0);
        assert!(!geometry.is_truncated("anything", &measure(14.0)));
        let nan = SidebarLabelGeometry {
            nav_column_px: f32::NAN,
            ..SidebarLabelGeometry::DESKTOP
        };
        assert!(!nan.is_truncated("anything", &measure(14.0)));
    }
}

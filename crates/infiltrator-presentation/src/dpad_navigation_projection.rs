//! Presentation read-model projection for D-pad navigation and focus halos (FLPARITY-027).

use infiltrator_contract::dpad_navigation::FocusHaloSpec;

/// Projection of visual focus halo properties for UI controls.
#[derive(Debug, Clone, PartialEq)]
pub struct DpadFocusStyleProjection {
    pub is_focused: bool,
    pub is_tv_mode: bool,
    pub scale: f32,
    pub ring_width_dp: f32,
    pub glow_radius_dp: f32,
    pub elevation_gain_dp: f32,
    pub should_center_scroll: bool,
}

impl DpadFocusStyleProjection {
    pub fn of(is_focused: bool, is_tv_mode: bool) -> Self {
        let spec = FocusHaloSpec::default();
        if is_focused && is_tv_mode {
            Self {
                is_focused: true,
                is_tv_mode: true,
                scale: spec.scale_factor,
                ring_width_dp: spec.ring_width_dp,
                glow_radius_dp: spec.glow_radius_dp,
                elevation_gain_dp: spec.elevation_gain_dp,
                should_center_scroll: spec.auto_scroll_to_center,
            }
        } else if is_focused {
            Self {
                is_focused: true,
                is_tv_mode: false,
                scale: 1.0,
                ring_width_dp: 1.5,
                glow_radius_dp: 0.0,
                elevation_gain_dp: 1.0,
                should_center_scroll: false,
            }
        } else {
            Self {
                is_focused: false,
                is_tv_mode,
                scale: 1.0,
                ring_width_dp: 0.0,
                glow_radius_dp: 0.0,
                elevation_gain_dp: 0.0,
                should_center_scroll: false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tv_focused_element_gets_scaled_and_glow_halo() {
        let style = DpadFocusStyleProjection::of(true, true);
        assert!(style.is_focused);
        assert!(style.is_tv_mode);
        assert_eq!(style.scale, 1.05);
        assert_eq!(style.ring_width_dp, 2.0);
        assert_eq!(style.glow_radius_dp, 6.0);
        assert!(style.should_center_scroll);
    }

    #[test]
    fn desktop_focused_element_has_crisp_ring_without_halo() {
        let style = DpadFocusStyleProjection::of(true, false);
        assert!(style.is_focused);
        assert!(!style.is_tv_mode);
        assert_eq!(style.scale, 1.0);
        assert_eq!(style.glow_radius_dp, 0.0);
        assert!(!style.should_center_scroll);
    }

    #[test]
    fn unfocused_element_has_zero_ring_and_glow() {
        let style = DpadFocusStyleProjection::of(false, true);
        assert!(!style.is_focused);
        assert_eq!(style.scale, 1.0);
        assert_eq!(style.ring_width_dp, 0.0);
        assert_eq!(style.glow_radius_dp, 0.0);
    }
}

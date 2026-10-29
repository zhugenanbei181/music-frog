//! Directional pad (D-pad) navigation and focus halo specifications for Android TV & Leanback (FLPARITY-027).

use serde::{Deserialize, Serialize};

/// Remote controller D-pad directional action keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DpadKeyAction {
    Up,
    Down,
    Left,
    Right,
    Select,
    Back,
    Menu,
}

impl DpadKeyAction {
    pub const fn is_directional(self) -> bool {
        matches!(self, Self::Up | Self::Down | Self::Left | Self::Right)
    }
}

/// Focus Halo and glow ring specification for Leanback TV navigation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FocusHaloSpec {
    /// Smooth scale multiplier when element is focused (default 1.05x).
    pub scale_factor: f32,
    /// Outer glow blur radius in density-independent pixels (dp).
    pub glow_radius_dp: f32,
    /// Inner focus stroke ring width in dp.
    pub ring_width_dp: f32,
    /// Elevation gain applied when element is focused in dp.
    pub elevation_gain_dp: f32,
    /// Whether list items should scroll to center automatically upon receiving focus.
    pub auto_scroll_to_center: bool,
}

impl Default for FocusHaloSpec {
    fn default() -> Self {
        Self {
            scale_factor: 1.05,
            glow_radius_dp: 6.0,
            ring_width_dp: 2.0,
            elevation_gain_dp: 8.0,
            auto_scroll_to_center: true,
        }
    }
}

/// Linear 1D/2D focus topology tracker to prevent focus traps on TV surfaces.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DpadFocusTopology {
    pub nodes: Vec<String>,
    pub current_index: Option<usize>,
}

impl DpadFocusTopology {
    pub fn new(nodes: Vec<String>) -> Self {
        let current_index = if nodes.is_empty() { None } else { Some(0) };
        Self {
            nodes,
            current_index,
        }
    }

    pub fn current_focused(&self) -> Option<&str> {
        self.current_index
            .and_then(|idx| self.nodes.get(idx).map(String::as_str))
    }

    /// Navigate to previous/next focus node safely without overflowing or trapping.
    pub fn step(&mut self, forward: bool) -> Option<&str> {
        if self.nodes.is_empty() {
            return None;
        }
        let curr = self.current_index.unwrap_or(0);
        let next = if forward {
            (curr + 1).min(self.nodes.len() - 1)
        } else {
            curr.saturating_sub(1)
        };
        self.current_index = Some(next);
        self.current_focused()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpad_actions_classify_directionals() {
        assert!(DpadKeyAction::Up.is_directional());
        assert!(DpadKeyAction::Down.is_directional());
        assert!(DpadKeyAction::Left.is_directional());
        assert!(DpadKeyAction::Right.is_directional());
        assert!(!DpadKeyAction::Select.is_directional());
        assert!(!DpadKeyAction::Back.is_directional());
        assert!(!DpadKeyAction::Menu.is_directional());
    }

    #[test]
    fn focus_halo_spec_provides_sound_leanback_defaults() {
        let spec = FocusHaloSpec::default();
        assert_eq!(spec.scale_factor, 1.05);
        assert_eq!(spec.glow_radius_dp, 6.0);
        assert_eq!(spec.ring_width_dp, 2.0);
        assert!(spec.auto_scroll_to_center);
    }

    #[test]
    fn focus_topology_steps_without_trapping() {
        let mut topo = DpadFocusTopology::new(vec![
            "card_connect".to_owned(),
            "card_nodes".to_owned(),
            "card_rules".to_owned(),
        ]);

        assert_eq!(topo.current_focused(), Some("card_connect"));
        assert_eq!(topo.step(true), Some("card_nodes"));
        assert_eq!(topo.step(true), Some("card_rules"));
        // Clamps at end, doesn't overflow
        assert_eq!(topo.step(true), Some("card_rules"));
        // Steps back
        assert_eq!(topo.step(false), Some("card_nodes"));
        assert_eq!(topo.step(false), Some("card_connect"));
        // Clamps at start
        assert_eq!(topo.step(false), Some("card_connect"));
    }
}

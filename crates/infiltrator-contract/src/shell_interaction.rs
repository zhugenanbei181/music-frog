//! Shared cross-platform interaction baseline: docked search, elastic sheet,
//! long-press reorder, and button hover/lift feedback (FLPARITY-011).

use serde::{Deserialize, Serialize};

/// State of the docked search bar in the application shell.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockedSearchState {
    #[default]
    Closed,
    Focused,
    ActiveQuery {
        query: String,
        match_count: usize,
    },
}

/// Target expansion tier for modal or bottom bottom sheets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SheetHeightTier {
    Collapsed,
    HalfExpanded,
    FullyExpanded,
}

/// Responsive sizing specification for a stretchy sheet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SheetSizingSpec {
    pub current_height_px: f32,
    pub tier: SheetHeightTier,
    pub available_height_px: f32,
    pub is_dragging: bool,
}

impl SheetSizingSpec {
    pub fn of(tier: SheetHeightTier, available_height_px: f32) -> Self {
        let current_height_px = match tier {
            SheetHeightTier::Collapsed => 0.0,
            SheetHeightTier::HalfExpanded => (available_height_px * 0.5).clamp(240.0, 480.0),
            SheetHeightTier::FullyExpanded => (available_height_px * 0.9).max(320.0),
        };
        Self {
            current_height_px,
            tier,
            available_height_px,
            is_dragging: false,
        }
    }

    pub fn can_scroll_content(&self) -> bool {
        self.tier == SheetHeightTier::FullyExpanded
    }
}

/// Explicit state machine for long-press drag-and-drop reordering.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LongPressReorderState {
    #[default]
    Idle,
    PressPending {
        index: usize,
    },
    Dragging {
        from_index: usize,
        current_target_index: usize,
    },
    Dropped {
        from_index: usize,
        to_index: usize,
    },
    Cancelled,
}

/// iOS-style toggle switch visual and interaction state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwitchInteractionState {
    Off,
    On,
    Disabled,
}

impl SwitchInteractionState {
    pub fn is_active(self) -> bool {
        matches!(self, Self::On)
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::Off => Self::On,
            Self::On => Self::Off,
            Self::Disabled => Self::Disabled,
        }
    }
}

/// Button hover and press elevation feedback without layout relayout.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ButtonLiftFeedback {
    pub lift_y_px: f32,
    pub scale: f32,
    pub elevation_step: u8,
}

impl ButtonLiftFeedback {
    pub const REST: Self = Self {
        lift_y_px: 0.0,
        scale: 1.0,
        elevation_step: 0,
    };

    pub const HOVERED: Self = Self {
        lift_y_px: -2.0,
        scale: 1.0,
        elevation_step: 2,
    };

    pub const PRESSED: Self = Self {
        lift_y_px: 0.0,
        scale: 0.98,
        elevation_step: 1,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sheet_sizing_clamps_within_available_height() {
        let spec_half = SheetSizingSpec::of(SheetHeightTier::HalfExpanded, 600.0);
        assert_eq!(spec_half.current_height_px, 300.0);
        assert!(!spec_half.can_scroll_content());
        let spec_full = SheetSizingSpec::of(SheetHeightTier::FullyExpanded, 800.0);
        assert_eq!(spec_full.current_height_px, 720.0);
        assert!(spec_full.can_scroll_content());
    }

    #[test]
    fn long_press_reorder_transitions() {
        let mut state = LongPressReorderState::Idle;
        assert_eq!(state, LongPressReorderState::Idle);
        state = LongPressReorderState::PressPending { index: 2 };
        assert_eq!(state, LongPressReorderState::PressPending { index: 2 });
        state = LongPressReorderState::Dragging {
            from_index: 2,
            current_target_index: 5,
        };
        assert_eq!(
            state,
            LongPressReorderState::Dragging {
                from_index: 2,
                current_target_index: 5
            }
        );
        state = LongPressReorderState::Dropped {
            from_index: 2,
            to_index: 5,
        };
        assert_eq!(
            state,
            LongPressReorderState::Dropped {
                from_index: 2,
                to_index: 5
            }
        );
    }

    #[test]
    fn switch_toggle_handles_disabled_honestly() {
        assert_eq!(
            SwitchInteractionState::Off.toggle(),
            SwitchInteractionState::On
        );
        assert_eq!(
            SwitchInteractionState::On.toggle(),
            SwitchInteractionState::Off
        );
        assert_eq!(
            SwitchInteractionState::Disabled.toggle(),
            SwitchInteractionState::Disabled
        );
    }

    #[test]
    fn button_lift_constants_match_m3_elevation() {
        assert_eq!(ButtonLiftFeedback::REST.lift_y_px, 0.0);
        assert_eq!(ButtonLiftFeedback::HOVERED.lift_y_px, -2.0);
        assert_eq!(ButtonLiftFeedback::PRESSED.scale, 0.98);
    }
}

//! Bidirectional (BiDi) text layout direction, OpenType font variations, and RTL mirroring.

use bevy::ecs::resource::Resource;
use bevy::ui::prelude::{FlexDirection, UiRect};

/// Layout writing and flow direction.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LayoutDirection {
    #[default]
    Ltr,
    Rtl,
}

impl LayoutDirection {
    pub fn is_rtl(&self) -> bool {
        matches!(self, LayoutDirection::Rtl)
    }

    /// Mirror horizontal padding / margins if direction is RTL.
    pub fn mirror_rect(&self, rect: UiRect) -> UiRect {
        if self.is_rtl() {
            UiRect {
                left: rect.right,
                right: rect.left,
                top: rect.top,
                bottom: rect.bottom,
            }
        } else {
            rect
        }
    }

    /// Mirror flex row direction if direction is RTL.
    pub fn mirror_flex_direction(&self, dir: FlexDirection) -> FlexDirection {
        if self.is_rtl() {
            match dir {
                FlexDirection::Row => FlexDirection::RowReverse,
                FlexDirection::RowReverse => FlexDirection::Row,
                other => other,
            }
        } else {
            dir
        }
    }
}

/// OpenType variable font axis modulation parameters (Weight, Slant, Width, OpticalSize).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontVariationAxes {
    pub weight: f32, // [100.0..900.0]
    pub slant: f32,  // [-10.0..0.0]
    pub width: f32,  // [75.0..125.0]
}

impl Default for FontVariationAxes {
    fn default() -> Self {
        Self {
            weight: 400.0,
            slant: 0.0,
            width: 100.0,
        }
    }
}

impl FontVariationAxes {
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            weight: self.weight + (other.weight - self.weight) * t,
            slant: self.slant + (other.slant - self.slant) * t,
            width: self.width + (other.width - self.width) * t,
        }
    }
}

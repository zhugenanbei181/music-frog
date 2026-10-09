//! Bidirectional (BiDi) text layout direction, OpenType font variations, and RTL mirroring.

use bevy::ecs::resource::Resource;
use bevy::text::Justify;
use bevy::ui::prelude::{AlignItems, FlexDirection, JustifyContent, UiRect};

/// Inclusive `[min, max]` domain of the OpenType `wght` axis.
pub const WEIGHT_AXIS: (f32, f32) = (100.0, 900.0);
/// Inclusive `[min, max]` domain of the OpenType `slnt` axis.
pub const SLANT_AXIS: (f32, f32) = (-10.0, 0.0);
/// Inclusive `[min, max]` domain of the OpenType `wdth` axis.
pub const WIDTH_AXIS: (f32, f32) = (75.0, 125.0);

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

    /// Mirror inline-axis item alignment for RTL.
    pub fn mirror_align_items(&self, align: AlignItems) -> AlignItems {
        if !self.is_rtl() {
            return align;
        }
        match align {
            AlignItems::Start => AlignItems::End,
            AlignItems::End => AlignItems::Start,
            AlignItems::StartSafe => AlignItems::EndSafe,
            AlignItems::EndSafe => AlignItems::StartSafe,
            AlignItems::FlexStart => AlignItems::FlexEnd,
            AlignItems::FlexEnd => AlignItems::FlexStart,
            AlignItems::FlexStartSafe => AlignItems::FlexEndSafe,
            AlignItems::FlexEndSafe => AlignItems::FlexStartSafe,
            other => other,
        }
    }

    /// Mirror main-axis content distribution for RTL.
    pub fn mirror_justify_content(&self, justify: JustifyContent) -> JustifyContent {
        if !self.is_rtl() {
            return justify;
        }
        match justify {
            JustifyContent::Start => JustifyContent::End,
            JustifyContent::End => JustifyContent::Start,
            JustifyContent::StartSafe => JustifyContent::EndSafe,
            JustifyContent::EndSafe => JustifyContent::StartSafe,
            JustifyContent::FlexStart => JustifyContent::FlexEnd,
            JustifyContent::FlexEnd => JustifyContent::FlexStart,
            JustifyContent::FlexStartSafe => JustifyContent::FlexEndSafe,
            JustifyContent::FlexEndSafe => JustifyContent::FlexStartSafe,
            other => other,
        }
    }

    /// Mirror absolute text alignment for RTL. Direction-relative variants
    /// (`Start` / `End`) already resolve at shaping time and stay untouched.
    pub fn mirror_justify(&self, justify: Justify) -> Justify {
        if !self.is_rtl() {
            return justify;
        }
        match justify {
            Justify::Left => Justify::Right,
            Justify::Right => Justify::Left,
            other => other,
        }
    }

    /// Mirror an item's leading `x` offset inside a container: the item keeps
    /// the same gap from the opposite edge. The transform is an involution for
    /// in-bounds items, so applying it twice restores the LTR offset.
    pub fn mirror_x(&self, x: f32, container_width: f32, item_width: f32) -> f32 {
        if self.is_rtl() {
            container_width - x - item_width
        } else {
            x
        }
    }

    /// Resolve the effective flex / text alignment pair for this direction.
    pub fn mirror_layout(&self, flex: FlexDirection, justify: Justify) -> (FlexDirection, Justify) {
        (
            self.mirror_flex_direction(flex),
            self.mirror_justify(justify),
        )
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

/// Interpolate one axis and clamp the result into its OpenType domain.
fn lerp_axis(from: f32, to: f32, t: f32, axis: (f32, f32)) -> f32 {
    let blended = from + (to - from) * t.clamp(0.0, 1.0);
    blended.clamp(axis.0, axis.1)
}

impl FontVariationAxes {
    /// Clamp every axis into its OpenType domain.
    pub fn clamped(&self) -> Self {
        Self {
            weight: self.weight.clamp(WEIGHT_AXIS.0, WEIGHT_AXIS.1),
            slant: self.slant.clamp(SLANT_AXIS.0, SLANT_AXIS.1),
            width: self.width.clamp(WIDTH_AXIS.0, WIDTH_AXIS.1),
        }
    }

    /// Interpolate each axis toward `other`, clamping `t` and the result domain.
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {
            weight: lerp_axis(self.weight, other.weight, t, WEIGHT_AXIS),
            slant: lerp_axis(self.slant, other.slant, t, SLANT_AXIS),
            width: lerp_axis(self.width, other.width, t, WIDTH_AXIS),
        }
    }

    /// Interpolate a single `wght` value, clamped to the weight domain.
    pub fn interpolate_weight(from: f32, to: f32, t: f32) -> f32 {
        lerp_axis(from, to, t, WEIGHT_AXIS)
    }

    /// The variable-font weight at a semantic ratio in `[0, 1]`.
    pub fn weight_at_ratio(ratio: f32) -> f32 {
        lerp_axis(WEIGHT_AXIS.0, WEIGHT_AXIS.1, ratio, WEIGHT_AXIS)
    }
}

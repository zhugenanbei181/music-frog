//! The safe area of a window and the conversions into it (EFFECTS-006 #23).
//!
//! A phone with a notch, a tablet with a gesture bar and a desktop window all
//! answer "which pixels may I paint over?" differently: one reports device
//! pixels, one reports scaled pixels, one reports nothing at all. The product
//! converts every answer into one logical-pixel shape — [`SafeAreaInsets`] —
//! so a surface never has to know which platform it runs on.

/// The safe area measured in device pixels, as a platform reports it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DeviceSafeArea {
    /// The inset of the top edge, in device pixels.
    pub top: u32,
    /// The inset of the right edge, in device pixels.
    pub right: u32,
    /// The inset of the bottom edge, in device pixels.
    pub bottom: u32,
    /// The inset of the left edge, in device pixels.
    pub left: u32,
}

/// The margins a surface must not paint over, in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SafeAreaInsets {
    /// The inset of the top edge.
    pub top: f32,
    /// The inset of the right edge.
    pub right: f32,
    /// The inset of the bottom edge.
    pub bottom: f32,
    /// The inset of the left edge.
    pub left: f32,
}

impl SafeAreaInsets {
    /// The empty safe area: a desktop window without any reserved margin.
    pub const NONE: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    /// The same inset on every edge.
    pub const fn uniform(inset: f32) -> Self {
        Self {
            top: inset,
            right: inset,
            bottom: inset,
            left: inset,
        }
    }

    /// One inset per edge.
    pub const fn from_edges(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// The largest inset on any edge.
    pub const fn max(self) -> f32 {
        let mut largest = self.top;
        if self.right > largest {
            largest = self.right;
        }
        if self.bottom > largest {
            largest = self.bottom;
        }
        if self.left > largest {
            largest = self.left;
        }
        largest
    }

    /// Whether every edge is empty.
    pub const fn is_empty(self) -> bool {
        self.top == 0.0 && self.right == 0.0 && self.bottom == 0.0 && self.left == 0.0
    }

    /// These insets with `padding` added on every edge.
    pub fn expanded(self, padding: f32) -> Self {
        Self {
            top: self.top + padding,
            right: self.right + padding,
            bottom: self.bottom + padding,
            left: self.left + padding,
        }
    }

    /// The insets that place a content box in the middle of a window.
    ///
    /// The leftover space is split evenly between the two opposite edges, and
    /// a content box larger than the window collapses to the empty inset
    /// instead of a negative margin.
    pub fn around(content: (f32, f32), window: (f32, f32)) -> Self {
        let horizontal = ((window.0 - content.0) / 2.0).max(0.0);
        let vertical = ((window.1 - content.1) / 2.0).max(0.0);
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    /// Convert a platform safe area from device pixels into logical pixels.
    ///
    /// A non-finite or non-positive scale factor falls back to 1.0: a broken
    /// scale must not turn the whole window into a negative inset.
    pub fn from_device_pixels(area: DeviceSafeArea, scale_factor: f32) -> Self {
        let factor = if scale_factor.is_finite() && scale_factor > 0.0 {
            scale_factor
        } else {
            1.0
        };
        Self {
            top: area.top as f32 / factor,
            right: area.right as f32 / factor,
            bottom: area.bottom as f32 / factor,
            left: area.left as f32 / factor,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_empty_safe_area_is_empty() {
        assert!(SafeAreaInsets::NONE.is_empty());
        assert_eq!(SafeAreaInsets::NONE.max(), 0.0);
        assert!(!SafeAreaInsets::uniform(4.0).is_empty());
        assert_eq!(SafeAreaInsets::uniform(4.0).max(), 4.0);
    }

    #[test]
    fn the_largest_edge_wins_the_maximum() {
        let insets = SafeAreaInsets::from_edges(1.0, 9.0, 3.0, 5.0);
        assert_eq!(insets.max(), 9.0);
    }

    #[test]
    fn the_device_pixels_convert_into_logical_pixels() {
        let area = DeviceSafeArea {
            top: 60,
            right: 0,
            bottom: 120,
            left: 0,
        };
        let insets = SafeAreaInsets::from_device_pixels(area, 2.0);
        assert_eq!(insets.top, 30.0);
        assert_eq!(insets.bottom, 60.0);
        assert_eq!(insets.left, 0.0);
        // A broken scale falls back to the device pixels themselves.
        let broken = SafeAreaInsets::from_device_pixels(area, 0.0);
        assert_eq!(broken.top, 60.0);
        let infinite = SafeAreaInsets::from_device_pixels(area, f32::INFINITY);
        assert_eq!(infinite.bottom, 120.0);
    }

    #[test]
    fn a_centred_content_box_converts_into_the_margins() {
        let insets = SafeAreaInsets::around((800.0, 400.0), (1000.0, 600.0));
        assert_eq!(insets.left, 100.0);
        assert_eq!(insets.right, 100.0);
        assert_eq!(insets.top, 100.0);
        assert_eq!(insets.bottom, 100.0);
        // A content box larger than the window never produces a negative
        // margin.
        let overflow = SafeAreaInsets::around((1200.0, 800.0), (1000.0, 600.0));
        assert_eq!(overflow, SafeAreaInsets::NONE);
    }

    #[test]
    fn expanding_adds_the_padding_on_every_edge() {
        let insets = SafeAreaInsets::from_edges(10.0, 20.0, 30.0, 40.0);
        let expanded = insets.expanded(4.0);
        assert_eq!(expanded.top, 14.0);
        assert_eq!(expanded.right, 24.0);
        assert_eq!(expanded.bottom, 34.0);
        assert_eq!(expanded.left, 44.0);
    }
}

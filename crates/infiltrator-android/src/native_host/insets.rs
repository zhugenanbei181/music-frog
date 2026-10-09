//! BANDROID-006: `WindowInsets` projected onto the shared safe-area seam.
//!
//! The Kotlin host reads `WindowInsetsCompat` (status bar, navigation bar,
//! display cutout and the soft keyboard) in physical pixels and pushes them
//! with the real display density. This module converts them into the
//! toolkit-neutral [`SafeAreaInsets`] the shared shell consumes, keeping the
//! permanent safe area (status/nav/cutout) separate from the transient IME
//! avoidance so a keyboard change never double-adds the system bars.

use infiltrator_contract::shell_gesture::SafeAreaInsets;

/// One set of physical-pixel insets. Negative values are clamped to `0` when
/// converted: a host may not shrink the usable canvas below its own bounds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EdgeInsetsPx {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

impl EdgeInsetsPx {
    pub const ZERO: Self = Self {
        top: 0,
        right: 0,
        bottom: 0,
        left: 0,
    };

    pub const fn new(top: i32, right: i32, bottom: i32, left: i32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// The per-edge maximum of two inset sets. Android reports status bar,
    /// navigation bar and display cutout as independent sources; their union
    /// is what actually bounds the content.
    pub const fn union(self, other: Self) -> Self {
        Self {
            top: max(self.top, other.top),
            right: max(self.right, other.right),
            bottom: max(self.bottom, other.bottom),
            left: max(self.left, other.left),
        }
    }

    pub const fn is_zero(self) -> bool {
        self.top == 0 && self.right == 0 && self.bottom == 0 && self.left == 0
    }

    /// Convert to logical pixels using the real display density.
    pub fn to_logical(self, density: f32) -> SafeAreaInsets {
        SafeAreaInsets::new(
            px_to_logical(self.top, density),
            px_to_logical(self.right, density),
            px_to_logical(self.bottom, density),
            px_to_logical(self.left, density),
        )
    }
}

const fn max(a: i32, b: i32) -> i32 {
    if a > b { a } else { b }
}

/// One physical pixel divided by a sane density. A non-finite or non-positive
/// density falls back to `1.0` rather than producing an unbounded inset.
fn px_to_logical(px: i32, density: f32) -> f32 {
    let density = if density.is_finite() && density > 0.0 {
        density
    } else {
        1.0
    };
    (px.max(0) as f32) / density
}

/// One raw `WindowInsets` observation pushed by the native Activity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawInsetsPx {
    /// Display density (`resources.displayMetrics.density`).
    pub density: f32,
    /// Union of status bar, navigation bar and display cutout, in pixels.
    pub system_bars: EdgeInsetsPx,
    /// The soft-keyboard (`ime()`) insets, in pixels.
    pub ime: EdgeInsetsPx,
}

impl RawInsetsPx {
    pub const fn new(density: f32, system_bars: EdgeInsetsPx, ime: EdgeInsetsPx) -> Self {
        Self {
            density,
            system_bars,
            ime,
        }
    }

    /// Decode the wire form the Kotlin host sends: density plus the eight
    /// physical-pixel edges (system bars then IME).
    #[allow(clippy::too_many_arguments)]
    pub const fn from_edges(
        density: f32,
        system_top: i32,
        system_right: i32,
        system_bottom: i32,
        system_left: i32,
        ime_top: i32,
        ime_right: i32,
        ime_bottom: i32,
        ime_left: i32,
    ) -> Self {
        Self {
            density,
            system_bars: EdgeInsetsPx::new(system_top, system_right, system_bottom, system_left),
            ime: EdgeInsetsPx::new(ime_top, ime_right, ime_bottom, ime_left),
        }
    }
}

/// The typed insets the shell consumes: the permanent safe area and the
/// transient keyboard avoidance, both in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeInsets {
    pub safe_area: SafeAreaInsets,
    pub ime: SafeAreaInsets,
}

impl NativeInsets {
    pub const ZERO: Self = Self {
        safe_area: SafeAreaInsets::ZERO,
        ime: SafeAreaInsets::ZERO,
    };

    /// Convert one raw observation.
    pub fn from_raw(raw: RawInsetsPx) -> Self {
        Self {
            safe_area: raw.system_bars.to_logical(raw.density),
            ime: raw.ime.to_logical(raw.density),
        }
    }

    /// The permanent safe area (never includes the keyboard).
    pub const fn safe_area(self) -> SafeAreaInsets {
        self.safe_area
    }

    /// Whether the soft keyboard currently consumes screen space.
    pub fn ime_open(self) -> bool {
        self.ime.top > 0.0 || self.ime.right > 0.0 || self.ime.bottom > 0.0 || self.ime.left > 0.0
    }
}

impl Default for NativeInsets {
    fn default() -> Self {
        Self::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_convert_through_the_real_density() {
        let raw = RawInsetsPx::new(
            2.75,
            EdgeInsetsPx::new(88, 0, 132, 0),
            EdgeInsetsPx::new(0, 0, 800, 0),
        );
        let insets = NativeInsets::from_raw(raw);
        assert_eq!(insets.safe_area.top, 32.0);
        assert_eq!(insets.safe_area.bottom, 48.0);
        assert_eq!(insets.ime.bottom, 800.0 / 2.75);
        assert!(insets.ime_open());
    }

    #[test]
    fn negative_and_non_finite_inputs_collapse() {
        let raw = RawInsetsPx::new(
            f32::NAN,
            EdgeInsetsPx::new(-10, 0, 20, -4),
            EdgeInsetsPx::ZERO,
        );
        let insets = NativeInsets::from_raw(raw);
        assert_eq!(insets.safe_area.top, 0.0);
        assert_eq!(insets.safe_area.left, 0.0);
        assert_eq!(insets.safe_area.bottom, 20.0);
        assert!(!insets.ime_open());

        // A zero/negative density never divides to infinity.
        let degenerate = RawInsetsPx::new(0.0, EdgeInsetsPx::new(10, 0, 0, 0), EdgeInsetsPx::ZERO);
        assert_eq!(NativeInsets::from_raw(degenerate).safe_area.top, 10.0);
    }

    #[test]
    fn the_system_bar_union_takes_the_largest_edge() {
        let bars = EdgeInsetsPx::new(24, 0, 48, 0);
        let cutout = EdgeInsetsPx::new(72, 0, 0, 0);
        let union = bars.union(cutout);
        assert_eq!(union.top, 72);
        assert_eq!(union.bottom, 48);
        assert!(!union.is_zero());
        assert!(EdgeInsetsPx::ZERO.is_zero());
    }

    #[test]
    fn the_keyboard_inset_never_leaks_into_the_permanent_safe_area() {
        let raw = RawInsetsPx::from_edges(1.0, 24, 0, 48, 0, 0, 0, 900, 0);
        let insets = NativeInsets::from_raw(raw);
        assert_eq!(insets.safe_area.bottom, 48.0);
        assert_eq!(insets.ime.bottom, 900.0);
        assert!(insets.ime_open());
        assert_eq!(
            insets.safe_area(),
            SafeAreaInsets::new(24.0, 0.0, 48.0, 0.0)
        );
    }

    #[test]
    fn a_keyboard_change_does_not_double_add_system_bars() {
        let closed = NativeInsets::from_raw(RawInsetsPx::new(
            2.0,
            EdgeInsetsPx::new(40, 0, 80, 0),
            EdgeInsetsPx::ZERO,
        ));
        let opened = NativeInsets::from_raw(RawInsetsPx::new(
            2.0,
            EdgeInsetsPx::new(40, 0, 80, 0),
            EdgeInsetsPx::new(0, 0, 1000, 0),
        ));
        assert_eq!(closed.safe_area, opened.safe_area);
        assert!(!closed.ime_open());
        assert!(opened.ime_open());
    }
}

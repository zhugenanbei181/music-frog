//! The spacing ladder, the layout grid and the physical-size whitelist
//! (EFFECTS-002 #6 and #8).
//!
//! Two kinds of length live in the product and they must not be confused:
//!
//! * a **layout length** — a gap, a pad, an inset. It comes from the ladder
//!   below and sits on the 8pt grid (4pt at the half step);
//! * a **physical size** — a hairline, a caret, a touch target. It is not a
//!   layout decision but a physical fact of the display or of the hand, so it
//!   lives in a closed whitelist instead of on the ladder.
//!
//! A hard-coded length outside the ladder is therefore either a layout bug or
//! an undeclared physical size; the whitelist is what tells the two apart.
//!
//! The module owns three closed sets, one per kind of length:
//!
//! * [`SpaceStep`] — the layout ladder, published as rungs instead of raw
//!   numbers, so a gap can only be a value the ladder declares;
//! * [`PhysicalSize`] — the physical sizes the display or the hand dictates;
//! * [`ChromeHeight`] — the reserved height of a chrome element.
//!
//! [`classify_length`] is the single verdict of the module: it names the set
//! a length belongs to, and a length that belongs to none is a defect rather
//! than a style choice.

use crate::design_tokens::metrics;

/// The primary layout grid: 8pt.
pub const GRID_UNIT_PX: f32 = 8.0;

/// The half grid step: 4pt.
pub const GRID_HALF_UNIT_PX: f32 = 4.0;

/// The tightest layout gap (4px).
pub const XS: f32 = 4.0;
/// The default layout gap (8px).
pub const SM: f32 = 8.0;
/// The comfortable layout gap (12px).
pub const MD: f32 = 12.0;
/// The standard layout gap (16px).
pub const LG: f32 = 16.0;
/// The wide layout gap (20px).
pub const XL: f32 = 20.0;
/// The widest layout gap (24px).
pub const XXL: f32 = 24.0;
/// Mobile screen edge margin (compact smartphones: 16px).
pub const SCREEN_MARGIN_MOBILE: f32 = 16.0;
/// Tablet screen edge margin (medium viewports: 24px).
pub const SCREEN_MARGIN_TABLET: f32 = 24.0;

/// One rung of the spacing ladder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpaceStep {
    /// The tightest gap (4px).
    Xs,
    /// The default gap (8px).
    Sm,
    /// The comfortable gap (12px).
    Md,
    /// The standard gap (16px).
    Lg,
    /// The wide gap (20px).
    Xl,
    /// The widest gap (24px).
    Xxl,
}

impl SpaceStep {
    /// Every rung of the ladder, from the tightest to the widest.
    pub const ALL: [Self; 6] = [Self::Xs, Self::Sm, Self::Md, Self::Lg, Self::Xl, Self::Xxl];

    /// The length of this rung in logical pixels.
    pub const fn px(self) -> f32 {
        match self {
            Self::Xs => XS,
            Self::Sm => SM,
            Self::Md => MD,
            Self::Lg => LG,
            Self::Xl => XL,
            Self::Xxl => XXL,
        }
    }

    /// The position of this rung in the ladder.
    pub const fn index(self) -> usize {
        match self {
            Self::Xs => 0,
            Self::Sm => 1,
            Self::Md => 2,
            Self::Lg => 3,
            Self::Xl => 4,
            Self::Xxl => 5,
        }
    }

    /// The rung that carries exactly this length.
    ///
    /// The ladder is a closed set: a length that is not one of its rungs has
    /// no rung at all. The lookup never rounds, because rounding a length
    /// that was never declared would hide the defect the guard looks for.
    pub fn from_px(px: f32) -> Option<Self> {
        Self::ALL.into_iter().find(|step| step.px() == px)
    }
}

/// The two units the layout is measured in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GridUnit {
    /// The primary 8pt grid.
    Primary,
    /// The 4pt half step.
    Half,
}

impl GridUnit {
    /// Every grid unit.
    pub const ALL: [Self; 2] = [Self::Primary, Self::Half];

    /// The length of this unit in logical pixels.
    pub const fn px(self) -> f32 {
        match self {
            Self::Primary => GRID_UNIT_PX,
            Self::Half => GRID_HALF_UNIT_PX,
        }
    }

    /// The stable name of this unit.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Primary => "8pt",
            Self::Half => "4pt",
        }
    }
}

/// The screen edge margins the shell reserves (EFFECTS-002 #6).
///
/// A screen margin is a layout length like any other: it is a rung of the
/// ladder, so the compact and the medium shell keep the same rhythm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScreenMargin {
    /// The margin of a compact smartphone screen.
    Mobile,
    /// The margin of a medium (tablet) screen.
    Tablet,
}

impl ScreenMargin {
    /// Every screen margin.
    pub const ALL: [Self; 2] = [Self::Mobile, Self::Tablet];

    /// The length of this margin in logical pixels.
    pub const fn px(self) -> f32 {
        match self {
            Self::Mobile => SCREEN_MARGIN_MOBILE,
            Self::Tablet => SCREEN_MARGIN_TABLET,
        }
    }

    /// The stable name of this margin.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Mobile => "mobile",
            Self::Tablet => "tablet",
        }
    }
}

/// Whether a length sits on the 4pt half-grid.
///
/// The comparison is tolerant on purpose: a length that arrives from a
/// percentage or a device scale factor is on the grid even when its binary
/// representation is not exact.
pub fn is_on_grid(px: f32) -> bool {
    if !px.is_finite() {
        return false;
    }
    let steps = px / GRID_HALF_UNIT_PX;
    (steps - steps.round()).abs() <= 1e-4
}

/// Snap a length to the nearest 4pt half-grid step.
pub fn snap_to_grid(px: f32) -> f32 {
    if !px.is_finite() {
        return 0.0;
    }
    (px / GRID_HALF_UNIT_PX).round() * GRID_HALF_UNIT_PX
}

/// The physical sizes the product paints (EFFECTS-002 #8).
///
/// These are physical facts, not layout choices: the hairline is one device
/// pixel, the caret is the width of a text cursor, and the touch targets are
/// the minimums the accessibility guidelines ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PhysicalSize {
    /// The hairline border of a card or control.
    Hairline,
    /// The text caret.
    Cursor,
    /// The focus ring drawn around the focused control.
    FocusRing,
    /// The minimum touch target (Apple HIG compact).
    TouchTargetCompact,
    /// The standard touch target (Android HIG / WCAG 2.5.5).
    TouchTargetStandard,
}

impl PhysicalSize {
    /// Every physical size the product paints.
    pub const ALL: [Self; 5] = [
        Self::Hairline,
        Self::Cursor,
        Self::FocusRing,
        Self::TouchTargetCompact,
        Self::TouchTargetStandard,
    ];

    /// The length of this physical size in logical pixels.
    pub const fn px(self) -> f32 {
        match self {
            Self::Hairline => metrics::HAIRLINE,
            Self::Cursor => metrics::CURSOR,
            Self::FocusRing => metrics::FOCUS_RING,
            Self::TouchTargetCompact => metrics::TOUCH_TARGET_COMPACT,
            Self::TouchTargetStandard => metrics::MIN_TOUCH_TARGET,
        }
    }

    /// The physical size that carries exactly this length.
    ///
    /// The focus ring shares the caret's length, so the caret is the owner
    /// that is reported for that length.
    pub fn from_px(px: f32) -> Option<Self> {
        Self::ALL.into_iter().find(|size| size.px() == px)
    }
}

/// The closed whitelist of physical lengths a surface may paint directly.
///
/// The set is closed on purpose: a hard-coded length outside it is a defect,
/// not a style choice. The focus ring shares the caret's length, so the
/// whitelist lists the length once.
pub const PHYSICAL_SIZE_WHITELIST: [f32; 4] = [
    metrics::HAIRLINE,
    metrics::CURSOR,
    metrics::TOUCH_TARGET_COMPACT,
    metrics::MIN_TOUCH_TARGET,
];

/// Whether a length is one of the whitelisted physical sizes.
pub fn is_allowed_physical_px(px: f32) -> bool {
    PHYSICAL_SIZE_WHITELIST
        .iter()
        .any(|allowed| (*allowed - px).abs() <= f32::EPSILON)
}

/// The height a chrome element reserves (EFFECTS-002 #8).
///
/// The bottom navigation bar and the top application bar are not layout gaps:
/// they are the height of a real element, so they live in their own closed
/// set instead of on the spacing ladder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChromeHeight {
    /// The bottom navigation bar of a mobile shell.
    BottomNav,
    /// The top application bar of a mobile shell.
    TopAppBar,
}

impl ChromeHeight {
    /// Every chrome height.
    pub const ALL: [Self; 2] = [Self::BottomNav, Self::TopAppBar];

    /// The length of this chrome height in logical pixels.
    pub const fn px(self) -> f32 {
        match self {
            Self::BottomNav => metrics::BOTTOM_NAV_HEIGHT,
            Self::TopAppBar => metrics::TOP_APP_BAR_HEIGHT,
        }
    }

    /// The stable name of this chrome height.
    pub const fn label(self) -> &'static str {
        match self {
            Self::BottomNav => "bottom-nav",
            Self::TopAppBar => "top-app-bar",
        }
    }

    /// The chrome height that carries exactly this length.
    pub fn from_px(px: f32) -> Option<Self> {
        Self::ALL.into_iter().find(|height| height.px() == px)
    }
}

/// The closed set a length travels in (EFFECTS-002 #6 and #8).
///
/// The verdict is what tells a layout bug from a declared physical fact: a
/// length that belongs to no channel is a defect, because every length the
/// product paints is either a rung of the ladder, a whitelisted physical
/// size or the reserved height of a chrome element.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LengthChannel {
    /// A layout length that sits on the spacing ladder.
    Layout(SpaceStep),
    /// A physical size the display or the hand dictates.
    Physical(PhysicalSize),
    /// The reserved height of a chrome element.
    Chrome(ChromeHeight),
}

/// The channel a length belongs to, if it belongs to one.
///
/// The lookup is exact: a length that is one step away from a rung belongs
/// to no channel, because a surface that wants a rung asks for the rung.
pub fn classify_length(px: f32) -> Option<LengthChannel> {
    if let Some(step) = SpaceStep::from_px(px) {
        return Some(LengthChannel::Layout(step));
    }
    if let Some(size) = PhysicalSize::from_px(px) {
        return Some(LengthChannel::Physical(size));
    }
    ChromeHeight::from_px(px).map(LengthChannel::Chrome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ladder_is_the_grid_and_nothing_else() {
        for step in SpaceStep::ALL {
            assert!(
                is_on_grid(step.px()),
                "{step:?} ({}) left the 4pt grid",
                step.px()
            );
            assert!(step.px() > 0.0);
        }
        assert_eq!(SpaceStep::ALL[0].px(), XS);
        assert_eq!(SpaceStep::ALL[5].px(), XXL);
        for pair in SpaceStep::ALL.windows(2) {
            assert!(pair[0].px() < pair[1].px(), "the ladder must be ordered");
        }
    }

    #[test]
    fn the_grid_snap_rounds_to_the_half_step() {
        assert_eq!(snap_to_grid(5.0), 4.0);
        assert_eq!(snap_to_grid(6.0), 8.0);
        assert_eq!(snap_to_grid(0.0), 0.0);
        assert_eq!(snap_to_grid(f32::NAN), 0.0);
        assert!(!is_on_grid(5.0));
        assert!(is_on_grid(12.0));
        assert!(!is_on_grid(f32::INFINITY));
    }

    #[test]
    fn the_physical_whitelist_is_closed() {
        assert!(is_allowed_physical_px(PhysicalSize::Hairline.px()));
        assert!(is_allowed_physical_px(PhysicalSize::Cursor.px()));
        assert!(is_allowed_physical_px(PhysicalSize::FocusRing.px()));
        assert!(is_allowed_physical_px(
            PhysicalSize::TouchTargetCompact.px()
        ));
        assert!(is_allowed_physical_px(
            PhysicalSize::TouchTargetStandard.px()
        ));
        // A layout length is not a physical size.
        assert!(!is_allowed_physical_px(SpaceStep::Lg.px()));
        assert!(!is_allowed_physical_px(12.0));
        assert!(!is_allowed_physical_px(0.0));
        for size in PhysicalSize::ALL {
            assert!(size.px() > 0.0);
        }
    }

    #[test]
    fn a_rung_is_found_only_by_its_own_length() {
        assert_eq!(SpaceStep::from_px(8.0), Some(SpaceStep::Sm));
        assert_eq!(SpaceStep::from_px(4.0), Some(SpaceStep::Xs));
        assert_eq!(SpaceStep::from_px(24.0), Some(SpaceStep::Xxl));
        // The ladder never rounds a length it does not publish.
        assert_eq!(SpaceStep::from_px(18.0), None);
        assert_eq!(SpaceStep::from_px(0.0), None);
        assert_eq!(SpaceStep::from_px(f32::NAN), None);
        for step in SpaceStep::ALL {
            assert_eq!(SpaceStep::from_px(step.px()), Some(step));
        }
    }

    #[test]
    fn the_grid_units_are_the_ladder_itself() {
        assert_eq!(GridUnit::Primary.px(), 8.0);
        assert_eq!(GridUnit::Half.px(), 4.0);
        assert_eq!(GridUnit::Primary.px(), 2.0 * GridUnit::Half.px());
        for unit in GridUnit::ALL {
            assert!(SpaceStep::from_px(unit.px()).is_some());
            assert!(is_on_grid(unit.px()));
            assert!(!unit.label().is_empty());
        }
    }

    #[test]
    fn the_screen_margins_sit_on_the_ladder() {
        assert_eq!(ScreenMargin::Mobile.px(), 16.0);
        assert_eq!(ScreenMargin::Tablet.px(), 24.0);
        for margin in ScreenMargin::ALL {
            assert!(SpaceStep::from_px(margin.px()).is_some());
            assert!(!margin.label().is_empty());
        }
    }

    #[test]
    fn a_physical_size_is_found_only_by_its_own_length() {
        assert_eq!(PhysicalSize::from_px(1.0), Some(PhysicalSize::Hairline));
        assert_eq!(
            PhysicalSize::from_px(44.0),
            Some(PhysicalSize::TouchTargetCompact)
        );
        assert_eq!(
            PhysicalSize::from_px(48.0),
            Some(PhysicalSize::TouchTargetStandard)
        );
        // The focus ring shares the caret's length, so the caret owns it.
        assert_eq!(PhysicalSize::from_px(2.0), Some(PhysicalSize::Cursor));
        assert_eq!(PhysicalSize::from_px(16.0), None);
        for size in PhysicalSize::ALL {
            assert!(PhysicalSize::from_px(size.px()).is_some());
        }
    }

    #[test]
    fn every_length_is_routed_to_exactly_one_channel() {
        for step in SpaceStep::ALL {
            assert_eq!(
                classify_length(step.px()),
                Some(LengthChannel::Layout(step))
            );
        }
        for size in PhysicalSize::ALL {
            let channel = classify_length(size.px());
            assert!(matches!(channel, Some(LengthChannel::Physical(_))));
        }
        for height in ChromeHeight::ALL {
            assert_eq!(
                classify_length(height.px()),
                Some(LengthChannel::Chrome(height))
            );
        }
        // A length that belongs to no set is a defect, not a style choice.
        assert_eq!(classify_length(18.0), None);
        assert_eq!(classify_length(0.0), None);
        assert_eq!(classify_length(f32::NAN), None);
    }

    #[test]
    fn the_channels_never_overlap() {
        // A physical size is not a rung of the ladder, and a chrome height is
        // neither: the closed sets must not blur into each other.
        for step in SpaceStep::ALL {
            for size in PhysicalSize::ALL {
                assert_ne!(size.px(), step.px());
            }
            for height in ChromeHeight::ALL {
                assert_ne!(height.px(), step.px());
            }
        }
        for size in PhysicalSize::ALL {
            for height in ChromeHeight::ALL {
                assert_ne!(height.px(), size.px());
            }
        }
    }
}

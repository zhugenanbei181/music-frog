//! Canonical shell design tokens shared by both surfaces (DUAL-15-14).
//!
//! Both peer products share this neutral authority. This module carries the
//! numbers once: Iced builds its `view::theme` token constants directly from
//! [`skin_core`] and the spacing/radius/metric ladders below (compiler-bound),
//! while the business-agnostic Bevy widget layer — which by charter cannot
//! depend on this crate — mirrors the same numbers, enforced by the dual
//! headless test `the_widget_palette_mirrors_the_shared_design_tokens`.
//!
//! The scope is deliberately the tokens both surfaces actually consume:
//! the core skin palette (canvas / surfaces / ink / accent / semantic colors),
//! the interaction palette every programmable surface exposes (overlay scrim,
//! hover/pressed washes, focus ring, disabled ink), the spacing ladder, the
//! corner radii and the hairline width. Surface-only decoration (icon-tile
//! tints, per-surface type scales, accent-tinted button derivations) stays
//! with its surface.
//!
//! Four submodules carry the finer contracts of the token system:
//!
//! * [`palette`] — the semantic colour roles, the eight-step surface ladder,
//!   the two-stop gradient, the highlight stroke and the WCAG text contrast
//!   ladder (EFFECTS-001);
//! * [`space`] — the 8pt/4pt layout grid, the closed physical-size whitelist
//!   and the chrome heights, plus the one verdict that names the channel a
//!   length travels in (EFFECTS-002 #6, #8);
//! * [`radius`] — the mutually exclusive corner-radius roles and the radius
//!   each role owns (EFFECTS-002 #7);
//! * [`elevation`] — the elevation levels, the ladder and the two-layer
//!   shadow specification (EFFECTS-002 #9).

use crate::theme::ThemeSkin;

/// One sRGBA token, channel-exact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RgbaToken {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl RgbaToken {
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
}

/// The semantic core of one skin's palette, shared by both surfaces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkinCorePalette {
    /// Window/page backdrop.
    pub canvas: RgbaToken,
    /// Sidebar rail fill (one step off the canvas).
    pub sidebar: RgbaToken,
    /// Card / panel fill.
    pub card: RgbaToken,
    /// Hairline around cards and controls.
    pub card_border: RgbaToken,
    /// Recessed control fill (segmented tracks, inputs, idle chips).
    pub control_bg: RgbaToken,
    /// Primary reading ink.
    pub ink: RgbaToken,
    /// Secondary reading ink; the light skin stays opaque for readable contrast.
    pub ink_dim: RgbaToken,
    /// Accent fill / accent ink.
    pub accent: RgbaToken,
    /// Ink drawn on top of the accent.
    pub on_accent: RgbaToken,
    pub success: RgbaToken,
    pub warning: RgbaToken,
    pub danger: RgbaToken,
}

/// The canonical semantic palette for one skin.
pub const fn skin_core(skin: ThemeSkin) -> SkinCorePalette {
    match skin {
        ThemeSkin::Dark => SkinCorePalette {
            canvas: RgbaToken::rgb(0.082, 0.094, 0.102),
            sidebar: RgbaToken::rgb(0.106, 0.118, 0.125),
            card: RgbaToken::rgb(0.145, 0.161, 0.173),
            card_border: RgbaToken::rgba(0.85, 0.90, 0.95, 0.10),
            control_bg: RgbaToken::rgba(1.0, 1.0, 1.0, 0.07),
            ink: RgbaToken::rgb(0.93, 0.95, 0.94),
            ink_dim: RgbaToken::rgba(0.88, 0.90, 0.92, 0.65),
            accent: RgbaToken::rgb(0.12, 0.56, 0.96),
            on_accent: RgbaToken::rgb(1.0, 1.0, 1.0),
            success: RgbaToken::rgb(0.24, 0.78, 0.44),
            warning: RgbaToken::rgb(0.96, 0.62, 0.15),
            danger: RgbaToken::rgb(0.96, 0.35, 0.32),
        },
        ThemeSkin::Light => SkinCorePalette {
            canvas: RgbaToken::rgb(0.953, 0.957, 0.957),
            sidebar: RgbaToken::rgb(0.965, 0.969, 0.969),
            card: RgbaToken::rgb(0.988, 0.992, 0.988),
            card_border: RgbaToken::rgba(0.18, 0.22, 0.20, 0.10),
            control_bg: RgbaToken::rgba(0.0, 0.0, 0.0, 0.05),
            ink: RgbaToken::rgb(0.12, 0.15, 0.14),
            ink_dim: RgbaToken::rgb(0.24, 0.28, 0.26),
            accent: RgbaToken::rgb(0.04, 0.44, 0.88),
            on_accent: RgbaToken::rgb(1.0, 1.0, 1.0),
            success: RgbaToken::rgb(0.18, 0.68, 0.38),
            warning: RgbaToken::rgb(0.88, 0.52, 0.05),
            danger: RgbaToken::rgb(0.88, 0.24, 0.22),
        },
        ThemeSkin::Forest => SkinCorePalette {
            canvas: RgbaToken::rgb(0.937, 0.961, 0.925),
            sidebar: RgbaToken::rgb(0.851, 0.910, 0.843),
            card: RgbaToken::rgb(0.973, 0.984, 0.961),
            card_border: RgbaToken::rgba(0.341, 0.439, 0.353, 0.22),
            control_bg: RgbaToken::rgba(0.341, 0.439, 0.353, 0.09),
            ink: RgbaToken::rgb(0.122, 0.208, 0.145),
            ink_dim: RgbaToken::rgba(0.341, 0.439, 0.353, 0.75),
            accent: RgbaToken::rgb(0.188, 0.435, 0.306),
            on_accent: RgbaToken::rgb(1.0, 1.0, 1.0),
            success: RgbaToken::rgb(0.243, 0.490, 0.314),
            warning: RgbaToken::rgb(0.663, 0.439, 0.157),
            danger: RgbaToken::rgb(0.702, 0.231, 0.275),
        },
        ThemeSkin::Amoled => SkinCorePalette {
            canvas: RgbaToken::rgb(0.0, 0.0, 0.0),
            sidebar: RgbaToken::rgb(0.051, 0.059, 0.067),
            card: RgbaToken::rgb(0.086, 0.098, 0.110),
            card_border: RgbaToken::rgba(0.85, 0.90, 0.95, 0.12),
            control_bg: RgbaToken::rgba(1.0, 1.0, 1.0, 0.09),
            ink: RgbaToken::rgb(0.973, 0.980, 0.988),
            ink_dim: RgbaToken::rgba(0.90, 0.92, 0.94, 0.68),
            accent: RgbaToken::rgb(0.12, 0.56, 0.96),
            on_accent: RgbaToken::rgb(1.0, 1.0, 1.0),
            success: RgbaToken::rgb(0.063, 0.725, 0.506),
            warning: RgbaToken::rgb(0.96, 0.62, 0.15),
            danger: RgbaToken::rgb(0.96, 0.35, 0.32),
        },
    }
}

/// The interaction/overlay tokens both surfaces paint (DUAL-15-14).
///
/// These are the programmable interaction surfaces the two toolkits both
/// expose: the modal/overlay backdrop (`scrim`), the neutral control washes
/// (`hover`, `pressed`), the keyboard `focus_ring` and the dimmed
/// `disabled_ink`. Accent-tinted hover derivations (primary/danger buttons)
/// and shadows are surface-local decoration and stay out of this palette.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkinInteractionPalette {
    /// Modal / overlay backdrop painted over the page content.
    pub scrim: RgbaToken,
    /// Neutral control hover wash, painted over the control surface.
    pub hover: RgbaToken,
    /// Neutral control pressed wash; always stronger than [`Self::hover`].
    pub pressed: RgbaToken,
    /// Keyboard focus ring around the focused control. Product rule: the
    /// focus ring is the skin accent, kept as its own seam so the ring can
    /// change independently of selection fills.
    pub focus_ring: RgbaToken,
    /// Disabled / tertiary ink: dimmed labels, hints and placeholders. Always
    /// dimmer than [`SkinCorePalette::ink_dim`], which is the readable
    /// secondary ink.
    pub disabled_ink: RgbaToken,
}

/// The canonical interaction palette for one skin.
pub const fn skin_interaction(skin: ThemeSkin) -> SkinInteractionPalette {
    match skin {
        ThemeSkin::Dark => SkinInteractionPalette {
            scrim: RgbaToken::rgba(0.0, 0.0, 0.0, 0.50),
            hover: RgbaToken::rgba(1.0, 1.0, 1.0, 0.08),
            pressed: RgbaToken::rgba(1.0, 1.0, 1.0, 0.14),
            focus_ring: RgbaToken::rgb(0.12, 0.56, 0.96),
            // The disabled ink is a readable label, not a whisper: at 0.35 it
            // measured ≈3:1 against the dark card, which fails the product's
            // own legibility floor for a label a user must still read. 0.55
            // measures ≈4.9:1 on the same card and stays dimmer than the
            // secondary ink.
            disabled_ink: RgbaToken::rgba(0.88, 0.90, 0.92, 0.55),
        },
        ThemeSkin::Light => SkinInteractionPalette {
            scrim: RgbaToken::rgba(0.0, 0.0, 0.0, 0.40),
            hover: RgbaToken::rgba(0.0, 0.0, 0.0, 0.06),
            pressed: RgbaToken::rgba(0.0, 0.0, 0.0, 0.12),
            focus_ring: RgbaToken::rgb(0.04, 0.44, 0.88),
            disabled_ink: RgbaToken::rgba(0.24, 0.28, 0.26, 0.38),
        },
        ThemeSkin::Forest => SkinInteractionPalette {
            scrim: RgbaToken::rgba(0.122, 0.208, 0.145, 0.42),
            hover: RgbaToken::rgba(0.122, 0.208, 0.145, 0.06),
            pressed: RgbaToken::rgba(0.122, 0.208, 0.145, 0.12),
            focus_ring: RgbaToken::rgb(0.188, 0.435, 0.306),
            disabled_ink: RgbaToken::rgba(0.341, 0.439, 0.353, 0.45),
        },
        ThemeSkin::Amoled => SkinInteractionPalette {
            scrim: RgbaToken::rgba(0.0, 0.0, 0.0, 0.55),
            hover: RgbaToken::rgba(1.0, 1.0, 1.0, 0.08),
            pressed: RgbaToken::rgba(1.0, 1.0, 1.0, 0.14),
            focus_ring: RgbaToken::rgb(0.12, 0.56, 0.96),
            disabled_ink: RgbaToken::rgba(0.90, 0.92, 0.94, 0.38),
        },
    }
}

/// The semantic colour roles of the product (EFFECTS-001).
#[path = "palette.rs"]
pub mod palette;

/// The spacing ladder, the layout grid and the physical-size whitelist
/// (EFFECTS-002).
#[path = "space.rs"]
pub mod space;

/// The corner-radius ladder and the role each rung owns (EFFECTS-002).
#[path = "radius.rs"]
pub mod radius;

/// The elevation ladder and the two-layer shadow specification (EFFECTS-002).
#[path = "elevation.rs"]
pub mod elevation;

/// The canonical scrollbar metrics and interaction contract across surfaces.
#[path = "scrollbar.rs"]
pub mod scrollbar;

/// Continuous curvature / superellipse specification (Squircle / G2 continuity).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CornerCurvature {
    pub radius_px: f32,
    /// Curvature smoothing: 0.0 = classic arc (G1), 0.6 = standard squircle, 1.0 = full superellipse (G2).
    pub smoothing: f32,
}

impl CornerCurvature {
    pub const fn new(radius_px: f32, smoothing: f32) -> Self {
        Self {
            radius_px,
            smoothing,
        }
    }

    pub const fn circular(radius_px: f32) -> Self {
        Self {
            radius_px,
            smoothing: 0.0,
        }
    }

    pub const fn squircle(radius_px: f32) -> Self {
        Self {
            radius_px,
            smoothing: 0.6,
        }
    }
}

/// Shared typography scale (logical pixels).
pub mod type_scale {
    pub const DISPLAY: f32 = 22.0;
    pub const TITLE: f32 = 20.0;
    pub const HEADING: f32 = 20.0;
    pub const BODY: f32 = 15.0;
    /// Large reading / prominent body text (16px).
    pub const BODY_LARGE: f32 = 16.0;
    /// Standard mobile label / button caption (14px).
    pub const LABEL_LARGE: f32 = 14.0;
    pub const CAPTION: f32 = 12.0;
    /// Small mobile auxiliary label (11px).
    pub const LABEL_SMALL: f32 = 11.0;
    pub const TAG: f32 = 10.0;
    pub const MONO: f32 = 13.0;
}

/// Shared control metrics (logical pixels).
pub mod metrics {
    /// Hairline border width.
    pub const HAIRLINE: f32 = 1.0;
    /// The width of a text caret (the physical cursor of an input field).
    pub const CURSOR: f32 = 2.0;
    /// The width of the focus ring drawn around the focused control.
    pub const FOCUS_RING: f32 = 2.0;
    /// Minimum mobile touch target dimension (px) for touch accessibility (WCAG 2.5.5 / Android HIG).
    pub const MIN_TOUCH_TARGET: f32 = 48.0;
    /// Compact mobile touch target (Apple HIG minimum: 44px).
    pub const TOUCH_TARGET_COMPACT: f32 = 44.0;
    /// Comfortable mobile touch target (Android HIG standard: 48px).
    pub const TOUCH_TARGET_COMFORTABLE: f32 = 48.0;
    /// Standard mobile bottom navigation bar height (56px).
    pub const BOTTOM_NAV_HEIGHT: f32 = 56.0;
    /// Standard mobile top app bar / header height (64px).
    pub const TOP_APP_BAR_HEIGHT: f32 = 64.0;
}

/// Mobile navigation and interaction tokens for each skin (BANDROID-027.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MobileThemeTokens {
    /// Bottom navigation bar container background.
    pub bottom_nav_bg: RgbaToken,
    /// Active indicator pill background for bottom navigation.
    pub bottom_nav_indicator: RgbaToken,
    /// Mobile touch ripple / splash feedback wash.
    pub touch_ripple: RgbaToken,
    /// Swipe-to-action primary indicator color.
    pub swipe_action: RgbaToken,
    /// Swipe-to-action destructive / delete indicator color.
    pub swipe_destructive: RgbaToken,
}

pub const fn mobile_theme_tokens(skin: ThemeSkin) -> MobileThemeTokens {
    let core = skin_core(skin);
    let interaction = skin_interaction(skin);
    match skin {
        ThemeSkin::Dark => MobileThemeTokens {
            bottom_nav_bg: core.sidebar,
            bottom_nav_indicator: RgbaToken::rgba(
                core.accent.r,
                core.accent.g,
                core.accent.b,
                0.24,
            ),
            touch_ripple: interaction.pressed,
            swipe_action: core.accent,
            swipe_destructive: core.danger,
        },
        ThemeSkin::Light => MobileThemeTokens {
            bottom_nav_bg: core.sidebar,
            bottom_nav_indicator: RgbaToken::rgba(
                core.accent.r,
                core.accent.g,
                core.accent.b,
                0.16,
            ),
            touch_ripple: interaction.pressed,
            swipe_action: core.accent,
            swipe_destructive: core.danger,
        },
        ThemeSkin::Forest => MobileThemeTokens {
            bottom_nav_bg: core.sidebar,
            bottom_nav_indicator: RgbaToken::rgba(
                core.accent.r,
                core.accent.g,
                core.accent.b,
                0.20,
            ),
            touch_ripple: interaction.pressed,
            swipe_action: core.accent,
            swipe_destructive: core.danger,
        },
        ThemeSkin::Amoled => MobileThemeTokens {
            bottom_nav_bg: core.sidebar,
            bottom_nav_indicator: RgbaToken::rgba(
                core.accent.r,
                core.accent.g,
                core.accent.b,
                0.28,
            ),
            touch_ripple: interaction.pressed,
            swipe_action: core.accent,
            swipe_destructive: core.danger,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_skin_has_a_distinct_core_palette() {
        let dark = skin_core(ThemeSkin::Dark);
        let light = skin_core(ThemeSkin::Light);
        let forest = skin_core(ThemeSkin::Forest);
        let amoled = skin_core(ThemeSkin::Amoled);
        assert_ne!(dark.canvas, light.canvas);
        assert_ne!(dark.accent, light.accent);
        assert_ne!(light.canvas, forest.canvas);
        assert_ne!(dark.canvas, amoled.canvas);
        assert_eq!(amoled.canvas, RgbaToken::rgb(0.0, 0.0, 0.0));
        for skin in ThemeSkin::ALL {
            let core = skin_core(skin);
            assert_eq!(core.on_accent.a, 1.0);
            assert!(core.canvas.r.is_finite() && core.accent.b.is_finite());
            assert_ne!(
                core.ink, core.canvas,
                "the reading ink must not vanish into the canvas"
            );
        }
    }

    #[test]
    fn every_skin_defines_the_shared_interaction_tokens() {
        for skin in ThemeSkin::ALL {
            let interaction = skin_interaction(skin);
            let core = skin_core(skin);
            let channels = [
                interaction.scrim,
                interaction.hover,
                interaction.pressed,
                interaction.focus_ring,
                interaction.disabled_ink,
            ];
            for token in channels {
                assert!(
                    token.r.is_finite()
                        && token.g.is_finite()
                        && token.b.is_finite()
                        && token.a.is_finite(),
                    "{skin:?} interaction token has a non-finite channel"
                );
            }
            // The overlay scrim is a real translucent wash: it must actually
            // cover without going fully opaque.
            assert!(
                interaction.scrim.a > 0.0 && interaction.scrim.a < 1.0,
                "{skin:?} scrim must be a translucent backdrop"
            );
            assert!(
                interaction.pressed.a > interaction.hover.a,
                "{skin:?} pressed wash must read stronger than hover"
            );
            assert_eq!(
                interaction.focus_ring, core.accent,
                "the focus ring is the skin accent by product rule"
            );
            assert!(
                interaction.disabled_ink.a > 0.0 && interaction.disabled_ink.a < core.ink_dim.a,
                "{skin:?} disabled ink must stay dimmer than secondary ink"
            );
        }
    }

    #[test]
    fn the_interaction_washes_stay_distinct_across_skins() {
        let dark = skin_interaction(ThemeSkin::Dark);
        let light = skin_interaction(ThemeSkin::Light);
        let forest = skin_interaction(ThemeSkin::Forest);
        let amoled = skin_interaction(ThemeSkin::Amoled);
        assert_ne!(dark.hover, light.hover);
        assert_ne!(light.hover, forest.hover);
        assert_ne!(dark.scrim.a, amoled.scrim.a);
        assert_ne!(light.disabled_ink, dark.disabled_ink);
    }

    #[test]
    fn every_skin_defines_mobile_tokens_and_ladders_are_positive() {
        for skin in ThemeSkin::ALL {
            let mobile = mobile_theme_tokens(skin);
            assert!(mobile.bottom_nav_bg.a > 0.0);
            assert!(mobile.bottom_nav_indicator.a > 0.0);
            assert!(mobile.touch_ripple.a > 0.0);
            assert!(mobile.swipe_action.a > 0.0);
            assert!(mobile.swipe_destructive.a > 0.0);
        }
        assert_eq!(metrics::MIN_TOUCH_TARGET, 48.0);
        assert_eq!(metrics::TOUCH_TARGET_COMPACT, 44.0);
        assert_eq!(metrics::TOUCH_TARGET_COMFORTABLE, 48.0);
        assert_eq!(metrics::BOTTOM_NAV_HEIGHT, 56.0);
        assert_eq!(metrics::TOP_APP_BAR_HEIGHT, 64.0);
        assert_eq!(space::SCREEN_MARGIN_MOBILE, 16.0);
        assert_eq!(space::SCREEN_MARGIN_TABLET, 24.0);
        assert_eq!(radius::BOTTOM_SHEET, 28.0);
        assert_eq!(type_scale::BODY_LARGE, 16.0);
        assert_eq!(type_scale::LABEL_LARGE, 14.0);
        assert_eq!(type_scale::LABEL_SMALL, 11.0);
    }

    #[test]
    fn the_spacing_and_radius_ladders_are_positive_and_ordered() {
        let ladder = [
            space::XS,
            space::SM,
            space::MD,
            space::LG,
            space::XL,
            space::XXL,
        ];
        assert!(ladder.windows(2).all(|pair| pair[0] < pair[1]));
        // The ladders are const-folded product numbers; the relations are
        // checked once at compile time so the runtime test stays honest.
        const _: () = assert!(radius::XS > 0.0);
        const _: () = assert!(radius::SM > radius::XS);
        const _: () = assert!(radius::CONTROL > radius::SM);
        const _: () = assert!(radius::CARD > radius::CONTROL);
        const _: () = assert!(radius::MODAL > radius::CARD);
        const _: () = assert!(radius::PILL > radius::MODAL);
        const _: () = assert!(metrics::HAIRLINE > 0.0);
    }

    /// The radius roles are mutually exclusive: a centered dialog, a
    /// panel-style dialog and a bottom sheet each have exactly one rung,
    /// and no two roles share a rung.
    #[test]
    fn every_radius_role_owns_exactly_one_rung() {
        // The three dialog roles are decided by the contract: a centered
        // dialog is the modal rung, a panel-style dialog is the card rung
        // and a bottom sheet is the bottom sheet rung.
        assert_eq!(radius::MODAL, 24.0, "the centered dialog role");
        assert_eq!(radius::CARD, 16.0, "the panel-style dialog role");
        assert_eq!(radius::BOTTOM_SHEET, 28.0, "the bottom sheet role");
        // The centered dialog is rounder than the panel-style one, and the
        // bottom sheet is rounder than the centered dialog. The roles are
        // constants, so the ladder between them is checked as a constant.
        const {
            assert!(radius::MODAL > radius::CARD);
            assert!(radius::BOTTOM_SHEET > radius::MODAL);
        }
    }

    #[test]
    fn corner_curvature_constructors_are_valid() {
        let circ = CornerCurvature::circular(16.0);
        assert_eq!(circ.radius_px, 16.0);
        assert_eq!(circ.smoothing, 0.0);

        let squircle = CornerCurvature::squircle(16.0);
        assert_eq!(squircle.radius_px, 16.0);
        assert_eq!(squircle.smoothing, 0.6);

        let custom = CornerCurvature::new(24.0, 1.0);
        assert_eq!(custom.radius_px, 24.0);
        assert_eq!(custom.smoothing, 1.0);
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn typography_scale_and_elevation_are_valid() {
        assert!(type_scale::TAG < type_scale::CAPTION);
        assert!(type_scale::CAPTION < type_scale::BODY);
        assert!(type_scale::BODY < type_scale::HEADING);
        assert!(type_scale::HEADING <= type_scale::TITLE);
        assert!(type_scale::TITLE < type_scale::DISPLAY);

        assert!(elevation::LOW.blur_radius < elevation::MEDIUM.blur_radius);
        assert!(elevation::MEDIUM.blur_radius < elevation::HIGH.blur_radius);
        assert!(elevation::LOW.key_alpha <= elevation::MEDIUM.key_alpha);
        assert!(elevation::MEDIUM.key_alpha <= elevation::HIGH.key_alpha);
    }
}

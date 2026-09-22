//! Canonical shell design tokens shared by both surfaces (DUAL-15-14).
//!
//! The Iced desktop look is the product reference. This module carries the
//! numbers once: Iced builds its `view::theme` token constants directly from
//! [`skin_core`] and the spacing/radius/metric ladders below (compiler-bound),
//! while the business-agnostic Bevy widget layer — which by charter cannot
//! depend on this crate — mirrors the same numbers, enforced by the dual
//! headless test `the_widget_palette_mirrors_the_shared_design_tokens` and the
//! numeric source scan in `scripts/quality/multimodal-shell-guard.py`.
//!
//! The scope is deliberately the tokens both surfaces actually consume:
//! the core skin palette (canvas / surfaces / ink / accent / semantic colors),
//! the interaction palette every programmable surface exposes (overlay scrim,
//! hover/pressed washes, focus ring, disabled ink), the spacing ladder, the
//! corner radii and the hairline width. Surface-only decoration (icon-tile
//! tints, shadows, per-surface type scales, accent-tinted button derivations)
//! stays with its surface: shadows are not programmable on either toolkit.

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
    /// Dimmed ink (captions, secondary labels); carries its own alpha.
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
            ink_dim: RgbaToken::rgba(0.24, 0.28, 0.26, 0.65),
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
            disabled_ink: RgbaToken::rgba(0.88, 0.90, 0.92, 0.35),
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

/// Spacing ladder (logical pixels) both shells lay out with.
pub mod space {
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 8.0;
    pub const MD: f32 = 12.0;
    pub const LG: f32 = 16.0;
    pub const XL: f32 = 20.0;
    pub const XXL: f32 = 24.0;
}

/// Corner radius ladder (logical pixels).
pub mod radius {
    pub const CARD: f32 = 16.0;
    pub const CONTROL: f32 = 10.0;
}

/// Shared control metrics (logical pixels).
pub mod metrics {
    /// Hairline border width.
    pub const HAIRLINE: f32 = 1.0;
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
        const _: () = assert!(radius::CONTROL > 0.0);
        const _: () = assert!(radius::CARD > radius::CONTROL);
        const _: () = assert!(metrics::HAIRLINE > 0.0);
    }
}

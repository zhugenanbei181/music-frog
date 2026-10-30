//! Material 3 surface elevation hierarchy, 5-level containers, and paired semantic colors (FLPARITY-014, FLPARITY-044).
//!
//! Defines:
//! * `SurfaceRole`: fine-grained 8-step surface hierarchy matching FlClash v0.9.2 M3 specs;
//! * `SemanticContainerPair`: container and on-container pairs for Primary, Success, Warning, and Danger;
//! * `MaterialSurfaceTokens`: concrete RGBA tokens for all 4 skins (Dark, Light, Forest, Amoled);
//! * Mathematical WCAG AA (>= 4.5:1) contrast validation across all container pairs and step distinctiveness;
//! * FlClash v0.9.2 default color palette calibration and contrast verification.

use crate::color_science::{MaterialColorScheme, TonalPalette, wcag_contrast_ratio};
use crate::design_tokens::{RgbaToken, skin_core};
use crate::theme::{AppearanceMode, ThemeSkin};
use serde::{Deserialize, Serialize};

/// Fine-grained surface elevation and container roles matching Material Design 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceRole {
    /// Main window or application page backdrop.
    Canvas,
    /// Navigation rail or sidebar beside the canvas.
    Sidebar,
    /// Deepest recessed container (e.g. inverted track or search well).
    ContainerLowest,
    /// Low-elevation container (e.g. secondary group card).
    ContainerLow,
    /// Resting default card or panel fill.
    ContainerCard,
    /// Raised card on hover or high-priority inspection panel.
    ContainerHigh,
    /// Highest elevation surface container (e.g. active header or modal dialog card).
    ContainerHighest,
    /// Floating layer above the layout (menu, popover, contextual drawer).
    FloatingOverlay,
}

/// Container and on-container color pair for semantic status badges and action cards.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SemanticContainerPair {
    /// Background container tint fill.
    pub container: RgbaToken,
    /// High-contrast text and icon ink drawn inside the container.
    pub on_container: RgbaToken,
}

impl SemanticContainerPair {
    pub const fn new(container: RgbaToken, on_container: RgbaToken) -> Self {
        Self {
            container,
            on_container,
        }
    }

    pub fn contrast_ratio(&self) -> f32 {
        wcag_contrast_ratio(self.on_container, self.container)
    }

    pub fn meets_wcag_aa(&self) -> bool {
        self.contrast_ratio() >= 4.5
    }
}

/// Complete Material 3 surface and semantic container palette for one skin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialSurfaceTokens {
    pub canvas: RgbaToken,
    pub sidebar: RgbaToken,
    pub container_lowest: RgbaToken,
    pub container_low: RgbaToken,
    pub container_card: RgbaToken,
    pub container_high: RgbaToken,
    pub container_highest: RgbaToken,
    pub floating_overlay: RgbaToken,
    pub outline: RgbaToken,
    pub outline_variant: RgbaToken,
    pub text_primary: RgbaToken,
    pub text_secondary: RgbaToken,
    pub primary_pair: SemanticContainerPair,
    pub success_pair: SemanticContainerPair,
    pub warning_pair: SemanticContainerPair,
    pub danger_pair: SemanticContainerPair,
}

impl MaterialSurfaceTokens {
    /// Resolve concrete M3 tokens for any theme skin, calibrated against FlClash v0.9.2 benchmarks.
    pub const fn of(skin: ThemeSkin) -> Self {
        let core = skin_core(skin);
        match skin {
            ThemeSkin::Dark => Self {
                // FlClash v0.9.2 High-Contrast Dark: crisp deep base with clearly tiered containers
                canvas: core.canvas,
                sidebar: core.sidebar,
                container_lowest: RgbaToken::rgb(0.025, 0.030, 0.035), // #060809
                container_low: RgbaToken::rgb(0.080, 0.090, 0.105),    // #14171b
                container_card: core.card,
                container_high: RgbaToken::rgb(0.160, 0.180, 0.205), // #292e35
                container_highest: RgbaToken::rgb(0.210, 0.235, 0.265), // #363c44
                floating_overlay: RgbaToken::rgb(0.180, 0.205, 0.235), // #2e343c
                outline: RgbaToken::rgb(0.380, 0.420, 0.470),        // #616b78 (crisp boundary)
                outline_variant: RgbaToken::rgb(0.220, 0.250, 0.285), // #384049
                text_primary: core.ink,
                text_secondary: core.ink_dim,
                primary_pair: SemanticContainerPair::new(
                    RgbaToken::rgb(0.080, 0.220, 0.380),
                    RgbaToken::rgb(0.750, 0.880, 1.000),
                ),
                success_pair: SemanticContainerPair::new(
                    RgbaToken::rgb(0.050, 0.220, 0.120),
                    RgbaToken::rgb(0.550, 0.950, 0.700),
                ),
                warning_pair: SemanticContainerPair::new(
                    RgbaToken::rgb(0.280, 0.160, 0.000),
                    RgbaToken::rgb(1.000, 0.880, 0.650),
                ),
                danger_pair: SemanticContainerPair::new(
                    RgbaToken::rgb(0.300, 0.080, 0.080),
                    RgbaToken::rgb(1.000, 0.750, 0.720),
                ),
            },
            ThemeSkin::Light => Self {
                canvas: core.canvas,
                sidebar: core.sidebar,
                container_lowest: RgbaToken::rgb(1.000, 1.000, 1.000), // #ffffff
                container_low: RgbaToken::rgb(0.957, 0.965, 0.973),    // #f4f6f8
                container_card: core.card,
                container_high: RgbaToken::rgb(0.925, 0.933, 0.945), // #eceef1
                container_highest: RgbaToken::rgb(0.890, 0.902, 0.914), // #e3e6e9
                floating_overlay: RgbaToken::rgb(1.000, 1.000, 1.000), // #ffffff
                outline: RgbaToken::rgb(0.451, 0.467, 0.498),        // #73777f
                outline_variant: RgbaToken::rgb(0.769, 0.780, 0.812), // #c4c7cf
                text_primary: core.ink,
                text_secondary: core.ink_dim,
                primary_pair: SemanticContainerPair::new(
                    RgbaToken::rgb(0.835, 0.910, 1.000),
                    RgbaToken::rgb(0.000, 0.118, 0.239),
                ),
                success_pair: SemanticContainerPair::new(
                    RgbaToken::rgb(0.722, 0.961, 0.800),
                    RgbaToken::rgb(0.000, 0.129, 0.055),
                ),
                warning_pair: SemanticContainerPair::new(
                    RgbaToken::rgb(1.000, 0.871, 0.659),
                    RgbaToken::rgb(0.153, 0.098, 0.000),
                ),
                danger_pair: SemanticContainerPair::new(
                    RgbaToken::rgb(1.000, 0.855, 0.839),
                    RgbaToken::rgb(0.255, 0.000, 0.008),
                ),
            },
            ThemeSkin::Forest => Self {
                // The outer paper, rail, card, and ink roles come from skin_core;
                // only the M3 elevation steps are specialized here.
                canvas: core.canvas,
                sidebar: core.sidebar,
                container_lowest: RgbaToken::rgb(0.973, 0.984, 0.961), // #f8fbf5
                container_low: RgbaToken::rgb(0.910, 0.941, 0.898),    // #e8f0e5
                container_card: core.card,
                container_high: RgbaToken::rgb(0.878, 0.918, 0.851), // #e0ead9
                container_highest: RgbaToken::rgb(0.835, 0.886, 0.800), // #d5e2cc
                floating_overlay: RgbaToken::rgb(1.000, 1.000, 1.000), // #ffffff
                outline: RgbaToken::rgb(0.400, 0.520, 0.420),        // #66856b
                outline_variant: RgbaToken::rgb(0.757, 0.820, 0.745), // #c1d1be
                text_primary: core.ink,
                text_secondary: core.ink_dim,
                primary_pair: SemanticContainerPair::new(
                    RgbaToken::rgb(0.863, 0.933, 0.886), // #dceee2
                    RgbaToken::rgb(0.055, 0.243, 0.118), // #0e3e1e
                ),
                success_pair: SemanticContainerPair::new(
                    RgbaToken::rgb(0.847, 0.953, 0.863), // #d8f3dc
                    RgbaToken::rgb(0.043, 0.275, 0.098), // #0b4619
                ),
                warning_pair: SemanticContainerPair::new(
                    RgbaToken::rgb(1.000, 0.953, 0.804), // #fff3cd
                    RgbaToken::rgb(0.400, 0.302, 0.012), // #664d03
                ),
                danger_pair: SemanticContainerPair::new(
                    RgbaToken::rgb(0.973, 0.843, 0.855), // #f8d7da
                    RgbaToken::rgb(0.345, 0.082, 0.110), // #58151c
                ),
            },
            ThemeSkin::Amoled => Self {
                canvas: core.canvas,
                sidebar: core.sidebar,
                container_lowest: RgbaToken::rgb(0.000, 0.000, 0.000), // #000000
                container_low: RgbaToken::rgb(0.051, 0.067, 0.082),    // #0d1115
                container_card: core.card,
                container_high: RgbaToken::rgb(0.106, 0.133, 0.169), // #1b222b
                container_highest: RgbaToken::rgb(0.141, 0.176, 0.220), // #242d38
                floating_overlay: RgbaToken::rgb(0.094, 0.125, 0.161), // #182029
                outline: RgbaToken::rgb(0.250, 0.300, 0.360),        // #404c5c
                outline_variant: RgbaToken::rgb(0.140, 0.170, 0.210), // #242b36
                text_primary: core.ink,
                text_secondary: core.ink_dim,
                primary_pair: SemanticContainerPair::new(
                    RgbaToken::rgb(0.094, 0.239, 0.400),
                    RgbaToken::rgb(0.706, 0.843, 1.000),
                ),
                success_pair: SemanticContainerPair::new(
                    RgbaToken::rgb(0.055, 0.239, 0.141),
                    RgbaToken::rgb(0.486, 0.949, 0.659),
                ),
                warning_pair: SemanticContainerPair::new(
                    RgbaToken::rgb(0.271, 0.169, 0.000),
                    RgbaToken::rgb(1.000, 0.867, 0.702),
                ),
                danger_pair: SemanticContainerPair::new(
                    RgbaToken::rgb(0.286, 0.094, 0.094),
                    RgbaToken::rgb(1.000, 0.706, 0.671),
                ),
            },
        }
    }

    /// Resolve the surface and semantic roles for an HCT-generated scheme.
    pub fn from_scheme(scheme: &MaterialColorScheme, mode: AppearanceMode) -> Self {
        let (success_container_tone, success_ink_tone, warning_container_tone, warning_ink_tone) =
            match mode {
                AppearanceMode::Light => (90, 10, 90, 10),
                AppearanceMode::Dark => (30, 90, 30, 90),
            };
        let success = TonalPalette::of(142.0, 48.0);
        let warning = TonalPalette::of(82.0, 48.0);
        Self {
            canvas: scheme.background,
            sidebar: scheme.surface_container_low,
            container_lowest: scheme.surface_container_lowest,
            container_low: scheme.surface_container_low,
            container_card: scheme.surface_container,
            container_high: scheme.surface_container_high,
            container_highest: scheme.surface_container_highest,
            floating_overlay: scheme.surface_container_highest,
            outline: scheme.outline,
            outline_variant: scheme.outline_variant,
            text_primary: scheme.on_surface,
            text_secondary: scheme.on_surface_variant,
            primary_pair: SemanticContainerPair::new(
                scheme.primary_container,
                scheme.on_primary_container,
            ),
            success_pair: SemanticContainerPair::new(
                success.tone(success_container_tone),
                success.tone(success_ink_tone),
            ),
            warning_pair: SemanticContainerPair::new(
                warning.tone(warning_container_tone),
                warning.tone(warning_ink_tone),
            ),
            danger_pair: SemanticContainerPair::new(
                scheme.error_container,
                scheme.on_error_container,
            ),
        }
    }

    /// Lookup color by SurfaceRole.
    pub fn role(&self, role: SurfaceRole) -> RgbaToken {
        match role {
            SurfaceRole::Canvas => self.canvas,
            SurfaceRole::Sidebar => self.sidebar,
            SurfaceRole::ContainerLowest => self.container_lowest,
            SurfaceRole::ContainerLow => self.container_low,
            SurfaceRole::ContainerCard => self.container_card,
            SurfaceRole::ContainerHigh => self.container_high,
            SurfaceRole::ContainerHighest => self.container_highest,
            SurfaceRole::FloatingOverlay => self.floating_overlay,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_semantic_container_pairs_meet_wcag_aa_contrast() {
        for skin in ThemeSkin::ALL {
            let tokens = MaterialSurfaceTokens::of(skin);
            assert!(
                tokens.primary_pair.meets_wcag_aa(),
                "{skin:?} primary pair failed contrast: {}",
                tokens.primary_pair.contrast_ratio()
            );
            assert!(
                tokens.success_pair.meets_wcag_aa(),
                "{skin:?} success pair failed contrast: {}",
                tokens.success_pair.contrast_ratio()
            );
            assert!(
                tokens.warning_pair.meets_wcag_aa(),
                "{skin:?} warning pair failed contrast: {}",
                tokens.warning_pair.contrast_ratio()
            );
            assert!(
                tokens.danger_pair.meets_wcag_aa(),
                "{skin:?} danger pair failed contrast: {}",
                tokens.danger_pair.contrast_ratio()
            );
        }
    }

    #[test]
    fn dark_mode_containers_have_clear_distinguishable_elevation() {
        let dark = MaterialSurfaceTokens::of(ThemeSkin::Dark);
        // Card should clearly contrast against Canvas
        let card_canvas_ratio = wcag_contrast_ratio(dark.container_card, dark.canvas);
        assert!(
            card_canvas_ratio >= 1.20,
            "card vs canvas ratio: {card_canvas_ratio}"
        );

        // High container contrasts against Card
        let high_card_ratio = wcag_contrast_ratio(dark.container_high, dark.container_card);
        assert!(
            high_card_ratio >= 1.15,
            "high vs card ratio: {high_card_ratio}"
        );

        // Highest container contrasts against High
        let highest_high_ratio = wcag_contrast_ratio(dark.container_highest, dark.container_high);
        assert!(
            highest_high_ratio >= 1.15,
            "highest vs high ratio: {highest_high_ratio}"
        );
    }

    #[test]
    fn flclash_v092_calibrated_color_benchmark_meets_wcag_and_elevation_spec() {
        for skin in ThemeSkin::ALL {
            let tokens = MaterialSurfaceTokens::of(skin);

            // Primary text on card must meet WCAG AA (>= 4.5:1)
            let text_card_contrast =
                wcag_contrast_ratio(tokens.text_primary, tokens.container_card);
            assert!(
                text_card_contrast >= 4.5,
                "{skin:?} text_primary on card contrast was {text_card_contrast}"
            );

            // Secondary text on card must meet readable floor (>= 4.5:1 in M3)
            let text_sec_contrast =
                wcag_contrast_ratio(tokens.text_secondary, tokens.container_card);
            assert!(
                text_sec_contrast >= 4.5,
                "{skin:?} text_secondary on card contrast was {text_sec_contrast}"
            );

            // Outline boundary must be non-zero distinct against card
            let outline_contrast = wcag_contrast_ratio(tokens.outline, tokens.container_card);
            assert!(
                outline_contrast >= 1.5,
                "{skin:?} outline boundary contrast was {outline_contrast}"
            );
        }
    }

    #[test]
    fn all_skin_core_roles_are_the_material_surface_source() {
        for skin in ThemeSkin::ALL {
            let core = skin_core(skin);
            let m3 = MaterialSurfaceTokens::of(skin);
            assert_eq!(core.canvas, m3.canvas, "{skin:?} canvas drifted");
            assert_eq!(core.sidebar, m3.sidebar, "{skin:?} sidebar drifted");
            assert_eq!(core.card, m3.container_card, "{skin:?} card drifted");
            assert_eq!(core.ink, m3.text_primary, "{skin:?} primary ink drifted");
            assert_eq!(
                core.ink_dim, m3.text_secondary,
                "{skin:?} secondary ink drifted"
            );
            assert!(
                wcag_contrast_ratio(core.ink, core.card) >= 4.5,
                "{skin:?} primary ink does not meet WCAG AA"
            );
            assert!(
                wcag_contrast_ratio(core.ink_dim, core.card) >= 4.5,
                "{skin:?} secondary ink does not meet WCAG AA"
            );
        }
    }

    #[test]
    fn generated_surface_roles_meet_contrast_floors_in_both_modes() {
        use crate::color_science::{HctColor, MaterialCorePalette};

        let core = MaterialCorePalette::of_seed(HctColor::from_rgb(0.12, 0.56, 0.96));
        for (mode, scheme) in [
            (
                AppearanceMode::Light,
                MaterialColorScheme::light_from_core(&core),
            ),
            (
                AppearanceMode::Dark,
                MaterialColorScheme::dark_high_contrast_from_core(&core),
            ),
        ] {
            let tokens = MaterialSurfaceTokens::from_scheme(&scheme, mode);
            assert!(wcag_contrast_ratio(tokens.text_primary, tokens.container_card) >= 4.5);
            assert!(wcag_contrast_ratio(tokens.text_secondary, tokens.container_card) >= 4.5);
            assert!(wcag_contrast_ratio(tokens.outline, tokens.container_card) >= 1.5);
            assert!(tokens.primary_pair.meets_wcag_aa());
            assert!(tokens.success_pair.meets_wcag_aa());
            assert!(tokens.warning_pair.meets_wcag_aa());
            assert!(tokens.danger_pair.meets_wcag_aa());
        }
    }
}

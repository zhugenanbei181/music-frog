//! The resolved, toolkit-neutral color roles for a paintable appearance.
//!
//! Surface implementations consume this value as an adapter input. The
//! component palette functions remain implementation details of this owner;
//! renderers must not combine them into a second color source.

use crate::color_science::{HctColor, MaterialColorScheme};
use crate::design_tokens::palette::{SurfaceStep, surface_ladder};
use crate::design_tokens::{
    MobileThemeTokens, RgbaToken, SkinCorePalette, SkinInteractionPalette, mobile_theme_tokens,
    skin_core, skin_interaction,
};
use crate::palette_preference::{PaletteStylePreference, derive_core_palette};
use crate::surface_hierarchy::{MaterialSurfaceTokens, SemanticContainerPair};
use crate::theme::{AppearanceMode, SystemAppearance, ThemePreference, ThemeSkin};
use serde::{Deserialize, Serialize};
use zhuge_ui_tokens::WCAG_AA_NORMAL;
use zhuge_ui_tokens::surface;

fn contrast_ratio(first: RgbaToken, second: RgbaToken) -> f32 {
    surface::contrast_ratio(first.as_rgba(), second.as_rgba())
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum ThemePaletteSource {
    BuiltIn(ThemeSkin),
    Seeded {
        seed: DynamicColorSeed,
        appearance: AppearanceMode,
    },
}

/// Validated sRGB seed supplied by a host dynamic-color provider or user.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct DynamicColorSeed(RgbaToken);

/// The operating system's dynamic accent color observation. Unobserved is a
/// real observation state: hosts without a dynamic-color API report it instead
/// of pretending to hold a seed.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub enum SystemDynamicColor {
    /// The host has not reported a system accent seed.
    #[default]
    Unobserved,
    /// A current validated sRGB accent seed from the host.
    Seed(DynamicColorSeed),
}

/// One typed observation of the host's theme state. Both fields carry their
/// own unobserved state, so a host adapter never has to fake a value it
/// cannot read.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct SystemThemeObservation {
    pub appearance: SystemAppearance,
    pub dynamic_color: SystemDynamicColor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DynamicColorSeedError {
    NonFiniteChannel,
    ChannelOutOfRange,
}

impl DynamicColorSeed {
    pub fn new(r: f32, g: f32, b: f32) -> Result<Self, DynamicColorSeedError> {
        if !r.is_finite() || !g.is_finite() || !b.is_finite() {
            return Err(DynamicColorSeedError::NonFiniteChannel);
        }
        if !(0.0..=1.0).contains(&r) || !(0.0..=1.0).contains(&g) || !(0.0..=1.0).contains(&b) {
            return Err(DynamicColorSeedError::ChannelOutOfRange);
        }
        Ok(Self(RgbaToken::rgb(r, g, b)))
    }

    pub fn from_u32_srgb(rgb: u32) -> Self {
        let r = ((rgb >> 16) & 0xFF) as f32 / 255.0;
        let g = ((rgb >> 8) & 0xFF) as f32 / 255.0;
        let b = (rgb & 0xFF) as f32 / 255.0;
        Self(RgbaToken::rgb(r, g, b))
    }

    pub const fn color(self) -> RgbaToken {
        self.0
    }

    fn hct(self) -> HctColor {
        HctColor::from_rgb(self.0.r, self.0.g, self.0.b)
    }
}

/// Complete semantic palette after theme preference resolution.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct ThemePalette {
    pub source: ThemePaletteSource,
    pub core: SkinCorePalette,
    pub interaction: SkinInteractionPalette,
    pub mobile: MobileThemeTokens,
    pub surfaces: MaterialSurfaceTokens,
    pub surface_ladder: [RgbaToken; 8],
}

impl ThemePalette {
    pub const fn of_skin(skin: ThemeSkin) -> Self {
        Self {
            source: ThemePaletteSource::BuiltIn(skin),
            core: skin_core(skin),
            interaction: skin_interaction(skin),
            mobile: mobile_theme_tokens(skin),
            surfaces: MaterialSurfaceTokens::of(skin),
            surface_ladder: surface_ladder(skin),
        }
    }

    /// Derive a Monet-style palette for a seed, style, and an explicit light/dark mode.
    pub fn from_seed_and_style(
        seed: DynamicColorSeed,
        style: PaletteStylePreference,
        appearance: AppearanceMode,
    ) -> Self {
        let core_palette = derive_core_palette(seed.hct(), style);
        let scheme = match appearance {
            AppearanceMode::Light => MaterialColorScheme::light_from_core(&core_palette),
            AppearanceMode::Dark => {
                MaterialColorScheme::dark_high_contrast_from_core(&core_palette)
            }
        };
        let surfaces = MaterialSurfaceTokens::from_scheme(&scheme, appearance);
        let core = SkinCorePalette {
            canvas: surfaces.canvas,
            sidebar: surfaces.sidebar,
            card: surfaces.container_card,
            card_border: surfaces.outline,
            control_bg: surfaces.container_low,
            ink: surfaces.text_primary,
            ink_dim: surfaces.text_secondary,
            accent: scheme.primary,
            on_accent: scheme.on_primary,
            success: surfaces.success_pair.on_container,
            warning: surfaces.warning_pair.on_container,
            danger: surfaces.danger_pair.on_container,
        };
        let skin = match appearance {
            AppearanceMode::Light => ThemeSkin::Light,
            AppearanceMode::Dark => ThemeSkin::Dark,
        };
        let mut interaction = skin_interaction(skin);
        interaction.focus_ring = scheme.primary;
        let accent = scheme.primary;
        let mobile = MobileThemeTokens {
            bottom_nav_bg: surfaces.sidebar,
            bottom_nav_indicator: RgbaToken::rgba(accent.r, accent.g, accent.b, 0.22),
            touch_ripple: interaction.pressed,
            swipe_action: accent,
            swipe_destructive: surfaces.danger_pair.on_container,
        };
        let surface_ladder = match appearance {
            AppearanceMode::Light => [
                surfaces.canvas,
                surfaces.sidebar,
                surfaces.container_card,
                surfaces.container_high,
                surfaces.container_low,
                surfaces.container_highest,
                surfaces.floating_overlay,
                surfaces.container_lowest,
            ],
            AppearanceMode::Dark => [
                surfaces.canvas,
                surfaces.sidebar,
                surfaces.container_card,
                surfaces.container_high,
                surfaces.container_low,
                surfaces.container_lowest,
                surfaces.floating_overlay,
                surfaces.container_highest,
            ],
        };

        Self {
            source: ThemePaletteSource::Seeded { seed, appearance },
            core,
            interaction,
            mobile,
            surfaces,
            surface_ladder,
        }
    }

    /// Assemble the full runtime palette for a seed and an explicit light/dark mode.
    pub fn from_seed(seed: DynamicColorSeed, appearance: AppearanceMode) -> Self {
        Self::from_seed_and_style(seed, PaletteStylePreference::TonalSpot, appearance)
    }

    pub fn meets_text_contrast(&self) -> bool {
        contrast_ratio(self.surfaces.text_primary, self.surfaces.container_card) >= WCAG_AA_NORMAL
            && contrast_ratio(self.surfaces.text_secondary, self.surfaces.container_card)
                >= WCAG_AA_NORMAL
    }

    pub fn semantic_pairs(&self) -> [SemanticContainerPair; 4] {
        [
            self.surfaces.primary_pair,
            self.surfaces.success_pair,
            self.surfaces.warning_pair,
            self.surfaces.danger_pair,
        ]
    }

    pub const fn surface(&self, step: SurfaceStep) -> RgbaToken {
        self.surface_ladder[step.index()]
    }
}

/// Authority readback tying the user's theme preference to its resolved skin
/// and exact palette. OS unobserved is retained; the documented cold-start
/// policy still resolves it through `SystemAppearance::mode()`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ResolvedThemePalette {
    pub preference: ThemePreference,
    pub system_appearance: SystemAppearance,
    pub system_dynamic_color: SystemDynamicColor,
    pub skin: ThemeSkin,
    pub palette: ThemePalette,
}

impl ResolvedThemePalette {
    pub fn resolve_with_style(
        preference: ThemePreference,
        system_appearance: SystemAppearance,
        system_dynamic_color: SystemDynamicColor,
        style: PaletteStylePreference,
    ) -> Self {
        let skin = preference.resolve(system_appearance);
        let palette = match (preference, system_dynamic_color) {
            (ThemePreference::System, SystemDynamicColor::Seed(seed)) => {
                ThemePalette::from_seed_and_style(seed, style, system_appearance.mode())
            }
            _ => ThemePalette::of_skin(skin),
        };
        Self {
            preference,
            system_appearance,
            system_dynamic_color,
            skin,
            palette,
        }
    }

    pub fn resolve(
        preference: ThemePreference,
        system_appearance: SystemAppearance,
        system_dynamic_color: SystemDynamicColor,
    ) -> Self {
        Self::resolve_with_style(
            preference,
            system_appearance,
            system_dynamic_color,
            PaletteStylePreference::TonalSpot,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_palette_assembles_each_role_from_contract_owners() {
        for skin in ThemeSkin::ALL {
            let palette = ThemePalette::of_skin(skin);
            assert_eq!(palette.core, skin_core(skin));
            assert_eq!(palette.interaction, skin_interaction(skin));
            assert_eq!(palette.mobile, mobile_theme_tokens(skin));
            assert_eq!(palette.surfaces, MaterialSurfaceTokens::of(skin));
            assert_eq!(palette.surface_ladder, surface_ladder(skin));
            assert_eq!(palette.surface(SurfaceStep::Canvas), palette.core.canvas);
            assert_eq!(palette.surface(SurfaceStep::Rail), palette.core.sidebar);
            assert_eq!(palette.surface(SurfaceStep::Card), palette.core.card);
            assert_eq!(palette.surfaces.text_primary, palette.core.ink);
            assert_eq!(palette.surfaces.text_secondary, palette.core.ink_dim);
            assert!(palette.meets_text_contrast(), "{skin:?} text contrast");
            assert!(
                palette
                    .semantic_pairs()
                    .iter()
                    .all(|pair| pair.meets_wcag_aa())
            );
        }
    }

    #[test]
    fn seeded_palettes_keep_one_seed_and_meet_contrast_in_both_modes() {
        let seed = DynamicColorSeed::new(0.12, 0.56, 0.96).expect("valid sRGB seed");
        for appearance in [AppearanceMode::Light, AppearanceMode::Dark] {
            let palette = ThemePalette::from_seed(seed, appearance);
            assert_eq!(
                palette.source,
                ThemePaletteSource::Seeded { seed, appearance }
            );
            assert!(palette.meets_text_contrast());
            assert!(
                palette
                    .semantic_pairs()
                    .iter()
                    .all(|pair| pair.meets_wcag_aa())
            );
            assert!(palette.core.accent.a > 0.0);
            assert_eq!(
                palette.surface(SurfaceStep::Canvas),
                palette.surfaces.canvas
            );
            assert_eq!(palette.surface(SurfaceStep::Rail), palette.surfaces.sidebar);
            assert_eq!(
                palette.surface(SurfaceStep::Card),
                palette.surfaces.container_card
            );
        }
    }

    #[test]
    fn preference_readback_preserves_os_observation_and_resolves_one_skin() {
        let system = ResolvedThemePalette::resolve(
            ThemePreference::System,
            SystemAppearance::Observed(AppearanceMode::Light),
            SystemDynamicColor::Unobserved,
        );
        assert_eq!(system.skin, ThemeSkin::Light);
        assert_eq!(
            system.palette.source,
            ThemePaletteSource::BuiltIn(ThemeSkin::Light)
        );
        assert_eq!(
            system.system_appearance,
            SystemAppearance::Observed(AppearanceMode::Light)
        );

        let cold_start = ResolvedThemePalette::resolve(
            ThemePreference::System,
            SystemAppearance::Unobserved,
            SystemDynamicColor::Unobserved,
        );
        assert_eq!(cold_start.skin, ThemeSkin::Dark);
        assert_eq!(cold_start.system_appearance, SystemAppearance::Unobserved);

        let fixed = ResolvedThemePalette::resolve(
            ThemePreference::Fixed(ThemeSkin::Forest),
            SystemAppearance::Observed(AppearanceMode::Dark),
            SystemDynamicColor::Unobserved,
        );
        assert_eq!(fixed.skin, ThemeSkin::Forest);
    }

    #[test]
    fn system_dynamic_color_is_used_only_when_preference_follows_the_os() {
        let seed = DynamicColorSeed::new(0.12, 0.56, 0.96).expect("valid sRGB seed");
        let system = ResolvedThemePalette::resolve(
            ThemePreference::System,
            SystemAppearance::Observed(AppearanceMode::Dark),
            SystemDynamicColor::Seed(seed),
        );
        assert_eq!(system.skin, ThemeSkin::Dark);
        assert_eq!(
            system.palette.source,
            ThemePaletteSource::Seeded {
                seed,
                appearance: AppearanceMode::Dark
            }
        );

        let fixed = ResolvedThemePalette::resolve(
            ThemePreference::Fixed(ThemeSkin::Forest),
            SystemAppearance::Observed(AppearanceMode::Dark),
            SystemDynamicColor::Seed(seed),
        );
        assert_eq!(fixed.skin, ThemeSkin::Forest);
        assert_eq!(
            fixed.palette.source,
            ThemePaletteSource::BuiltIn(ThemeSkin::Forest)
        );
    }

    #[test]
    fn resolved_readback_reports_the_dynamic_color_observation_itself() {
        let seed = DynamicColorSeed::new(0.12, 0.56, 0.96).expect("valid sRGB seed");
        let observed = ResolvedThemePalette::resolve(
            ThemePreference::System,
            SystemAppearance::Observed(AppearanceMode::Light),
            SystemDynamicColor::Seed(seed),
        );
        assert_eq!(
            observed.system_dynamic_color,
            SystemDynamicColor::Seed(seed)
        );

        let unobserved = ResolvedThemePalette::resolve(
            ThemePreference::System,
            SystemAppearance::Unobserved,
            SystemDynamicColor::Unobserved,
        );
        assert_eq!(
            unobserved.system_dynamic_color,
            SystemDynamicColor::Unobserved
        );
        assert_eq!(
            unobserved.palette.source,
            ThemePaletteSource::BuiltIn(ThemeSkin::Dark)
        );
    }

    #[test]
    fn dynamic_color_seed_rejects_invalid_srgb_channels() {
        assert_eq!(
            DynamicColorSeed::new(f32::NAN, 0.0, 0.0),
            Err(DynamicColorSeedError::NonFiniteChannel)
        );
        assert_eq!(
            DynamicColorSeed::new(0.0, 1.01, 0.0),
            Err(DynamicColorSeedError::ChannelOutOfRange)
        );
    }
}

//! The resolved, toolkit-neutral color roles for a paintable appearance.
//!
//! Surface implementations consume this value as an adapter input. The
//! component palette functions remain implementation details of this owner;
//! renderers must not combine them into a second color source.

use crate::color_science::{
    HctColor, MaterialColorScheme, MaterialCorePalette, wcag_contrast_ratio,
};
use crate::design_tokens::palette::{SurfaceStep, surface_ladder};
use crate::design_tokens::{
    MobileThemeTokens, RgbaToken, SkinCorePalette, SkinInteractionPalette, mobile_theme_tokens,
    skin_core, skin_interaction,
};
use crate::surface_hierarchy::{MaterialSurfaceTokens, SemanticContainerPair};
use crate::theme::{AppearanceMode, SystemAppearance, ThemePreference, ThemeSkin};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ThemePaletteSource {
    BuiltIn(ThemeSkin),
    Seeded {
        seed: DynamicColorSeed,
        appearance: AppearanceMode,
    },
}

/// Validated sRGB seed supplied by a host dynamic-color provider or user.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DynamicColorSeed(RgbaToken);

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

    pub const fn color(self) -> RgbaToken {
        self.0
    }

    fn hct(self) -> HctColor {
        HctColor::from_rgb(self.0.r, self.0.g, self.0.b)
    }
}

/// Complete semantic palette after theme preference resolution.
#[derive(Clone, Copy, Debug, PartialEq)]
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

    /// Derive a Monet-style palette for a seed and an explicit light/dark mode.
    pub fn from_seed(seed: DynamicColorSeed, appearance: AppearanceMode) -> Self {
        let core_palette = MaterialCorePalette::of_seed(seed.hct());
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

    pub fn meets_text_contrast(&self) -> bool {
        wcag_contrast_ratio(self.surfaces.text_primary, self.surfaces.container_card) >= 4.5
            && wcag_contrast_ratio(self.surfaces.text_secondary, self.surfaces.container_card)
                >= 4.5
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
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedThemePalette {
    pub preference: ThemePreference,
    pub system_appearance: SystemAppearance,
    pub skin: ThemeSkin,
    pub palette: ThemePalette,
}

impl ResolvedThemePalette {
    pub fn resolve(
        preference: ThemePreference,
        system_appearance: SystemAppearance,
        system_dynamic_seed: Option<DynamicColorSeed>,
    ) -> Self {
        let skin = preference.resolve(system_appearance);
        let palette = match (preference, system_dynamic_seed) {
            (ThemePreference::System, Some(seed)) => {
                ThemePalette::from_seed(seed, system_appearance.mode())
            }
            _ => ThemePalette::of_skin(skin),
        };
        Self {
            preference,
            system_appearance,
            skin,
            palette,
        }
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
            None,
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
            None,
        );
        assert_eq!(cold_start.skin, ThemeSkin::Dark);
        assert_eq!(cold_start.system_appearance, SystemAppearance::Unobserved);

        let fixed = ResolvedThemePalette::resolve(
            ThemePreference::Fixed(ThemeSkin::Forest),
            SystemAppearance::Observed(AppearanceMode::Dark),
            None,
        );
        assert_eq!(fixed.skin, ThemeSkin::Forest);
    }

    #[test]
    fn system_dynamic_color_is_used_only_when_preference_follows_the_os() {
        let seed = DynamicColorSeed::new(0.12, 0.56, 0.96).expect("valid sRGB seed");
        let system = ResolvedThemePalette::resolve(
            ThemePreference::System,
            SystemAppearance::Observed(AppearanceMode::Dark),
            Some(seed),
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
            Some(seed),
        );
        assert_eq!(fixed.skin, ThemeSkin::Forest);
        assert_eq!(
            fixed.palette.source,
            ThemePaletteSource::BuiltIn(ThemeSkin::Forest)
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

//! Single projection of the host theme facts onto the authority readback.
//!
//! Every surface consumes this projection instead of resolving the palette
//! from its own local preference mirror, so one owner answers what the user
//! sees for a given preference and host observation.

use infiltrator_contract::palette_preference::{PalettePreferenceSnapshot, SeedColorSource};
use infiltrator_contract::theme::ThemePreference;
use infiltrator_contract::theme_palette::{
    DynamicColorSeed, ResolvedThemePalette, SystemDynamicColor, SystemThemeObservation,
    ThemePalette,
};

/// Resolve the one paintable readback for the user's theme preference, host
/// observation, and palette preference (custom seed color and style).
pub fn project_theme_readback_with_palette(
    preference: ThemePreference,
    observation: SystemThemeObservation,
    palette_preference: PalettePreferenceSnapshot,
) -> ResolvedThemePalette {
    match palette_preference.seed {
        SeedColorSource::Custom { rgb } => {
            let seed = DynamicColorSeed::from_u32_srgb(rgb);
            let dynamic_color = SystemDynamicColor::Seed(seed);
            let skin = preference.resolve(observation.appearance);
            let palette = ThemePalette::from_seed_and_style(
                seed,
                palette_preference.style,
                observation.appearance.mode(),
            );
            ResolvedThemePalette {
                preference,
                system_appearance: observation.appearance,
                system_dynamic_color: dynamic_color,
                skin,
                palette,
            }
        }
        SeedColorSource::FollowTheme => ResolvedThemePalette::resolve_with_style(
            preference,
            observation.appearance,
            observation.dynamic_color,
            palette_preference.style,
        ),
    }
}

/// Resolve the one paintable readback for the user's preference and the
/// host's latest observation with default palette preference.
pub fn project_theme_readback(
    preference: ThemePreference,
    observation: SystemThemeObservation,
) -> ResolvedThemePalette {
    project_theme_readback_with_palette(
        preference,
        observation,
        PalettePreferenceSnapshot::default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::palette_preference::PaletteStylePreference;
    use infiltrator_contract::theme::{AppearanceMode, SystemAppearance, ThemeSkin};
    use infiltrator_contract::theme_palette::{
        DynamicColorSeed, SystemDynamicColor, ThemePaletteSource,
    };

    #[test]
    fn system_preference_consumes_the_reported_seed_palette() {
        let seed = DynamicColorSeed::new(0.12, 0.56, 0.96).expect("valid sRGB seed");
        let readback = project_theme_readback(
            ThemePreference::System,
            SystemThemeObservation {
                appearance: SystemAppearance::Observed(AppearanceMode::Dark),
                dynamic_color: SystemDynamicColor::Seed(seed),
            },
        );
        assert_eq!(readback.skin, ThemeSkin::Dark);
        assert_eq!(
            readback.system_dynamic_color,
            SystemDynamicColor::Seed(seed)
        );
        assert_eq!(
            readback.palette.source,
            ThemePaletteSource::Seeded {
                seed,
                appearance: AppearanceMode::Dark
            }
        );
    }

    #[test]
    fn unobserved_host_facts_fall_back_to_the_built_in_skin() {
        let readback = project_theme_readback(ThemePreference::System, Default::default());
        assert_eq!(readback.skin, ThemeSkin::Dark);
        assert_eq!(
            readback.palette.source,
            ThemePaletteSource::BuiltIn(ThemeSkin::Dark)
        );
    }

    #[test]
    fn fixed_preference_ignores_the_host_seed() {
        let seed = DynamicColorSeed::new(0.9, 0.4, 0.1).expect("valid sRGB seed");
        let readback = project_theme_readback(
            ThemePreference::Fixed(ThemeSkin::Amoled),
            SystemThemeObservation {
                appearance: SystemAppearance::Observed(AppearanceMode::Light),
                dynamic_color: SystemDynamicColor::Seed(seed),
            },
        );
        assert_eq!(readback.skin, ThemeSkin::Amoled);
        assert_eq!(
            readback.palette.source,
            ThemePaletteSource::BuiltIn(ThemeSkin::Amoled)
        );
    }

    #[test]
    fn custom_seed_and_style_drives_resolved_theme_readback() {
        let custom_snapshot = PalettePreferenceSnapshot {
            seed: SeedColorSource::Custom { rgb: 0x33691E },
            style: PaletteStylePreference::Vibrant,
            revision: 1,
            has_unsaved_draft: false,
            last_saved_epoch_ms: 100,
        };
        let readback = project_theme_readback_with_palette(
            ThemePreference::Fixed(ThemeSkin::Dark),
            SystemThemeObservation {
                appearance: SystemAppearance::Observed(AppearanceMode::Dark),
                dynamic_color: SystemDynamicColor::Unobserved,
            },
            custom_snapshot,
        );
        let expected_seed = DynamicColorSeed::from_u32_srgb(0x33691E);
        assert_eq!(
            readback.system_dynamic_color,
            SystemDynamicColor::Seed(expected_seed)
        );
        assert_eq!(
            readback.palette.source,
            ThemePaletteSource::Seeded {
                seed: expected_seed,
                appearance: AppearanceMode::Dark,
            }
        );
    }
}

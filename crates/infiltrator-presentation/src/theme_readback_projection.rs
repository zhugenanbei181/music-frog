//! Single projection of the host theme facts onto the authority readback.
//!
//! Every surface consumes this projection instead of resolving the palette
//! from its own local preference mirror, so one owner answers what the user
//! sees for a given preference and host observation.

use infiltrator_contract::theme::ThemePreference;
use infiltrator_contract::theme_palette::{ResolvedThemePalette, SystemThemeObservation};

/// Resolve the one paintable readback for the user's preference and the
/// host's latest observation. The contract owner keeps the priority rule;
/// this projection only funnels the facts into it.
pub fn project_theme_readback(
    preference: ThemePreference,
    observation: SystemThemeObservation,
) -> ResolvedThemePalette {
    ResolvedThemePalette::resolve(
        preference,
        observation.appearance,
        observation.dynamic_color,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
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
}

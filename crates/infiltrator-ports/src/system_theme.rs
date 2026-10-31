//! Outbound port for the host's appearance and dynamic accent observations.

use infiltrator_contract::theme_palette::SystemThemeObservation;

/// Port the host implements to publish the current system theme observation.
pub trait SystemThemeObservationPort: Send + Sync {
    fn observe(&self) -> SystemThemeObservation;
}

/// Honest fallback adapter for hosts that expose no theme observation API.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnobservedSystemTheme;

impl SystemThemeObservationPort for UnobservedSystemTheme {
    fn observe(&self) -> SystemThemeObservation {
        SystemThemeObservation::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::theme::{AppearanceMode, SystemAppearance};
    use infiltrator_contract::theme_palette::{DynamicColorSeed, SystemDynamicColor};

    #[test]
    fn unobserved_adapter_reports_both_facts_as_unobserved() {
        let observation = UnobservedSystemTheme.observe();
        assert_eq!(observation.appearance, SystemAppearance::Unobserved);
        assert_eq!(observation.dynamic_color, SystemDynamicColor::Unobserved);
    }

    #[test]
    fn observation_carries_each_reported_fact_without_faking_the_other() {
        let seed = DynamicColorSeed::new(0.12, 0.56, 0.96).expect("valid sRGB seed");
        let observation = SystemThemeObservation {
            appearance: SystemAppearance::Observed(AppearanceMode::Light),
            dynamic_color: SystemDynamicColor::Seed(seed),
        };
        assert_eq!(observation.appearance.mode(), AppearanceMode::Light);
        assert_eq!(observation.dynamic_color, SystemDynamicColor::Seed(seed));
    }
}

use super::*;

impl ProductSurfaceReader {
    /// One authoritative theme readback per snapshot: the stored preference
    /// plus the host observation (unobserved unless a host adapter is injected).
    pub(super) fn theme_readback(
        &self,
        hydrated_settings: Option<&Result<AppSettings, Failure>>,
    ) -> ResolvedThemePalette {
        let preference = hydrated_settings
            .and_then(|result| result.as_ref().ok())
            .map(|settings| ThemePreference::from_setting(&settings.theme))
            .unwrap_or_default();
        let observation = self
            .system_theme
            .as_ref()
            .map(|port| port.observe())
            .unwrap_or_default();
        let palette = self
            .palette_preference
            .as_ref()
            .map(PalettePreferenceApplication::snapshot)
            .unwrap_or_default();
        project_theme_readback_with_palette(preference, observation, palette)
    }
}

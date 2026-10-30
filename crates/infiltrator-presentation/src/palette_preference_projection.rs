//! Presentation projections for the user-owned palette preference (FLPARITY-036).
//!
//! Every surface consumes the same scheme derivation: one seed source plus
//! one style collapses into one `MaterialColorScheme` per mode. The theme
//! path (`SeedColorSource::FollowTheme`) stays with the existing theme owner
//! and projects to `None`, so no surface invents a seed.

use infiltrator_contract::color_science::MaterialColorScheme;
use infiltrator_contract::palette_preference::PalettePreferenceSnapshot;
use infiltrator_contract::palette_preference::PaletteStylePreference;
use infiltrator_contract::palette_preference::SeedColorSource;
use infiltrator_contract::palette_preference::derive_core_palette;

/// The light/dark mode a scheme is generated for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaletteMode {
    Light,
    Dark,
}

/// One selectable palette style row for the preference page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteStyleOption {
    pub style: PaletteStylePreference,
    pub label: String,
    pub is_selected: bool,
}

/// The presentation model of the palette preference page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PalettePreferencePageProjection {
    pub seed_label: String,
    pub styles: Vec<PaletteStyleOption>,
    pub has_unsaved_draft: bool,
    pub revision: u64,
}

/// Project the saved preference into the page model.
pub fn project_palette_preference(
    snapshot: &PalettePreferenceSnapshot,
) -> PalettePreferencePageProjection {
    let seed_label = match snapshot.seed {
        SeedColorSource::FollowTheme => "跟随主题".to_owned(),
        SeedColorSource::Custom { rgb } => format!("#{rgb:06X}"),
    };
    let styles = PaletteStylePreference::ALL
        .iter()
        .map(|style| PaletteStyleOption {
            style: *style,
            label: style.label_zh().to_owned(),
            is_selected: *style == snapshot.style,
        })
        .collect();
    PalettePreferencePageProjection {
        seed_label,
        styles,
        has_unsaved_draft: snapshot.has_unsaved_draft,
        revision: snapshot.revision,
    }
}

/// Derive the single authoritative `MaterialColorScheme` for a mode.
///
/// `FollowTheme` projects to `None`: the existing theme owner keeps the
/// palette path until the shared-theme wiring consumes the preference.
pub fn project_palette_scheme(
    seed: SeedColorSource,
    style: PaletteStylePreference,
    mode: PaletteMode,
) -> Option<MaterialColorScheme> {
    let hct = seed.hct()?;
    let core = derive_core_palette(hct, style);
    Some(match mode {
        PaletteMode::Light => MaterialColorScheme::light_from_core(&core),
        PaletteMode::Dark => MaterialColorScheme::dark_from_core(&core),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follow_theme_projects_to_none() {
        assert_eq!(
            project_palette_scheme(
                SeedColorSource::FollowTheme,
                PaletteStylePreference::TonalSpot,
                PaletteMode::Dark,
            ),
            None
        );
    }

    #[test]
    fn custom_seed_produces_light_and_dark_schemes() {
        let seed = SeedColorSource::Custom { rgb: 0x33691E };
        let light =
            project_palette_scheme(seed, PaletteStylePreference::Vibrant, PaletteMode::Light)
                .expect("a custom seed must project a scheme");
        let dark = project_palette_scheme(seed, PaletteStylePreference::Vibrant, PaletteMode::Dark)
            .expect("a custom seed must project a scheme");
        assert_ne!(
            light.primary, dark.primary,
            "light and dark primary tokens live on different tones"
        );
        assert_ne!(
            light.background, dark.background,
            "light and dark modes must differ in surfaces"
        );
    }

    #[test]
    fn page_projection_labels_seed_and_marks_selected_style() {
        let snapshot = PalettePreferenceSnapshot {
            seed: SeedColorSource::Custom { rgb: 0x1976D2 },
            style: PaletteStylePreference::FruitSalad,
            revision: 2,
            has_unsaved_draft: true,
            last_saved_epoch_ms: 7,
        };
        let page = project_palette_preference(&snapshot);
        assert_eq!(page.seed_label, "#1976D2");
        assert!(page.has_unsaved_draft);
        assert_eq!(page.revision, 2);
        assert_eq!(page.styles.len(), PaletteStylePreference::ALL.len());
        let selected: Vec<_> = page
            .styles
            .iter()
            .filter(|option| option.is_selected)
            .collect();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].style, PaletteStylePreference::FruitSalad);
    }

    #[test]
    fn follow_theme_seed_is_labeled_in_chinese() {
        let snapshot = PalettePreferenceSnapshot::default();
        let page = project_palette_preference(&snapshot);
        assert_eq!(page.seed_label, "跟随主题");
        assert!(!page.has_unsaved_draft);
    }
}

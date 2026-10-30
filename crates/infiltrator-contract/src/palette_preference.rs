//! User-owned palette preference contract (FLPARITY-036).
//!
//! The seed color and the Material palette style are a product preference
//! owned by the application layer, never invented by a UI surface. This
//! module fixes the only facts a surface may read: which styles exist, what
//! each style does to the six core tonal palettes, and which typed commands
//! and receipts move a preference from draft to saved state.

use crate::color_science::HctColor;
use crate::color_science::MaterialCorePalette;
use crate::color_science::TonalPalette;
use serde::Deserialize;
use serde::Serialize;

/// The Material 3 dynamic-color palette styles the product exposes.
///
/// Every style is a fixed (hue rotation, chroma) table applied to the seed;
/// `TonalSpot` is the Material baseline and matches the shipped default
/// core-palette derivation exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PaletteStylePreference {
    /// The Material baseline: balanced primary chroma, muted neutrals.
    #[default]
    TonalSpot,
    /// High-chroma primary; vivid accents, still neutral surfaces.
    Vibrant,
    /// Maximum primary chroma with rotated hues; deliberately expressive.
    Expressive,
    /// Low-chroma washed tones rotated away from the seed hue.
    Spritz,
    /// Primary follows the seed chroma; balanced content surfaces.
    Content,
    /// Primary keeps the exact seed hue and caps its chroma.
    Fidelity,
    /// Fruit-salad split: primary and tertiary live in different hue families.
    FruitSalad,
    /// Every family keeps the seed hue; chroma carries the hierarchy.
    Rainbow,
}

/// The (hue rotation, chroma) table one style applies to the six families.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StylePaletteParams {
    pub primary_hue_rotation: f32,
    pub primary_chroma: f32,
    pub secondary_hue_rotation: f32,
    pub secondary_chroma: f32,
    pub tertiary_hue_rotation: f32,
    pub tertiary_chroma: f32,
    pub neutral_hue_rotation: f32,
    pub neutral_chroma: f32,
    pub neutral_variant_hue_rotation: f32,
    pub neutral_variant_chroma: f32,
}

impl PaletteStylePreference {
    /// Every style in stable enumeration order.
    pub const ALL: [Self; 8] = [
        Self::TonalSpot,
        Self::Vibrant,
        Self::Expressive,
        Self::Spritz,
        Self::Content,
        Self::Fidelity,
        Self::FruitSalad,
        Self::Rainbow,
    ];

    /// The parameter table of this style.
    pub const fn params(self) -> StylePaletteParams {
        match self {
            Self::TonalSpot => StylePaletteParams {
                primary_hue_rotation: 0.0,
                primary_chroma: 48.0,
                secondary_hue_rotation: 0.0,
                secondary_chroma: 16.0,
                tertiary_hue_rotation: 60.0,
                tertiary_chroma: 24.0,
                neutral_hue_rotation: 0.0,
                neutral_chroma: 4.0,
                neutral_variant_hue_rotation: 0.0,
                neutral_variant_chroma: 8.0,
            },
            Self::Vibrant => StylePaletteParams {
                primary_hue_rotation: 0.0,
                primary_chroma: 80.0,
                secondary_hue_rotation: 0.0,
                secondary_chroma: 24.0,
                tertiary_hue_rotation: 0.0,
                tertiary_chroma: 32.0,
                neutral_hue_rotation: 0.0,
                neutral_chroma: 10.0,
                neutral_variant_hue_rotation: 0.0,
                neutral_variant_chroma: 12.0,
            },
            Self::Expressive => StylePaletteParams {
                primary_hue_rotation: 15.0,
                primary_chroma: 100.0,
                secondary_hue_rotation: 15.0,
                secondary_chroma: 24.0,
                tertiary_hue_rotation: 90.0,
                tertiary_chroma: 32.0,
                neutral_hue_rotation: 15.0,
                neutral_chroma: 8.0,
                neutral_variant_hue_rotation: 15.0,
                neutral_variant_chroma: 12.0,
            },
            Self::Spritz => StylePaletteParams {
                primary_hue_rotation: 116.0,
                primary_chroma: 16.0,
                secondary_hue_rotation: 116.0,
                secondary_chroma: 16.0,
                tertiary_hue_rotation: 116.0,
                tertiary_chroma: 24.0,
                neutral_hue_rotation: 116.0,
                neutral_chroma: 2.0,
                neutral_variant_hue_rotation: 116.0,
                neutral_variant_chroma: 2.0,
            },
            Self::Content => StylePaletteParams {
                primary_hue_rotation: 0.0,
                primary_chroma: 50.0,
                secondary_hue_rotation: 0.0,
                secondary_chroma: 25.0,
                tertiary_hue_rotation: 60.0,
                tertiary_chroma: 25.0,
                neutral_hue_rotation: 0.0,
                neutral_chroma: 10.0,
                neutral_variant_hue_rotation: 0.0,
                neutral_variant_chroma: 12.0,
            },
            Self::Fidelity => StylePaletteParams {
                primary_hue_rotation: 0.0,
                primary_chroma: 50.0,
                secondary_hue_rotation: 0.0,
                secondary_chroma: 24.0,
                tertiary_hue_rotation: 60.0,
                tertiary_chroma: 32.0,
                neutral_hue_rotation: 0.0,
                neutral_chroma: 10.0,
                neutral_variant_hue_rotation: 0.0,
                neutral_variant_chroma: 12.0,
            },
            Self::FruitSalad => StylePaletteParams {
                primary_hue_rotation: 90.0,
                primary_chroma: 55.0,
                secondary_hue_rotation: 90.0,
                secondary_chroma: 24.0,
                tertiary_hue_rotation: 50.0,
                tertiary_chroma: 36.0,
                neutral_hue_rotation: 90.0,
                neutral_chroma: 10.0,
                neutral_variant_hue_rotation: 90.0,
                neutral_variant_chroma: 12.0,
            },
            Self::Rainbow => StylePaletteParams {
                primary_hue_rotation: 0.0,
                primary_chroma: 55.0,
                secondary_hue_rotation: 0.0,
                secondary_chroma: 24.0,
                tertiary_hue_rotation: 60.0,
                tertiary_chroma: 36.0,
                neutral_hue_rotation: 0.0,
                neutral_chroma: 10.0,
                neutral_variant_hue_rotation: 0.0,
                neutral_variant_chroma: 12.0,
            },
        }
    }

    /// Localized Chinese display label.
    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::TonalSpot => "色调点",
            Self::Vibrant => "鲜艳",
            Self::Expressive => "表现力",
            Self::Spritz => "清新",
            Self::Content => "内容",
            Self::Fidelity => "保真",
            Self::FruitSalad => "缤纷",
            Self::Rainbow => "彩虹",
        }
    }
}

/// Where the seed color comes from: the product theme or a user-picked color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SeedColorSource {
    /// Follow the product theme seed; surfaces keep their existing palette path.
    #[default]
    FollowTheme,
    /// A user-picked 24-bit sRGB seed (`0xRRGGBB`, alpha is rejected).
    Custom {
        /// The 24-bit sRGB seed value.
        rgb: u32,
    },
}

impl SeedColorSource {
    /// Validate a custom seed: 24-bit sRGB only.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::FollowTheme => Ok(()),
            Self::Custom { rgb } if *rgb <= 0xFF_FFFF => Ok(()),
            Self::Custom { rgb } => Err(format!("种子色 0x{rgb:06X} 超出 24 位 sRGB 范围")),
        }
    }

    /// The HCT representation of the seed, if custom.
    pub fn hct(self) -> Option<HctColor> {
        match self {
            Self::FollowTheme => None,
            Self::Custom { rgb } => Some(HctColor::from_rgb(
                ((rgb >> 16) & 0xFF) as f32 / 255.0,
                ((rgb >> 8) & 0xFF) as f32 / 255.0,
                (rgb & 0xFF) as f32 / 255.0,
            )),
        }
    }
}

/// The saved palette preference: one seed source plus one style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct PalettePreference {
    pub seed: SeedColorSource,
    pub style: PaletteStylePreference,
    pub revision: u64,
    pub last_saved_epoch_ms: u64,
}

/// Update the unsaved draft of the palette preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UpdatePalettePreferenceDraftCommand {
    pub revision: u64,
    pub seed: SeedColorSource,
    pub style: PaletteStylePreference,
}

/// Persist the current draft as the saved preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SavePalettePreferenceCommand {
    pub revision: u64,
}

/// The typed outcome of a palette preference mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PalettePreferenceMutationReceipt {
    /// The mutation applied; carries the revision now visible.
    Applied { revision: u64 },
    /// The command's revision fence lost a race with a newer mutation.
    RevisionStale { current_revision: u64 },
    /// The seed color failed validation.
    InvalidSeed { reason: String },
}

/// The safe surface snapshot of the palette preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct PalettePreferenceSnapshot {
    pub seed: SeedColorSource,
    pub style: PaletteStylePreference,
    pub revision: u64,
    pub has_unsaved_draft: bool,
    pub last_saved_epoch_ms: u64,
}

impl PalettePreference {
    /// The surface-safe snapshot of this saved preference.
    pub fn to_snapshot(&self, has_unsaved_draft: bool) -> PalettePreferenceSnapshot {
        PalettePreferenceSnapshot {
            seed: self.seed,
            style: self.style,
            revision: self.revision,
            has_unsaved_draft,
            last_saved_epoch_ms: self.last_saved_epoch_ms,
        }
    }
}

/// Derive the six Material core palettes from a seed under one style.
///
/// `TonalSpot` is exactly the shipped `MaterialCorePalette::of_seed`.
pub fn derive_core_palette(seed: HctColor, style: PaletteStylePreference) -> MaterialCorePalette {
    let p = style.params();
    let rotated = |base: f32, rotation: f32| (base + rotation).rem_euclid(360.0);
    MaterialCorePalette {
        primary: TonalPalette::of(rotated(seed.hue, p.primary_hue_rotation), p.primary_chroma),
        secondary: TonalPalette::of(
            rotated(seed.hue, p.secondary_hue_rotation),
            p.secondary_chroma,
        ),
        tertiary: TonalPalette::of(
            rotated(seed.hue, p.tertiary_hue_rotation),
            p.tertiary_chroma,
        ),
        neutral: TonalPalette::of(rotated(seed.hue, p.neutral_hue_rotation), p.neutral_chroma),
        neutral_variant: TonalPalette::of(
            rotated(seed.hue, p.neutral_variant_hue_rotation),
            p.neutral_variant_chroma,
        ),
        error: TonalPalette::of(25.0, 84.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tonal_spot_matches_the_default_core_palette() {
        let seed = HctColor::from_rgb(0.4, 0.31, 0.64);
        let baseline = MaterialCorePalette::of_seed(seed);
        let styled = derive_core_palette(seed, PaletteStylePreference::TonalSpot);
        assert_eq!(baseline, styled, "TonalSpot must be the shipped default");
    }

    #[test]
    fn vibrant_carries_more_primary_chroma_than_tonal_spot() {
        let seed = HctColor::from_rgb(0.2, 0.5, 0.8);
        let tonal = derive_core_palette(seed, PaletteStylePreference::TonalSpot);
        let vibrant = derive_core_palette(seed, PaletteStylePreference::Vibrant);
        assert!(vibrant.primary.chroma > tonal.primary.chroma);
    }

    #[test]
    fn expressive_rotates_the_primary_hue() {
        let seed = HctColor::from_rgb(0.9, 0.3, 0.3);
        let tonal = derive_core_palette(seed, PaletteStylePreference::TonalSpot);
        let expressive = derive_core_palette(seed, PaletteStylePreference::Expressive);
        assert!(
            (expressive.primary.hue - tonal.primary.hue).abs() > 5.0,
            "the 15-degree rotation must be visible"
        );
    }

    #[test]
    fn spritz_rotates_neutrals_away_from_the_seed() {
        let seed = HctColor::from_rgb(0.1, 0.6, 0.4);
        let tonal = derive_core_palette(seed, PaletteStylePreference::TonalSpot);
        let spritz = derive_core_palette(seed, PaletteStylePreference::Spritz);
        assert!(
            (spritz.neutral.hue - tonal.neutral.hue).abs() > 100.0,
            "the 116-degree neutral rotation must be visible"
        );
        assert!(spritz.neutral.chroma < tonal.neutral.chroma);
    }

    #[test]
    fn every_style_is_enumerable_and_labeled() {
        assert_eq!(PaletteStylePreference::ALL.len(), 8);
        for style in PaletteStylePreference::ALL {
            assert!(!style.label_zh().is_empty());
            let p = style.params();
            assert!(p.primary_chroma > 0.0);
            assert!(p.secondary_chroma > 0.0);
        }
    }

    #[test]
    fn seed_validation_rejects_alpha_and_accepts_24bit() {
        assert!(SeedColorSource::FollowTheme.validate().is_ok());
        assert!(
            SeedColorSource::Custom { rgb: 0xFF_FFFF }
                .validate()
                .is_ok()
        );
        let invalid = SeedColorSource::Custom { rgb: 0x01FF_FFFF };
        assert!(invalid.validate().is_err());
        assert!(SeedColorSource::Custom { rgb: 0x33691E }.hct().is_some());
        assert!(SeedColorSource::FollowTheme.hct().is_none());
    }

    #[test]
    fn snapshot_carries_the_unsaved_draft_flag() {
        let pref = PalettePreference {
            seed: SeedColorSource::Custom { rgb: 0x33691E },
            style: PaletteStylePreference::FruitSalad,
            revision: 3,
            last_saved_epoch_ms: 42,
        };
        let clean = pref.to_snapshot(false);
        assert!(!clean.has_unsaved_draft);
        assert_eq!(clean.revision, 3);
        let dirty = pref.to_snapshot(true);
        assert!(dirty.has_unsaved_draft);
    }
}

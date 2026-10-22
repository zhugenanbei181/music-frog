//! Mini sparkline: the tiny trend a card wears (EFFECTS-017 #64).
//!
//! The widget layer knows nothing about latency: the caller hands over the
//! normalized sample heights (0.0..=1.0, oldest first) and the widget draws
//! them. The raster is the shared chart rasterizer
//! ([`crate::chart::sparkline_image`]), so the curve is anti-aliased by the
//! same code the big charts use — a hand-rolled second rasterizer would
//! band exactly where the trend has to be readable.
//!
//! The sparkline owns a private image: the samples change, the image is
//! rewritten in place, and the handle is released with the node.

use crate::chart::sparkline_image;
use crate::palette::UiPalette;
use crate::palette::rem;
use bevy::asset::Assets;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::image::Image;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::Node;
use bevy::ui::widget::ImageNode;

/// The mini sparkline's width in baseline pixels.
pub const MINI_SPARKLINE_WIDTH_PX: f32 = 56.0;

/// The mini sparkline's height in baseline pixels.
pub const MINI_SPARKLINE_HEIGHT_PX: f32 = 20.0;

/// The semantic tone of a mini sparkline's line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SparklineTone {
    /// The accent line: the default trend.
    #[default]
    Accent,
    /// The success line.
    Success,
    /// The warning line.
    Warning,
    /// The danger line.
    Danger,
    /// The dim line: a trend with no semantic weight.
    Muted,
}

/// Whether the mini sparkline paints the area under its line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SparklineFill {
    /// Only the line is painted.
    #[default]
    None,
    /// The area under the line is washed.
    Wash,
}

/// The mini sparkline mounted on a node.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct MiniSparkline {
    /// The normalized sample heights, oldest first (0.0..=1.0).
    pub samples: Vec<f32>,
    /// The tone of the line.
    pub tone: SparklineTone,
    /// Whether the area under the line is washed.
    pub fill: SparklineFill,
}

/// The signature of the last raster of a mini sparkline: what it was
/// painted from. An unchanged signature costs nothing.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct MiniSparklinePaint {
    /// A hash of the sample heights the raster was made from.
    pub samples_hash: u64,
    /// The tone the line was painted in.
    pub tone: SparklineTone,
    /// Whether the area under the line was washed.
    pub fill: SparklineFill,
    /// The ink the line was painted in.
    pub ink: Color,
}

/// The ink a mini sparkline's line is painted in. Pure function.
pub fn sparkline_ink(tone: SparklineTone, palette: &UiPalette) -> Color {
    match tone {
        SparklineTone::Accent => palette.accent,
        SparklineTone::Success => palette.success,
        SparklineTone::Warning => palette.warning,
        SparklineTone::Danger => palette.danger,
        SparklineTone::Muted => palette.ink_dim,
    }
}

/// A hash of the sample heights: the raster only changes when the samples
/// do.
fn samples_hash(samples: &[f32]) -> u64 {
    samples.iter().fold(0u64, |acc, sample| {
        acc.wrapping_mul(31)
            .wrapping_add(u64::from(sample.to_bits()))
    })
}

/// The mini sparkline scene: a fixed strip the trend is drawn into.
pub fn mini_sparkline_scene(
    samples: Vec<f32>,
    tone: SparklineTone,
    fill: SparklineFill,
) -> impl Scene + use<> {
    bsn! {
        Node {
            width: rem(MINI_SPARKLINE_WIDTH_PX),
            height: rem(MINI_SPARKLINE_HEIGHT_PX),
            flex_shrink: 0.0,
        }
        MiniSparkline { samples: { samples }, tone: { tone }, fill: { fill } }
    }
}

/// Repaint every mini sparkline from the live samples and palette.
///
/// The sparkline owns a private image: an unchanged sparkline costs
/// nothing, and a changed one rewrites its own raster in place.
pub fn sync_mini_sparklines(
    palette: Res<UiPalette>,
    images: Option<ResMut<Assets<Image>>>,
    sparks: Query<(
        Entity,
        &MiniSparkline,
        Option<&MiniSparklinePaint>,
        Option<&ImageNode>,
    )>,
    mut commands: Commands,
) {
    let Some(mut images) = images else {
        return;
    };
    for (entity, spark, paint, image) in &sparks {
        let ink = sparkline_ink(spark.tone, &palette);
        let hash = samples_hash(&spark.samples);
        if let Some(paint) = paint
            && paint.samples_hash == hash
            && paint.tone == spark.tone
            && paint.fill == spark.fill
            && paint.ink == ink
        {
            continue;
        }
        let wash = match spark.fill {
            SparklineFill::None => None,
            SparklineFill::Wash => Some(palette.ink_wash(ink)),
        };
        let raster = sparkline_image(
            &spark.samples,
            MINI_SPARKLINE_WIDTH_PX.round() as u32,
            MINI_SPARKLINE_HEIGHT_PX.round() as u32,
            ink,
            wash,
        );
        match image {
            Some(node) => {
                if let Some(mut existing) = images.get_mut(&node.image) {
                    *existing = raster;
                }
            }
            None => {
                let handle = images.add(raster);
                commands.entity(entity).insert(ImageNode {
                    image: handle,
                    ..ImageNode::default()
                });
            }
        }
        commands.entity(entity).insert(MiniSparklinePaint {
            samples_hash: hash,
            tone: spark.tone,
            fill: spark.fill,
            ink,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    #[test]
    fn every_tone_paints_its_own_ink() {
        let palette = UiPalette::new(&Theme::dark());
        assert_eq!(
            sparkline_ink(SparklineTone::Accent, &palette),
            palette.accent
        );
        assert_eq!(
            sparkline_ink(SparklineTone::Success, &palette),
            palette.success
        );
        assert_eq!(
            sparkline_ink(SparklineTone::Warning, &palette),
            palette.warning
        );
        assert_eq!(
            sparkline_ink(SparklineTone::Danger, &palette),
            palette.danger
        );
        assert_eq!(
            sparkline_ink(SparklineTone::Muted, &palette),
            palette.ink_dim
        );
    }

    #[test]
    fn the_samples_hash_follows_the_samples() {
        assert_eq!(
            samples_hash(&[0.0, 0.5, 1.0]),
            samples_hash(&[0.0, 0.5, 1.0])
        );
        assert_ne!(
            samples_hash(&[0.0, 0.5, 1.0]),
            samples_hash(&[0.0, 0.5, 0.9])
        );
        assert_ne!(samples_hash(&[0.0, 0.5]), samples_hash(&[0.0, 0.5, 1.0]));
    }

    #[test]
    fn an_empty_history_has_one_signature() {
        assert_eq!(samples_hash(&[]), samples_hash(&[]));
    }
}

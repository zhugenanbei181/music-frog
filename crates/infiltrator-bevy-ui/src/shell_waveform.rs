//! Update one retained native texture from the shared immutable waveform bars.
use crate::surface::LatestSurfaceSnapshot;
use bevy::asset::Assets;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Local, Query, Res, ResMut};
use bevy::image::Image;
use bevy::ui::widget::ImageNode;
use infiltrator_bevy_widgets::chart::sparkline_image;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_contract::mini_hud::MiniHudWaveformStrip;

#[derive(Component, Clone, Default)]
pub struct ShellWaveform;

pub fn sync(
    snapshot: Option<Res<LatestSurfaceSnapshot>>,
    palette: Res<UiPalette>,
    images: Option<ResMut<Assets<Image>>>,
    slots: Query<(Entity, Option<&ImageNode>), With<ShellWaveform>>,
    mut previous: Local<Option<(Vec<f32>, Color, Color)>>,
    mut commands: Commands,
) {
    let Some(mut images) = images else {
        return;
    };
    let bars: Vec<_> = snapshot
        .as_ref()
        .map(|snapshot| {
            snapshot
                .0
                .shell_readout
                .waveform
                .down
                .iter()
                .map(|bar| MiniHudWaveformStrip::bar_fraction(*bar))
                .collect()
        })
        .unwrap_or_default();
    let signature = (bars.clone(), palette.accent, palette.chart_fill_down());
    if previous.as_ref() == Some(&signature)
        && slots
            .iter()
            .all(|(_, node)| node.is_some_and(|node| images.get(&node.image).is_some()))
    {
        return;
    }
    for (entity, node) in &slots {
        let image = sparkline_image(
            &bars,
            MiniHudWaveformStrip::WIDTH_PX,
            MiniHudWaveformStrip::HEIGHT_PX,
            palette.accent,
            Some(palette.chart_fill_down()),
        );
        if let Some(node) = node {
            if let Some(mut existing) = images.get_mut(&node.image) {
                *existing = image;
            }
        } else {
            commands.entity(entity).insert(ImageNode {
                image: images.add(image),
                ..Default::default()
            });
        }
    }
    *previous = Some(signature);
}

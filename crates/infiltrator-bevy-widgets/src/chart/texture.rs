//! Per-chart writable textures. Shared ImageNode handles are never write targets.

use super::donut::DonutChartPlate;
use super::histogram::HistogramPlate;
use super::topology::{TopologyPaint, TopologyPlate};
use super::{ChartPaint, ChartPlate};
use crate::globe::{GlobePaint, GlobePlate};
use bevy::asset::{Assets, Handle};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{QueryData, QueryFilter, With, Without};
use bevy::ecs::system::{Commands, Query};
use bevy::image::Image;
use bevy::ui::widget::ImageNode;
use std::any::TypeId;

#[derive(Component)]
struct ChartTexture {
    image: Handle<Image>,
    plate: TypeId,
}

#[derive(QueryData)]
pub struct ChartTextureView {
    entity: Entity,
    node: Option<&'static ImageNode>,
    texture: Option<&'static ChartTexture>,
}

impl ChartTextureViewItem<'_, '_> {
    /// Whether this texture is fully bound to an active asset in images.
    pub(crate) fn is_bound<P: Component>(&self, images: &Assets<Image>) -> bool {
        let role = TypeId::of::<P>();
        if let Some(texture) = self
            .texture
            .filter(|texture| images.contains(&texture.image))
        {
            texture.plate == role && self.node.is_some_and(|node| node.image == texture.image)
        } else {
            false
        }
    }

    /// Repaint only the owned asset; restoring a binding needs no rasterization.
    pub(crate) fn sync<P: Component>(
        &self,
        images: &mut Assets<Image>,
        repaint: bool,
        render: impl FnOnce() -> Image,
        commands: &mut Commands,
    ) -> bool {
        let role = TypeId::of::<P>();
        if let Some(texture) = self
            .texture
            .filter(|texture| images.contains(&texture.image))
        {
            let repaint = repaint || texture.plate != role;
            if repaint && let Some(mut image) = images.get_mut(&texture.image) {
                *image = render();
            }
            if texture.plate != role {
                commands.entity(self.entity).insert(ChartTexture {
                    image: texture.image.clone(),
                    plate: role,
                });
            }
            if self.node.is_none_or(|node| node.image != texture.image) {
                let mut node = self.node.cloned().unwrap_or_default();
                node.image = texture.image.clone();
                commands.entity(self.entity).insert(node);
            }
            return repaint;
        }
        let handle = images.add(render());
        let mut node = self.node.cloned().unwrap_or_default();
        node.image = handle.clone();
        commands.entity(self.entity).insert((
            node,
            ChartTexture {
                image: handle,
                plate: role,
            },
        ));
        true
    }

    /// Release the owned texture while the plate stays mounted.
    ///
    /// The sleeping and closed paths need the raster gone without retiring
    /// the plate: the image asset is freed once its last strong handle
    /// drops, and the binding that pointed at it is removed with it.
    pub(crate) fn release_texture(&self, commands: &mut Commands) {
        let Some(texture) = self.texture else {
            return;
        };
        let mut entity = commands.entity(self.entity);
        entity.remove::<(ChartTexture, ChartPaint, TopologyPaint, GlobePaint)>();
        if self.node.is_some_and(|node| node.image == texture.image) {
            entity.remove::<ImageNode>();
        }
    }
}

#[derive(QueryFilter)]
pub(crate) struct RetiredChart {
    owned: With<ChartTexture>,
    waveform: Without<ChartPlate>,
    donut: Without<DonutChartPlate>,
    histogram: Without<HistogramPlate>,
    topology: Without<TopologyPlate>,
    globe: Without<GlobePlate>,
}

/// Retiring a plate on a retained entity also drops its private texture owner.
/// Despawn naturally drops both components; AssetPlugin tracks last-handle drops.
pub(crate) fn release_retired(
    charts: Query<ChartTextureView, RetiredChart>,
    mut commands: Commands,
) {
    for view in &charts {
        view.release_texture(&mut commands);
    }
}

//! Bind the existing squircle material to retained cards after native layout.
use crate::palette::UiPalette;
use crate::shader_fx::{ModernSurfaceElevation, ModernSurfaceMaterial};
use crate::surface::SurfacePanel;
use crate::theme::CornerCurvature;
use bevy::asset::{Assets, Handle};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{QueryData, With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::ui::{BackgroundColor, ComputedNode};
use bevy::ui_render::ui_material::MaterialNode;

/// A host can select the flat renderer without changing controls or layout.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SurfaceShaderMode {
    #[default]
    Shader,
    Flat,
}

/// One writable material per card, retained across temporary layout suspension.
#[derive(Component)]
pub(crate) struct SurfaceMaterial(Handle<ModernSurfaceMaterial>);

#[derive(QueryData)]
pub(crate) struct SurfaceRetirementView {
    entity: Entity,
    owned: &'static SurfaceMaterial,
    material: Option<&'static MaterialNode<ModernSurfaceMaterial>>,
}

#[derive(QueryData)]
#[query_data(mutable)]
pub struct SurfaceShaderView {
    entity: Entity,
    computed: &'static ComputedNode,
    fill: &'static mut BackgroundColor,
    material: Option<&'static MaterialNode<ModernSurfaceMaterial>>,
    owned: Option<&'static SurfaceMaterial>,
}

/// Removing the card role on a retained entity must also release its owner.
pub(crate) fn release_retired(
    cards: Query<SurfaceRetirementView, Without<SurfacePanel>>,
    mut commands: Commands,
) {
    for card in &cards {
        let mut entity = commands.entity(card.entity);
        entity.remove::<SurfaceMaterial>();
        if card.material.is_some_and(|node| node.0 == card.owned.0) {
            entity.remove::<MaterialNode<ModernSurfaceMaterial>>();
        }
    }
}

/// Only changed uniforms touch the asset store; idle cards require no upload.
pub fn sync(
    palette: Res<UiPalette>,
    mode: Res<SurfaceShaderMode>,
    materials: Option<ResMut<Assets<ModernSurfaceMaterial>>>,
    mut cards: Query<SurfaceShaderView, With<SurfacePanel>>,
    mut commands: Commands,
) {
    let Some(mut materials) = materials else {
        return;
    };
    for mut card in &mut cards {
        let dimensions = card.computed.size() * card.computed.inverse_scale_factor();
        let radii = card.computed.border_radius;
        let corner = radii.top_left;
        let uniform_corners = corner.is_finite()
            && corner.min_element() >= 0.0
            && corner.x == corner.y
            && radii.top_right == corner
            && radii.bottom_right == corner
            && radii.bottom_left == corner;
        if *mode == SurfaceShaderMode::Flat
            || !dimensions.is_finite()
            || dimensions.min_element() <= 0.0
            || !uniform_corners
        {
            if card.material.is_some() {
                // Drop this binding, never delete an asset another consumer holds.
                commands
                    .entity(card.entity)
                    .remove::<MaterialNode<ModernSurfaceMaterial>>();
            }
            if *mode == SurfaceShaderMode::Flat && card.owned.is_some() {
                commands.entity(card.entity).remove::<SurfaceMaterial>();
            }
            if card.fill.0 != palette.surface {
                card.fill.0 = palette.surface;
            }
            continue;
        }
        let radius = corner.x * card.computed.inverse_scale_factor();
        let curvature = CornerCurvature::squircle(radius);
        // The `SurfacePanel` card marker is the existing card role: a real card
        // carries the low card-depth elevation so `shadow.wesl` runs in
        // production instead of a permanently dead `None`.
        let desired = ModernSurfaceMaterial::card(
            dimensions,
            curvature.radius_px,
            curvature.smoothing,
            palette.surface,
            palette.border,
            palette.hairline_px,
            ModernSurfaceElevation::Low,
        );
        if let Some(owned) = card.owned.filter(|owned| materials.contains(&owned.0)) {
            if materials.get(&owned.0) != Some(&desired)
                && let Some(mut material) = materials.get_mut(&owned.0)
            {
                *material = desired;
            }
            if card.material.is_none_or(|node| node.0 != owned.0) {
                commands
                    .entity(card.entity)
                    .insert(MaterialNode(owned.0.clone()));
            }
        } else {
            let handle = materials.add(desired);
            commands
                .entity(card.entity)
                .insert((MaterialNode(handle.clone()), SurfaceMaterial(handle)));
        }
        // Avoid a native circular fill behind the shader's superellipse corners.
        if card.fill.0 != Color::NONE {
            card.fill.0 = Color::NONE;
        }
    }
}

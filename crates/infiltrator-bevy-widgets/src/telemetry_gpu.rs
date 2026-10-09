//! Optional Mesh2d telemetry path with a guaranteed CPU texture fallback.
//!
//! BANDROID-013 (GPU half): [`crate::chart::mesh`]'s WESL prototype and
//! [`crate::chart::topology::build_topology_chart_mesh`] were dead outside
//! tests. This module owns the telemetry WESL handles and the per-view
//! [`Mesh2d`] asset lifecycle behind an explicit [`TelemetryRenderMode`]
//! toggle.
//!
//! The default is [`TelemetryRenderMode::Cpu`]: the existing
//! [`crate::chart::texture`] path paints unchanged and no mesh is built.
//! Selecting [`TelemetryRenderMode::Gpu`] prepares and retains one [`Mesh`]
//! per topology view. The CPU texture keeps painting as the guaranteed
//! fallback because this widget layer owns no camera and therefore cannot
//! present the mesh itself.
//!
//! Remaining for a real GPU draw (charter §1.2): a host camera plus a
//! `Material2d` pipeline for `telemetry.wesl` and the Mesh2d view transform.
//! This module only wires the asset owner and the toggle; it does not claim
//! the draw is installed.

use crate::chart::topology::{TopologyPlate, TopologySpec, build_topology_chart_mesh};
use crate::palette::UiPalette;
use bevy::app::{App, Plugin, PreStartup, Update};
use bevy::asset::{AssetServer, Assets, Handle, embedded_asset};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::Without;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::render::RenderPlugin;
use bevy::render::mesh::{Mesh, Mesh2d};
use bevy::shader::Shader;

/// Which renderer owns the telemetry/topology visualization.
///
/// [`TelemetryRenderMode::Cpu`] (the default) keeps the existing
/// `chart::texture` raster path. [`TelemetryRenderMode::Gpu`] prepares and
/// retains the optional [`Mesh2d`] path; the CPU texture stays mounted as the
/// guaranteed fallback.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TelemetryRenderMode {
    #[default]
    Cpu,
    Gpu,
}

/// App-scoped telemetry WESL handles, retained instead of forgotten.
///
/// Mirrors [`crate::shader_assets`]: the renderer loads the modules once and
/// this owner keeps them alive for the App's lifetime. A renderless app has no
/// `Shader` store, so the handle list stays empty and nothing is leaked.
#[derive(Resource, Default)]
pub struct TelemetryShaderLibraries {
    handles: Vec<Handle<Shader>>,
}

impl TelemetryShaderLibraries {
    /// The retained WESL handles. Empty until a renderer loads them.
    pub fn handles(&self) -> &[Handle<Shader>] {
        &self.handles
    }
}

/// One writable [`Mesh`] per topology view, retained across temporary
/// suspension and reused while the content signature is unchanged.
#[derive(Component)]
pub struct TelemetryMeshOwner {
    /// The owned mesh asset. Retiring the owner drops the last strong handle.
    pub mesh: Handle<Mesh>,
    spec: TopologySpec,
    palette: UiPalette,
}

impl TelemetryMeshOwner {
    fn matches(&self, spec: &TopologySpec, palette: &UiPalette) -> bool {
        self.palette == *palette && self.spec.same_content(spec)
    }
}

/// Install the optional telemetry GPU path. Frontends add this once; the mode
/// resource defaults to the CPU fallback.
pub struct TelemetryGpuPlugin;

impl Plugin for TelemetryGpuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TelemetryRenderMode>();
        app.init_resource::<TelemetryShaderLibraries>();
        // The renderer owns the WESL modules. A renderless app has no `Shader`
        // store and must not load them, mirroring `shader_assets::install`.
        if app.is_plugin_added::<RenderPlugin>() {
            install_shader_assets(app);
        }
        app.add_systems(
            Update,
            (sync_telemetry_meshes, release_retired_telemetry_meshes),
        );
    }
}

/// Register the telemetry WESL modules and their retained-handle loader once.
fn install_shader_assets(app: &mut App) {
    embedded_asset!(app, "shaders/telemetry.wesl");
    embedded_asset!(app, "shaders/telemetry_vertex.wesl");
    app.init_resource::<TelemetryShaderLibraries>();
    app.add_systems(PreStartup, load_telemetry_shaders);
}

fn load_telemetry_shaders(
    server: Res<AssetServer>,
    mut libraries: ResMut<TelemetryShaderLibraries>,
) {
    if !libraries.handles.is_empty() {
        return;
    }
    // Explicit handles keep the WESL modules and their package imports
    // available to the pinned Bevy linker for this App's lifetime.
    libraries.handles.extend([
        server.load("embedded://infiltrator_bevy_widgets/shaders/telemetry.wesl"),
        server.load("embedded://infiltrator_bevy_widgets/shaders/telemetry_vertex.wesl"),
    ]);
}

/// A topology view only has drawable geometry when it has a real size and at
/// least one node; empty/zero-size specs keep the CPU fallback instead of
/// uploading invalid geometry.
fn mesh_is_drawable(spec: &TopologySpec) -> bool {
    spec.width > 0 && spec.height > 0 && !spec.nodes.is_empty()
}

/// Prepare, reuse and repair one [`Mesh`] per topology view in GPU mode.
///
/// A static frame (same spec content, palette and size) never touches the
/// asset store, so no upload event is emitted. A real content or theme change
/// updates the existing handle in place. Without a `Mesh` asset store the
/// system does nothing and the CPU texture path stays authoritative.
pub fn sync_telemetry_meshes(
    mode: Option<Res<TelemetryRenderMode>>,
    palette: Res<UiPalette>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    views: Query<(
        Entity,
        &TopologyPlate,
        Option<&TelemetryMeshOwner>,
        Option<&Mesh2d>,
    )>,
    mut commands: Commands,
) {
    let gpu = mode.as_deref().copied() == Some(TelemetryRenderMode::Gpu);
    let Some(mut meshes) = meshes else {
        return;
    };

    for (entity, plate, owner, binding) in &views {
        let spec = &plate.0;
        if !gpu || !mesh_is_drawable(spec) {
            if owner.is_some() {
                let mut entity = commands.entity(entity);
                entity.remove::<TelemetryMeshOwner>();
                if binding.is_some() {
                    entity.remove::<Mesh2d>();
                }
            }
            continue;
        }

        if let Some(owner) = owner.filter(|owner| meshes.contains(&owner.mesh)) {
            if !owner.matches(spec, &palette) {
                let data = build_topology_chart_mesh(spec, &palette);
                if let Some(mut mesh) = meshes.get_mut(&owner.mesh) {
                    *mesh = data.to_bevy_mesh();
                }
                commands.entity(entity).insert(TelemetryMeshOwner {
                    mesh: owner.mesh.clone(),
                    spec: spec.clone(),
                    palette: *palette,
                });
            }
            if binding.is_none_or(|binding| binding.0 != owner.mesh) {
                commands.entity(entity).insert(Mesh2d(owner.mesh.clone()));
            }
        } else {
            let data = build_topology_chart_mesh(spec, &palette);
            let handle = meshes.add(data.to_bevy_mesh());
            commands.entity(entity).insert((
                Mesh2d(handle.clone()),
                TelemetryMeshOwner {
                    mesh: handle,
                    spec: spec.clone(),
                    palette: *palette,
                },
            ));
        }
    }
}

/// Retiring the topology role on a retained entity also releases its private
/// mesh owner and binding. Despawn drops both naturally; AssetPlugin frees the
/// mesh when the last strong handle drops.
pub(crate) fn release_retired_telemetry_meshes(
    views: Query<(Entity, &TelemetryMeshOwner, Option<&Mesh2d>), Without<TopologyPlate>>,
    mut commands: Commands,
) {
    for (entity, owner, binding) in &views {
        let mut entity = commands.entity(entity);
        entity.remove::<TelemetryMeshOwner>();
        if binding.is_some_and(|binding| binding.0 == owner.mesh) {
            entity.remove::<Mesh2d>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::MinimalPlugins;
    use bevy::asset::{AssetApp, AssetPlugin};

    #[test]
    fn shader_owner_retains_two_handles_and_loads_once() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        embedded_asset!(app, "shaders/telemetry.wesl");
        embedded_asset!(app, "shaders/telemetry_vertex.wesl");
        app.init_asset::<Shader>();
        app.init_resource::<TelemetryShaderLibraries>();
        app.add_systems(PreStartup, load_telemetry_shaders);

        app.update();
        let first = app.world().resource::<TelemetryShaderLibraries>().handles()[0].clone();
        assert_eq!(
            app.world()
                .resource::<TelemetryShaderLibraries>()
                .handles()
                .len(),
            2
        );

        app.update();
        let libraries = app.world().resource::<TelemetryShaderLibraries>();
        assert_eq!(libraries.handles().len(), 2, "the load runs exactly once");
        assert_eq!(
            libraries.handles()[0],
            first,
            "the retained handle identity is stable across frames"
        );
    }
}

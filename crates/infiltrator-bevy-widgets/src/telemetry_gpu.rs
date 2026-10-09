//! Optional Mesh2d telemetry path with a guaranteed CPU texture fallback.
//!
//! BANDROID-013 (GPU half): [`crate::chart::mesh`]'s WESL prototype and
//! [`crate::chart::topology::build_topology_chart_mesh`] were dead outside
//! tests. This module owns the telemetry WESL handles, the per-view
//! [`Mesh2d`] + [`MeshMaterial2d`] asset lifecycle, and the [`Material2d`]
//! pipeline behind an explicit [`TelemetryRenderMode`] toggle.
//!
//! The default is [`TelemetryRenderMode::Cpu`]: the existing
//! [`crate::chart::texture`] path paints unchanged and no mesh is built.
//! Selecting [`TelemetryRenderMode::Gpu`] prepares and retains one [`Mesh`]
//! and one [`TelemetryMaterial`] per topology view, bound through
//! [`MeshMaterial2d`]. The CPU texture keeps painting as the guaranteed
//! fallback because this widget layer owns no camera and therefore cannot
//! present the mesh itself.
//!
//! The render-side layer is complete: the Mesh2d view projection
//! (`@group(0) @binding(0)` `View.clip_from_world`), the `Material2d` bind
//! group (`@group(2) @binding(0)` [`TelemetryUniform`]) and the vertex
//! attribute locations (position 0, uv 2, color 4) all match the pinned Bevy
//! 0.20.0 GA conventions. A host only needs a `Camera2d` (plus a transform on
//! the plate) to draw it; this module owns the asset/binding lifecycle.

use crate::chart::topology::{TopologyPlate, TopologySpec, build_topology_chart_mesh};
use crate::palette::UiPalette;
use bevy::app::{App, Plugin, PreStartup, Update};
use bevy::asset::{Asset, AssetServer, Assets, Handle, embedded_asset};
use bevy::color::LinearRgba;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{QueryData, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::reflect::TypePath;
use bevy::render::RenderPlugin;
use bevy::render::mesh::{Mesh, Mesh2d};
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::{Shader, ShaderRef};
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dPlugin, MeshMaterial2d};

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

/// GPU ABI for `telemetry_uniform.wesl`; field order and types must match the
/// WESL `TelemetryUniform` struct exactly.
#[derive(ShaderType, Clone, Copy, Debug, PartialEq)]
pub struct TelemetryUniform {
    /// Neutral tint multiplied into the vertex ink (white keeps baked colors).
    pub tint: LinearRgba,
    /// Global alpha multiplier.
    pub opacity: f32,
    /// Vertical alpha falloff along the ribbon `uv.y` (0.0 = flat).
    pub fade: f32,
}

impl TelemetryUniform {
    /// The neutral telemetry ink: vertex colors stay authoritative, with the
    /// same vertical fade the CPU raster path paints.
    pub const fn neutral() -> Self {
        Self {
            tint: LinearRgba::WHITE,
            opacity: 1.0,
            fade: 0.75,
        }
    }
}

/// The Mesh2d material binding [`TelemetryUniform`] to `telemetry.wesl`.
///
/// One material per topology view; its uniform is theme-neutral and therefore
/// reused in place across content and theme changes.
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone, PartialEq)]
pub struct TelemetryMaterial {
    #[uniform(0)]
    pub uniform: TelemetryUniform,
}

impl TelemetryMaterial {
    /// Create the neutral telemetry material.
    pub const fn new() -> Self {
        Self {
            uniform: TelemetryUniform::neutral(),
        }
    }
}

impl Default for TelemetryMaterial {
    fn default() -> Self {
        Self::new()
    }
}

impl Material2d for TelemetryMaterial {
    fn vertex_shader() -> ShaderRef {
        "embedded://infiltrator_bevy_widgets/shaders/telemetry_vertex.wesl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://infiltrator_bevy_widgets/shaders/telemetry.wesl".into()
    }

    // `telemetry.wesl` emits straight (non-premultiplied) alpha, so the
    // pipeline must use the matching alpha-blend state, not the opaque default.
    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

/// One writable [`Mesh`] and its [`TelemetryMaterial`] per topology view,
/// retained across temporary suspension and reused while the content
/// signature is unchanged.
#[derive(Component)]
pub struct TelemetryMeshOwner {
    /// The owned mesh asset. Retiring the owner drops the last strong handle.
    pub mesh: Handle<Mesh>,
    /// The owned material asset, reused in place while the role lives.
    pub material: Handle<TelemetryMaterial>,
    spec: TopologySpec,
    palette: UiPalette,
}

impl TelemetryMeshOwner {
    fn matches(&self, spec: &TopologySpec, palette: &UiPalette) -> bool {
        self.palette == *palette && self.spec.same_content(spec)
    }
}

/// The per-view render view: the plate, its owner and both bindings.
#[derive(QueryData)]
pub struct TelemetryRenderView {
    entity: Entity,
    plate: &'static TopologyPlate,
    owner: Option<&'static TelemetryMeshOwner>,
    mesh: Option<&'static Mesh2d>,
    material: Option<&'static MeshMaterial2d<TelemetryMaterial>>,
}

/// A retired topology entity that still carries the owned telemetry role.
#[derive(QueryData)]
pub(crate) struct TelemetryRetirementView {
    entity: Entity,
    owner: &'static TelemetryMeshOwner,
    mesh: Option<&'static Mesh2d>,
    material: Option<&'static MeshMaterial2d<TelemetryMaterial>>,
}

/// Install the optional telemetry GPU path. Frontends add this once; the mode
/// resource defaults to the CPU fallback.
pub struct TelemetryGpuPlugin;

impl Plugin for TelemetryGpuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TelemetryRenderMode>();
        app.init_resource::<TelemetryShaderLibraries>();
        // The renderer owns the WESL modules and the Material2d pipeline. A
        // renderless app has no `Shader`/render sub-app and must not install
        // them, mirroring `shader_assets::install` and `ModernSurfacePlugin`.
        if app.is_plugin_added::<RenderPlugin>() {
            install_shader_assets(app);
            install_material_pipeline(app);
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
    embedded_asset!(app, "shaders/telemetry_uniform.wesl");
    app.init_resource::<TelemetryShaderLibraries>();
    app.add_systems(PreStartup, load_telemetry_shaders);
}

/// Install the real [`Material2d`] pipeline for [`TelemetryMaterial`].
fn install_material_pipeline(app: &mut App) {
    app.add_plugins(Material2dPlugin::<TelemetryMaterial>::default());
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
        server.load("embedded://infiltrator_bevy_widgets/shaders/telemetry_uniform.wesl"),
    ]);
}

/// A topology view only has drawable geometry when it has a real size and at
/// least one node; empty/zero-size specs keep the CPU fallback instead of
/// uploading invalid geometry.
fn mesh_is_drawable(spec: &TopologySpec) -> bool {
    spec.width > 0 && spec.height > 0 && !spec.nodes.is_empty()
}

/// Release the owned role and any binding that still points at its handles.
///
/// Removal and despawn are independent retirement paths; both route through
/// this helper so a foreign binding is never deleted by mistake.
fn detach_telemetry_bindings(
    entity: Entity,
    owner: &TelemetryMeshOwner,
    mesh: Option<&Mesh2d>,
    material: Option<&MeshMaterial2d<TelemetryMaterial>>,
    commands: &mut Commands,
) {
    let mut entity = commands.entity(entity);
    entity.remove::<TelemetryMeshOwner>();
    if mesh.is_some_and(|binding| binding.0 == owner.mesh) {
        entity.remove::<Mesh2d>();
    }
    if material.is_some_and(|binding| binding.0 == owner.material) {
        entity.remove::<MeshMaterial2d<TelemetryMaterial>>();
    }
}

/// Prepare, reuse and repair one [`Mesh`] + [`TelemetryMaterial`] per topology
/// view in GPU mode.
///
/// A static frame (same spec content, palette and size) never touches the
/// asset stores, so no upload event is emitted. A real content or theme change
/// updates the existing mesh in place and keeps the material handle. A missing
/// asset is repaired without replacing the surviving handle. Without the
/// stores the system does nothing and the CPU texture path stays authoritative.
pub fn sync_telemetry_meshes(
    mode: Option<Res<TelemetryRenderMode>>,
    palette: Res<UiPalette>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<TelemetryMaterial>>>,
    views: Query<TelemetryRenderView>,
    mut commands: Commands,
) {
    let gpu = mode.as_deref().copied() == Some(TelemetryRenderMode::Gpu);
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        return;
    };

    for view in &views {
        let spec = &view.plate.0;
        if !gpu || !mesh_is_drawable(spec) {
            if let Some(owner) = view.owner {
                detach_telemetry_bindings(
                    view.entity,
                    owner,
                    view.mesh,
                    view.material,
                    &mut commands,
                );
            }
            continue;
        }

        let owned = view.owner;
        let mesh_handle = owned
            .filter(|owner| meshes.contains(&owner.mesh))
            .map(|owner| owner.mesh.clone());
        let material_handle = owned
            .filter(|owner| materials.contains(&owner.material))
            .map(|owner| owner.material.clone());

        let rebuild = match (owned, &mesh_handle) {
            (Some(owner), Some(_)) => !owner.matches(spec, &palette),
            _ => true,
        };
        let mesh = match mesh_handle {
            Some(handle) if !rebuild => handle,
            Some(handle) => {
                if let Some(mut mesh) = meshes.get_mut(&handle) {
                    *mesh = build_topology_chart_mesh(spec, &palette).to_bevy_mesh();
                }
                handle
            }
            None => meshes.add(build_topology_chart_mesh(spec, &palette).to_bevy_mesh()),
        };
        // The material carries only the theme-neutral ABI, so it is created
        // once and reused in place; a theme switch keeps the handle.
        let material = material_handle.unwrap_or_else(|| materials.add(TelemetryMaterial::new()));

        let refresh_owner = owned.is_none_or(|owner| {
            owner.mesh != mesh || owner.material != material || !owner.matches(spec, &palette)
        });
        if refresh_owner {
            commands.entity(view.entity).insert(TelemetryMeshOwner {
                mesh: mesh.clone(),
                material: material.clone(),
                spec: spec.clone(),
                palette: *palette,
            });
        }
        if view.mesh.is_none_or(|binding| binding.0 != mesh) {
            commands.entity(view.entity).insert(Mesh2d(mesh));
        }
        if view.material.is_none_or(|binding| binding.0 != material) {
            commands
                .entity(view.entity)
                .insert(MeshMaterial2d(material));
        }
    }
}

/// Retiring the topology role on a retained entity also releases its private
/// mesh/material owner and bindings. Despawn drops both naturally; AssetPlugin
/// frees an asset when the last strong handle drops.
pub(crate) fn release_retired_telemetry_meshes(
    views: Query<TelemetryRetirementView, Without<TopologyPlate>>,
    mut commands: Commands,
) {
    for view in &views {
        detach_telemetry_bindings(
            view.entity,
            view.owner,
            view.mesh,
            view.material,
            &mut commands,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::MinimalPlugins;
    use bevy::asset::{AssetApp, AssetPlugin};

    #[test]
    fn shader_owner_retains_three_handles_and_loads_once() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        embedded_asset!(app, "shaders/telemetry.wesl");
        embedded_asset!(app, "shaders/telemetry_vertex.wesl");
        embedded_asset!(app, "shaders/telemetry_uniform.wesl");
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
            3
        );

        app.update();
        let libraries = app.world().resource::<TelemetryShaderLibraries>();
        assert_eq!(libraries.handles().len(), 3, "the load runs exactly once");
        assert_eq!(
            libraries.handles()[0],
            first,
            "the retained handle identity is stable across frames"
        );
    }

    #[test]
    fn telemetry_uniform_matches_the_neutral_abi() {
        let uniform = TelemetryUniform::neutral();
        assert_eq!(uniform.tint, LinearRgba::WHITE);
        assert_eq!(uniform.opacity, 1.0);
        assert_eq!(uniform.fade, 0.75);
        assert_eq!(TelemetryMaterial::new().uniform, uniform);
        assert_eq!(
            <TelemetryMaterial as Material2d>::alpha_mode(&TelemetryMaterial::new()),
            AlphaMode2d::Blend,
            "the straight-alpha fragment must blend, not render opaque"
        );
    }
}

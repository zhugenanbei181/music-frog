//! BANDROID-013 (GPU half): the optional Mesh2d telemetry path, its CPU
//! fallback, static-frame reuse and owned-handle retirement.
use super::render_cache_tests::modifications;
use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::{AssetApp, AssetPlugin, Assets, Handle};
use bevy::ecs::entity::Entity;
use bevy::ecs::message::MessageCursor;
use bevy::image::Image;
use bevy::render::mesh::{Mesh, Mesh2d};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::sprite_render::MeshMaterial2d;
use bevy::ui::widget::ImageNode;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::chart::topology::{
    NodeCategory, TopologyLink, TopologyNode, TopologyPlate, TopologySpec, topology_scene,
};
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::telemetry_gpu::{
    TelemetryMaterial, TelemetryMeshOwner, TelemetryRenderMode, TelemetryShaderLibraries,
    TelemetryUniform,
};
use infiltrator_bevy_widgets::theme::{Theme, ThemeSkin};
use std::thread::sleep;
use std::time::Duration;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.init_asset::<Mesh>();
    app.init_asset::<TelemetryMaterial>();
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

fn image_handle(app: &App, entity: Entity) -> Handle<Image> {
    app.world().get::<ImageNode>(entity).unwrap().image.clone()
}

fn mesh_handle(app: &App, entity: Entity) -> Handle<Mesh> {
    app.world()
        .get::<TelemetryMeshOwner>(entity)
        .expect("the GPU owner must exist")
        .mesh
        .clone()
}

fn material_handle(app: &App, entity: Entity) -> Handle<TelemetryMaterial> {
    app.world()
        .get::<MeshMaterial2d<TelemetryMaterial>>(entity)
        .expect("the GPU material binding must exist")
        .0
        .clone()
}

fn spawn_topology(app: &mut App) -> Entity {
    let mut link = TopologyLink::new("inbound", "outbound", 5_000_000.0);
    link.highlighted = true;
    let spec = TopologySpec::new(
        vec![
            TopologyNode::new("inbound", "Inbound", NodeCategory::Inbound, 0.1, 0.5),
            TopologyNode::new("outbound", "Outbound", NodeCategory::Outbound, 0.9, 0.5),
        ],
        vec![link],
        200,
        100,
    )
    .with_flow(0.0, 4.0);
    let entity = app
        .world_mut()
        .commands()
        .spawn_scene(topology_scene(spec))
        .id();
    app.world_mut().flush();
    entity
}

#[test]
fn mode_toggles_between_gpu_mesh_and_cpu_fallback_without_leaking() {
    let mut app = app();
    let entity = spawn_topology(&mut app);
    app.update();
    app.update();

    // Default CPU: the texture path is the only path; no mesh is built.
    assert!(app.world().get::<TelemetryMeshOwner>(entity).is_none());
    assert!(app.world().get::<Mesh2d>(entity).is_none());
    assert!(
        app.world()
            .get::<MeshMaterial2d<TelemetryMaterial>>(entity)
            .is_none()
    );
    assert!(
        app.world()
            .resource::<Assets<Image>>()
            .contains(&image_handle(&app, entity))
    );
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
    assert_eq!(app.world().resource::<Assets<TelemetryMaterial>>().len(), 0);

    // GPU: the mesh/material owner and both bindings appear; the CPU fallback
    // stays mounted.
    *app.world_mut().resource_mut::<TelemetryRenderMode>() = TelemetryRenderMode::Gpu;
    app.update();
    app.update();
    let owned_id = mesh_handle(&app, entity).id();
    let owned_material = material_handle(&app, entity);
    let owned_material_id = owned_material.id();
    assert!(app.world().get::<Mesh2d>(entity).is_some());
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
    assert_eq!(app.world().resource::<Assets<TelemetryMaterial>>().len(), 1);
    assert_eq!(
        app.world()
            .resource::<Assets<TelemetryMaterial>>()
            .get(&owned_material)
            .unwrap()
            .uniform,
        TelemetryUniform::neutral()
    );
    assert!(
        app.world()
            .resource::<Assets<Image>>()
            .contains(&image_handle(&app, entity)),
        "the CPU texture is the guaranteed fallback while GPU is active"
    );
    // The test must not keep a strong handle if it expects the asset freed.
    drop(owned_material);

    // Back to CPU: the owner and both bindings are released and the assets
    // return to baseline.
    *app.world_mut().resource_mut::<TelemetryRenderMode>() = TelemetryRenderMode::Cpu;
    app.update();
    app.update();
    assert!(app.world().get::<TelemetryMeshOwner>(entity).is_none());
    assert!(app.world().get::<Mesh2d>(entity).is_none());
    assert!(
        app.world()
            .get::<MeshMaterial2d<TelemetryMaterial>>(entity)
            .is_none()
    );
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
    assert!(!app.world().resource::<Assets<Mesh>>().contains(owned_id));
    assert_eq!(app.world().resource::<Assets<TelemetryMaterial>>().len(), 0);
    assert!(
        !app.world()
            .resource::<Assets<TelemetryMaterial>>()
            .contains(owned_material_id)
    );
}

#[test]
fn gpu_mode_falls_back_to_cpu_when_mesh_assets_are_unavailable() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    // No `Assets<Mesh>`: the GPU path is unavailable and must fall back.
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    *app.world_mut().resource_mut::<TelemetryRenderMode>() = TelemetryRenderMode::Gpu;
    let entity = spawn_topology(&mut app);
    app.update();
    app.update();
    assert!(app.world().get::<TelemetryMeshOwner>(entity).is_none());
    assert!(app.world().get::<Mesh2d>(entity).is_none());
    assert!(
        app.world()
            .get::<MeshMaterial2d<TelemetryMaterial>>(entity)
            .is_none()
    );
    assert!(
        app.world()
            .resource::<Assets<Image>>()
            .contains(&image_handle(&app, entity)),
        "the CPU texture still paints when the GPU path is unavailable"
    );
}

#[test]
fn static_telemetry_frames_do_not_reupload_the_mesh() {
    let mut app = app();
    *app.world_mut().resource_mut::<TelemetryRenderMode>() = TelemetryRenderMode::Gpu;
    let entity = spawn_topology(&mut app);
    app.update();
    app.update();
    let handle = mesh_handle(&app, entity);
    let material = material_handle(&app, entity);
    let mut cursor = MessageCursor::default();
    modifications(&app, &mut cursor, handle.id());

    for _ in 0..8 {
        sleep(Duration::from_millis(20));
        app.update();
        assert_eq!(
            modifications(&app, &mut cursor, handle.id()),
            0,
            "a flow-phase advance must neither rebuild nor re-upload the mesh"
        );
    }

    // A real content change updates the same mesh identity exactly once and
    // reuses the material handle.
    app.world_mut()
        .get_mut::<TopologyPlate>(entity)
        .unwrap()
        .0
        .links[0]
        .bandwidth_bps = 50_000_000.0;
    app.update();
    assert_eq!(modifications(&app, &mut cursor, handle.id()), 1);
    assert_eq!(mesh_handle(&app, entity).id(), handle.id());
    assert_eq!(
        material_handle(&app, entity).id(),
        material.id(),
        "a content change reuses the owned material handle"
    );

    // A theme switch refreshes the mesh too, still reusing the material.
    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(ThemeSkin::Light));
    app.update();
    app.update();
    assert_eq!(modifications(&app, &mut cursor, handle.id()), 1);
    assert_eq!(mesh_handle(&app, entity).id(), handle.id());
    assert_eq!(material_handle(&app, entity).id(), material.id());
    drop(handle);
    drop(material);
}

#[test]
fn a_missing_telemetry_asset_is_repaired_without_replacing_the_surviving_handle() {
    let mut app = app();
    *app.world_mut().resource_mut::<TelemetryRenderMode>() = TelemetryRenderMode::Gpu;
    let entity = spawn_topology(&mut app);
    app.update();
    app.update();
    let mesh = mesh_handle(&app, entity);
    let material = material_handle(&app, entity);

    // Removing the mesh asset rebuilds only the mesh; the material survives.
    app.world_mut()
        .resource_mut::<Assets<Mesh>>()
        .remove(mesh.id());
    app.update();
    app.update();
    let repaired_mesh = mesh_handle(&app, entity);
    assert_ne!(repaired_mesh.id(), mesh.id());
    assert!(
        app.world()
            .resource::<Assets<Mesh>>()
            .contains(repaired_mesh.id())
    );
    assert_eq!(
        material_handle(&app, entity).id(),
        material.id(),
        "the surviving material handle is reused, not replaced"
    );

    // Removing the material asset rebuilds only the material; the mesh survives.
    app.world_mut()
        .resource_mut::<Assets<TelemetryMaterial>>()
        .remove(material.id());
    app.update();
    app.update();
    let repaired_material = material_handle(&app, entity);
    assert_ne!(repaired_material.id(), material.id());
    assert_eq!(
        mesh_handle(&app, entity).id(),
        repaired_mesh.id(),
        "the surviving mesh handle is reused, not replaced"
    );
}

#[test]
fn retiring_telemetry_view_releases_its_owned_mesh_and_material() {
    let mut app = app();
    *app.world_mut().resource_mut::<TelemetryRenderMode>() = TelemetryRenderMode::Gpu;
    let entity = spawn_topology(&mut app);
    app.update();
    app.update();
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
    assert_eq!(app.world().resource::<Assets<TelemetryMaterial>>().len(), 1);
    let owned_id = mesh_handle(&app, entity).id();
    let owned_material = material_handle(&app, entity).id();

    app.world_mut().entity_mut(entity).remove::<TopologyPlate>();
    app.update();
    app.update();
    assert!(app.world().get::<TelemetryMeshOwner>(entity).is_none());
    assert!(app.world().get::<Mesh2d>(entity).is_none());
    assert!(
        app.world()
            .get::<MeshMaterial2d<TelemetryMaterial>>(entity)
            .is_none()
    );
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
    assert!(!app.world().resource::<Assets<Mesh>>().contains(owned_id));
    assert_eq!(app.world().resource::<Assets<TelemetryMaterial>>().len(), 0);
    assert!(
        !app.world()
            .resource::<Assets<TelemetryMaterial>>()
            .contains(owned_material)
    );
}

#[test]
fn one_hundred_gpu_cpu_mount_retire_cycles_return_assets_to_baseline() {
    let mut app = app();
    app.update();
    let mesh_baseline = app.world().resource::<Assets<Mesh>>().len();
    let material_baseline = app.world().resource::<Assets<TelemetryMaterial>>().len();
    for cycle in 0..100 {
        let entity = spawn_topology(&mut app);
        app.update();
        *app.world_mut().resource_mut::<TelemetryRenderMode>() = TelemetryRenderMode::Gpu;
        app.update();
        app.update();
        assert_eq!(
            app.world().resource::<Assets<Mesh>>().len(),
            mesh_baseline + 1,
            "gpu mesh {cycle}"
        );
        assert_eq!(
            app.world().resource::<Assets<TelemetryMaterial>>().len(),
            material_baseline + 1,
            "gpu material {cycle}"
        );
        *app.world_mut().resource_mut::<TelemetryRenderMode>() = TelemetryRenderMode::Cpu;
        app.update();
        app.update();
        assert_eq!(
            app.world().resource::<Assets<Mesh>>().len(),
            mesh_baseline,
            "cpu mesh {cycle}"
        );
        assert_eq!(
            app.world().resource::<Assets<TelemetryMaterial>>().len(),
            material_baseline,
            "cpu material {cycle}"
        );
        *app.world_mut().resource_mut::<TelemetryRenderMode>() = TelemetryRenderMode::Gpu;
        app.update();
        app.update();
        assert_eq!(
            app.world().resource::<Assets<Mesh>>().len(),
            mesh_baseline + 1,
            "restore mesh {cycle}"
        );
        assert_eq!(
            app.world().resource::<Assets<TelemetryMaterial>>().len(),
            material_baseline + 1,
            "restore material {cycle}"
        );
        app.world_mut().despawn(entity);
        app.update();
        app.update();
        assert_eq!(
            app.world().resource::<Assets<Mesh>>().len(),
            mesh_baseline,
            "retire mesh {cycle}"
        );
        assert_eq!(
            app.world().resource::<Assets<TelemetryMaterial>>().len(),
            material_baseline,
            "retire material {cycle}"
        );
    }
}

#[test]
fn telemetry_shader_owner_is_registered_and_empty_without_a_renderer() {
    let mut app = app();
    app.update();
    let libraries = app.world().resource::<TelemetryShaderLibraries>();
    assert!(
        libraries.handles().is_empty(),
        "a renderless app has no Shader store; the owner retains nothing"
    );
}

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
use bevy::ui::widget::ImageNode;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::chart::topology::{
    NodeCategory, TopologyLink, TopologyNode, TopologyPlate, TopologySpec, topology_scene,
};
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::telemetry_gpu::{
    TelemetryMeshOwner, TelemetryRenderMode, TelemetryShaderLibraries,
};
use infiltrator_bevy_widgets::theme::{Theme, ThemeSkin};
use std::thread::sleep;
use std::time::Duration;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.init_asset::<Mesh>();
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
            .resource::<Assets<Image>>()
            .contains(&image_handle(&app, entity))
    );
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);

    // GPU: the mesh owner and binding appear; the CPU fallback stays mounted.
    *app.world_mut().resource_mut::<TelemetryRenderMode>() = TelemetryRenderMode::Gpu;
    app.update();
    app.update();
    let owned_id = mesh_handle(&app, entity).id();
    assert!(app.world().get::<Mesh2d>(entity).is_some());
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
    assert!(
        app.world()
            .resource::<Assets<Image>>()
            .contains(&image_handle(&app, entity)),
        "the CPU texture is the guaranteed fallback while GPU is active"
    );

    // Back to CPU: the owner and binding are released and the mesh returns.
    *app.world_mut().resource_mut::<TelemetryRenderMode>() = TelemetryRenderMode::Cpu;
    app.update();
    app.update();
    assert!(app.world().get::<TelemetryMeshOwner>(entity).is_none());
    assert!(app.world().get::<Mesh2d>(entity).is_none());
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
    assert!(!app.world().resource::<Assets<Mesh>>().contains(owned_id));
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

    // A real content change updates the same identity exactly once.
    app.world_mut()
        .get_mut::<TopologyPlate>(entity)
        .unwrap()
        .0
        .links[0]
        .bandwidth_bps = 50_000_000.0;
    app.update();
    assert_eq!(modifications(&app, &mut cursor, handle.id()), 1);
    assert_eq!(mesh_handle(&app, entity).id(), handle.id());

    // A theme switch refreshes the mesh too.
    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(ThemeSkin::Light));
    app.update();
    app.update();
    assert_eq!(modifications(&app, &mut cursor, handle.id()), 1);
    drop(handle);
}

#[test]
fn retiring_telemetry_view_releases_its_owned_mesh() {
    let mut app = app();
    *app.world_mut().resource_mut::<TelemetryRenderMode>() = TelemetryRenderMode::Gpu;
    let entity = spawn_topology(&mut app);
    app.update();
    app.update();
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
    let owned_id = mesh_handle(&app, entity).id();

    app.world_mut().entity_mut(entity).remove::<TopologyPlate>();
    app.update();
    app.update();
    assert!(app.world().get::<TelemetryMeshOwner>(entity).is_none());
    assert!(app.world().get::<Mesh2d>(entity).is_none());
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
    assert!(!app.world().resource::<Assets<Mesh>>().contains(owned_id));
}

#[test]
fn one_hundred_gpu_cpu_mount_retire_cycles_return_mesh_assets_to_baseline() {
    let mut app = app();
    app.update();
    let baseline = app.world().resource::<Assets<Mesh>>().len();
    for cycle in 0..100 {
        let entity = spawn_topology(&mut app);
        app.update();
        *app.world_mut().resource_mut::<TelemetryRenderMode>() = TelemetryRenderMode::Gpu;
        app.update();
        app.update();
        assert_eq!(
            app.world().resource::<Assets<Mesh>>().len(),
            baseline + 1,
            "gpu {cycle}"
        );
        *app.world_mut().resource_mut::<TelemetryRenderMode>() = TelemetryRenderMode::Cpu;
        app.update();
        app.update();
        assert_eq!(
            app.world().resource::<Assets<Mesh>>().len(),
            baseline,
            "cpu {cycle}"
        );
        *app.world_mut().resource_mut::<TelemetryRenderMode>() = TelemetryRenderMode::Gpu;
        app.update();
        app.update();
        assert_eq!(
            app.world().resource::<Assets<Mesh>>().len(),
            baseline + 1,
            "restore {cycle}"
        );
        app.world_mut().despawn(entity);
        app.update();
        app.update();
        assert_eq!(
            app.world().resource::<Assets<Mesh>>().len(),
            baseline,
            "retire {cycle}"
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

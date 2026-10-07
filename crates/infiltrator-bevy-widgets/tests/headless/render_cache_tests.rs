//! Observe real asset writes, material bindings, and preserved card identities.
use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::{Asset, AssetApp, AssetEvent, AssetId, AssetPlugin, Assets, Handle};
use bevy::color::{Color, LinearRgba};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::message::{MessageCursor, Messages};
use bevy::ecs::system::{Commands, Res};
use bevy::image::Image;
use bevy::math::Vec2;
use bevy::scene::{CommandsSceneExt, ScenePlugin, bsn};
use bevy::ui::widget::{ImageNode, Text};
use bevy::ui::{BackgroundColor, ComputedNode, ResolvedBorderRadius};
use bevy::ui_render::ui_material::MaterialNode;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::chart::{ChartPlate, chart_scene};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::shader_fx::{ModernSurfaceMaterial, ModernSurfacePlugin};
use infiltrator_bevy_widgets::surface::{SurfacePanel, surface_scene};
use infiltrator_bevy_widgets::surface_shader::SurfaceShaderMode;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::theme::{Theme, ThemeSkin};

pub(super) fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

pub(super) fn modifications<A: Asset>(
    app: &App,
    cursor: &mut MessageCursor<AssetEvent<A>>,
    id: AssetId<A>,
) -> usize {
    cursor
        .read(app.world().resource::<Messages<AssetEvent<A>>>())
        .filter(|event| event.is_modified(id))
        .count()
}

pub(super) fn image_handle(app: &App, entity: Entity) -> Handle<Image> {
    app.world().get::<ImageNode>(entity).unwrap().image.clone()
}

#[test]
fn unchanged_charts_do_not_upload_and_equal_length_updates_repaint() {
    let mut app = app();
    app.add_systems(Startup, |mut commands: Commands| {
        commands.spawn_scene(chart_scene(
            vec![0.0, f32::NAN, 1.0],
            vec![0.5, 1.0, 0.0],
            200.0,
            60.0,
        ));
    });
    app.update();
    let entity = app
        .world_mut()
        .query::<(Entity, &ChartPlate)>()
        .single(app.world())
        .unwrap()
        .0;
    let handle = image_handle(&app, entity);
    app.update();
    let mut cursor = MessageCursor::default();
    modifications(&app, &mut cursor, handle.id());
    for _ in 0..8 {
        app.update();
        assert_eq!(modifications(&app, &mut cursor, handle.id()), 0);
    }
    let before = app
        .world()
        .resource::<Assets<Image>>()
        .get(&handle)
        .unwrap()
        .data
        .clone();
    app.world_mut()
        .get_mut::<ChartPlate>(entity)
        .unwrap()
        .0
        .down[1] = 0.0;
    app.update();
    app.update();
    assert_eq!(modifications(&app, &mut cursor, handle.id()), 1);
    assert_eq!(image_handle(&app, entity).id(), handle.id());
    assert_ne!(
        app.world()
            .resource::<Assets<Image>>()
            .get(&handle)
            .unwrap()
            .data,
        before
    );
    app.update();
    assert_eq!(modifications(&app, &mut cursor, handle.id()), 0);
}

#[test]
fn a_missing_chart_texture_is_rebuilt_without_replacing_the_control() {
    let mut app = app();
    app.add_systems(Startup, |mut commands: Commands| {
        commands.spawn_scene(chart_scene(vec![0.0, 1.0], vec![1.0, 0.0], 200.0, 60.0));
    });
    app.update();
    let entity = app
        .world_mut()
        .query::<(Entity, &ChartPlate)>()
        .single(app.world())
        .unwrap()
        .0;
    let old = image_handle(&app, entity);
    app.world_mut()
        .resource_mut::<Assets<Image>>()
        .remove(old.id());
    app.update();
    let repaired = image_handle(&app, entity);
    let image = &app
        .world()
        .resource::<Assets<Image>>()
        .get(&repaired)
        .unwrap();
    assert_eq!(image.texture_descriptor.size.width, 200);
    assert_eq!(image.texture_descriptor.size.height, 60);
    assert_ne!(old.id(), repaired.id());
    assert_eq!(
        app.world().get::<ChartPlate>(entity).unwrap().0.up,
        vec![0.0, 1.0]
    );
}

#[test]
fn surface_uniforms_follow_layout_and_theme_then_flat_mode_restores_the_card() {
    let mut app = app();
    app.add_plugins(ModernSurfacePlugin);
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(surface_scene(
                vec![Box::new(bsn! { Text("retained control") })],
                &palette,
            ));
        },
    );
    app.update();
    let entity = app
        .world_mut()
        .query::<(Entity, &SurfacePanel)>()
        .single(app.world())
        .unwrap()
        .0;
    let child = app.world().get::<Children>(entity).unwrap()[0];
    {
        let mut computed = app.world_mut().get_mut::<ComputedNode>(entity).unwrap();
        computed.size = Vec2::new(560.0, 320.0);
        computed.inverse_scale_factor = 0.5;
        computed.border_radius = ResolvedBorderRadius {
            top_left: Vec2::splat(8.0),
            top_right: Vec2::splat(8.0),
            bottom_right: Vec2::splat(8.0),
            bottom_left: Vec2::splat(8.0),
        };
    }
    app.update();
    let handle = app
        .world()
        .get::<MaterialNode<ModernSurfaceMaterial>>(entity)
        .unwrap()
        .0
        .clone();
    let uniform = app
        .world()
        .resource::<Assets<ModernSurfaceMaterial>>()
        .get(&handle)
        .unwrap()
        .uniform;
    assert_eq!(uniform.dimensions, Vec2::new(280.0, 160.0));
    assert_eq!(
        uniform.radius, 4.0,
        "retain the actual card's logical radius"
    );
    assert_eq!(
        uniform.color,
        LinearRgba::from(app.world().resource::<UiPalette>().surface)
    );
    assert_eq!(
        app.world().get::<BackgroundColor>(entity).unwrap().0,
        Color::NONE
    );
    app.update();
    let mut cursor = MessageCursor::default();
    modifications(&app, &mut cursor, handle.id());
    for _ in 0..8 {
        app.update();
        assert_eq!(modifications(&app, &mut cursor, handle.id()), 0);
    }
    app.world_mut()
        .get_mut::<ComputedNode>(entity)
        .unwrap()
        .size
        .x = 720.0;
    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(ThemeSkin::Light));
    app.update();
    let updated = app
        .world()
        .resource::<Assets<ModernSurfaceMaterial>>()
        .get(&handle)
        .unwrap()
        .uniform;
    assert_eq!(updated.dimensions, Vec2::new(360.0, 160.0));
    assert_ne!(updated.color, uniform.color);
    assert_eq!(
        updated.color,
        LinearRgba::from(app.world().resource::<UiPalette>().surface)
    );
    assert_eq!(
        app.world()
            .get::<MaterialNode<ModernSurfaceMaterial>>(entity)
            .unwrap()
            .0
            .id(),
        handle.id()
    );
    drop(handle);
    *app.world_mut().resource_mut::<SurfaceShaderMode>() = SurfaceShaderMode::Flat;
    app.update();
    app.update(); // AssetPlugin consumes last-strong-handle drops in PreUpdate.
    assert!(
        app.world()
            .get::<MaterialNode<ModernSurfaceMaterial>>(entity)
            .is_none()
    );
    assert_eq!(
        app.world()
            .resource::<Assets<ModernSurfaceMaterial>>()
            .len(),
        0
    );
    assert_eq!(
        app.world().get::<BackgroundColor>(entity).unwrap().0,
        app.world().resource::<UiPalette>().surface
    );
    assert_eq!(app.world().get::<Children>(entity).unwrap()[0], child);
    assert_eq!(
        app.world().get::<Text>(child).unwrap().0,
        "retained control"
    );
    *app.world_mut().resource_mut::<SurfaceShaderMode>() = SurfaceShaderMode::Shader;
    app.update();
    let restored = app
        .world()
        .get::<MaterialNode<ModernSurfaceMaterial>>(entity)
        .unwrap();
    assert_eq!(
        app.world()
            .resource::<Assets<ModernSurfaceMaterial>>()
            .get(&restored.0)
            .unwrap()
            .uniform,
        updated
    );
    assert_eq!(app.world().get::<Children>(entity).unwrap()[0], child);
    app.world_mut()
        .get_mut::<ComputedNode>(entity)
        .unwrap()
        .border_radius
        .top_left = Vec2::splat(24.0);
    app.update();
    assert!(
        app.world()
            .get::<MaterialNode<ModernSurfaceMaterial>>(entity)
            .is_none()
    );
    assert_eq!(
        app.world().get::<BackgroundColor>(entity).unwrap().0,
        app.world().resource::<UiPalette>().surface
    );
    assert_eq!(app.world().get::<Children>(entity).unwrap()[0], child);
}

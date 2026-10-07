//! Writable ownership, reference release and bounded route-like renderer churn.
use super::render_cache_tests::{app, image_handle, modifications};
use bevy::app::App;
use bevy::asset::{AssetApp, Assets};
use bevy::ecs::entity::Entity;
use bevy::ecs::message::MessageCursor;
use bevy::image::Image;
use bevy::math::Vec2;
use bevy::scene::{CommandsSceneExt, bsn};
use bevy::shader::Shader;
use bevy::ui::widget::{ImageNode, Text};
use bevy::ui::{ComputedNode, ResolvedBorderRadius};
use bevy::ui_render::ui_material::MaterialNode;
use infiltrator_bevy_widgets::chart::donut::{DonutChartPlate, DonutChartSpec, donut_chart_scene};
use infiltrator_bevy_widgets::chart::histogram::{HistogramPlate, HistogramSpec, histogram_scene};
use infiltrator_bevy_widgets::chart::topology::{TopologyPlate, TopologySpec, topology_scene};
use infiltrator_bevy_widgets::chart::{ChartPlate, chart_scene};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::shader_fx::{ModernSurfaceMaterial, ModernSurfacePlugin};
use infiltrator_bevy_widgets::surface::{SurfacePanel, surface_scene};
use infiltrator_bevy_widgets::surface_shader::SurfaceShaderMode;

fn surface(app: &mut App) -> Entity {
    let palette = *app.world().resource::<UiPalette>();
    let entity = app
        .world_mut()
        .commands()
        .spawn_scene(surface_scene(
            vec![Box::new(bsn! { Text("retained child") })],
            &palette,
        ))
        .id();
    app.world_mut().flush();
    let mut computed = app.world_mut().get_mut::<ComputedNode>(entity).unwrap();
    computed.size = Vec2::new(200.0, 80.0);
    computed.border_radius = ResolvedBorderRadius {
        top_left: Vec2::splat(4.0),
        top_right: Vec2::splat(4.0),
        bottom_right: Vec2::splat(4.0),
        bottom_left: Vec2::splat(4.0),
    };
    entity
}

fn charts(app: &mut App) -> [Entity; 4] {
    let mut commands = app.world_mut().commands();
    let entities = [
        commands
            .spawn_scene(chart_scene(vec![0.0, 1.0], vec![1.0, 0.0], 32.0, 24.0))
            .id(),
        commands
            .spawn_scene(donut_chart_scene(DonutChartSpec::new(vec![], 32, 24)))
            .id(),
        commands
            .spawn_scene(histogram_scene(HistogramSpec::new(vec![], 32, 24)))
            .id(),
        commands
            .spawn_scene(topology_scene(TopologySpec::new(vec![], vec![], 32, 24)))
            .id(),
    ];
    app.world_mut().flush();
    entities
}

#[test]
fn foreign_texture_bindings_are_isolated_and_owned_bindings_recover_without_upload() {
    let mut app = app();
    let entities = charts(&mut app);
    let shared = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::default());
    let before = app
        .world()
        .resource::<Assets<Image>>()
        .get(&shared)
        .unwrap()
        .data
        .clone();
    for entity in entities {
        app.world_mut().entity_mut(entity).insert(ImageNode {
            image: shared.clone(),
            ..ImageNode::default()
        });
    }
    app.update();
    app.update();
    let mut cursor = MessageCursor::default();
    modifications(&app, &mut cursor, shared.id());
    let handles = entities.map(|entity| image_handle(&app, entity));
    for handle in &handles {
        assert_ne!(handle.id(), shared.id());
        assert_eq!(
            app.world()
                .resource::<Assets<Image>>()
                .get(handle)
                .unwrap()
                .width(),
            32
        );
    }
    assert_eq!(
        app.world()
            .resource::<Assets<Image>>()
            .get(&shared)
            .unwrap()
            .data,
        before
    );
    for entity in entities {
        app.world_mut().get_mut::<ImageNode>(entity).unwrap().image = shared.clone();
    }
    app.update();
    app.update();
    for (entity, handle) in entities.into_iter().zip(&handles) {
        assert_eq!(image_handle(&app, entity).id(), handle.id());
        assert_eq!(modifications(&app, &mut cursor, handle.id()), 0);
    }
    assert_eq!(modifications(&app, &mut cursor, shared.id()), 0);
    assert_eq!(
        app.world()
            .resource::<Assets<Image>>()
            .get(&shared)
            .unwrap()
            .data,
        before
    );
}

#[test]
fn every_chart_repairs_missing_assets_even_without_changed_samples() {
    let mut app = app();
    let entities = charts(&mut app);
    app.update();
    app.update();
    let ids = entities.map(|entity| image_handle(&app, entity).id());
    for id in ids {
        app.world_mut().resource_mut::<Assets<Image>>().remove(id);
    }
    app.update();
    for (entity, old) in entities.into_iter().zip(ids) {
        let handle = image_handle(&app, entity);
        assert_ne!(handle.id(), old);
        let image = app
            .world()
            .resource::<Assets<Image>>()
            .get(&handle)
            .unwrap();
        assert_eq!((image.width(), image.height()), (32, 24));
    }
    assert_eq!(app.world().resource::<Assets<Image>>().len(), 4);
}

#[test]
fn changing_chart_role_on_the_same_entity_reuses_texture_and_repaints_content() {
    let mut app = app();
    let wave = charts(&mut app)[0];
    app.update();
    let handle = image_handle(&app, wave);
    let spec = app.world().get::<ChartPlate>(wave).unwrap().clone();
    let before = app
        .world()
        .resource::<Assets<Image>>()
        .get(&handle)
        .unwrap()
        .data
        .clone();
    app.world_mut()
        .entity_mut(wave)
        .remove::<ChartPlate>()
        .insert(DonutChartPlate(DonutChartSpec::new(vec![], 32, 24)));
    app.update();
    assert_eq!(image_handle(&app, wave).id(), handle.id());
    assert_ne!(
        app.world()
            .resource::<Assets<Image>>()
            .get(&handle)
            .unwrap()
            .data,
        before
    );
    app.world_mut()
        .entity_mut(wave)
        .remove::<DonutChartPlate>()
        .insert(spec);
    app.update();
    assert_eq!(image_handle(&app, wave).id(), handle.id());
    assert_eq!(
        app.world()
            .resource::<Assets<Image>>()
            .get(&handle)
            .unwrap()
            .data,
        before
    );
}

#[test]
fn foreign_material_bindings_are_not_written_and_missing_owned_material_recovers() {
    let mut app = app();
    app.add_plugins(ModernSurfacePlugin);
    let card = surface(&mut app);
    let palette = *app.world().resource::<UiPalette>();
    let foreign_value =
        ModernSurfaceMaterial::squircle(Vec2::new(42.0, 18.0), 3.0, 0.2, palette.accent);
    let foreign = app
        .world_mut()
        .resource_mut::<Assets<ModernSurfaceMaterial>>()
        .add(foreign_value.clone());
    app.world_mut()
        .entity_mut(card)
        .insert(MaterialNode(foreign.clone()));
    app.update();
    let own = app
        .world()
        .get::<MaterialNode<ModernSurfaceMaterial>>(card)
        .unwrap()
        .0
        .clone();
    assert_ne!(own.id(), foreign.id());
    assert_eq!(
        app.world()
            .resource::<Assets<ModernSurfaceMaterial>>()
            .get(&foreign)
            .unwrap(),
        &foreign_value
    );
    app.update();
    let mut cursor = MessageCursor::default();
    modifications(&app, &mut cursor, own.id());
    app.world_mut()
        .entity_mut(card)
        .insert(MaterialNode(foreign.clone()));
    app.update();
    app.update();
    assert_eq!(
        app.world()
            .get::<MaterialNode<ModernSurfaceMaterial>>(card)
            .unwrap()
            .0
            .id(),
        own.id()
    );
    assert_eq!(modifications(&app, &mut cursor, own.id()), 0);
    let expected = app
        .world()
        .resource::<Assets<ModernSurfaceMaterial>>()
        .get(&own)
        .unwrap()
        .clone();
    app.world_mut()
        .resource_mut::<Assets<ModernSurfaceMaterial>>()
        .remove(own.id());
    app.update();
    let repaired = &app
        .world()
        .get::<MaterialNode<ModernSurfaceMaterial>>(card)
        .unwrap()
        .0;
    assert_ne!(repaired.id(), own.id());
    assert_eq!(
        app.world()
            .resource::<Assets<ModernSurfaceMaterial>>()
            .get(repaired)
            .unwrap(),
        &expected
    );
    assert_eq!(
        app.world()
            .resource::<Assets<ModernSurfaceMaterial>>()
            .get(&foreign)
            .unwrap(),
        &foreign_value
    );
}

#[test]
fn temporary_layout_suspension_reuses_material_and_flat_mode_respects_shared_readers() {
    let mut app = app();
    app.add_plugins(ModernSurfacePlugin);
    let card = surface(&mut app);
    app.update();
    let handle = app
        .world()
        .get::<MaterialNode<ModernSurfaceMaterial>>(card)
        .unwrap()
        .0
        .clone();
    let uniform = app
        .world()
        .resource::<Assets<ModernSurfaceMaterial>>()
        .get(&handle)
        .unwrap()
        .uniform;
    let reader = app.world_mut().spawn(MaterialNode(handle.clone())).id();
    app.world_mut().get_mut::<ComputedNode>(card).unwrap().size = Vec2::ZERO;
    app.update();
    assert!(
        app.world()
            .get::<MaterialNode<ModernSurfaceMaterial>>(card)
            .is_none()
    );
    assert_eq!(
        app.world()
            .resource::<Assets<ModernSurfaceMaterial>>()
            .len(),
        1
    );
    app.world_mut().get_mut::<ComputedNode>(card).unwrap().size = Vec2::new(200.0, 80.0);
    app.update();
    assert_eq!(
        app.world()
            .get::<MaterialNode<ModernSurfaceMaterial>>(card)
            .unwrap()
            .0
            .id(),
        handle.id()
    );
    *app.world_mut().resource_mut::<SurfaceShaderMode>() = SurfaceShaderMode::Flat;
    app.update();
    app.update();
    assert!(
        app.world()
            .get::<MaterialNode<ModernSurfaceMaterial>>(card)
            .is_none()
    );
    assert_eq!(
        app.world()
            .get::<MaterialNode<ModernSurfaceMaterial>>(reader)
            .unwrap()
            .0
            .id(),
        handle.id()
    );
    assert_eq!(
        app.world()
            .resource::<Assets<ModernSurfaceMaterial>>()
            .get(&handle)
            .unwrap()
            .uniform,
        uniform
    );
    app.world_mut().despawn(reader);
    drop(handle);
    app.update();
    assert_eq!(
        app.world()
            .resource::<Assets<ModernSurfaceMaterial>>()
            .len(),
        0
    );
}

#[test]
fn removing_render_roles_on_retained_entities_releases_owned_assets() {
    let mut app = app();
    app.add_plugins(ModernSurfacePlugin);
    let card = surface(&mut app);
    let [wave, donut, histogram, topology] = charts(&mut app);
    app.update();
    assert_eq!(app.world().resource::<Assets<Image>>().len(), 4);
    assert_eq!(
        app.world()
            .resource::<Assets<ModernSurfaceMaterial>>()
            .len(),
        1
    );
    app.world_mut().entity_mut(wave).remove::<ChartPlate>();
    app.world_mut()
        .entity_mut(donut)
        .remove::<DonutChartPlate>();
    app.world_mut()
        .entity_mut(histogram)
        .remove::<HistogramPlate>();
    app.world_mut()
        .entity_mut(topology)
        .remove::<TopologyPlate>();
    app.world_mut().entity_mut(card).remove::<SurfacePanel>();
    app.update();
    app.update();
    assert_eq!(app.world().resource::<Assets<Image>>().len(), 0);
    assert_eq!(
        app.world()
            .resource::<Assets<ModernSurfaceMaterial>>()
            .len(),
        0
    );
    for entity in [wave, donut, histogram, topology] {
        assert!(app.world().get_entity(entity).is_ok());
        assert!(app.world().get::<ImageNode>(entity).is_none());
    }
    assert!(app.world().get_entity(card).is_ok());
    assert!(
        app.world()
            .get::<MaterialNode<ModernSurfaceMaterial>>(card)
            .is_none()
    );
}

#[test]
fn one_hundred_mount_flat_restore_despawn_cycles_return_assets_to_baseline() {
    let mut app = app();
    app.add_plugins(ModernSurfacePlugin);
    app.update();
    let images = app.world().resource::<Assets<Image>>().len();
    let materials = app
        .world()
        .resource::<Assets<ModernSurfaceMaterial>>()
        .len();
    for cycle in 0..100 {
        let card = surface(&mut app);
        let entities = charts(&mut app);
        app.update();
        assert_eq!(
            app.world().resource::<Assets<Image>>().len(),
            images + 4,
            "mount {cycle}"
        );
        assert_eq!(
            app.world()
                .resource::<Assets<ModernSurfaceMaterial>>()
                .len(),
            materials + 1,
            "mount {cycle}"
        );
        *app.world_mut().resource_mut::<SurfaceShaderMode>() = SurfaceShaderMode::Flat;
        app.update();
        app.update();
        assert_eq!(
            app.world()
                .resource::<Assets<ModernSurfaceMaterial>>()
                .len(),
            materials,
            "flat {cycle}"
        );
        *app.world_mut().resource_mut::<SurfaceShaderMode>() = SurfaceShaderMode::Shader;
        app.update();
        assert_eq!(
            app.world()
                .resource::<Assets<ModernSurfaceMaterial>>()
                .len(),
            materials + 1,
            "restore {cycle}"
        );
        for entity in entities.into_iter().chain([card]) {
            app.world_mut().despawn(entity);
        }
        app.update();
        app.update();
        assert_eq!(
            app.world().resource::<Assets<Image>>().len(),
            images,
            "retire {cycle}"
        );
        assert_eq!(
            app.world()
                .resource::<Assets<ModernSurfaceMaterial>>()
                .len(),
            materials,
            "retire {cycle}"
        );
    }
}

#[test]
fn headless_material_install_preserves_a_preexisting_shader_store() {
    let mut app = app();
    app.init_asset::<Shader>();
    let source = "@fragment fn fragment() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }";
    let shader = app
        .world_mut()
        .resource_mut::<Assets<Shader>>()
        .add(Shader::from_wesl(source, "other.wesl"));
    app.add_plugins(ModernSurfacePlugin);
    app.update();
    assert_eq!(app.world().resource::<Assets<Shader>>().len(), 1);
    assert_eq!(
        app.world()
            .resource::<Assets<Shader>>()
            .get(&shader)
            .unwrap()
            .path,
        "other.wesl"
    );
}

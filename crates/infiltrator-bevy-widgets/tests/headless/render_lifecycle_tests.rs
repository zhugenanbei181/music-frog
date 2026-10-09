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
use infiltrator_bevy_widgets::shader_fx::{
    CARD_INSTANCE_LAYOUT, CardInstance, CardInstanceBatchBuilder, CardInstanceSyncState,
    CardRenderStrategy, MAX_CARD_INSTANCES_PER_BATCH, MIN_EDGE_AA_PX, ModernSurfaceMaterial,
    ModernSurfacePlugin, SdfSquircle, sdf_edge_coverage, sdf_edge_coverage_cpu,
};
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

fn sample_instance(x: f32) -> CardInstance {
    CardInstance::new(
        [x, 0.0],
        [100.0, 50.0],
        [8.0, 0.6, 1.0, 0.0],
        [1.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
    )
}

#[test]
fn sdf_edge_coverage_is_monotonic_and_bounded_near_the_edge() {
    let derivative = 1.0_f32;
    let mut previous = f32::INFINITY;
    let mut distance = -3.0_f32;
    while distance <= 3.0 {
        let coverage = sdf_edge_coverage(distance, derivative);
        assert!(
            (0.0..=1.0).contains(&coverage),
            "coverage out of range at {distance}: {coverage}"
        );
        assert!(
            coverage <= previous + 1e-6,
            "coverage must be monotonic non-increasing at {distance}: {coverage} > {previous}"
        );
        previous = coverage;
        distance += 0.05;
    }
    assert_eq!(sdf_edge_coverage(-10.0, derivative), 1.0);
    assert_eq!(sdf_edge_coverage(10.0, derivative), 0.0);
    assert!(
        (sdf_edge_coverage(0.0, derivative) - 0.5).abs() < 1e-6,
        "the analytic band centres coverage on the edge"
    );
    // A vanishing derivative still yields the minimum AA band.
    assert_eq!(
        sdf_edge_coverage(0.0, 0.0),
        sdf_edge_coverage(0.0, MIN_EDGE_AA_PX)
    );

    // The explicit CPU fallback is the linear ramp and saturates identically.
    assert_eq!(sdf_edge_coverage_cpu(-1.0, 1.0), 1.0);
    assert_eq!(sdf_edge_coverage_cpu(1.0, 1.0), 0.0);
    assert!((sdf_edge_coverage_cpu(0.25, 1.0) - 0.75).abs() < 1e-6);

    // The squircle methods expose both paths and stay monotone across the edge.
    let squircle = SdfSquircle::new(Vec2::new(100.0, 100.0), 20.0, 0.6, 1.0);
    assert_eq!(squircle.coverage_at(Vec2::ZERO, 1.0), 1.0);
    assert_eq!(squircle.edge_coverage_at(Vec2::ZERO, 1.0), 1.0);
    let mut last_cpu = f32::INFINITY;
    let mut last_gpu = f32::INFINITY;
    for step in 0..80 {
        let point = Vec2::new(48.0 + step as f32 * 0.05, 0.0);
        let cpu = squircle.coverage_at(point, 1.0);
        let gpu = squircle.edge_coverage_at(point, 1.0);
        assert!(cpu <= last_cpu + 1e-6);
        assert!(gpu <= last_gpu + 1e-6);
        last_cpu = cpu;
        last_gpu = gpu;
    }
}

#[test]
fn instancing_builder_packs_batches_and_enforces_the_cap() {
    let mut builder = CardInstanceBatchBuilder::new();
    assert_eq!(builder.cap(), MAX_CARD_INSTANCES_PER_BATCH);
    assert!(builder.is_empty());
    assert_eq!(builder.remaining(), MAX_CARD_INSTANCES_PER_BATCH);

    let accepted = builder.extend(
        (0..(MAX_CARD_INSTANCES_PER_BATCH + 10)).map(|index| sample_instance(index as f32)),
    );
    assert_eq!(accepted, MAX_CARD_INSTANCES_PER_BATCH);
    assert_eq!(builder.len(), MAX_CARD_INSTANCES_PER_BATCH);
    assert!(builder.is_full());
    assert_eq!(builder.remaining(), 0);
    assert!(!builder.push(sample_instance(999.0)));
    assert_eq!(builder.overflow_count(), 11);

    let batch = builder.build();
    assert_eq!(batch.instance_count(), MAX_CARD_INSTANCES_PER_BATCH as u32);
    assert_eq!(
        batch.instance_bytes(),
        MAX_CARD_INSTANCES_PER_BATCH as u64 * u64::from(CardInstance::BYTE_SIZE)
    );
    assert_eq!(batch.overflow_count(), 11);

    let descriptor = batch.draw_descriptor();
    assert_eq!(descriptor.layout.stride, CardInstance::BYTE_SIZE);
    assert_eq!(
        descriptor.instance_count,
        MAX_CARD_INSTANCES_PER_BATCH as u32
    );
    assert_eq!(
        descriptor.buffer_byte_len,
        MAX_CARD_INSTANCES_PER_BATCH as u64 * u64::from(CardInstance::BYTE_SIZE)
    );

    let mut expected_offset = 0_u32;
    for attribute in CARD_INSTANCE_LAYOUT.attributes {
        assert_eq!(attribute.offset, expected_offset);
        expected_offset += attribute.format.byte_size();
    }
    assert_eq!(expected_offset, CARD_INSTANCE_LAYOUT.stride);

    // The cap can be lowered but never raised above the global maximum.
    assert_eq!(
        CardInstanceBatchBuilder::with_cap(usize::MAX).cap(),
        MAX_CARD_INSTANCES_PER_BATCH
    );
    let mut small = CardInstanceBatchBuilder::with_cap(3);
    assert_eq!(
        small.extend((0..5).map(|index| sample_instance(index as f32))),
        3
    );
    assert_eq!(small.overflow_count(), 2);
}

#[test]
fn degradation_switch_selects_fallback_in_the_sync_path() {
    let mut app = app();
    app.add_plugins(ModernSurfacePlugin);
    let card = surface(&mut app);
    app.update();

    assert!(app.world().resource::<CardRenderStrategy>().uses_gpu());
    let state = app.world().resource::<CardInstanceSyncState>();
    assert_eq!(state.batch().instance_count(), 1);
    assert_eq!(
        state.applied_strategy(),
        Some(CardRenderStrategy::GpuInstanced)
    );

    *app.world_mut().resource_mut::<CardRenderStrategy>() = CardRenderStrategy::Flat;
    app.update();
    let state = app.world().resource::<CardInstanceSyncState>();
    assert!(
        state.batch().is_empty(),
        "Flat selects the fallback and emits no instanced batch"
    );
    assert_eq!(state.applied_strategy(), Some(CardRenderStrategy::Flat));

    assert!(!CardRenderStrategy::Flat.uses_gpu());
    assert!(!CardRenderStrategy::CpuFallback.uses_gpu());
    assert!(CardRenderStrategy::CpuFallback.uses_cpu_fallback());
    assert!(!CardRenderStrategy::CpuFallback.emits_instanced_batch());

    *app.world_mut().resource_mut::<CardRenderStrategy>() = CardRenderStrategy::CpuFallback;
    app.update();
    assert!(
        app.world()
            .resource::<CardInstanceSyncState>()
            .batch()
            .is_empty()
    );
    assert!(app.world().get_entity(card).is_ok());
}

#[test]
fn static_frames_do_not_re_upload_the_instance_batch() {
    let mut app = app();
    app.add_plugins(ModernSurfacePlugin);
    let card = surface(&mut app);
    app.update();
    let serial = app
        .world()
        .resource::<CardInstanceSyncState>()
        .upload_serial();
    assert!(serial >= 1);

    for _ in 0..8 {
        app.update();
        assert_eq!(
            app.world()
                .resource::<CardInstanceSyncState>()
                .upload_serial(),
            serial,
            "a static frame must not re-upload the instance batch"
        );
    }

    // A real geometry change commits exactly one new batch.
    app.world_mut()
        .get_mut::<ComputedNode>(card)
        .unwrap()
        .size
        .x = 320.0;
    app.update();
    assert_eq!(
        app.world()
            .resource::<CardInstanceSyncState>()
            .upload_serial(),
        serial + 1
    );
    app.update();
    assert_eq!(
        app.world()
            .resource::<CardInstanceSyncState>()
            .upload_serial(),
        serial + 1
    );
}

#[test]
fn one_hundred_instance_mount_retire_cycles_return_to_asset_baseline() {
    let mut app = app();
    app.add_plugins(ModernSurfacePlugin);
    app.update();
    let materials = app
        .world()
        .resource::<Assets<ModernSurfaceMaterial>>()
        .len();

    for cycle in 0..100 {
        let card = surface(&mut app);
        app.update();
        assert_eq!(
            app.world()
                .resource::<Assets<ModernSurfaceMaterial>>()
                .len(),
            materials + 1,
            "mount {cycle}"
        );
        assert_eq!(
            app.world()
                .resource::<CardInstanceSyncState>()
                .batch()
                .instance_count(),
            1,
            "batch {cycle}"
        );

        app.world_mut().despawn(card);
        app.update();
        app.update();
        assert_eq!(
            app.world()
                .resource::<Assets<ModernSurfaceMaterial>>()
                .len(),
            materials,
            "retire {cycle}"
        );
        assert!(
            app.world()
                .resource::<CardInstanceSyncState>()
                .batch()
                .is_empty(),
            "empty batch {cycle}"
        );
    }
}

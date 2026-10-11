//! Headless tests for the scrollarea wrapper and its logical scroll bounds.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::system::{Commands, Res};
use bevy::math::Vec2;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerDrag, PointerDragStart};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::ScrollPosition;
use bevy::ui::prelude::{ComputedNode, Node, Overflow};
use bevy::ui_widgets::{Checkbox, ScrollArea};
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::checkbox::checkbox_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::scrollarea::{clamp_scroll, scrollarea_scene};
use infiltrator_bevy_widgets::theme::Theme;
fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

fn touch_pointer(position: Vec2, width: u32, height: u32) -> Pointer {
    Pointer::new(
        PointerId::Touch(0),
        Location {
            target: NormalizedRenderTarget::None { width, height },
            position,
        },
    )
}

fn begin_drag(app: &mut App, entity: Entity, pointer: Pointer) {
    let camera = app.world_mut().spawn_empty().id();
    app.world_mut().trigger(PointerDragStart {
        entity,
        pointer,
        button: PointerButton::Primary,
        hit: HitData::new(camera, 0.0, None, None),
    });
}

#[test]
fn scrollarea_spawns_official_viewport_over_token_chrome() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            let content = checkbox_scene("node-a".to_string(), false, &palette);
            commands.spawn_scene(scrollarea_scene(Box::new(content), 240.0, &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut areas = world.query::<&ScrollArea>();
    assert_eq!(areas.iter(world).count(), 1, "one official viewport");

    let world = app.world_mut();
    let mut positions = world.query::<&ScrollPosition>();
    assert!(
        positions.iter(world).next().is_some(),
        "ScrollArea requires the scroll position state"
    );

    let world = app.world_mut();
    let mut checkboxes = world.query::<&Checkbox>();
    assert_eq!(
        checkboxes.iter(world).count(),
        1,
        "the composed content scene is hosted"
    );
}

#[test]
fn scroll_clamps_into_content_range() {
    assert_eq!(clamp_scroll(-50.0, 800.0, 300.0), 0.0);
    assert_eq!(clamp_scroll(250.0, 800.0, 300.0), 250.0);
    assert_eq!(clamp_scroll(999.0, 800.0, 300.0), 500.0);
}

#[test]
fn short_content_never_scrolls() {
    assert_eq!(clamp_scroll(120.0, 200.0, 300.0), 0.0);
    assert_eq!(clamp_scroll(0.0, 300.0, 300.0), 0.0);
}

#[test]
fn pointer_drag_drives_scrollarea_position_and_clamps() {
    let mut app = headless_app();
    let root = app
        .world_mut()
        .spawn((
            Node {
                overflow: Overflow::scroll_y(),
                ..Default::default()
            },
            ScrollArea,
            ScrollPosition::default(),
        ))
        .id();

    {
        let mut entity_mut = app.world_mut().entity_mut(root);
        let computed = ComputedNode {
            size: Vec2::new(300.0, 400.0),
            content_size: Vec2::new(300.0, 1000.0),
            inverse_scale_factor: 1.0,
            ..Default::default()
        };
        entity_mut.insert(computed);
    }
    app.update();

    assert_eq!(app.world().get::<ScrollPosition>(root).unwrap().y, 0.0);

    // Drag states begin explicitly before deltas are accepted.
    let pointer = touch_pointer(Vec2::new(10.0, 10.0), 300, 400);
    begin_drag(&mut app, root, pointer.clone());
    app.update();

    // Drag upwards by 100px -> content scrolls down (scroll position increases)
    app.world_mut().trigger(PointerDrag {
        entity: root,
        pointer: pointer.clone(),
        button: PointerButton::Primary,
        distance: Vec2::new(0.0, -100.0),
        delta: Vec2::new(0.0, -100.0),
    });
    app.update();
    assert_eq!(app.world().get::<ScrollPosition>(root).unwrap().y, 100.0);

    // Drag downwards by 40px -> content scrolls up (scroll position decreases)
    app.world_mut().trigger(PointerDrag {
        entity: root,
        pointer: pointer.clone(),
        button: PointerButton::Primary,
        distance: Vec2::new(0.0, 40.0),
        delta: Vec2::new(0.0, 40.0),
    });
    app.update();
    assert_eq!(app.world().get::<ScrollPosition>(root).unwrap().y, 60.0);

    // Drag downwards past top boundary -> clamps to 0.0
    app.world_mut().trigger(PointerDrag {
        entity: root,
        pointer,
        button: PointerButton::Primary,
        distance: Vec2::new(0.0, 500.0),
        delta: Vec2::new(0.0, 500.0),
    });
    app.update();
    assert_eq!(app.world().get::<ScrollPosition>(root).unwrap().y, 0.0);
}

#[test]
fn pointer_drag_started_on_a_descendant_scrolls_its_scrollarea_ancestor() {
    let mut app = headless_app();
    let root = app
        .world_mut()
        .spawn((
            Node {
                overflow: Overflow::scroll_y(),
                ..Default::default()
            },
            ScrollArea,
            ScrollPosition::default(),
        ))
        .id();
    let child = app.world_mut().spawn((Node::default(), ChildOf(root))).id();

    {
        let mut root_entity = app.world_mut().entity_mut(root);
        let computed = ComputedNode {
            size: Vec2::new(300.0, 400.0),
            content_size: Vec2::new(300.0, 1000.0),
            inverse_scale_factor: 1.0,
            ..Default::default()
        };
        root_entity.insert(computed);

        let mut child_entity = app.world_mut().entity_mut(child);
        let computed = ComputedNode {
            size: Vec2::new(300.0, 80.0),
            inverse_scale_factor: 1.0,
            ..Default::default()
        };
        child_entity.insert(computed);
    }
    app.update();

    let pointer = touch_pointer(Vec2::new(10.0, 40.0), 300, 400);
    begin_drag(&mut app, child, pointer.clone());
    app.update();

    app.world_mut().trigger(PointerDrag {
        entity: child,
        pointer,
        button: PointerButton::Primary,
        distance: Vec2::new(0.0, -100.0),
        delta: Vec2::new(0.0, -100.0),
    });
    app.update();

    assert_eq!(app.world().get::<ScrollPosition>(root).unwrap().y, 100.0);
}

#[test]
fn pointer_drag_uses_logical_delta_for_logical_scroll_position() {
    let mut app = headless_app();
    let root = app
        .world_mut()
        .spawn((
            Node {
                overflow: Overflow::scroll_y(),
                ..Default::default()
            },
            ScrollArea,
            ScrollPosition::default(),
        ))
        .id();

    {
        let mut entity_mut = app.world_mut().entity_mut(root);
        let computed = ComputedNode {
            size: Vec2::new(600.0, 800.0),
            content_size: Vec2::new(600.0, 1600.0),
            inverse_scale_factor: 0.5,
            ..Default::default()
        };
        entity_mut.insert(computed);
    }
    app.update();

    let pointer = touch_pointer(Vec2::new(20.0, 20.0), 600, 800);
    begin_drag(&mut app, root, pointer.clone());
    app.update();

    app.world_mut().trigger(PointerDrag {
        entity: root,
        pointer,
        button: PointerButton::Primary,
        distance: Vec2::new(0.0, -100.0),
        delta: Vec2::new(0.0, -100.0),
    });
    app.update();

    assert_eq!(app.world().get::<ScrollPosition>(root).unwrap().y, 100.0);
}

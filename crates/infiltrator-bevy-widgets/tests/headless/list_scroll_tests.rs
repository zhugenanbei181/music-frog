//! Headless tests for the list's official scroll primitives: the official
//! scrollbar bound to the list's scroll container, the official thumb that
//! tracks the scroll position, the official drag that scrolls the viewport,
//! and the official scroll-into-view that reveals a row.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Res};
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::math::Vec2;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerDrag, PointerDragStart};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::prelude::ComputedNode;
use bevy::ui::{ScrollPosition, UiGlobalTransform};
use bevy::ui_widgets::{ControlOrientation, ScrollArea, Scrollbar, ScrollbarThumb};
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::list::scrollbar::{ListScrollContainer, ListScrollbar};
use infiltrator_bevy_widgets::list::{List, ListSelection, list_row_scene, list_scene};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::theme::Theme;

/// The measured viewport: 300x400 logical pixels of visible content over a
/// 1200px tall column.
const VIEWPORT: Vec2 = Vec2::new(300.0, 400.0);
/// The measured content height behind the viewport.
const CONTENT_HEIGHT: f32 = 1200.0;
/// The measured scrollbar track: 8px wide, as tall as the viewport.
const TRACK: Vec2 = Vec2::new(8.0, 400.0);

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    // The widget layer's navigation seam reads the arrow-key state, so the
    // headless composition owns the key resource even though it never presses
    // a key.
    app.init_resource::<ButtonInput<KeyCode>>();
    app
}

/// Mount the shared list with `row_count` rows and the given selection.
fn mount_list(app: &mut App, row_count: usize, selected: Option<usize>) {
    app.add_systems(
        Startup,
        move |mut commands: Commands, palette: Res<UiPalette>| {
            let rows = (0..row_count)
                .map(|index| list_row_scene(format!("proxy {index}"), false, &palette))
                .collect();
            commands.spawn_scene(list_scene(rows, selected));
        },
    );
    app.update();
}

/// The list's scroll container: the node that carries the official
/// `ScrollArea` and the list's rows.
fn scroll_container(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut containers = world.query_filtered::<Entity, With<ListScrollContainer>>();
    containers
        .iter(world)
        .next()
        .expect("the list mounts one official scroll container")
}

/// The official scrollbar the list mounted beside its container.
fn scrollbar(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut bars = world.query_filtered::<Entity, With<Scrollbar>>();
    bars.iter(world)
        .next()
        .expect("the list mounts one official scrollbar")
}

/// The official thumb inside the scrollbar.
fn scrollbar_thumb(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut thumbs = world.query_filtered::<Entity, With<ScrollbarThumb>>();
    thumbs
        .iter(world)
        .next()
        .expect("the official scrollbar mounts one official thumb")
}

/// Stand in for the layout pass: the headless host has no UI layout, so the
/// measured geometry the official widgets read is stamped by hand.
fn measure(app: &mut App, entity: Entity, size: Vec2, content_height: f32) {
    let mut entity_mut = app.world_mut().entity_mut(entity);
    entity_mut.insert(ComputedNode {
        size,
        content_size: Vec2::new(size.x, content_height),
        inverse_scale_factor: 1.0,
        ..Default::default()
    });
}

fn touch_pointer(position: Vec2) -> Pointer {
    Pointer::new(
        PointerId::Touch(0),
        Location {
            target: NormalizedRenderTarget::None {
                width: 300,
                height: 400,
            },
            position,
        },
    )
}

#[test]
fn list_mounts_the_official_scrollbar_bound_to_its_scroll_container() {
    let mut app = headless_app();
    mount_list(&mut app, 4, Some(1));

    let container = scroll_container(&mut app);
    let bar = scrollbar(&mut app);
    let thumb = scrollbar_thumb(&mut app);

    let world = app.world_mut();
    assert!(
        world.get::<ScrollArea>(container).is_some(),
        "the list's column carries the official ScrollArea"
    );
    assert!(
        world.get::<List>(container).is_some(),
        "the list's column is the list itself"
    );
    assert!(
        world.get::<ListScrollbar>(bar).is_some(),
        "the bar is the list's own scrollbar"
    );

    let scrollbar = world.get::<Scrollbar>(bar).expect("official scrollbar");
    assert_eq!(
        scrollbar.target, container,
        "the official scrollbar drives the list's own scroll container"
    );
    assert_eq!(scrollbar.orientation, ControlOrientation::Vertical);
    assert!(
        scrollbar.min_thumb_length > 0.0,
        "the thumb keeps a minimum length so it never vanishes"
    );

    let world = app.world_mut();
    let children = world.get::<Children>(bar).expect("the bar owns the thumb");
    assert!(
        children.contains(&thumb),
        "the official thumb is a child of the official scrollbar"
    );
    assert_eq!(
        world.get::<Children>(container).expect("rows").len(),
        4,
        "the container still owns exactly the caller's rows"
    );
}

#[test]
fn official_scrollbar_thumb_tracks_the_scroll_position() {
    let mut app = headless_app();
    mount_list(&mut app, 4, None);

    let container = scroll_container(&mut app);
    let bar = scrollbar(&mut app);
    let thumb = scrollbar_thumb(&mut app);
    measure(&mut app, container, VIEWPORT, CONTENT_HEIGHT);
    measure(&mut app, bar, TRACK, TRACK.y);
    app.update();

    // The thumb is as long as the visible share of the content: 400 * 400/1200.
    let expected_thumb_height = TRACK.y * (VIEWPORT.y / CONTENT_HEIGHT);
    let world = app.world_mut();
    let thumb_node = world.get::<ComputedNode>(thumb).expect("thumb node");
    assert!(
        (thumb_node.size.y - expected_thumb_height).abs() < 0.01,
        "the official thumb is sized by the content ratio, got {}",
        thumb_node.size.y
    );
    let at_top = world
        .get::<UiGlobalTransform>(thumb)
        .expect("thumb transform")
        .affine()
        .translation
        .y;

    // Scrolling the viewport moves the thumb down by the same proportion.
    app.world_mut()
        .entity_mut(container)
        .insert(ScrollPosition(Vec2::new(0.0, 400.0)));
    app.update();

    let world = app.world_mut();
    let thumb_node = world.get::<ComputedNode>(thumb).expect("thumb node");
    assert!(
        (thumb_node.size.y - expected_thumb_height).abs() < 0.01,
        "the thumb keeps its proportional length while the list scrolls"
    );
    let after_scroll = world
        .get::<UiGlobalTransform>(thumb)
        .expect("thumb transform")
        .affine()
        .translation
        .y;
    assert!(
        after_scroll > at_top,
        "the official thumb follows the scroll position ({at_top} -> {after_scroll})"
    );
}

#[test]
fn dragging_the_official_scrollbar_thumb_scrolls_the_list() {
    let mut app = headless_app();
    mount_list(&mut app, 4, None);

    let container = scroll_container(&mut app);
    let bar = scrollbar(&mut app);
    let thumb = scrollbar_thumb(&mut app);
    measure(&mut app, container, VIEWPORT, CONTENT_HEIGHT);
    measure(&mut app, bar, TRACK, TRACK.y);
    app.update();

    let pointer = touch_pointer(Vec2::new(295.0, 40.0));
    let camera = app.world_mut().spawn_empty().id();
    app.world_mut().trigger(PointerDragStart {
        entity: thumb,
        pointer: pointer.clone(),
        button: PointerButton::Primary,
        hit: HitData::new(camera, 0.0, None, None),
    });
    app.update();

    app.world_mut().trigger(PointerDrag {
        entity: thumb,
        pointer,
        button: PointerButton::Primary,
        distance: Vec2::new(0.0, 100.0),
        delta: Vec2::new(0.0, 100.0),
    });
    app.update();

    let world = app.world_mut();
    let position = world
        .get::<ScrollPosition>(container)
        .expect("the container owns the scroll position");
    assert!(
        position.y > 0.0,
        "the official drag scrolls the list's own container, got {}",
        position.y
    );
    assert!(
        position.y <= CONTENT_HEIGHT - VIEWPORT.y,
        "the official drag stays inside the scrollable range, got {}",
        position.y
    );
}

#[test]
fn revealing_a_selected_row_goes_through_the_official_scroll_into_view() {
    let mut app = headless_app();
    mount_list(&mut app, 4, None);

    let container = scroll_container(&mut app);
    measure(&mut app, container, VIEWPORT, CONTENT_HEIGHT);
    app.world_mut()
        .entity_mut(container)
        .insert(UiGlobalTransform::from_xy(0.0, 0.0));

    // The fourth row sits below the viewport's fold.
    let world = app.world_mut();
    let mut containers = world.query_filtered::<(Entity, &Children), With<ListScrollContainer>>();
    let (_, children) = containers.iter(world).next().expect("one list");
    let row = children[3];
    let mut row_mut = app.world_mut().entity_mut(row);
    row_mut.insert(ComputedNode {
        size: Vec2::new(300.0, 40.0),
        inverse_scale_factor: 1.0,
        ..Default::default()
    });
    row_mut.insert(UiGlobalTransform::from_xy(150.0, 700.0));

    app.world_mut()
        .entity_mut(container)
        .insert(ListSelection(Some(3)));
    app.update();

    // The official observer scrolls the viewport the minimum distance that
    // brings the row's bottom edge to the viewport's bottom edge. The row
    // sits at 680..720 in content space and the viewport spans 0..400, so the
    // official arithmetic lands on 720 + 200 - 400 = 520.
    let world = app.world_mut();
    let position = world
        .get::<ScrollPosition>(container)
        .expect("the container owns the scroll position");
    assert_eq!(
        position.y, 520.0,
        "the official scroll-into-view moved the viewport, not a local offset"
    );
}

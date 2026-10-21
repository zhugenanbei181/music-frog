//! BANDROID-026.2: 50-row scrollable list and three-button/menu acceptance page tests.
//!
//! Validates:
//! - 50-row list mounts under the SCN-10-ACCEPTANCE-LIST scenario ID
//! - three navigation buttons (Top / Reveal / Bottom) reveal their row
//!   through the official `ScrollIntoView` mechanism
//! - action menu selection and state machine transitions
//! - 025 diagnostic layer classification with SCN-10-ACCEPTANCE-LIST

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::input::touch::TouchPhase;
use bevy::math::Vec2;
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::prelude::ComputedNode;
use bevy::ui::widget::Text;
use bevy::ui::{ScrollPosition, UiGlobalTransform};
use bevy::ui_widgets::Activate;
use infiltrator_bevy_ui::gesture::{TouchLayerDiagnosticCategory, TouchLayerDiagnosticEntry};
use infiltrator_bevy_ui::pages::acceptance_list::{
    ACCEPTANCE_LIST_SCENARIO_ID, ACCEPTANCE_ROW_COUNT, ACCEPTANCE_ROW_HEIGHT_PX,
    ACCEPTANCE_VIEWPORT_HEIGHT_PX, AcceptanceListPlugin, AcceptanceListState, AcceptanceMenuButton,
    AcceptanceMenuChoice, AcceptanceRevealOutcome, AcceptanceRowItem, AcceptanceRowsContainer,
    AcceptanceScrollToBottomButton, AcceptanceScrollToRevealButton, AcceptanceScrollToTopButton,
    acceptance_list_scene,
};
use infiltrator_bevy_widgets::list::{ScrollOffsetOutcome, SelectionOutcome};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::theme::Theme;

/// The measured viewport: the acceptance list's viewport over its column.
const VIEWPORT: Vec2 = Vec2::new(300.0, ACCEPTANCE_VIEWPORT_HEIGHT_PX);
/// The measured content height: every row of the acceptance dataset.
const CONTENT_HEIGHT: f32 = ACCEPTANCE_ROW_COUNT as f32 * ACCEPTANCE_ROW_HEIGHT_PX;
/// The middle row's top edge in content space.
const ACCEPTANCE_REVEAL_ROW_TOP_PX: f32 =
    ACCEPTANCE_ROW_COUNT as f32 / 2.0 * ACCEPTANCE_ROW_HEIGHT_PX;
/// The offset the official mechanism lands on when the middle row is
/// revealed from the top: the row's bottom edge at the viewport's bottom.
const ACCEPTANCE_REVEAL_BOTTOM_PX: f32 =
    ACCEPTANCE_REVEAL_ROW_TOP_PX + ACCEPTANCE_ROW_HEIGHT_PX - ACCEPTANCE_VIEWPORT_HEIGHT_PX;

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
    app.add_plugins(AcceptanceListPlugin);
    app
}

/// Mount the acceptance page scene and stamp the geometry the official
/// scroll-into-view observer reads. The headless host has no UI layout, so
/// the measured sizes and transforms are written by hand.
fn mount_acceptance_page(app: &mut App) {
    let palette = UiPalette::new(&Theme::dark());
    let state = AcceptanceListState::default();
    app.world_mut()
        .commands()
        .spawn_scene(acceptance_list_scene(&state, &palette));
    app.update();

    let container = container(app);
    app.world_mut().entity_mut(container).insert(ComputedNode {
        size: VIEWPORT,
        content_size: Vec2::new(VIEWPORT.x, CONTENT_HEIGHT),
        inverse_scale_factor: 1.0,
        ..Default::default()
    });
    app.world_mut()
        .entity_mut(container)
        .insert(UiGlobalTransform::from_xy(
            VIEWPORT.x * 0.5,
            VIEWPORT.y * 0.5,
        ));

    let rows: Vec<(Entity, usize)> = {
        let world = app.world_mut();
        let mut query =
            world.query_filtered::<(Entity, &AcceptanceRowItem), With<AcceptanceRowItem>>();
        query
            .iter(world)
            .map(|(entity, item)| (entity, item.0))
            .collect()
    };
    for (entity, _index) in rows {
        app.world_mut().entity_mut(entity).insert(ComputedNode {
            size: Vec2::new(VIEWPORT.x, ACCEPTANCE_ROW_HEIGHT_PX),
            inverse_scale_factor: 1.0,
            ..Default::default()
        });
    }
    restamp_rows(app);
}

/// Re-stamp the rows' measured transforms for the viewport's current offset.
///
/// The scrolling content travels with the viewport, so the layout pass would
/// move every row by the offset. The headless host has no layout, so the
/// measured transforms are written by hand after every scroll.
fn restamp_rows(app: &mut App) {
    let offset = viewport_offset(app);
    let rows: Vec<(Entity, usize)> = {
        let world = app.world_mut();
        let mut query =
            world.query_filtered::<(Entity, &AcceptanceRowItem), With<AcceptanceRowItem>>();
        query
            .iter(world)
            .map(|(entity, item)| (entity, item.0))
            .collect()
    };
    for (entity, index) in rows {
        let center_y =
            index as f32 * ACCEPTANCE_ROW_HEIGHT_PX + ACCEPTANCE_ROW_HEIGHT_PX * 0.5 - offset;
        app.world_mut()
            .entity_mut(entity)
            .insert(UiGlobalTransform::from_xy(VIEWPORT.x * 0.5, center_y));
    }
}

/// The acceptance list's official scroll container.
fn container(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut query = world.query_filtered::<Entity, With<AcceptanceRowsContainer>>();
    query
        .iter(world)
        .next()
        .expect("the acceptance page mounts one official scroll container")
}

/// The official viewport's scroll offset, in logical pixels.
fn viewport_offset(app: &mut App) -> f32 {
    let container = container(app);
    let world = app.world_mut();
    world
        .get::<ScrollPosition>(container)
        .expect("the container owns the official scroll position")
        .0
        .y
}

/// The one button of the given kind.
fn button<M: Component>(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut query = world.query_filtered::<Entity, With<M>>();
    query.iter(world).next().expect("button")
}

#[test]
fn acceptance_list_mounts_50_rows_under_scenario_id() {
    let mut app = test_app();
    let palette = UiPalette::new(&Theme::dark());
    let state = AcceptanceListState::default();

    assert_eq!(state.scenario_id, ACCEPTANCE_LIST_SCENARIO_ID);
    assert_eq!(state.items.len(), ACCEPTANCE_ROW_COUNT);
    assert_eq!(state.items.len(), 50);
    assert!(state.list.is_at_top());
    assert_eq!(state.list.scroll_offset_px(), 0.0);

    // Initial virtual window with overscan
    let window = state.list.window();
    assert_eq!(window.start, 0);
    // Viewport height 440px / 44px row height = 10 visible + 2 overscan = 12 items
    assert!(window.visible_count <= 14);

    app.world_mut()
        .commands()
        .spawn_scene(acceptance_list_scene(&state, &palette));
    app.update();

    let world = app.world_mut();
    let mut texts = world.query::<&Text>();
    let found_scenario = texts
        .iter(world)
        .any(|t| t.0.contains(ACCEPTANCE_LIST_SCENARIO_ID));
    assert!(
        found_scenario,
        "Scenario ID must be rendered in scene header"
    );

    let mut buttons_top = world.query::<&AcceptanceScrollToTopButton>();
    assert_eq!(buttons_top.iter(world).count(), 1, "Top button mounted");

    let mut buttons_reveal = world.query::<&AcceptanceScrollToRevealButton>();
    assert_eq!(
        buttons_reveal.iter(world).count(),
        1,
        "Reveal button mounted"
    );

    let mut buttons_bottom = world.query::<&AcceptanceScrollToBottomButton>();
    assert_eq!(
        buttons_bottom.iter(world).count(),
        1,
        "Bottom button mounted"
    );

    let mut buttons_menu = world.query::<&AcceptanceMenuButton>();
    assert_eq!(buttons_menu.iter(world).count(), 1, "Menu button mounted");

    // Every row of the dataset is mounted: the official scroll-into-view
    // mechanism needs a row entity to reveal.
    let world = app.world_mut();
    let mut rows = world.query_filtered::<Entity, With<AcceptanceRowItem>>();
    assert_eq!(
        rows.iter(world).count(),
        ACCEPTANCE_ROW_COUNT,
        "every acceptance row is mounted"
    );
}

#[test]
fn three_buttons_reveal_rows_through_the_official_scroll_mechanism() {
    let mut app = test_app();
    mount_acceptance_page(&mut app);

    // 1. The list starts at its top.
    assert_eq!(viewport_offset(&mut app), 0.0);

    // 2. The Bottom button reveals the last row. The official mechanism
    // scrolls the minimum distance that brings the row into view, so the last
    // row lands on the viewport's bottom edge: the viewport shows the list's
    // end.
    let bottom = button::<AcceptanceScrollToBottomButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: bottom });
    app.update();
    assert_eq!(
        viewport_offset(&mut app),
        CONTENT_HEIGHT - ACCEPTANCE_VIEWPORT_HEIGHT_PX,
        "revealing the last row brings the viewport to the list's end"
    );
    restamp_rows(&mut app);
    assert_eq!(
        app.world()
            .resource::<AcceptanceListState>()
            .last_reveal_outcome,
        Some(AcceptanceRevealOutcome::RevealRequested { index: 49 })
    );

    // 3. The Reveal button reveals the middle row. The official mechanism
    // scrolls the minimum distance that brings the row into view, so the
    // middle row lands on the viewport's top edge — the row is revealed, it
    // is not centred.
    let reveal = button::<AcceptanceScrollToRevealButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: reveal });
    app.update();
    let reveal_top = ACCEPTANCE_REVEAL_ROW_TOP_PX;
    assert_eq!(
        viewport_offset(&mut app),
        reveal_top,
        "the official mechanism reveals the row, it does not centre it"
    );
    assert!(
        reveal_top < CONTENT_HEIGHT - ACCEPTANCE_VIEWPORT_HEIGHT_PX,
        "a revealed row is not the list's end"
    );
    restamp_rows(&mut app);
    assert_eq!(
        app.world()
            .resource::<AcceptanceListState>()
            .last_reveal_outcome,
        Some(AcceptanceRevealOutcome::RevealRequested { index: 25 })
    );

    // 4. The typed model mirrors the official viewport, so the acceptance
    // model and the rendered list can never disagree about where the list is.
    app.update();
    let state = app.world().resource::<AcceptanceListState>();
    assert_eq!(
        state.last_scroll_offset_outcome,
        Some(ScrollOffsetOutcome::Changed {
            previous_offset_px: CONTENT_HEIGHT - ACCEPTANCE_VIEWPORT_HEIGHT_PX,
            offset_px: reveal_top,
        })
    );
    assert!(!state.list.is_at_top());
    assert!(!state.list.is_at_bottom());

    // 5. Revealing the same row from the top scrolls the other way: the
    // official mechanism still moves the minimum distance, so the row lands
    // on the viewport's bottom edge.
    let top = button::<AcceptanceScrollToTopButton>(&mut app);
    app.world_mut().commands().trigger(Activate { entity: top });
    app.update();
    assert_eq!(
        viewport_offset(&mut app),
        0.0,
        "revealing the first row brings the viewport back to the list's top"
    );
    restamp_rows(&mut app);
    assert_eq!(
        app.world()
            .resource::<AcceptanceListState>()
            .last_reveal_outcome,
        Some(AcceptanceRevealOutcome::RevealRequested { index: 0 })
    );

    let reveal = button::<AcceptanceScrollToRevealButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: reveal });
    app.update();
    assert_eq!(
        viewport_offset(&mut app),
        ACCEPTANCE_REVEAL_BOTTOM_PX,
        "revealing the middle row from the top lands it on the viewport's bottom edge"
    );
}

#[test]
fn a_reveal_of_a_row_that_is_not_mounted_is_typed() {
    let mut app = test_app();
    let mut state = AcceptanceListState::default();
    // The row is inside the dataset but no row entity is mounted for it.
    let outcome = {
        let world = app.world_mut();
        let mut commands = world.commands();
        state.reveal_row(&mut commands, 25, None)
    };
    assert_eq!(
        outcome,
        AcceptanceRevealOutcome::RowNotMounted { index: 25 }
    );
    assert_eq!(state.last_reveal_outcome, Some(outcome));

    // A row outside the dataset is a different fact.
    let outcome = {
        let world = app.world_mut();
        let mut commands = world.commands();
        state.reveal_row(&mut commands, ACCEPTANCE_ROW_COUNT + 1, None)
    };
    assert_eq!(
        outcome,
        AcceptanceRevealOutcome::IndexOutOfRange {
            index: ACCEPTANCE_ROW_COUNT + 1,
            item_count: ACCEPTANCE_ROW_COUNT,
        }
    );
    assert_eq!(state.last_reveal_outcome, Some(outcome));
}

#[test]
fn action_menu_and_row_selection_state_machine() {
    let mut state = AcceptanceListState::default();

    // Toggle menu
    assert!(!state.menu_open);
    assert!(state.toggle_menu());
    assert!(state.menu_open);
    assert!(!state.toggle_menu());
    assert!(!state.menu_open);

    // Apply SelectMiddle choice
    state.apply_menu_choice(AcceptanceMenuChoice::SelectMiddle);
    assert_eq!(state.list.selected_index(), Some(25));
    assert!(state.items[25].selected);
    assert!(!state.items[0].selected);
    assert!(!state.menu_open);

    // Apply ClearSelection choice
    state.apply_menu_choice(AcceptanceMenuChoice::ClearSelection);
    assert_eq!(state.list.selected_index(), None);
    assert!(!state.items[25].selected);

    // Apply SelectFirst choice
    state.apply_menu_choice(AcceptanceMenuChoice::SelectFirst);
    assert_eq!(state.list.selected_index(), Some(0));
    assert!(state.items[0].selected);
    assert!(state.list.is_at_top());

    // Single item select
    let outcome = state.select_item(10);
    assert_eq!(
        outcome,
        SelectionOutcome::Changed {
            previous_index: Some(0),
            selected_index: Some(10),
        }
    );
    assert!(state.items[10].selected);
    assert!(!state.items[0].selected);
}

#[test]
fn diagnostic_classification_records_025_categories() {
    let mut state = AcceptanceListState::default();

    // 1. Success entry
    let success_entry = TouchLayerDiagnosticEntry {
        scenario_id: state.scenario_id.to_string(),
        raw_coords: Some(Vec2::new(100.0, 200.0)),
        density: 2.0,
        touch_input: Some((TouchPhase::Moved, Vec2::new(100.0, 250.0))),
        picking_hit: Some(Entity::from_bits(42)),
        gesture_mode: "Pull".into(),
        scroll_before: Some(Vec2::new(0.0, 0.0)),
        scroll_after: Some(Vec2::new(0.0, 50.0)),
        rendered: true,
    };
    assert_eq!(
        success_entry.classify(),
        None,
        "Successful touch must classify to None"
    );
    state.record_diagnostic(success_entry);
    assert!(state.last_diagnostic.is_some());
    assert_eq!(state.diagnostic_log.len(), 1);

    // 2. Touch not entered Bevy
    let not_entered = TouchLayerDiagnosticEntry {
        scenario_id: state.scenario_id.to_string(),
        raw_coords: Some(Vec2::new(100.0, 200.0)),
        density: 2.0,
        touch_input: None,
        picking_hit: None,
        gesture_mode: "".into(),
        scroll_before: None,
        scroll_after: None,
        rendered: false,
    };
    assert_eq!(
        not_entered.classify(),
        Some(TouchLayerDiagnosticCategory::TouchNotEnteredBevy)
    );
    state.record_diagnostic(not_entered);

    // 3. Touch missed UI
    let missed_ui = TouchLayerDiagnosticEntry {
        scenario_id: state.scenario_id.to_string(),
        raw_coords: Some(Vec2::new(100.0, 200.0)),
        density: 2.0,
        touch_input: Some((TouchPhase::Started, Vec2::new(100.0, 200.0))),
        picking_hit: None,
        gesture_mode: "".into(),
        scroll_before: None,
        scroll_after: None,
        rendered: false,
    };
    assert_eq!(
        missed_ui.classify(),
        Some(TouchLayerDiagnosticCategory::TouchMissedUi)
    );
    state.record_diagnostic(missed_ui);

    // 4. Drag not driven scroll
    let drag_not_driven = TouchLayerDiagnosticEntry {
        scenario_id: state.scenario_id.to_string(),
        raw_coords: Some(Vec2::new(100.0, 200.0)),
        density: 2.0,
        touch_input: Some((TouchPhase::Moved, Vec2::new(100.0, 250.0))),
        picking_hit: Some(Entity::from_bits(42)),
        gesture_mode: "Pull".into(),
        scroll_before: Some(Vec2::new(0.0, 0.0)),
        scroll_after: Some(Vec2::new(0.0, 0.0)), // zero delta!
        rendered: false,
    };
    assert_eq!(
        drag_not_driven.classify(),
        Some(TouchLayerDiagnosticCategory::DragNotDrivenScroll)
    );
    state.record_diagnostic(drag_not_driven);

    // 5. Position not rendered
    let pos_not_rendered = TouchLayerDiagnosticEntry {
        scenario_id: state.scenario_id.to_string(),
        raw_coords: Some(Vec2::new(100.0, 200.0)),
        density: 2.0,
        touch_input: Some((TouchPhase::Moved, Vec2::new(100.0, 250.0))),
        picking_hit: Some(Entity::from_bits(42)),
        gesture_mode: "Pull".into(),
        scroll_before: Some(Vec2::new(0.0, 0.0)),
        scroll_after: Some(Vec2::new(0.0, 50.0)),
        rendered: false, // not rendered!
    };
    assert_eq!(
        pos_not_rendered.classify(),
        Some(TouchLayerDiagnosticCategory::PositionNotRendered)
    );
    state.record_diagnostic(pos_not_rendered);

    assert_eq!(state.diagnostic_log.len(), 5);
}

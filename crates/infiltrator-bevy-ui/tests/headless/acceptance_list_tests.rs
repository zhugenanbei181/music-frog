//! BANDROID-026.2: 50-row virtual list and three-button/menu acceptance page tests.
//!
//! Validates:
//! - 50-row virtual list mounts under SCN-10-ACCEPTANCE-LIST scenario ID
//! - Three navigation buttons (Top / Center / Bottom) drive typed `VirtualListState` outcomes
//! - Action menu selection and state machine transitions
//! - 025 diagnostic layer classification with SCN-10-ACCEPTANCE-LIST

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::input::touch::TouchPhase;
use bevy::math::Vec2;
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::widget::Text;
use infiltrator_bevy_ui::gesture::{TouchLayerDiagnosticCategory, TouchLayerDiagnosticEntry};
use infiltrator_bevy_ui::pages::acceptance_list::{
    ACCEPTANCE_LIST_SCENARIO_ID, ACCEPTANCE_ROW_COUNT, AcceptanceListPlugin, AcceptanceListState,
    AcceptanceMenuButton, AcceptanceMenuChoice, AcceptanceScrollToBottomButton,
    AcceptanceScrollToCenterButton, AcceptanceScrollToTopButton, acceptance_list_scene,
};
use infiltrator_bevy_widgets::list::{ScrollOffsetOutcome, ScrollToItemOutcome, SelectionOutcome};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::theme::Theme;

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
    app.add_plugins(AcceptanceListPlugin);
    app
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

    let mut buttons_center = world.query::<&AcceptanceScrollToCenterButton>();
    assert_eq!(
        buttons_center.iter(world).count(),
        1,
        "Center button mounted"
    );

    let mut buttons_bottom = world.query::<&AcceptanceScrollToBottomButton>();
    assert_eq!(
        buttons_bottom.iter(world).count(),
        1,
        "Bottom button mounted"
    );

    let mut buttons_menu = world.query::<&AcceptanceMenuButton>();
    assert_eq!(buttons_menu.iter(world).count(), 1, "Menu button mounted");
}

#[test]
fn three_buttons_drive_typed_scrolling_outcomes() {
    let mut state = AcceptanceListState::default();

    // 1. Initial position is at top
    assert!(state.list.is_at_top());

    // 2. Scroll to Bottom (item 49)
    let bottom_outcome = state.scroll_to_bottom();
    match bottom_outcome {
        ScrollOffsetOutcome::Changed { offset_px, .. }
        | ScrollOffsetOutcome::Clamped { offset_px, .. } => {
            assert!(offset_px > 1000.0);
        }
        other => panic!("expected Changed or Clamped, got {other:?}"),
    }
    assert!(state.list.is_at_bottom());
    assert_eq!(state.last_scroll_offset_outcome, Some(bottom_outcome));

    // 3. Scroll to Center (row 25)
    let center_outcome = state.scroll_to_center();
    match center_outcome {
        ScrollToItemOutcome::Scrolled {
            index, offset_px, ..
        } => {
            assert_eq!(index, 25);
            // row 25 center is 25 * 44 + 22 = 1122px, minus 220px viewport half = ~902px
            assert!(offset_px > 800.0 && offset_px < 1000.0);
        }
        other => panic!("expected Scrolled, got {other:?}"),
    }
    assert!(!state.list.is_at_top());
    assert!(!state.list.is_at_bottom());
    assert_eq!(state.last_scroll_item_outcome, Some(center_outcome));

    // 4. Scroll to Top (item 0)
    let top_outcome = state.scroll_to_top();
    match top_outcome {
        ScrollOffsetOutcome::Changed { offset_px, .. }
        | ScrollOffsetOutcome::Clamped { offset_px, .. } => {
            assert_eq!(offset_px, 0.0);
        }
        other => panic!("expected Changed or Clamped to 0.0, got {other:?}"),
    }
    assert!(state.list.is_at_top());
    assert_eq!(state.last_scroll_offset_outcome, Some(top_outcome));
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

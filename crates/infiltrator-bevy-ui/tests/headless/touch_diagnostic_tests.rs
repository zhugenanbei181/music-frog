//! BANDROID-025.1: Touch layer diagnostics and scenario identification tests.
//!
//! Verifies:
//! - Four failure classification categories:
//!   1. TouchNotEnteredBevy
//!   2. TouchMissedUi
//!   3. DragNotDrivenScroll
//!   4. PositionNotRendered
//! - Success classification when touch flows through all 4 layers
//! - Scenario IDs shared with Compose (e.g. SCN-01-OVERVIEW, SCN-05-CONNECTIONS)
//! - Structured diagnostic log formatting for logcat inspection

use bevy::ecs::entity::Entity;
use bevy::input::touch::TouchPhase;
use bevy::math::Vec2;
use infiltrator_bevy_ui::gesture::{
    TouchLayerDiagnosticCategory, TouchLayerDiagnosticEntry, TouchLayerDiagnosticLog,
};

#[test]
fn test_touch_diagnostic_four_failure_categories() {
    let dummy_entity = Entity::PLACEHOLDER;

    // 1. TouchNotEnteredBevy: raw coords recorded from MotionEvent, but touch_input is None
    let not_entered = TouchLayerDiagnosticEntry {
        scenario_id: "SCN-01-OVERVIEW".to_string(),
        raw_coords: Some(Vec2::new(100.0, 200.0)),
        density: 2.625,
        touch_input: None,
        picking_hit: None,
        gesture_mode: "Undecided".to_string(),
        scroll_before: None,
        scroll_after: None,
        rendered: false,
    };
    assert_eq!(
        not_entered.classify(),
        Some(TouchLayerDiagnosticCategory::TouchNotEnteredBevy)
    );
    assert_eq!(
        not_entered.classify().unwrap().label(),
        "touch_not_entered_bevy"
    );
    assert!(
        not_entered
            .format_log()
            .contains("category=touch_not_entered_bevy")
    );

    // 2. TouchMissedUi: touch input arrived in Bevy, but Picking hit nothing
    let missed_ui = TouchLayerDiagnosticEntry {
        scenario_id: "SCN-02-PROXIES".to_string(),
        raw_coords: Some(Vec2::new(500.0, 800.0)),
        density: 2.625,
        touch_input: Some((TouchPhase::Started, Vec2::new(500.0, 800.0))),
        picking_hit: None,
        gesture_mode: "Undecided".to_string(),
        scroll_before: None,
        scroll_after: None,
        rendered: false,
    };
    assert_eq!(
        missed_ui.classify(),
        Some(TouchLayerDiagnosticCategory::TouchMissedUi)
    );
    assert_eq!(missed_ui.classify().unwrap().label(), "touch_missed_ui");
    assert!(missed_ui.format_log().contains("category=touch_missed_ui"));

    // 3. DragNotDrivenScroll: dragging occurred on hit UI node, but scroll position didn't change (delta == 0)
    let drag_stalled = TouchLayerDiagnosticEntry {
        scenario_id: "SCN-05-CONNECTIONS".to_string(),
        raw_coords: Some(Vec2::new(200.0, 400.0)),
        density: 2.625,
        touch_input: Some((TouchPhase::Moved, Vec2::new(200.0, 350.0))),
        picking_hit: Some(dummy_entity),
        gesture_mode: "Recognizer".to_string(),
        scroll_before: Some(Vec2::new(0.0, 100.0)),
        scroll_after: Some(Vec2::new(0.0, 100.0)), // No delta
        rendered: false,
    };
    assert_eq!(
        drag_stalled.classify(),
        Some(TouchLayerDiagnosticCategory::DragNotDrivenScroll)
    );
    assert_eq!(
        drag_stalled.classify().unwrap().label(),
        "drag_not_driven_scroll"
    );
    assert!(
        drag_stalled
            .format_log()
            .contains("category=drag_not_driven_scroll")
    );

    // 4. PositionNotRendered: scroll position changed, but no visual render update occurred
    let position_not_rendered = TouchLayerDiagnosticEntry {
        scenario_id: "SCN-06-LOGS".to_string(),
        raw_coords: Some(Vec2::new(200.0, 400.0)),
        density: 2.625,
        touch_input: Some((TouchPhase::Moved, Vec2::new(200.0, 300.0))),
        picking_hit: Some(dummy_entity),
        gesture_mode: "Recognizer".to_string(),
        scroll_before: Some(Vec2::new(0.0, 100.0)),
        scroll_after: Some(Vec2::new(0.0, 150.0)), // Moved by 50px
        rendered: false,                           // Not rendered!
    };
    assert_eq!(
        position_not_rendered.classify(),
        Some(TouchLayerDiagnosticCategory::PositionNotRendered)
    );
    assert_eq!(
        position_not_rendered.classify().unwrap().label(),
        "position_not_rendered"
    );
    assert!(
        position_not_rendered
            .format_log()
            .contains("category=position_not_rendered")
    );
}

#[test]
fn test_touch_diagnostic_success_pipeline() {
    let dummy_entity = Entity::PLACEHOLDER;

    let success = TouchLayerDiagnosticEntry {
        scenario_id: "SCN-01-OVERVIEW".to_string(),
        raw_coords: Some(Vec2::new(100.0, 200.0)),
        density: 3.0,
        touch_input: Some((TouchPhase::Moved, Vec2::new(100.0, 150.0))),
        picking_hit: Some(dummy_entity),
        gesture_mode: "Pull".to_string(),
        scroll_before: Some(Vec2::new(0.0, 0.0)),
        scroll_after: Some(Vec2::new(0.0, 50.0)),
        rendered: true,
    };

    assert_eq!(success.classify(), None);
    assert!(success.format_log().contains("category=SUCCESS"));
    assert!(success.format_log().contains("rendered=true"));
}

#[test]
fn test_touch_diagnostic_log_resource() {
    let mut log = TouchLayerDiagnosticLog::default();
    assert!(!log.enabled);
    assert!(log.entries.is_empty());

    log.enabled = true;
    log.current_scenario = "SCN-03-PROFILES".to_string();
    assert_eq!(log.current_scenario, "SCN-03-PROFILES");
}

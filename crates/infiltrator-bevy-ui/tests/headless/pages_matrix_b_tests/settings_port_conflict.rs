//! Behavior cases for settings port conflict.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::pages::settings::settings_port_conflict::PortConflictOutcomeText;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_output::CommandOutput;
use lurker_core_contract::error::{ErrorCode, Failure};

/// The port-conflict row renders the repair's outcome, so a successful
/// repair is visible on the page instead of looking like the route-only
/// baseline.
#[test]
fn the_port_conflict_row_renders_the_repair_outcome() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let line = app
        .world_mut()
        .query_filtered::<Entity, With<PortConflictOutcomeText>>()
        .single(app.world())
        .expect("the port-conflict repair outcome line");
    let before = app
        .world()
        .get::<Text>(line)
        .expect("the outcome text")
        .0
        .clone();
    assert!(before.is_empty(), "no repair ran yet, got {before}");

    app.world_mut().commands().trigger(CommandExecutedEvent {
        command: UiCommand::RepairPortConflicts,
        request_id: RequestId::new(1),
        result: Ok(CommandOutput::Unit),
    });
    app.update();

    let repaired = app
        .world()
        .get::<Text>(line)
        .expect("the outcome text")
        .0
        .clone();
    assert!(
        repaired.contains("repaired"),
        "the successful repair is visible on the page, got {repaired}"
    );
}

/// A refused repair is rendered with the owner's typed failure.
#[test]
fn the_port_conflict_row_renders_a_refused_repair() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let line = app
        .world_mut()
        .query_filtered::<Entity, With<PortConflictOutcomeText>>()
        .single(app.world())
        .expect("the port-conflict repair outcome line");

    app.world_mut().commands().trigger(CommandExecutedEvent {
        command: UiCommand::RepairPortConflicts,
        request_id: RequestId::new(1),
        result: Err(Failure::new(
            ErrorCode::NotReady,
            "the port owner refused to release",
            true,
        )),
    });
    app.update();

    let refused = app
        .world()
        .get::<Text>(line)
        .expect("the outcome text")
        .0
        .clone();
    assert_eq!(refused, "the port owner refused to release");
}

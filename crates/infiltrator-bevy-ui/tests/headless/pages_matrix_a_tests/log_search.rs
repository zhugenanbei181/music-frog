//! test-intent: behavior
use super::{navigate_to, setup_matrix_a_app};
use crate::native_input::{click_entity, replace_text};
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::text::{TextColor, TextSpan};
use bevy::ui::prelude::{Display, Node};
use bevy::ui::widget::Text;
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_bevy_ui::command::DemoCommandSink;
use infiltrator_bevy_ui::pages::logs::{LogMessageText, LogsProjection, LogsProjectionUpdated};
use infiltrator_bevy_ui::pages::logs_rows::LogRowIdentity;
use infiltrator_bevy_ui::pages::logs_search::{ClearLogsSearch, LogsSearchError, LogsViewState};
use infiltrator_bevy_ui::route::Route;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface_snapshot::PageStatus;
use std::sync::Arc;

fn setup() -> (App, Arc<DemoCommandSink>, Entity) {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    app.add_plugins(ButtonPlugin);
    navigate_to(&mut app, Route::Logs);
    app.update();
    let field = app
        .world_mut()
        .query::<(Entity, &NativeTextField)>()
        .iter(app.world())
        .find(|(_, marker)| marker.0 == 11)
        .unwrap()
        .0;
    (app, sink, field)
}
fn edit(app: &mut App, field: Entity, value: &str) {
    click_entity(app, field);
    replace_text(app, value);
    app.update();
    app.update();
}
fn visible(app: &mut App) -> Vec<u64> {
    app.world_mut()
        .query::<(&LogRowIdentity, &Node)>()
        .iter(app.world())
        .filter(|(_, node)| node.display == Display::Flex)
        .map(|(identity, _)| identity.0)
        .collect()
}
fn highlights(app: &App, entity: Entity) -> Vec<String> {
    let accent = app.world().resource::<UiPalette>().accent;
    let mut values = Vec::new();
    if app.world().get::<TextColor>(entity).unwrap().0 == accent {
        values.push(app.world().get::<Text>(entity).unwrap().0.clone());
    }
    if let Some(children) = app.world().get::<Children>(entity) {
        for child in children {
            if app.world().get::<TextColor>(*child).unwrap().0 == accent {
                values.push(app.world().get::<TextSpan>(*child).unwrap().0.clone());
            }
        }
    }
    values
}
#[test]
fn native_regex_query_renders_real_spans_invalid_error_no_matches_and_clear() {
    let (mut app, sink, field) = setup();
    edit(&mut app, field, "github|timeout");
    assert_eq!(visible(&mut app), [1, 5]);
    let message = app
        .world_mut()
        .query::<(Entity, &LogMessageText)>()
        .iter(app.world())
        .find(|(_, marker)| marker.0 == 0)
        .unwrap()
        .0;
    assert_eq!(highlights(&app, message), ["github", "github"]);
    edit(&mut app, field, "(");
    assert!(visible(&mut app).is_empty());
    assert_eq!(
        app.world().resource::<LogsViewState>().0.status_key(),
        "logs_search_invalid"
    );
    let error = app
        .world_mut()
        .query_filtered::<&Node, With<LogsSearchError>>()
        .single(app.world())
        .unwrap();
    assert_eq!(error.display, Display::Flex);
    edit(&mut app, field, "no-match");
    assert!(visible(&mut app).is_empty());
    assert_eq!(
        app.world().resource::<LogsViewState>().0.status_key(),
        "logs_search_no_matches"
    );
    let clear = app
        .world_mut()
        .query_filtered::<Entity, With<ClearLogsSearch>>()
        .single(app.world())
        .unwrap();
    click_entity(&mut app, clear);
    app.update();
    assert!(
        app.world()
            .get::<TextField>(field)
            .unwrap()
            .0
            .text()
            .is_empty()
    );
    assert_eq!(visible(&mut app).len(), 5);
    assert!(
        sink.submitted().is_empty(),
        "search and recovery cannot submit controller commands"
    );
}
#[test]
fn native_log_query_survives_refresh_language_and_route_without_rebuilding_live_input() {
    let (mut app, sink, field) = setup();
    edit(&mut app, field, "github");
    let mut updated = LogsProjection::demo();
    updated.entries[0].message = "GitHub changed".into();
    app.world_mut().trigger(LogsProjectionUpdated(updated));
    app.update();
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("en-US");
    app.update();
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.text(),
        "github"
    );
    assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
    assert_eq!(visible(&mut app), [1]);
    navigate_to(&mut app, Route::Overview);
    navigate_to(&mut app, Route::Logs);
    app.update();
    let replacement = app
        .world_mut()
        .query::<(Entity, &NativeTextField)>()
        .iter(app.world())
        .find(|(_, marker)| marker.0 == 11)
        .unwrap()
        .0;
    assert_ne!(field, replacement);
    assert_eq!(
        app.world().get::<TextField>(replacement).unwrap().0.text(),
        "github"
    );
    assert!(sink.submitted().is_empty());
}

#[test]
fn log_source_failure_retains_actual_rows_and_new_session_does_not_reuse_old_facts() {
    let (mut app, _, field) = setup();
    edit(&mut app, field, "github");
    let mut failed = LogsProjection::demo();
    failed.status = PageStatus::Failed {
        failure: Failure::new(ErrorCode::Permission, "denied", false),
    };
    app.world_mut().trigger(LogsProjectionUpdated(failed));
    app.update();
    assert_eq!(visible(&mut app), [1]);
    assert_eq!(
        app.world().resource::<LogsViewState>().0.status_key(),
        "logs_search_unavailable"
    );
    let mut replacement = LogsProjection::demo();
    replacement.generation = 2;
    replacement.status = PageStatus::Loading;
    replacement.entries.clear();
    replacement.total_entries = 0;
    app.world_mut()
        .trigger(LogsProjectionUpdated(replacement.clone()));
    app.update();
    assert!(visible(&mut app).is_empty());
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.text(),
        "github"
    );
    replacement.status = PageStatus::Empty;
    app.world_mut().trigger(LogsProjectionUpdated(replacement));
    app.update();
    assert_eq!(
        app.world().resource::<LogsViewState>().0.status_key(),
        "logs_no_realtime_records"
    );
}

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
use bevy::ui_widgets::{Activate, ButtonPlugin};
use infiltrator_application::connection_grouping_fixtures::grouping_surface;
use infiltrator_bevy_ui::command::DemoCommandSink;
use infiltrator_bevy_ui::pages::connections::{
    CloseConnectionButton, ConnHostText, ConnectionsProjection, ConnectionsProjectionUpdated,
};
use infiltrator_bevy_ui::pages::connections_search::{
    ClearConnectionsSearch, ConnectionMatchTerm, ConnectionsSearchEmpty, ConnectionsSearchSummary,
};
use infiltrator_bevy_ui::pages::connections_view::{ConnectionRow, ConnectionsViewState};
use infiltrator_bevy_ui::route::Route;
use infiltrator_bevy_ui::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_contract::connection::ConnectionStreamPhase;
use std::sync::Arc;

fn setup() -> (App, Arc<DemoCommandSink>, Entity) {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    app.add_plugins(ButtonPlugin);
    navigate_to(&mut app, Route::Connections);
    let source = grouping_surface(app.world().resource::<LatestSurfaceSnapshot>().0.clone());
    app.world_mut().trigger(SurfaceSnapshotUpdated(source));
    app.update();
    app.update();
    let field = app
        .world_mut()
        .query::<(Entity, &NativeTextField)>()
        .iter(app.world())
        .find(|(_, field)| field.0 == 10)
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
fn visible(app: &mut App) -> Vec<usize> {
    app.world_mut()
        .query::<(&Node, &ConnectionRow)>()
        .iter(app.world())
        .filter(|(node, _)| node.display == Display::Flex)
        .map(|(_, row)| row.0)
        .collect()
}
fn highlight_text(app: &App, entity: Entity) -> Vec<String> {
    let accent = app.world().resource::<UiPalette>().accent;
    let mut text = Vec::new();
    if app
        .world()
        .get::<TextColor>(entity)
        .is_some_and(|color| color.0 == accent)
    {
        text.push(app.world().get::<Text>(entity).unwrap().0.clone());
    }
    if let Some(children) = app.world().get::<Children>(entity) {
        for child in children {
            if app
                .world()
                .get::<TextColor>(*child)
                .is_some_and(|color| color.0 == accent)
            {
                text.push(app.world().get::<TextSpan>(*child).unwrap().0.clone());
            }
        }
    }
    text
}

#[test]
fn native_connection_search_renders_actual_highlight_metadata_empty_and_clear_without_commands() {
    let (mut app, sink, field) = setup();
    edit(&mut app, field, "API-0");
    let matches = visible(&mut app);
    assert_eq!(matches.len(), 2);
    assert!(matches.contains(&0) && matches.contains(&8));
    let host = app
        .world_mut()
        .query::<(Entity, &ConnHostText)>()
        .iter(app.world())
        .find(|(_, host)| host.0 == 0)
        .unwrap()
        .0;
    assert_eq!(highlight_text(&app, host), ["api-0"]);
    edit(&mut app, field, "203.0.113.1");
    let metadata = app
        .world_mut()
        .query::<(Entity, &ConnectionMatchTerm)>()
        .iter(app.world())
        .find(|(_, row)| row.0 == 0)
        .unwrap()
        .0;
    assert_eq!(highlight_text(&app, metadata), ["203.0.113.1"]);
    edit(&mut app, field, "no-match");
    assert!(visible(&mut app).is_empty());
    let empty = app
        .world_mut()
        .query_filtered::<&Node, With<ConnectionsSearchEmpty>>()
        .single(app.world())
        .unwrap();
    assert_eq!(empty.display, Display::Flex);
    assert_eq!(
        app.world()
            .resource::<ConnectionsViewState>()
            .groups
            .matched_count(),
        0
    );
    let clear = app
        .world_mut()
        .query_filtered::<Entity, With<ClearConnectionsSearch>>()
        .single(app.world())
        .unwrap();
    click_entity(&mut app, clear);
    app.update();
    assert_eq!(app.world().get::<TextField>(field).unwrap().0.text(), "");
    assert_eq!(visible(&mut app).len(), 9);
    assert!(sink.submitted().is_empty());
}

#[test]
fn native_connection_search_survives_language_and_route_refresh_and_rejects_unavailable_destruction()
 {
    let (mut app, sink, field) = setup();
    edit(&mut app, field, "API-0");
    let selected = app.world().get::<TextField>(field).unwrap().0.selection();
    let caret = app.world().get::<TextField>(field).unwrap().0.cursor();
    app.insert_resource(UiLocale::new("en-US"));
    app.update();
    assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.selection(),
        selected
    );
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.cursor(),
        caret
    );
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.text(),
        "API-0"
    );
    let summary = app
        .world_mut()
        .query_filtered::<&Text, With<ConnectionsSearchSummary>>()
        .single(app.world())
        .unwrap();
    assert_eq!(summary.0, "Showing 2 of 9 connections");
    navigate_to(&mut app, Route::Rules);
    navigate_to(&mut app, Route::Connections);
    app.update();
    let restored = app
        .world_mut()
        .query::<(Entity, &NativeTextField)>()
        .iter(app.world())
        .find(|(_, field)| field.0 == 10)
        .unwrap()
        .0;
    assert_eq!(
        app.world().get::<TextField>(restored).unwrap().0.text(),
        "API-0"
    );
    assert_eq!(visible(&mut app).len(), 2);
    let unavailable = ConnectionsProjection {
        total_connections: 0,
        total_upload_bytes: 0,
        total_download_bytes: 0,
        stream_phase: ConnectionStreamPhase::Unavailable,
        connections: Vec::new(),
    };
    app.world_mut()
        .trigger(ConnectionsProjectionUpdated(unavailable));
    app.update();
    assert_eq!(visible(&mut app).len(), 2);
    assert!(
        !app.world()
            .resource::<ConnectionsViewState>()
            .groups
            .source_current()
    );
    let close = app
        .world_mut()
        .query::<(Entity, &CloseConnectionButton)>()
        .iter(app.world())
        .next()
        .unwrap()
        .0;
    app.world_mut().trigger(Activate { entity: close });
    app.update();
    assert!(sink.submitted().is_empty());
}

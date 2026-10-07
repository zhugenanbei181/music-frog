//! test-intent: behavior
use super::{navigate_to, setup_matrix_a_app};
use crate::native_input::{click_entity, replace_text};
use crate::support::subtree_has_text;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ui::prelude::{Display, Node};
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_application::connection_grouping_fixtures::grouping_snapshot;
use infiltrator_application::connection_rate_application::project_connection;
use infiltrator_bevy_ui::command::DemoCommandSink;
use infiltrator_bevy_ui::pages::connections::{
    ConnAggregationPill, ConnectionsProjection, ConnectionsProjectionUpdated,
};
use infiltrator_bevy_ui::pages::connections_groups::ConnectionGroupCard;
use infiltrator_bevy_ui::pages::connections_view::{ConnectionRow, ConnectionsViewState};
use infiltrator_bevy_ui::route::Route;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_contract::connection::ConnectionStreamPhase;
use infiltrator_domain::connection_view::ConnectionGroupingMode;
use infiltrator_domain::runtime::ConnectionSnapshot;
use std::sync::Arc;

fn publish(app: &mut App, snapshot: &ConnectionSnapshot) {
    app.world_mut()
        .trigger(ConnectionsProjectionUpdated(ConnectionsProjection {
            total_connections: snapshot.connections.len(),
            total_upload_bytes: snapshot.upload_total,
            total_download_bytes: snapshot.download_total,
            stream_phase: ConnectionStreamPhase::Live,
            connections: snapshot
                .connections
                .iter()
                .map(|row| project_connection(row, Default::default()))
                .collect(),
        }));
    app.update();
}
fn choose(app: &mut App, mode: ConnectionGroupingMode) {
    let entity = app
        .world_mut()
        .query::<(Entity, &ConnAggregationPill)>()
        .iter(app.world())
        .find(|(_, pill)| pill.0 == mode)
        .unwrap()
        .0;
    click_entity(app, entity);
    app.update();
}
fn cards(app: &mut App) -> Vec<(Entity, String)> {
    app.world_mut()
        .query::<(Entity, &ConnectionGroupCard)>()
        .iter(app.world())
        .map(|(entity, row)| (entity, row.0.clone()))
        .collect()
}

#[test]
fn native_grouping_renders_every_bucket_and_keeps_identity_on_language_change() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    app.add_plugins(ButtonPlugin);
    let (root, _) = navigate_to(&mut app, Route::Connections);
    publish(&mut app, &grouping_snapshot());
    choose(&mut app, ConnectionGroupingMode::ByProcess);
    let before = cards(&mut app);
    assert_eq!(before.len(), 8);
    assert!(before.iter().any(|(_, key)| key == "client-0"));
    assert!(before.iter().any(|(_, key)| key == "client-7"));
    assert!(subtree_has_text(app.world(), root, "↑ 2.00 KB / ↓ 4.00 KB"));
    app.insert_resource(UiLocale::new("en-US"));
    app.update();
    assert_eq!(cards(&mut app), before);
    assert!(subtree_has_text(
        app.world(),
        root,
        "By process · Groups: 8"
    ));
    assert!(subtree_has_text(app.world(), root, "2 connections"));
    assert!(sink.submitted().is_empty());
}

#[test]
fn native_grouping_search_refresh_and_return_to_flat_preserve_visibility() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    app.add_plugins(ButtonPlugin);
    let (root, _) = navigate_to(&mut app, Route::Connections);
    let mut snapshot = grouping_snapshot();
    publish(&mut app, &snapshot);
    choose(&mut app, ConnectionGroupingMode::ByProcess);
    let field = app
        .world_mut()
        .query::<(Entity, &NativeTextField)>()
        .iter(app.world())
        .find(|(_, field)| field.0 == 10)
        .unwrap()
        .0;
    click_entity(&mut app, field);
    replace_text(&mut app, "CLIENT-0");
    app.update();
    assert_eq!(cards(&mut app).len(), 1);
    assert_eq!(
        app.world().resource::<ConnectionsViewState>().groups.rows()[0].count,
        2
    );
    choose(&mut app, ConnectionGroupingMode::ByHost);
    assert_eq!(cards(&mut app)[0].1, "api-0.example.org");
    choose(&mut app, ConnectionGroupingMode::Flat);
    let visible: Vec<_> = app
        .world_mut()
        .query::<(&Node, &ConnectionRow)>()
        .iter(app.world())
        .filter(|(node, _)| node.display == Display::Flex)
        .map(|(_, row)| row.0)
        .collect();
    assert_eq!(visible.len(), 2);
    assert!(visible.contains(&0));
    assert!(visible.contains(&8));
    choose(&mut app, ConnectionGroupingMode::ByHost);
    snapshot
        .connections
        .retain(|row| row.id == "group-duplicate");
    snapshot.connections[0].upload = 4096;
    publish(&mut app, &snapshot);
    assert_eq!(cards(&mut app).len(), 1);
    assert!(subtree_has_text(app.world(), root, "↑ 4.00 KB / ↓ 2.00 KB"));
    click_entity(&mut app, field);
    replace_text(&mut app, "no-match");
    app.update();
    assert!(cards(&mut app).is_empty());
    assert!(sink.submitted().is_empty());
}

#[test]
fn connection_rows_reconcile_growth_and_shrink_and_restamp_stable_entities() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    navigate_to(&mut app, Route::Connections);
    let mut snapshot = grouping_snapshot();
    publish(&mut app, &snapshot);
    let before: Vec<_> = app
        .world_mut()
        .query::<(Entity, &ConnectionRow)>()
        .iter(app.world())
        .map(|(entity, row)| (entity, row.0))
        .collect();
    assert_eq!(before.len(), 9);
    snapshot.connections[0].metadata.host = "changed.example.org".into();
    snapshot.connections[0].upload = 8192;
    publish(&mut app, &snapshot);
    let after: Vec<_> = app
        .world_mut()
        .query::<(Entity, &ConnectionRow)>()
        .iter(app.world())
        .map(|(entity, row)| (entity, row.0))
        .collect();
    assert_eq!(after, before);
    let updated = before.iter().find(|(_, index)| *index == 0).unwrap().0;
    assert!(subtree_has_text(
        app.world(),
        updated,
        "changed.example.org:443"
    ));
    snapshot
        .connections
        .retain(|row| row.id == "group-duplicate");
    publish(&mut app, &snapshot);
    let remaining: Vec<_> = app
        .world_mut()
        .query::<(Entity, &ConnectionRow)>()
        .iter(app.world())
        .map(|(entity, row)| (entity, row.0))
        .collect();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].1, 0);
    assert!(
        before
            .iter()
            .all(|(entity, _)| app.world().get_entity(*entity).is_err())
    );
    assert!(sink.submitted().is_empty());
}

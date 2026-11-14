//! The Requests route mounts the tracker's own published records.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::image::Image;
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use infiltrator_bevy_ui::app::ContentSlot;
use infiltrator_bevy_ui::pages::requests::{
    RequestFeed, RequestRecordRow, RequestsPageRoot, requests_page,
};
use infiltrator_bevy_ui::route::{PageRoot, Route};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::theme::Theme;
use infiltrator_contract::request_tracker::RequestTrackerSnapshot;

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.world_mut().spawn(ContentSlot);
    app
}

#[test]
fn the_requests_route_mounts_every_published_record() {
    let mut app = test_app();
    let palette = UiPalette::new(&Theme::dark());
    let snapshot = RequestTrackerSnapshot::demo_fixture();
    app.world_mut()
        .commands()
        .spawn_scene(requests_page(&snapshot, RequestFeed::Recent, &palette));
    app.update();

    let world = app.world_mut();
    let mut roots = world.query_filtered::<Entity, With<RequestsPageRoot>>();
    let root = roots.single(world).expect("the Requests page mounted");
    let marker = world
        .get::<PageRoot>(root)
        .expect("the page names its route");
    assert_eq!(marker.0, Route::Requests);

    let mut rows = world.query::<&RequestRecordRow>();
    let ids: Vec<String> = rows.iter(world).map(|row| row.0.clone()).collect();
    assert_eq!(ids.len(), snapshot.records.len());
    for record in &snapshot.records {
        assert!(
            ids.contains(&record.id),
            "the published record {} reaches a row",
            record.id
        );
    }
}

#[test]
fn the_live_feed_mounts_only_the_owners_active_records() {
    let mut app = test_app();
    let palette = UiPalette::new(&Theme::dark());
    let snapshot = RequestTrackerSnapshot::demo_fixture();
    app.world_mut()
        .commands()
        .spawn_scene(requests_page(&snapshot, RequestFeed::Live, &palette));
    app.update();

    let world = app.world_mut();
    let mut rows = world.query::<&RequestRecordRow>();
    let ids: Vec<String> = rows.iter(world).map(|row| row.0.clone()).collect();
    assert_eq!(ids, vec!["req-2".to_owned()]);
}

//! Behavior cases for route.
//! test-intent: behavior

use super::*;

/// Navigating across multiple routes replaces the bounded subtree below ContentSlot
/// and stamps the corresponding PageRoot marker.
#[test]
fn route_switching_mounts_target_page_scene_idempotently() {
    let mut app = mounted_default();
    let world = app.world_mut();

    let mut roots = world.query::<&PageRoot>();
    assert_eq!(
        roots.iter(world).next().expect("overview page root").0,
        Route::Overview
    );

    // Navigate to Proxies route.
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Proxies));
    app.update();

    let world = app.world_mut();
    let mut roots = world.query::<&PageRoot>();
    assert_eq!(
        roots.iter(world).next().expect("proxies page root").0,
        Route::Proxies
    );

    // Navigate to Rules route.
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Rules));
    app.update();

    let world = app.world_mut();
    let mut roots = world.query::<&PageRoot>();
    assert_eq!(
        roots.iter(world).next().expect("rules page root").0,
        Route::Rules
    );

    // Navigate back to Overview.
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Overview));
    app.update();

    let world = app.world_mut();
    let mut roots = world.query::<&PageRoot>();
    assert_eq!(
        roots.iter(world).next().expect("back to overview").0,
        Route::Overview
    );
}

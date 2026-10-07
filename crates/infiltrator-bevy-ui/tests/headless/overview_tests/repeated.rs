//! Behavior cases for repeated.
//! test-intent: behavior

use super::*;

/// Re-triggering the same route must never stack pages: cross-frame it
/// is a no-op (ids stable), and two triggers flushed in the same frame
/// still converge on exactly one mounted page.
#[test]
fn repeated_same_route_triggers_do_not_stack() {
    let mut app = mounted_default();
    let (root_before, _) = page_root(app.world_mut());

    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Overview));
    app.update();
    let (root_after, _) = page_root(app.world_mut());
    assert_eq!(
        root_after, root_before,
        "settled same-route trigger is a no-op"
    );

    let mut commands = app.world_mut().commands();
    commands.trigger(RouteChanged(Route::Overview));
    commands.trigger(RouteChanged(Route::Overview));
    app.update();

    let world = app.world_mut();
    let mut roots = world.query::<&PageRoot>();
    assert_eq!(
        roots.iter(world).count(),
        1,
        "same-frame duplicate triggers converge on one page"
    );
    let (root_last, _) = page_root(world);
    assert_eq!(
        root_last, root_before,
        "replacement stays under one root id set"
    );
}

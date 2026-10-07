//! Behavior cases for overview mounts.
//! test-intent: behavior

use super::*;

/// The route mounts the Overview page as a child of the shell's content
/// slot, exactly once; the shell's title row shows the 核心概览 heading
/// and the banner carries the running state.
#[test]
fn overview_mounts_under_the_content_slot() {
    let mut app = mounted_default();
    let world = app.world_mut();
    let slot = content_slot(world);
    let (root, route) = page_root(world);
    assert_eq!(route, Route::Overview);
    assert_eq!(
        world.get::<ChildOf>(root).expect("page parented").0,
        slot,
        "page root is a direct child of the content slot"
    );

    let mut headings = world.query::<(&Text, &TextRole)>();
    let title = headings
        .iter(world)
        .find(|(text, role)| role.0 == Role::Heading && text.0 == "核心概览");
    assert!(
        title.is_some(),
        "the shell title row mounts the 核心概览 heading"
    );

    let (_, state_text, _) = line(world, OverviewLineKind::State);
    assert_eq!(state_text, "运行中", "the banner spells the running state");
}

//! Behavior cases for injected.
//! test-intent: behavior

use super::*;

/// The page renders whatever source the shell injects — proof that the
/// demo fixture and the real (future) source share one seam and nothing
/// on the page is welded to the fixture.
#[test]
fn injected_source_drives_the_page_not_the_demo_fixture() {
    let mut app = mounted_app_with(StubSource);
    let world = app.world_mut();
    let (_, upload, _) = line(world, OverviewLineKind::Upload);
    assert_eq!(upload, "↑ 244.14 KB/s", "stub rate, not the demo 1.40 MB/s");
    let (_, connections) = chip_value(world, OverviewChipKind::Connections);
    assert_eq!(connections, "3", "stub count, not the demo 12");
    let (_, memory) = chip_value(world, OverviewChipKind::Memory);
    assert_eq!(memory, "70.00 MB", "stub memory, not the demo 96");
    let (_, download) = chip_value(world, OverviewChipKind::Download);
    assert_eq!(download, "3.95 KB/s");
    assert!(
        pill_selected(world, ProxyMode::Direct),
        "stub mode selected"
    );
}

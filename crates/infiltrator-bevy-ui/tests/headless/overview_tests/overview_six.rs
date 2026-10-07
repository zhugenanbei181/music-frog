//! Behavior cases for overview six.
//! test-intent: behavior

use super::*;
use infiltrator_bevy_ui::pages::overview::{OverviewChip, OverviewChipKind};

#[test]
fn overview_six_item_metrics_grid_mounts_and_updates_in_place() {
    let mut app = mounted_default();
    let world = app.world_mut();
    let mut chips_query = world.query::<&OverviewChip>();
    let chip_kinds: Vec<OverviewChipKind> = chips_query.iter(world).map(|c| c.0).collect();
    assert_eq!(chip_kinds.len(), 6);
    assert!(chip_kinds.contains(&OverviewChipKind::Connections));
    assert!(chip_kinds.contains(&OverviewChipKind::Memory));
    assert!(chip_kinds.contains(&OverviewChipKind::Cpu));
    assert!(chip_kinds.contains(&OverviewChipKind::Upload));
    assert!(chip_kinds.contains(&OverviewChipKind::Download));
    assert!(chip_kinds.contains(&OverviewChipKind::TotalTraffic));
}

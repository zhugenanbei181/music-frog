//! Behavior cases for chips.
//! test-intent: behavior

use super::*;

/// The metrics band draws semantic plates: 上传 the up arrow, 下载 the
/// down arrow (never the Plus/FileText stand-ins), connections the
/// activity pulse and memory the zap.
#[test]
fn chips_carry_their_semantic_icon_plates() {
    let mut app = mounted_default();
    let world = app.world_mut();
    let mut chips = world.query::<(Entity, &OverviewChip)>();
    let mounted: Vec<(Entity, OverviewChipKind)> =
        chips.iter(world).map(|(id, chip)| (id, chip.0)).collect();
    let expected = [
        (OverviewChipKind::Connections, IconId::Activity),
        (OverviewChipKind::Memory, IconId::Zap),
        (OverviewChipKind::Cpu, IconId::Settings),
        (OverviewChipKind::Upload, IconId::ArrowUp),
        (OverviewChipKind::Download, IconId::ArrowDown),
        (OverviewChipKind::TotalTraffic, IconId::Globe),
    ];
    for (kind, want) in expected {
        let (chip_id, _) = mounted
            .iter()
            .find(|(_, mounted_kind)| *mounted_kind == kind)
            .unwrap_or_else(|| panic!("no {kind:?} chip mounted"));
        assert_eq!(
            chip_icon_plate(world, *chip_id),
            want,
            "{kind:?} chip draws its semantic plate"
        );
    }
}

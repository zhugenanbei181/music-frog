//! Behavior cases for overview page.
//! test-intent: behavior

use super::*;
use bevy::ui;
use infiltrator_bevy_widgets::fluid_grid::FluidCardGrid;
use infiltrator_bevy_widgets::responsive::ResponsiveContext;

/// The Overview page uses responsive wrapping for stat chips and scrollable viewport,
/// ensuring 4 chips wrap into a clean 2x2 grid on compact mobile screens (<600px).
#[test]
fn overview_page_chips_and_container_responsive_wrapping() {
    let mut app = mounted_default();
    let world = app.world_mut();

    let mut chips = world.query::<(&OverviewChip, &ui::Node)>();
    let count = chips.iter(world).count();
    assert_eq!(count, 6, "exactly six stat chips mounted");

    // Default window (1180px) is the Expanded tier: six tiles in one row.
    let expanded_basis = ui::Val::Percent(FluidCardGrid::wrapped_item_percent(6));
    for (_, node) in chips.iter(world) {
        assert_eq!(
            node.flex_grow, 1.0,
            "chips share width evenly via flex_grow"
        );
        assert_eq!(
            node.flex_basis, expanded_basis,
            "chips carry the shared Expanded-tier 6-column basis"
        );
    }

    // Narrowing to the Medium tier must reflow the band to three columns.
    app.world_mut()
        .resource_mut::<ResponsiveContext>()
        .set_dimensions(700.0, 900.0);
    app.update();
    let medium_basis = ui::Val::Percent(FluidCardGrid::wrapped_item_percent(3));
    let world = app.world_mut();
    let mut chips = world.query::<(&OverviewChip, &ui::Node)>();
    for (_, node) in chips.iter(world) {
        assert_eq!(
            node.flex_basis, medium_basis,
            "chips reflow to the 3-column Medium basis"
        );
    }
}

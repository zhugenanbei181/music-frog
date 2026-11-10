//! Behavior cases for the navigation-only rail.
//! test-intent: behavior
//!
//! The rail's contract has three parts, each asserted against the mounted tree
//! instead of a description:
//!
//! * **Content** — navigation and nothing else. The desktop reference gives the
//!   sidebar destinations only, and the space-utilization standard turns that
//!   into `chrome_band` 0%, so a status card, a brand block, a rate line or a
//!   preference button in this column is a gate failure, not a style choice.
//! * **Rows** — the contract registry's, filtered by what this surface owns. A
//!   surface keeps no second navigation list.
//! * **Geometry and reading** — the reference's measured row, with the current
//!   destination stated as a quiet wash plus a narrow indicator that the widget
//!   layer paints and this shell only mounts.

use super::*;
use crate::support::subtree_in_tree_order;
use bevy::color::Color;
use bevy::ui::prelude::Val;
use bevy::ui_widgets::ScrollArea;
use infiltrator_bevy_ui::shell_scene::nav::{
    NAV_INDICATOR_HEIGHT_PX, NAV_INDICATOR_WIDTH_PX, NAV_ROW_HEIGHT_PX, NAV_ROW_ICON_PX,
};
use infiltrator_bevy_widgets::palette::{NAV_SELECTION_WASH_ALPHA, rem};
use infiltrator_contract::responsive_viewport::SidebarForm;
use infiltrator_contract::shell_navigation::{NavigationTarget, desktop_items};
use infiltrator_contract::shell_sidebar_presentation::separator_for_form;
use infiltrator_contract::surface::SurfaceKind;

/// One rail row, read out of the world as owned values so a test can compare
/// several rows without holding a world borrow.
#[derive(Debug, PartialEq)]
struct Row {
    route: Route,
    active: bool,
    fill: Color,
    kind: NavRowKind,
    indicator_shown: bool,
    indicator_size: Option<(Val, Val)>,
    min_height: Val,
    icon_size: Option<Val>,
}

fn rail_rows(app: &mut App) -> Vec<Row> {
    let world = app.world_mut();
    let mut indicators = world.query::<(&NavIndicator, &Node)>();
    let mut nodes = world.query::<&Node>();
    let mut items = world.query::<(
        &SidebarNavItem,
        &NavActive,
        &NavRowKind,
        &BackgroundColor,
        &Children,
    )>();

    // The rows are read in the order the column paints them, not in the order
    // the archetype happens to store them: descending the rail's scroll area
    // is what makes this list the stack the user reads top to bottom.
    let rail = rail_entity(world);
    let mut found: Vec<Row> = Vec::new();
    for entity in subtree_in_tree_order(world, rail) {
        let Ok((item, active, kind, fill, children)) = items.get(world, entity) else {
            continue;
        };
        let node = nodes.get(world, entity).expect("a nav row is a node");
        let mut indicator_shown = false;
        let mut indicator_size = None;
        let mut icon_size = None;
        for child in children.iter() {
            if let Ok((_, ind)) = indicators.get(world, *child) {
                indicator_shown = ind.display == Display::Flex;
                indicator_size = Some((ind.width, ind.height));
            } else if let Ok(child_node) = nodes.get(world, *child)
                && child_node.width == rem(NAV_ROW_ICON_PX)
            {
                icon_size = Some(child_node.width);
            }
        }
        found.push(Row {
            route: item.0,
            active: active.0,
            fill: fill.0,
            kind: *kind,
            indicator_shown,
            indicator_size,
            min_height: node.min_height,
            icon_size,
        });
    }
    found
}

fn rail_children(app: &mut App) -> Vec<Entity> {
    let world = app.world_mut();
    let rail = rail_entity(world);
    world.get::<Children>(rail).expect("rail children").to_vec()
}

#[test]
fn the_rail_mounts_the_registry_rows_and_the_group_headings_only() {
    let mut app = mounted_shell();

    // One child: the navigation column. No brand block, no routing pills, no
    // master switches, no profile card, no shortcut matrix, no rate footer, no
    // foot caption, no preference buttons.
    let children = rail_children(&mut app);
    assert_eq!(children.len(), 1, "the rail's only block is the nav column");
    assert!(
        subtree_contains::<SidebarNavItem>(app.world(), children[0]),
        "the single block is the navigation column"
    );
    assert!(
        subtree_contains::<ScrollArea>(app.world(), children[0]),
        "the column is the shell's scroll area, so rows past the fold stay reachable"
    );

    let expected = desktop_items(SurfaceKind::BevyDesktop);
    let mounted: Vec<Route> = rail_rows(&mut app)
        .into_iter()
        .map(|row| row.route)
        .collect();
    let wanted: Vec<Route> = expected
        .iter()
        .map(|item| match item.target {
            NavigationTarget::Shared(page) => Route::from_shell_page(page),
            NavigationTarget::SurfaceLocal(id) => {
                panic!("the registry handed {id} to a surface that does not own it")
            }
        })
        .collect();
    assert_eq!(
        mounted, wanted,
        "the rows are the registry's, painted in registry order top to bottom"
    );
    assert_eq!(
        mounted.len(),
        7,
        "Bevy folds traffic into its overview, so it mounts the seven shared destinations"
    );

    let world = app.world_mut();
    let mut headings = world.query::<&NavGroupHeading>();
    let painted = headings.iter(world).count();
    let groups: Vec<usize> = expected
        .iter()
        .enumerate()
        .filter(|(index, item)| *index == 0 || item.group != expected[*index - 1].group)
        .map(|(index, _)| index)
        .collect();
    // Group membership is a registry fact; whether the column *paints* it is
    // the contract's separator policy, which currently answers `None` for
    // every desktop form (FlClash measures one uninterrupted destination
    // list). The column reads that policy, so this assertion fails if either
    // side drifts — and it never restates the answer as a local boolean.
    if separator_for_form(SidebarForm::Standard).paints_heading() {
        assert_eq!(painted, groups.len(), "one heading where each group begins");
    } else {
        assert_eq!(
            painted, 0,
            "the contract paints no group heading, so the column is one list"
        );
    }
}

#[test]
fn the_row_geometry_is_the_reference_measurement() {
    let mut app = mounted_shell();
    let rows = rail_rows(&mut app);
    assert_eq!(rows.len(), 7);
    let row_height = rem(NAV_ROW_HEIGHT_PX);
    let icon = rem(NAV_ROW_ICON_PX);
    for row in &rows {
        assert_eq!(
            row.min_height, row_height,
            "{:?} keeps the adjudicated 44u row height",
            row.route
        );
        assert_eq!(
            row.icon_size,
            Some(icon),
            "{:?} carries the reference's 24u icon box",
            row.route
        );
    }
}

/// Every row states the current destination the same way: a quiet wash and a
/// narrow indicator, never the solid accent capsule a chosen member wears.
fn assert_wash_reading(rows: &[Row], wash: Color) {
    for row in rows {
        assert_eq!(
            row.kind,
            NavRowKind::CurrentDestination,
            "a nav row is not a chosen member: {:?} must not wear the accent capsule",
            row.route
        );
        assert_eq!(
            row.fill,
            if row.active { wash } else { Color::NONE },
            "{:?} states where the user is with a wash over the column's ground",
            row.route
        );
        assert_eq!(
            row.indicator_shown, row.active,
            "{:?} shows its indicator exactly when it is current",
            row.route
        );
        if row.active {
            assert_eq!(
                row.indicator_size,
                Some((rem(NAV_INDICATOR_WIDTH_PX), rem(NAV_INDICATOR_HEIGHT_PX))),
                "the current row's indicator is the reference's 3u x 16u bar"
            );
        } else {
            assert!(
                row.indicator_size.is_some(),
                "{:?} still mounts its indicator so the stack's geometry cannot shift",
                row.route
            );
        }
    }
}

#[test]
fn the_current_destination_wears_a_wash_and_only_it_shows_the_indicator() {
    let mut app = mounted_shell();
    let dark = UiPalette::new(&Theme::dark());
    assert_eq!(NAV_SELECTION_WASH_ALPHA, 0.08);

    let rows = rail_rows(&mut app);
    assert_wash_reading(&rows, dark.nav_selection_fill());
    assert_eq!(
        rows.iter().filter(|row| row.active).count(),
        1,
        "exactly one row is the current destination"
    );
}

#[test]
fn a_route_switch_restamps_the_same_rows_and_moves_the_indicator() {
    let mut app = mounted_shell();
    let dark = UiPalette::new(&Theme::dark());
    let before = rail_rows(&mut app);
    let proxies = {
        let world = app.world_mut();
        let mut items = world.query::<(Entity, &SidebarNavItem)>();
        items
            .iter(world)
            .find(|(_, item)| item.0 == Route::Proxies)
            .expect("proxies row")
            .0
    };
    app.world_mut()
        .commands()
        .trigger(Activate { entity: proxies });
    app.world_mut()
        .insert_resource(ActiveRoute(Some(Route::Proxies)));
    app.update();

    let after = rail_rows(&mut app);
    assert_eq!(
        after.iter().map(|row| row.route).collect::<Vec<_>>(),
        before.iter().map(|row| row.route).collect::<Vec<_>>(),
        "the switch restamps the mounted rows; no row is remounted"
    );
    assert_wash_reading(&after, dark.nav_selection_fill());
    let current: Vec<Route> = after
        .iter()
        .filter(|row| row.active)
        .map(|row| row.route)
        .collect();
    assert_eq!(
        current,
        vec![Route::Proxies],
        "the wash and the indicator travelled to Proxies together"
    );
}

#[test]
fn a_folded_page_keeps_the_settings_row_current() {
    let mut app = mounted_shell();
    let dns = {
        let world = app.world_mut();
        let mut items = world.query::<(Entity, &SidebarNavItem)>();
        items
            .iter(world)
            .find(|(_, item)| item.0 == Route::Settings)
            .expect("settings row")
            .0
    };
    app.world_mut()
        .insert_resource(ActiveRoute(Some(Route::Dns)));
    app.update();

    let rows = rail_rows(&mut app);
    let current: Vec<Route> = rows
        .iter()
        .filter(|row| row.active)
        .map(|row| row.route)
        .collect();
    assert_eq!(
        current,
        vec![Route::Settings],
        "DNS folds under Settings, so the rail says so; the mount is unchanged ({dns:?})"
    );
}

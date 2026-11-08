//! Behavior cases for overview page.
//! test-intent: behavior

use super::*;
use bevy::app::App;
use bevy::ecs::component::Component;
use bevy::ecs::query::With;
use bevy::ui;
use bevy::ui::AlignSelf;
use infiltrator_bevy_widgets::fluid_grid::FluidCardGrid;
use infiltrator_bevy_widgets::responsive::ResponsiveContext;
use infiltrator_contract::design_resources;
use infiltrator_contract::overview_layout::OverviewCardKind;

/// The desktop dashboard opens with its dense module grid, not a full-width
/// globe visualization: the card band is an earlier sibling of the globe slot.
#[test]
fn overview_dashboard_grid_precedes_globe_exploration_slot() {
    let mut app = mounted_default();
    let world = app.world_mut();
    let mut card_bands = world.query::<(Entity, &OverviewCardSlots)>();
    let (card_band, _) = card_bands
        .iter(world)
        .next()
        .expect("the reorderable dashboard band mounts");
    let mut globe_slots = world.query::<(Entity, &GlobeSectionSlot)>();
    let (globe_slot, _) = globe_slots
        .iter(world)
        .next()
        .expect("the globe exploration slot mounts");

    let parent = world
        .get::<ChildOf>(globe_slot)
        .expect("the globe slot is parented")
        .0;
    let children = world
        .get::<Children>(parent)
        .expect("the dashboard band and globe slot share a parent");
    let card_index = children
        .iter()
        .position(|child| *child == card_band)
        .expect("the card band shares the page root");
    let globe_index = children
        .iter()
        .position(|child| *child == globe_slot)
        .expect("the globe slot shares the page root");
    assert!(
        card_index < globe_index,
        "the module grid must precede the globe slot"
    );
}

/// The globe exploration card stays in the lower-right band instead of
/// pushing the dashboard back into a single vertical column.
#[test]
fn overview_globe_slot_is_a_right_aligned_half_width_exploration_card() {
    let mut app = mounted_default();
    let world = app.world_mut();
    let mut slots = world.query::<(&GlobeSectionSlot, &ui::Node)>();
    let (_, node) = slots
        .iter(world)
        .next()
        .expect("the globe exploration slot mounts");
    assert_eq!(node.width, ui::Val::Percent(50.0));
    assert_eq!(node.align_self, AlignSelf::FlexEnd);
}

/// The page-root child that first carries `T`, as its index in the root's
/// child order. The Overview flow guards read this instead of matching
/// component names as strings.
fn root_child_index<T: Component>(world: &mut World, root: Entity) -> usize {
    let mut markers = world.query_filtered::<Entity, With<T>>();
    let marker = markers.iter(world).next().expect("the marker is mounted");
    let children: Vec<Entity> = world
        .get::<Children>(root)
        .expect("the page root has children")
        .iter()
        .copied()
        .collect();
    children
        .iter()
        .position(|child| *child == marker)
        .expect("the marker sits on a direct child of the page root")
}

/// The phone dashboard opens with the owner's control band — the outbound mode
/// selector, the two speed cards and the takeover switches — ahead of the
/// page's own detail cards, exactly the reference's first screen. The owner's
/// card order stays authoritative inside the band; the compact tier only decides
/// that the band precedes the detail cards instead of burying it under them.
#[test]
fn overview_compact_mounts_the_owner_band_before_the_detail_cards() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::new(DemoOverviewSource::running()));
    app.insert_resource(ShellLayoutState::from_width(390.0));
    app.update();

    let world = app.world_mut();
    let (root, _) = page_root(world);
    let banner = root_child_index::<OverviewStatusCard>(world, root);
    let band = root_child_index::<OverviewCardSlots>(world, root);
    let health = root_child_index::<OverviewServiceHealthCard>(world, root);
    let gauges = root_child_index::<OverviewResourceGaugeCard>(world, root);
    let policy = root_child_index::<OverviewPolicyGroupCard>(world, root);
    assert!(
        banner < band && band < health && health < gauges && gauges < policy,
        "the compact flow must read banner → owner band → service health → \
         resource gauges → policy grid, got {banner} {band} {health} {gauges} {policy}"
    );
}
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

#[test]
fn overview_cards_reflow_across_shared_tier_columns() {
    let mut app = mounted_default();

    let read_basis = |app: &mut App| {
        app.world_mut()
            .query::<(&OverviewCardSlot, &ui::Node)>()
            .iter(app.world())
            .map(|(_, node)| node.width)
            .collect::<Vec<_>>()
    };

    let compact_basis = ui::Val::Percent(FluidCardGrid::wrapped_item_percent(1));
    app.world_mut()
        .resource_mut::<ResponsiveContext>()
        .set_dimensions(390.0, 844.0);
    app.update();
    assert_eq!(
        read_basis(&mut app),
        vec![compact_basis; OverviewCardKind::DEFAULT_ORDER.len()]
    );

    let tablet_basis = ui::Val::Percent(FluidCardGrid::wrapped_item_percent(2));
    app.world_mut()
        .resource_mut::<ResponsiveContext>()
        .set_dimensions(720.0, 800.0);
    app.update();
    assert_eq!(
        read_basis(&mut app),
        vec![tablet_basis; OverviewCardKind::DEFAULT_ORDER.len()]
    );

    let wide_basis = ui::Val::Percent(FluidCardGrid::wrapped_item_percent(3));
    app.world_mut()
        .resource_mut::<ResponsiveContext>()
        .set_dimensions(1920.0, 1080.0);
    app.update();
    assert_eq!(
        read_basis(&mut app),
        vec![wide_basis; OverviewCardKind::DEFAULT_ORDER.len()]
    );
}

/// The hero is one wide card with a reading column and an action column: the
/// exit address is the page's one large reading, and the place and the
/// organisation sit under it as the provenance chips. A banner that buries the
/// address in a pill next to a bigger state word is not the reference hero.
#[test]
fn overview_hero_reads_the_exit_address_at_display_size() {
    let mut app = mounted_default();
    let world = app.world_mut();
    let mut banners = world.query::<(Entity, &OverviewStatusCard)>();
    let (banner, _) = banners.iter(world).next().expect("the hero band mounts");
    let inside = descendants(world, banner);

    let mut facts = world.query::<(Entity, &PublicIpText, &TextRole, &Text)>();
    let hero: Vec<(PublicIpTextKind, design_resources::TextRole, String)> = facts
        .iter(world)
        .filter(|(entity, ..)| inside.contains(entity))
        .map(|(_, marker, role, text)| (marker.0, role.0, text.0.clone()))
        .collect();
    assert!(
        hero.iter()
            .any(|(kind, role, value)| *kind == PublicIpTextKind::Ip
                && *role == design_resources::TextRole::Display
                && value == "203.0.113.7"),
        "the hero must carry the exit address at display size, got {hero:?}"
    );
    for kind in [PublicIpTextKind::Location, PublicIpTextKind::Isp] {
        assert!(
            hero.iter()
                .any(|(marked, role, _)| *marked == kind
                    && *role == design_resources::TextRole::Caption),
            "the hero must carry the {kind:?} provenance chip"
        );
    }

    let mut lines = world.query::<(Entity, &OverviewLine, &TextRole)>();
    let state_role = lines
        .iter(world)
        .filter(|(entity, ..)| inside.contains(entity))
        .find(|(_, marker, _)| marker.0 == OverviewLineKind::State)
        .map(|(_, _, role)| role.0)
        .expect("the hero carries the state word");
    assert_eq!(
        state_role,
        design_resources::TextRole::Heading,
        "the state word ranks below the exit address it qualifies"
    );
}

/// The page's largest reading owns its own band: the dual-series trend chart
/// is a sibling of the card grid, not a child of the traffic card. One slot
/// three times taller than the card beside it is what left a hole in the row.
#[test]
fn overview_trend_chart_is_a_page_band_not_a_traffic_card_child() {
    let mut app = mounted_default();
    let world = app.world_mut();
    let mut cards = world.query::<(Entity, &OverviewTrafficCard)>();
    let (traffic_card, _) = cards.iter(world).next().expect("the traffic card mounts");
    let inside = descendants(world, traffic_card);

    let mut details = world.query::<(Entity, &OverviewTrafficDetailSection)>();
    let (detail, _) = details
        .iter(world)
        .next()
        .expect("the trend section mounts");
    assert!(
        !inside.contains(&detail),
        "the trend chart must not stretch one card slot"
    );

    let band_parent = world
        .get::<ChildOf>(detail)
        .expect("the trend band is parented")
        .0;
    let mut bands = world.query::<(Entity, &OverviewCardSlots)>();
    let (card_band, _) = bands.iter(world).next().expect("the card band mounts");
    let card_band_parent = world
        .get::<ChildOf>(card_band)
        .expect("the card band is parented")
        .0;
    assert_eq!(band_parent, card_band_parent, "one page root orders both");
    let order: Vec<Entity> = world
        .get::<Children>(card_band_parent)
        .expect("the page root has children")
        .iter()
        .copied()
        .collect();
    let band_index = order
        .iter()
        .position(|entity| *entity == card_band)
        .expect("the card band is a page child");
    assert_eq!(
        order.iter().position(|entity| *entity == detail),
        Some(band_index + 1),
        "the trend band follows the card band it explains"
    );
}

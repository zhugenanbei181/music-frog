//! Behavior cases for the navigation-only rail.
//! test-intent: behavior
//!
//! The sidebar used to mount a second live copy of the system-proxy and TUN
//! switches beside the Overview band's cards, wired to the same commands. The
//! column is navigation only now, so the invariant this file holds is the
//! *one mount* rule: the dashboard's fixed controls exist exactly once in the
//! whole tree and the sidebar subtree carries none of them. The single mount's
//! command behaviour is `overview_master_switch_uses_shared_toggle_policy_and_command_sink`.

use super::*;
use bevy::ecs::world::World;
use infiltrator_bevy_ui::app::SidebarPanel;
use infiltrator_bevy_ui::pages::overview_cards::{SystemProxyMasterCard, TunMasterCard};
use infiltrator_bevy_ui::pages::overview_restamp::OverviewMasterSwitchButton;
use infiltrator_contract::system_toggle::SystemToggle;

fn descendants(world: &World, root: Entity) -> Vec<Entity> {
    let mut found = Vec::new();
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if let Some(children) = world.get::<Children>(entity) {
            for child in children.iter() {
                found.push(*child);
                stack.push(*child);
            }
        }
    }
    found
}

#[test]
fn the_master_switches_have_one_mount_and_the_sidebar_carries_none() {
    let mut app = setup_matrix_b_app(Arc::new(DemoCommandSink::accepting()));
    let world = app.world_mut();

    let mut proxy_cards = world.query::<(Entity, &SystemProxyMasterCard)>();
    let proxy_card: Vec<Entity> = proxy_cards.iter(world).map(|(entity, _)| entity).collect();
    assert_eq!(
        proxy_card.len(),
        1,
        "one system-proxy card in the whole tree"
    );
    let mut tun_cards = world.query::<(Entity, &TunMasterCard)>();
    assert_eq!(
        tun_cards.iter(world).count(),
        1,
        "one TUN card in the whole tree"
    );

    let mut buttons = world.query::<(Entity, &OverviewMasterSwitchButton)>();
    let switches: Vec<(Entity, SystemToggle)> = buttons
        .iter(world)
        .map(|(entity, button)| (entity, button.toggle))
        .collect();
    assert_eq!(
        switches.len(),
        2,
        "the dashboard mounts one actionable control per master toggle"
    );

    let mut rails = world.query::<(Entity, &SidebarPanel)>();
    let rail = rails.single(world).expect("the shell mounts one rail").0;
    let rail_subtree = descendants(world, rail);
    for (entity, _) in &switches {
        assert!(
            !rail_subtree.contains(entity),
            "the navigation column must not carry a second copy of a master switch"
        );
    }
    assert!(
        !rail_subtree.contains(&proxy_card[0]),
        "the navigation column must not carry the dashboard's cards"
    );
}

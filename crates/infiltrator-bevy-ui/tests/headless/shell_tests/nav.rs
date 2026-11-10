//! Behavior cases for nav.
//! test-intent: behavior

use super::*;
use infiltrator_contract::a11y::ShellA11yNode;
use infiltrator_contract::shell_navigation::{NavigationTarget, desktop_items};
use infiltrator_contract::surface::SurfaceKind;
use infiltrator_presentation::routing_intent_projection::routing_intent_copy;
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::routing_intent::RoutingIntent;

/// One named control, read out of the world as owned values.
#[derive(Debug, PartialEq)]
struct Named {
    label: String,
    disabled: bool,
    role: accesskit::Role,
}

/// The controls carrying `C` under `root`, in the order the surface paints
/// them.
///
/// The order is the claim being tested, so the read must descend the tree: an
/// ECS query iterates archetypes, and two runs of the same shell hand the same
/// seven rows over in a different sequence. Comparing that against the
/// registry's order measures storage, not the column the user reads.
fn named_in_painted_order<C: Component>(world: &World, root: Entity) -> Vec<Named> {
    crate::support::subtree_in_tree_order(world, root)
        .into_iter()
        .filter(|entity| world.get::<C>(*entity).is_some())
        .map(|entity| {
            let node = world
                .get::<AccessibilityNode>(entity)
                .unwrap_or_else(|| panic!("{entity:?} carries no accessibility row"));
            Named {
                label: node.label().unwrap_or_default().to_owned(),
                disabled: node.is_disabled(),
                role: node.role(),
            }
        })
        .collect()
}

#[test]
fn nav_entries_mode_pills_and_content_region_carry_semantics() {
    let mut app = mounted_shell();
    let world = app.world_mut();

    // The rows are the registry's, not the route list's: the shell mounts the
    // shared destinations this surface owns and folds the leaf pages under
    // Settings, where the settings index reaches them.
    let rail = rail_entity(world);
    let nav_labels = named_in_painted_order::<SidebarNavItem>(world, rail);
    let expected_nav_labels: Vec<Named> = desktop_items(SurfaceKind::BevyDesktop)
        .iter()
        .map(|item| {
            let NavigationTarget::Shared(page) = item.target else {
                panic!("Bevy owns no surface-local destination");
            };
            Named {
                label: Lang("zh-CN").tr(page.sidebar_key()).to_string(),
                disabled: false,
                role: accesskit::Role::Button,
            }
        })
        .collect();
    assert_eq!(
        nav_labels, expected_nav_labels,
        "every nav entry is a named button labelled by the registry's copy key, in registry order"
    );

    // The routing choices belong to the Overview page, not the shell: the
    // sidebar mounts navigation alone, so the rail carries no selector.
    let rail = rail_entity(world);
    let rail_rows = crate::support::subtree_in_tree_order(world, rail);
    let selectors = rail_rows
        .iter()
        .filter(|entity| {
            world
                .get::<OverviewRoutingIntentSegmentPill>(**entity)
                .is_some()
        })
        .count();
    assert_eq!(
        selectors, 0,
        "the navigation column publishes no routing choice"
    );

    let mut slots = world.query::<(&ContentSlot, &AccessibilityNode)>();
    let (_, region) = slots.single(world).expect("content region semantic node");
    assert_eq!(region.role(), accesskit::Role::Region);
    let region_label = Lang("zh-CN").tr(ShellA11yNode::ContentRegion.label_key());
    assert_eq!(region.label(), Some(region_label.as_ref()));
}

#[test]
fn the_dashboard_segment_publishes_every_routing_choice_with_its_shared_label() {
    use crate::support::headless_plugins;
    use bevy::ecs::query::With;
    use infiltrator_bevy_ui::route::{PagesPlugin, RouteChanged};

    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((ShellPlugin::default(), PagesPlugin::demo()));
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Overview));
    app.update();
    app.update();

    let world = app.world_mut();
    let root = world
        .query_filtered::<Entity, With<infiltrator_bevy_ui::route::PageRoot>>()
        .single(world)
        .expect("the overview page is mounted");
    let published = named_in_painted_order::<OverviewRoutingIntentSegmentPill>(world, root);
    let intents = [
        RoutingIntent::FollowRules,
        RoutingIntent::RouteAllThroughProxy,
        RoutingIntent::BypassProxy,
    ];
    let expected: Vec<Named> = intents
        .iter()
        .map(|intent| Named {
            label: routing_intent_copy(intent, &Lang("zh-CN")),
            disabled: false,
            role: accesskit::Role::Button,
        })
        .collect();
    assert_eq!(
        published, expected,
        "every routing choice carries its shared label, in the card's order"
    );
}

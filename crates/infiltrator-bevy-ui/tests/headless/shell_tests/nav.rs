//! Behavior cases for nav.
//! test-intent: behavior

use super::*;
use infiltrator_application::routing_intent_projection::routing_intent_copy;
use infiltrator_contract::a11y::ShellA11yNode;
use infiltrator_contract::routing_intent::RoutingIntent;

#[test]
fn nav_entries_mode_pills_and_content_region_carry_semantics() {
    let mut app = mounted_shell();
    let world = app.world_mut();

    let mut navs = world.query::<(&NavItem, &AccessibilityNode)>();
    let mut nav_labels: Vec<(String, bool)> = Vec::new();
    for (_, node) in navs.iter(world) {
        assert_eq!(node.role(), accesskit::Role::Button);
        nav_labels.push((
            node.label().expect("nav label").to_owned(),
            node.is_disabled(),
        ));
    }
    let expected_nav_labels: Vec<(String, bool)> = Route::ALL
        .iter()
        .map(|r| (r.label().to_owned(), false))
        .collect();
    assert_eq!(
        nav_labels, expected_nav_labels,
        "every nav entry is a named button; all 11 routes are enabled"
    );

    let mut pills = world.query::<(&OverviewRoutingIntentPill, &AccessibilityNode)>();
    let mut pill_labels: Vec<(RoutingIntent, String)> = Vec::new();
    for (pill, node) in pills.iter(world) {
        assert_eq!(node.role(), accesskit::Role::Button);
        pill_labels.push((
            pill.0.clone(),
            node.label().expect("routing label").to_owned(),
        ));
    }
    assert_eq!(
        pill_labels,
        [
            RoutingIntent::FollowRules,
            RoutingIntent::RouteAllThroughProxy,
            RoutingIntent::BypassProxy,
        ]
        .into_iter()
        .map(|intent| {
            let label = routing_intent_copy(&intent, "zh-CN");
            (intent, label)
        })
        .collect::<Vec<_>>(),
        "every routing choice carries its shared label"
    );

    let mut slots = world.query::<(&ContentSlot, &AccessibilityNode)>();
    let (_, region) = slots.single(world).expect("content region semantic node");
    assert_eq!(region.role(), accesskit::Role::Region);
    assert_eq!(
        region.label(),
        Some(ShellA11yNode::ContentRegion.label_zh())
    );
}

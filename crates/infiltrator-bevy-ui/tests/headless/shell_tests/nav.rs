//! Behavior cases for nav.
//! test-intent: behavior

use super::*;
use infiltrator_contract::a11y::ShellA11yNode;

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

    let mut pills = world.query::<(&OverviewModePill, &AccessibilityNode)>();
    let mut pill_labels: Vec<(ProxyMode, String)> = Vec::new();
    for (pill, node) in pills.iter(world) {
        assert_eq!(node.role(), accesskit::Role::Button);
        pill_labels.push((pill.0, node.label().expect("mode label").to_owned()));
    }
    assert_eq!(
        pill_labels,
        vec![
            (ProxyMode::Rule, "规则".to_owned()),
            (ProxyMode::Global, "全局".to_owned()),
            (ProxyMode::Direct, "直连".to_owned()),
        ],
        "every mode pill carries its mode name"
    );

    let mut slots = world.query::<(&ContentSlot, &AccessibilityNode)>();
    let (_, region) = slots.single(world).expect("content region semantic node");
    assert_eq!(region.role(), accesskit::Role::Region);
    assert_eq!(
        region.label(),
        Some(ShellA11yNode::ContentRegion.label_zh())
    );
}

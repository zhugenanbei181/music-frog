//! Behavior cases for rules projection.
//! test-intent: behavior

use super::*;
use infiltrator_bevy_widgets::localization::UiLocale;

#[test]
fn test_rules_projection_in_place_update() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    app.insert_resource(UiLocale::new("en-US"));
    let (root, _) = navigate_to(&mut app, Route::Rules);

    let mut updated = RulesProjection::demo();
    updated.total_rules = 5000;
    updated.default_action = "REJECT (阻断)".to_owned();
    updated.rules[0].hit_count = Some(9999);
    updated.rules[0].proxy = "国外媒体".to_owned();
    updated.providers[0].name = "custom-mrs-provider".to_owned();
    updated.providers[0].rule_count = 2000;

    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(updated));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "Rules · 5000 rules · 3 providers"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "Default match target: REJECT (阻断)"
    ));
    assert!(subtree_has_text(app.world(), root, "9999 local trace hits"));
    assert!(subtree_has_text(app.world(), root, "国外媒体"));
    assert!(subtree_has_text(app.world(), root, "custom-mrs-provider"));
    assert!(subtree_has_text(app.world(), root, "2000 rules (domain)"));
}

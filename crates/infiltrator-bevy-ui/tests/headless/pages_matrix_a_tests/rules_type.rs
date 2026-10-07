//! Behavior cases for rules type.
//! test-intent: behavior

use super::*;
use infiltrator_bevy_ui::surface::{DemoSurfaceSource, SurfaceSource};
use infiltrator_contract::rule_snapshot::RuleSnapshot;
use infiltrator_contract::surface_snapshot::{PageData, RulesPageSnapshot};
use infiltrator_domain::rules::matrix::RULE_TYPE_MATRIX;
use infiltrator_domain::rules::view::{RULE_PUBLISH_LIMIT, RULE_ROW_HEIGHT_PX};

/// DUAL-11-01: every catalogue spelling mounts with its shared label, and the
/// chip fill follows the shared family rather than a per-surface spelling list.
#[test]
fn test_rules_type_matrix_renders_every_shared_label() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let source = DemoSurfaceSource::running();
    let mut snapshot = SurfaceSource::surface_snapshot(&source);
    let rules: Vec<RuleSnapshot> = RULE_TYPE_MATRIX
        .iter()
        .enumerate()
        .map(|(index, spec)| RuleSnapshot {
            edit_id: None,
            raw: format!("{},payload,DIRECT", spec.name),
            source_ip: false,
            failure: None,
            id: index + 1,
            rule_type: spec.name.to_owned(),
            payload: "payload".to_owned(),
            proxy: "DIRECT".to_owned(),
            hit_count: Some(0),
            is_enabled: true,
            no_resolve: spec.accepts_no_resolve,
            last_hit_secs: None,
            is_shadowed: false,
            shadow_reason: None,
        })
        .collect();
    let matrix_len = rules.len();
    snapshot.pages.rules = PageData::ready(RulesPageSnapshot {
        document: None,
        total_rules: matrix_len,
        default_action: "DIRECT".to_owned(),
        providers: Vec::new(),
        rules,
        tracer: Default::default(),
        mrs_acceleration: Default::default(),
        hit_audit: None,
        rule_publish_limit: RULE_PUBLISH_LIMIT,
        provider_cache: Default::default(),
        etag_support: Default::default(),
        json_documents: Vec::new(),
    });

    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::new_surface(StaticRulesSurface { snapshot }));
    app.add_plugins(CommandPumpPlugin::new(sink as Arc<dyn UiCommandSink>));
    app.update();
    let (root, _) = navigate_to(&mut app, Route::Rules);

    let expected: Vec<String> = RULE_TYPE_MATRIX
        .iter()
        .map(|spec| spec.label.to_owned())
        .collect();
    // DUAL-11-08: the page mounts the shared render window, so the test uses a
    // viewport tall enough to show every catalogue row — the same mechanism a
    // user gets from a tall window — and asserts the whole catalogue renders.
    {
        let mut view = app.world_mut().resource_mut::<RulesViewState>();
        view.viewport_height_px = matrix_len as f32 * RULE_ROW_HEIGHT_PX;
    }
    app.update();
    for label in expected {
        assert!(
            subtree_has_text(app.world(), root, &label),
            "missing rendered label {label}"
        );
    }
    assert_eq!(count_with::<RuleTypeText>(&mut app), matrix_len);
    assert_eq!(count_with::<RuleTypeBadge>(&mut app), matrix_len);
}

//! Behavior cases for rules matrix.
//! test-intent: behavior

use super::*;
use infiltrator_domain::rules::edit::DEFAULT_RULE_TARGET;
use infiltrator_domain::rules::logical::{
    LOGICAL_OPERATOR_CHOICES, SUB_RULE_CONDITION_PRESETS, default_logical_draft,
};

#[test]
fn test_rules_matrix_covers_closed_items() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Rules);

    // 11-01: rule types render their shared catalogue label.
    let mut projection = RulesProjection::demo();
    projection.rules[0].rule_type = "PROCESS-NAME".to_owned();
    projection.rules[1].rule_type = "GEOIP".to_owned();
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection));
    app.update();
    assert!(subtree_has_text(app.world(), root, "ProcessName"));
    assert!(subtree_has_text(app.world(), root, "GeoIP"));
    // The typed chip behind every row label is mounted from the shared family.
    assert_eq!(count_with::<RuleTypeBadge>(&mut app), 5);

    // 11-03/04/13: shared MRS card, provider lifecycle and search/paging.
    assert!(subtree_has_text(app.world(), root, "MRS 加速就绪"));
    assert!(subtree_has_text(app.world(), root, "来源:"));
    assert!(subtree_has_text(app.world(), root, "第 1/1 页"));
    // 11-08: a complete list carries no truncation note.
    assert!(!subtree_has_text(app.world(), root, "发布视口已截断"));

    // 11-09/10: one toggle + one reorder pair per mounted rule row.
    assert_eq!(count_with::<RuleToggleButton>(&mut app), 5);
    assert_eq!(count_with::<RuleMoveUpButton>(&mut app), 5);
    assert_eq!(count_with::<RuleMoveDownButton>(&mut app), 5);

    // 11-11/12: the wizard exposes the shared type vocabulary + both actions.
    assert_eq!(count_with::<AddCustomRuleButton>(&mut app), 1);
    assert_eq!(count_with::<InjectGamePresetsButton>(&mut app), 1);

    // 11-02: the logical builder is mounted with the shared vocabulary, the
    // shared default draft and the shared preview expression.
    assert_eq!(
        count_with::<SubRuleOperatorChip>(&mut app),
        LOGICAL_OPERATOR_CHOICES.len()
    );
    assert_eq!(
        count_with::<SubRulePresetButton>(&mut app),
        SUB_RULE_CONDITION_PRESETS.len()
    );
    assert_eq!(count_with::<SubRuleConditionRow>(&mut app), 2);
    assert_eq!(count_with::<SubRuleInsertButton>(&mut app), 1);
    let mut preview_lines = app.world_mut().query::<(&Text, &SubRulePreviewLine)>();
    let preview = preview_lines
        .iter(app.world())
        .map(|(text, _)| text.0.clone())
        .next()
        .expect("sub-rule preview line");
    assert_eq!(
        preview,
        project_logical_rule(&default_logical_draft(DEFAULT_RULE_TARGET), "zh-CN").preview
    );
    assert!(preview.contains("AND,((DOMAIN-SUFFIX,company.com),(NETWORK,TCP)),PROXY"));
}

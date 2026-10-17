use super::*;
use infiltrator_shared::locales::Lang;

#[test]
fn captions_preserve_missing_stages_actual_line_counts_and_blocking_causes() {
    let columns = ProfileMixinColumn {
        role: ProfileMixinColumnRole::Overlay,
        content: "ipv6: true\n".into(),
        editable: true,
        line_count: 1,
    };
    assert_eq!(
        column_caption(&columns, &Lang("en-US")),
        "Mixin overlay · 1 lines · editable"
    );
    let report = ProfileMixinCascade::Merged {
        stages: vec![
            ProfileMixinCascadeStage {
                id: ProfileMixinCascadeStageId::Base,
                declaration: ProfileMixinStageDeclaration::Applied,
                line_count: 2,
            },
            ProfileMixinCascadeStage {
                id: ProfileMixinCascadeStageId::Subscription,
                declaration: ProfileMixinStageDeclaration::Undeclared,
                line_count: 2,
            },
        ],
        line_count: 2,
    };
    assert!(cascade_caption(&report, &Lang("en-US")).contains("Subscription (not declared)"));
    assert!(cascade_caption(&report, &Lang("zh-CN")).contains("未声明"));
    assert_eq!(
        composed_error(Some("cause {error}/中文🙂"), &Lang("en-US")),
        "Composition blocked: cause {error}/中文🙂"
    );
}

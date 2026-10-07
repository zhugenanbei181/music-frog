//! Behavior cases for mixin.
//! test-intent: behavior

use infiltrator_domain::mixin::{MixinConfig, RuleMixin, merge_profile_with_config_fidelity};
use infiltrator_domain::profile_options::strip_rule_lines;

/// The exact function the Mixin pane commit runs.
#[test]
fn test_mixin_save_path_preserves_comments_through_the_shared_fidelity_writer() {
    let source = "\
# 手写头注释
mode: rule   # 行内说明

dns:
  enable: true
";
    let mixin = MixinConfig {
        mode: Some("global".to_string()),
        ..Default::default()
    };
    let merged = merge_profile_with_config_fidelity(source, &mixin).expect("merge");
    assert!(
        merged.contains("# 手写头注释"),
        "top comment kept: {merged}"
    );
    assert!(
        merged.contains("# 行内说明"),
        "inline comment kept: {merged}"
    );
    assert!(merged.contains("mode: global"));
}

/// Mirrors the real Mixin-pane save flow: strip the outgoing mixin's appended
/// rules through the shared writer, then merge the new mixin through the
/// fidelity writer. Comments survive both steps and no rule duplicates.
#[test]
fn test_mixin_resave_cycle_keeps_comments_and_does_not_duplicate_rules() {
    let source = "\
# 头注释
mode: rule
rules:
  # 规则块
  - MATCH,DIRECT
";
    let first = MixinConfig {
        mode: Some("global".to_string()),
        rules: Some(RuleMixin {
            append: vec!["DOMAIN-SUFFIX,ads.example.com,REJECT".to_string()],
            ..Default::default()
        }),
        ..Default::default()
    };
    let once = merge_profile_with_config_fidelity(source, &first).expect("first mixin");
    assert!(once.contains("# 头注释"));
    assert!(once.contains("# 规则块"));
    assert_eq!(once.matches("ads.example.com").count(), 1);

    let removals = vec!["DOMAIN-SUFFIX,ads.example.com,REJECT".to_string()];
    let base = strip_rule_lines(&once, &removals);
    assert!(!base.contains("ads.example.com"), "strip is exact: {base}");
    assert!(base.contains("# 头注释") && base.contains("# 规则块"));

    let second = MixinConfig {
        mode: Some("direct".to_string()),
        ..Default::default()
    };
    let twice = merge_profile_with_config_fidelity(&base, &second).expect("re");
    assert!(twice.contains("mode: direct"));
    assert!(twice.contains("# 头注释"));
    assert!(twice.contains("# 规则块"));
}

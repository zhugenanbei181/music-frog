//! Unit tests for YAML editing, L3 anchor scanning & namespace rewriting.

use super::*;
use super::anchor::AnchorKind;
use super::mixin_fidelity::{apply_mixin_to_doc, can_apply_mixin_via_fidelity};
use std::collections::HashMap;

fn doc(s: &str) -> SourceDoc {
    SourceDoc::parse(s).expect("parse")
}

// --- Scenario A: Append into the rules block ----------------------------

#[test]
fn append_rule_keeps_comments_and_anchors_verbatim() {
    let input = "\
# user header comment
mode: rule

rules:
  # ad blocking, added by hand
  - DOMAIN-SUFFIX,ads.example.com,REJECT   # inline note
  - &catchall MATCH,DIRECT
proxies:
  - &hk HK-01
  - *hk
";
    let mut d = doc(input);
    d.append_rule("DOMAIN-SUFFIX,youtube.com,REJECT")
        .expect("append");
    let expected = "\
# user header comment
mode: rule

rules:
  # ad blocking, added by hand
  - DOMAIN-SUFFIX,ads.example.com,REJECT   # inline note
  - &catchall MATCH,DIRECT
  - DOMAIN-SUFFIX,youtube.com,REJECT
proxies:
  - &hk HK-01
  - *hk
";
    assert_eq!(d.render(), expected);
    let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&d.render()).unwrap();
    assert_eq!(value.get("rules").unwrap().as_sequence().unwrap().len(), 3);
}

#[test]
fn append_rule_creates_block_when_missing() {
    let mut d = doc("mode: rule\nlog-level: info\n");
    d.append_rule("MATCH,DIRECT").expect("append");
    assert_eq!(
        d.render(),
        "mode: rule\nlog-level: info\nrules:\n  - MATCH,DIRECT\n"
    );
}

#[test]
fn append_rule_fills_empty_header_without_trailing_newline() {
    let mut d = doc("port: 7890\nrules:");
    d.append_rule("MATCH,DIRECT").expect("append");
    assert_eq!(d.render(), "port: 7890\nrules:\n  - MATCH,DIRECT\n");
}

#[test]
fn append_rule_inherits_indent_of_existing_items() {
    let mut d = doc("rules:\n    - MATCH,DIRECT\n");
    d.append_rule("DOMAIN,x,PROXY").expect("append");
    assert_eq!(
        d.render(),
        "rules:\n    - MATCH,DIRECT\n    - DOMAIN,x,PROXY\n"
    );
}

// --- Scenario B: Remove one rule line -----------------------------------

#[test]
fn remove_rule_deletes_only_target_line_with_comment() {
    let input = "\
rules:
  - DOMAIN-SUFFIX,ads.com,REJECT
  - DOMAIN,keep.me,DIRECT   # stay
  - MATCH,DIRECT # drop me
proxies: []
";
    let mut d = doc(input);
    d.remove_rule("MATCH,DIRECT").expect("remove");
    assert_eq!(
        d.render(),
        "rules:\n  - DOMAIN-SUFFIX,ads.com,REJECT\n  - DOMAIN,keep.me,DIRECT   # stay\nproxies: []\n"
    );
}

#[test]
fn remove_rule_errors_when_missing_or_block_absent() {
    let mut d = doc("rules:\n  - MATCH,DIRECT\n");
    assert!(matches!(
        d.remove_rule("DOMAIN,x,REJECT"),
        Err(YamlEditError::RuleNotFound(_))
    ));
    let mut no_rules = doc("mode: rule\n");
    assert!(matches!(
        no_rules.remove_rule("MATCH,DIRECT"),
        Err(YamlEditError::RulesBlockMissing)
    ));
}

// --- Scenario C: Top-level scalar override -------------------------------

#[test]
fn set_top_scalar_touches_only_one_line() {
    let input = "\
# top comment
mode: rule   # rule mode
log-level: info
rules:
  - MATCH,DIRECT
";
    let mut d = doc(input);
    d.set_top_scalar("mode", "global").expect("set");
    let out = d.render();
    assert_eq!(
        out,
        "# top comment\nmode: global   # rule mode\nlog-level: info\nrules:\n  - MATCH,DIRECT\n"
    );
    let before: Vec<&str> = input.lines().collect();
    let after: Vec<&str> = out.lines().collect();
    assert_eq!(before.len(), after.len());
    for (i, (b, a)) in before.iter().zip(after.iter()).enumerate() {
        if i != 1 {
            assert_eq!(b, a, "line {i} changed");
        }
    }
}

#[test]
fn set_top_scalar_preserves_inline_comment_and_gap() {
    let mut d = doc("mode:   rule  # gap kept\n");
    d.set_top_scalar("mode", "global").expect("set");
    assert_eq!(d.render(), "mode:   global  # gap kept\n");
}

#[test]
fn set_top_scalar_errors_for_missing_and_block_keys() {
    let mut d = doc("mode: rule\ndns:\n  enable: true\n");
    assert!(matches!(
        d.set_top_scalar("ipv6", "true"),
        Err(YamlEditError::KeyNotFound(_))
    ));
    assert!(matches!(
        d.set_top_scalar("dns", "x"),
        Err(YamlEditError::Unsupported(_))
    ));
    assert_eq!(d.render(), "mode: rule\ndns:\n  enable: true\n");
}

// --- Scenario D: Boundaries ----------------------------------------------

#[test]
fn crlf_documents_stay_crlf() {
    let mut d = doc("mode: rule\r\nrules:\r\n  - MATCH,DIRECT\r\n");
    d.set_top_scalar("mode", "global").expect("set");
    d.append_rule("DOMAIN,x,PROXY").expect("append");
    d.remove_rule("MATCH,DIRECT").expect("remove");
    assert_eq!(
        d.render(),
        "mode: global\r\nrules:\r\n  - DOMAIN,x,PROXY\r\n"
    );
}

#[test]
fn bom_is_preserved() {
    let mut d = doc("\u{feff}mode: rule\nrules:\n  - MATCH,DIRECT\n");
    d.append_rule("MATCH,GLOBAL").expect("append");
    assert_eq!(
        d.render(),
        "\u{feff}mode: rule\nrules:\n  - MATCH,DIRECT\n  - MATCH,GLOBAL\n"
    );
}

#[test]
fn blank_lines_and_comments_inside_rules_block_survive() {
    let mut d = doc("rules:\n  - A\n\n  # note between items\n  - B\nproxies: []\n");
    d.append_rule("C").expect("append");
    assert_eq!(
        d.render(),
        "rules:\n  - A\n\n  # note between items\n  - B\n  - C\nproxies: []\n"
    );
}

#[test]
fn parse_render_roundtrip_is_byte_identical() {
    let input = "\u{feff}# c\nmode: rule\r\n\r\nrules:\r\n  - &a A\r\n  - *a\nlast: no-newline";
    assert_eq!(doc(input).render(), input);
}

#[test]
fn block_scalar_content_is_rejected_but_far_edits_allowed() {
    let mut folded = doc("rules:\n  - >-\n    MATCH,DIRECT\n");
    assert!(matches!(
        folded.append_rule("X,Y,Z"),
        Err(YamlEditError::BlockScalar(_))
    ));
    let mut header = doc("desc: |\n  text: kept\n");
    assert!(matches!(
        header.set_top_scalar("desc", "x"),
        Err(YamlEditError::BlockScalar(_))
    ));
    let mut far = doc("desc: |\n  multi\n  line\nmode: rule\n");
    far.set_top_scalar("mode", "global").expect("far edit");
    assert_eq!(far.render(), "desc: |\n  multi\n  line\nmode: global\n");
}

#[test]
fn unsupported_shapes_are_rejected_up_front() {
    assert!(matches!(
        SourceDoc::parse("a: 1\n---\nb: 2\n"),
        Err(YamlEditError::MultiDocument)
    ));
    assert!(matches!(
        SourceDoc::parse("a:\n\t- x\n"),
        Err(YamlEditError::TabIndentation(2))
    ));
    assert!(SourceDoc::parse("---\nmode: rule\n").is_ok());
    let mut flow = doc("rules: [MATCH,DIRECT]\n");
    assert!(matches!(
        flow.append_rule("X,Y,Z"),
        Err(YamlEditError::FlowSyntax(_))
    ));
    let mut seq = doc("- a\n- b\n");
    assert!(matches!(
        seq.append_rule("X,Y,Z"),
        Err(YamlEditError::Unsupported(_))
    ));
}

// --- Scenario E: L3 Anchor & Alias Scanning ------------------------------

#[test]
fn scan_anchors_and_aliases_detects_definitions_and_references() {
    let input = "\
# header with &fake_comment and *fake_comment
mode: &main_mode rule
proxies:
  - &hk_01 { name: \"HK-01 & Quoted * Test\", type: ss }
  - *hk_01
  - name: '*not_an_alias'
rules:
  - &catchall MATCH,DIRECT # note &fake
  - *catchall
  - DOMAIN-SUFFIX,*.google.com,DIRECT
  - DOMAIN-KEYWORD,foo&bar,DIRECT
desc: |
  &block_scalar_anchor
  *block_scalar_alias
";
    let d = doc(input);
    let occurrences = d.scan_anchors_and_aliases();

    assert_eq!(occurrences.len(), 5);

    // 1: &main_mode
    assert_eq!(occurrences[0].name, "main_mode");
    assert_eq!(occurrences[0].kind, AnchorKind::Anchor);
    assert_eq!(occurrences[0].line_idx, 1);

    // 2: &hk_01
    assert_eq!(occurrences[1].name, "hk_01");
    assert_eq!(occurrences[1].kind, AnchorKind::Anchor);
    assert_eq!(occurrences[1].line_idx, 3);

    // 3: *hk_01
    assert_eq!(occurrences[2].name, "hk_01");
    assert_eq!(occurrences[2].kind, AnchorKind::Alias);
    assert_eq!(occurrences[2].line_idx, 4);

    // 4: &catchall
    assert_eq!(occurrences[3].name, "catchall");
    assert_eq!(occurrences[3].kind, AnchorKind::Anchor);
    assert_eq!(occurrences[3].line_idx, 7);

    // 5: *catchall
    assert_eq!(occurrences[4].name, "catchall");
    assert_eq!(occurrences[4].kind, AnchorKind::Alias);
    assert_eq!(occurrences[4].line_idx, 8);

    assert_eq!(d.anchor_definitions(), vec!["main_mode", "hk_01", "catchall"]);
    assert_eq!(d.alias_references(), vec!["hk_01", "catchall"]);
    assert!(d.find_unresolved_aliases().is_empty());
}

#[test]
fn scan_detects_unresolved_aliases() {
    let input = "\
proxies:
  - *orphan_alias
  - &defined_anchor MATCH,DIRECT
  - *defined_anchor
";
    let d = doc(input);
    assert_eq!(d.find_unresolved_aliases(), vec!["orphan_alias"]);
}

#[test]
fn scan_handles_flow_collections_and_merge_keys() {
    let input = "\
base: &base_map { port: 7890 }
extended:
  <<: *base_map
  items: [ &i1 item1, &i2 item2, *i1, *i2 ]
";
    let d = doc(input);
    let occurrences = d.scan_anchors_and_aliases();
    assert_eq!(occurrences.len(), 6);
    assert_eq!(occurrences[0].name, "base_map");
    assert_eq!(occurrences[0].kind, AnchorKind::Anchor);
    assert_eq!(occurrences[1].name, "base_map");
    assert_eq!(occurrences[1].kind, AnchorKind::Alias);
    assert_eq!(occurrences[2].name, "i1");
    assert_eq!(occurrences[2].kind, AnchorKind::Anchor);
    assert_eq!(occurrences[3].name, "i2");
    assert_eq!(occurrences[3].kind, AnchorKind::Anchor);
    assert_eq!(occurrences[4].name, "i1");
    assert_eq!(occurrences[4].kind, AnchorKind::Alias);
    assert_eq!(occurrences[5].name, "i2");
    assert_eq!(occurrences[5].kind, AnchorKind::Alias);
}

// --- Scenario F: L3 Anchor Namespace Rewriting --------------------------

#[test]
fn rewrite_anchor_namespace_preserves_comments_and_formatting() {
    let input = "\
# Top configuration header with hand-written notes
# &fake_comment *fake_alias
mode: rule   # inline mode comment

rules:
  # Hand-crafted fallback
  - &catchall MATCH,DIRECT # note with &fake
  - *catchall

proxies:
  - &hk_01 { name: \"HK-01\", port: 8080 }
  - *hk_01
  - name: \"*quoted_string\"
";
    let mut d = doc(input);
    let count = d.rewrite_anchor_namespace("tenant1").expect("rewrite");
    assert_eq!(count, 4);

    let expected = "\
# Top configuration header with hand-written notes
# &fake_comment *fake_alias
mode: rule   # inline mode comment

rules:
  # Hand-crafted fallback
  - &tenant1_catchall MATCH,DIRECT # note with &fake
  - *tenant1_catchall

proxies:
  - &tenant1_hk_01 { name: \"HK-01\", port: 8080 }
  - *tenant1_hk_01
  - name: \"*quoted_string\"
";
    assert_eq!(d.render(), expected);

    // Verify valid YAML
    let val: serde_yaml_ng::Value = serde_yaml_ng::from_str(&d.render()).unwrap();
    let rules = val.get("rules").unwrap().as_sequence().unwrap();
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].as_str().unwrap(), "MATCH,DIRECT");
    assert_eq!(rules[1].as_str().unwrap(), "MATCH,DIRECT");
}

#[test]
fn rewrite_anchor_namespace_with_trailing_separator() {
    let input = "\
rules:
  - &a MATCH,DIRECT
  - *a
";
    let mut d = doc(input);
    d.rewrite_anchor_namespace("p1_").expect("rewrite");
    assert_eq!(d.render(), "rules:\n  - &p1_a MATCH,DIRECT\n  - *p1_a\n");

    let mut d2 = doc(input);
    d2.rewrite_anchor_namespace("ns-").expect("rewrite");
    assert_eq!(d2.render(), "rules:\n  - &ns-a MATCH,DIRECT\n  - *ns-a\n");
}

#[test]
fn rewrite_anchors_on_same_line_right_to_left() {
    let input = "group: [ &a alpha, &b beta, *a, *b ] # inline &comment\n";
    let mut d = doc(input);
    let mut map = HashMap::new();
    map.insert("a".to_string(), "node_alpha".to_string());
    map.insert("b".to_string(), "node_beta".to_string());
    let count = d.rewrite_anchors_with_map(&map).expect("rewrite");
    assert_eq!(count, 4);
    assert_eq!(
        d.render(),
        "group: [ &node_alpha alpha, &node_beta beta, *node_alpha, *node_beta ] # inline &comment\n"
    );
}

#[test]
fn rewrite_anchor_name_single_target() {
    let input = "\
# comment
- &old_target val
- *old_target
- &keep_other other
";
    let mut d = doc(input);
    d.rewrite_anchor_name("old_target", "new_target").unwrap();
    assert_eq!(
        d.render(),
        "# comment\n- &new_target val\n- *new_target\n- &keep_other other\n"
    );
}

#[test]
fn rewrite_rejects_invalid_names() {
    let mut d = doc("rules:\n  - &a MATCH,DIRECT\n");
    assert!(matches!(
        d.rewrite_anchor_namespace("invalid prefix with spaces"),
        Err(YamlEditError::Unsupported(_))
    ));
    assert!(matches!(
        d.rewrite_anchor_name("a", "b,c"),
        Err(YamlEditError::Unsupported(_))
    ));
    assert!(matches!(
        d.rewrite_anchor_name("a", "b:c"),
        Err(YamlEditError::Unsupported(_))
    ));
}

// --- Scenario G: Mixin Fidelity Integration -----------------------------

#[test]
fn apply_mixin_to_doc_preserves_100_percent_comments_and_anchors() {
    let input = "\
# 手写主配置注释
mixed-port: 7890
mode: rule   # 模式说明
log-level: info

rules:
  # 手写规则块注释
  - &catchall MATCH,DIRECT # 兜底锚点
  - DOMAIN,remove.me,REJECT
";
    let mut d = doc(input);
    let mixin = crate::mixin::MixinConfig {
        mode: Some("global".to_string()),
        mixed_port: Some(7891),
        rules: Some(crate::mixin::RuleMixin {
            delete: vec!["DOMAIN,remove.me,REJECT".to_string()],
            append: vec!["DOMAIN,appended.com,PROXY".to_string()],
            ..Default::default()
        }),
        ..Default::default()
    };

    assert!(can_apply_mixin_via_fidelity(&mixin));
    apply_mixin_to_doc(&mut d, &mixin).expect("apply mixin fidelity");

    let out = d.render();
    assert!(out.contains("# 手写主配置注释"), "header comment preserved");
    assert!(out.contains("# 模式说明"), "inline comment preserved");
    assert!(out.contains("# 手写规则块注释"), "block comment preserved");
    assert!(out.contains("&catchall"), "anchor definition preserved");
    assert!(out.contains("# 兜底锚点"), "anchor comment preserved");
    assert!(out.contains("mode: global"), "mode scalar updated");
    assert!(out.contains("mixed-port: 7891"), "mixed-port updated");
    assert!(out.contains("DOMAIN,appended.com,PROXY"), "rule appended");
    assert!(!out.contains("remove.me"), "rule removed");

    let val: serde_yaml_ng::Value = serde_yaml_ng::from_str(&out).unwrap();
    assert_eq!(val.get("mode").unwrap().as_str().unwrap(), "global");
    assert_eq!(val.get("mixed-port").unwrap().as_u64().unwrap(), 7891);
}

#[test]
fn can_apply_mixin_detects_complex_ast_features() {
    let simple_mixin = crate::mixin::MixinConfig {
        mode: Some("rule".to_string()),
        ..Default::default()
    };
    assert!(can_apply_mixin_via_fidelity(&simple_mixin));

    let complex_dns = crate::mixin::MixinConfig {
        dns: Some(serde_yaml_ng::Value::Mapping(Default::default())),
        ..Default::default()
    };
    assert!(!can_apply_mixin_via_fidelity(&complex_dns));

    let complex_rules = crate::mixin::MixinConfig {
        rules: Some(crate::mixin::RuleMixin {
            prepend: vec!["DOMAIN,x,DIRECT".to_string()],
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(!can_apply_mixin_via_fidelity(&complex_rules));
}

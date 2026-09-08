//! Headless integration test suite for Track 5: Group 09 (YAML AST & Diff) & Group 10 (QuickJS Sandbox).

use infiltrator_application::script_application::ScriptApplication;
use infiltrator_contract::script_sandbox::{ScriptLogLevel, ScriptSandboxStatus};
use infiltrator_contract::yaml_ast_diff::{DiffKind, FidelityGrade};
use infiltrator_domain::myers_diff;
use infiltrator_domain::yaml_edit::SourceDoc;

#[test]
fn test_track5_metric1_100_percent_yaml_ast_fidelity() {
    let raw_yaml = r#"# 顶层手写注释 (Top-level comment)
port: 7890
mode: rule   # 行尾注释 (Inline comment)

# 锚点定义与复用测试
custom-anchor: &my_anchor
  timeout: 300

rules:
  # 分流规则段注释
  - DOMAIN-SUFFIX,google.com,PROXY
  - &catchall MATCH,DIRECT
"#;

    let mut doc = SourceDoc::parse(raw_yaml).expect("SourceDoc parse should succeed");

    // 1. 保留注释与排版的同时修改顶层标量
    doc.set_top_scalar("port", "7891")
        .expect("set_top_scalar port");
    doc.set_top_scalar("mode", "global")
        .expect("set_top_scalar mode");

    // 2. 规则操作：安全追加与安全删除
    doc.append_rule("DOMAIN-KEYWORD,twitter,PROXY")
        .expect("append_rule");
    doc.remove_rule("DOMAIN-SUFFIX,google.com,PROXY")
        .expect("remove_rule");

    // 3. 锚点扫描与命名空间重写 (L3 Consistency)
    let occurrences = doc.scan_anchors_and_aliases();
    assert_eq!(occurrences.len(), 2, "Found &my_anchor and &catchall");
    assert_eq!(occurrences[0].name, "my_anchor");
    assert_eq!(occurrences[1].name, "catchall");

    let count = doc
        .rewrite_anchor_namespace("infiltrator")
        .expect("rewrite anchor namespace");
    assert_eq!(count, 2, "Both anchors rewritten with prefix");

    let rendered = doc.render();

    // 断言注释与排版 100% 保留
    assert!(rendered.contains("# 顶层手写注释 (Top-level comment)"));
    assert!(rendered.contains("# 行尾注释 (Inline comment)"));
    assert!(rendered.contains("# 分流规则段注释"));
    assert!(rendered.contains("&infiltrator_my_anchor"));
    assert!(rendered.contains("&infiltrator_catchall"));
    assert!(rendered.contains("port: 7891"));
    assert!(rendered.contains("mode: global"));
    assert!(rendered.contains("DOMAIN-KEYWORD,twitter,PROXY"));
    assert!(!rendered.contains("DOMAIN-SUFFIX,google.com,PROXY"));
}

#[test]
fn test_track5_metric2_syntax_preflight_and_diagnostics() {
    let valid_yaml = "port: 7890\nmode: rule\n";
    assert!(infiltrator_domain::config::preflight_yaml_syntax(valid_yaml).is_ok());

    let invalid_yaml = "port: 7890\nmode: [unclosed brackets";
    let diag = infiltrator_domain::config::preflight_yaml_syntax(invalid_yaml).unwrap_err();
    assert!(diag.line >= 2);
    assert!(!diag.message.is_empty());
}

#[test]
fn test_track5_metric3_myers_diff_and_snapshot_comparison() {
    let old_snapshot = r#"port: 7890
mode: rule
rules:
  - DOMAIN-SUFFIX,google.com,PROXY
  - MATCH,DIRECT
"#;

    let new_snapshot = r#"port: 7890
mode: global
rules:
  - DOMAIN-SUFFIX,google.com,PROXY
  - DOMAIN-KEYWORD,github,PROXY
  - MATCH,DIRECT
"#;

    let diff = myers_diff::compute_diff(old_snapshot, new_snapshot, "snapshot-001", "snapshot-002");

    assert!(!diff.is_empty());
    assert!(diff.has_differences());
    assert_eq!(diff.stats.unchanged, 4);
    assert_eq!(
        diff.stats.modifications, 1,
        "mode changed from rule to global"
    );
    assert_eq!(diff.stats.additions, 1, "added github rule");
    assert_eq!(diff.stats.deletions, 0);

    // 检查 Unified Diff
    assert!(
        diff.unified_lines
            .iter()
            .any(|l| l.kind == DiffKind::Insert && l.content.contains("github"))
    );
    assert!(
        diff.unified_lines
            .iter()
            .any(|l| l.kind == DiffKind::Delete || l.kind == DiffKind::Modify)
    );

    // 检查 Split Diff 并排对齐
    let mode_row = diff
        .split_rows
        .iter()
        .find(|r| r.kind == DiffKind::Modify)
        .expect("modify row");
    assert_eq!(mode_row.left.as_ref().unwrap().content, "mode: rule");
    assert_eq!(mode_row.right.as_ref().unwrap().content, "mode: global");
}

#[test]
fn test_track5_metric4_quickjs_sandbox_timeout_and_memory_guard() {
    let app = ScriptApplication::new();

    // 1. 超时熔断 (<500ms 限制)
    let loop_script = r#"function main(config, profile) {
        while (true) {
            // Infinite loop simulation
        }
        return config;
    }"#;
    let res_timeout = app.run_sandbox(loop_script, "port: 7890", None);
    assert_eq!(res_timeout.status, ScriptSandboxStatus::Timeout);
    assert!(res_timeout.has_error());
    assert!(res_timeout.error_detail.as_ref().unwrap().contains("超时"));

    // 2. 内存熔断 (64MB 限制)
    let huge_yaml = format!("port: 7890\npayload: '{}'", "X".repeat(70 * 1024 * 1024));
    let normal_script = "function main(config) { return config; }";
    let res_mem = app.run_sandbox(normal_script, &huge_yaml, None);
    assert_eq!(res_mem.status, ScriptSandboxStatus::MemoryExceeded);
    assert!(res_mem.has_error());
    assert!(res_mem.error_detail.as_ref().unwrap().contains("内存"));
}

#[test]
fn test_track5_metric5_console_log_interception_and_ast_diff_contrast() {
    let app = ScriptApplication::new();

    let script = r#"function main(config, profile) {
        console.info("Initializing QuickJS AST Transform");
        console.log("Adding Country Policy Groups");
        auto_country_groups(config);
        console.warn("Direct China routing rules injected");
        direct_china(config);
        console.log("AST Transform Completed Successfully");
        return config;
    }"#;

    let input_yaml = r#"proxies:
  - name: "🇭🇰 香港 IEPL 01"
    type: ss
  - name: "🇯🇵 日本 Tokyo 01"
    type: ss
rules:
  - MATCH,PROXY
"#;

    let snapshot = app.run_sandbox(script, input_yaml, Some("auto-country-groups"));

    assert_eq!(snapshot.status, ScriptSandboxStatus::Success);
    assert!(snapshot.is_success());
    assert!(!snapshot.has_error());

    // 验证 console.log 流式捕获
    assert_eq!(snapshot.console_logs.len(), 4);
    assert_eq!(snapshot.console_logs[0].level, ScriptLogLevel::Info);
    assert!(
        snapshot.console_logs[0]
            .message
            .contains("Initializing QuickJS")
    );
    assert_eq!(snapshot.console_logs[1].level, ScriptLogLevel::Log);
    assert_eq!(snapshot.console_logs[2].level, ScriptLogLevel::Warn);

    // 验证 AST 实时对比 Diff 嵌入
    let diff = snapshot.diff.expect("AST diff must be present");
    assert!(diff.has_differences());
    assert!(diff.stats.additions > 0);
    assert_eq!(diff.fidelity_grade, FidelityGrade::L2Layout);

    // 确认生成的输出 YAML 包含了新策略组和分流规则
    let output = snapshot.transformed_yaml.expect("transformed YAML");
    assert!(output.contains("🇭🇰 香港"));
    assert!(output.contains("GEOIP,CN,DIRECT"));
}

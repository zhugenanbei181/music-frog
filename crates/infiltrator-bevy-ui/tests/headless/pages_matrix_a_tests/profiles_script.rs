//! Behavior cases for profiles script.
//! test-intent: behavior

use super::*;

/// DUAL-10-12: the Bevy console renders the *shared* export projection — the
/// real file name, byte count, SHA-256 and the typed host outcome produced by
/// the shared application (the action runs in Iced; this card reads the same
/// published fact). It also states the honest `.js` note.
#[test]
fn test_profiles_script_console_renders_the_shared_export_projection() {
    use infiltrator_application::script_export_application::ScriptExportApplication;

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    let export = ScriptExportApplication::without_host_port()
        .export_directive_dsl(
            Some("bevy-export"),
            "function main(config) {\n  auto_country_groups(config);\n  return config;\n}",
            Some("auto-country-groups"),
        )
        .expect("compose export");
    let mut projection = editor_options_page_projection("mode: rule\n", None);
    projection.script_export = Some(export);
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(projection));
    app.update();

    assert!(
        subtree_has_text(app.world(), root, "bevy-export.js"),
        "the real export file name renders"
    );
    assert!(
        subtree_has_text(app.world(), root, "未写入："),
        "the typed unsupported host outcome renders"
    );
    assert!(
        subtree_has_text(app.world(), root, "不是 JavaScript"),
        "the honest directive-DSL note renders"
    );
    assert!(
        subtree_has_text(app.world(), root, "导出内容预览"),
        "the real exported bytes are previewed"
    );
}

/// DUAL-10-05/06/07/13/14: the Bevy console renders the *shared* script-sandbox
/// projection. The snapshot is produced by the real shared application; the
/// card lists only the directive that really ran, the hook stage, the breaker
/// limits, the captured console lines and the before/after YAML, and it shows
/// the safe-degradation record on a failed run.
#[test]
fn test_profiles_script_console_renders_the_shared_projection() {
    use infiltrator_application::script_application::ScriptApplication;

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    let application = ScriptApplication::new();
    let snapshot = application.run_sandbox(
        "function main(config, profile) {\n  console.log(\"bevy shared console\");\n  auto_country_groups(config);\n  return config;\n}",
        "proxies:\n  - name: 🇭🇰 HK 01\n    type: ss\n",
        Some("auto-country-groups"),
    );
    let mut projection = editor_options_page_projection("mode: rule\n", None);
    projection.script_sandbox = Some(snapshot);
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(projection));
    app.update();

    assert!(
        subtree_has_text(app.world(), root, "auto_country_groups ·"),
        "only the matched directive is claimed"
    );
    assert!(
        !subtree_has_text(app.world(), root, "direct_china ·"),
        "a directive that did not match is never shown as executed"
    );
    assert!(
        subtree_has_text(app.world(), root, "bevy shared console"),
        "the captured console line renders"
    );
    assert!(
        subtree_has_text(app.world(), root, "Pre-Merge"),
        "the real hook stage renders"
    );
    assert!(
        subtree_has_text(app.world(), root, "熔断状态"),
        "the breaker state renders"
    );
    assert!(
        subtree_has_text(app.world(), root, "内存预算 64MB"),
        "the sandbox resource limits render"
    );
    assert!(
        subtree_has_text(app.world(), root, "引擎能力: 不支持 JavaScript 语法"),
        "the negotiated engine capability states the no-JS limit"
    );
    assert!(
        subtree_has_text(app.world(), root, "变换后 YAML"),
        "the before/after preview renders"
    );

    // DUAL-10-13: a failed run degrades safely and the console says so.
    let degraded = application.run_sandbox(
        "function main(config) { remove_rules(config, \"([\"); return config; }",
        "port: 7890\n",
        None,
    );
    let mut projection = editor_options_page_projection("mode: rule\n", None);
    projection.script_sandbox = Some(degraded);
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(projection));
    app.update();
    assert!(
        subtree_has_text(app.world(), root, "安全降级：原配置保持不变"),
        "the safe-degradation notice renders"
    );
    assert!(
        !subtree_has_text(app.world(), root, "auto_country_groups ·"),
        "the failed projection no longer claims a directive"
    );
}

/// DUAL-10-01: the Bevy console renders whatever engine the shared read model
/// reports. A snapshot whose engine negotiated the JavaScript slot renders the
/// JS label and the JS capability line through the same card code, so an engine
/// swap needs no Bevy change.
#[test]
fn test_profiles_script_console_renders_an_injected_javascript_engine() {
    use infiltrator_contract::script_sandbox::{
        ScriptEngineCapabilities, ScriptEngineKind, ScriptSandboxSnapshot,
    };

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    let mut snapshot = ScriptSandboxSnapshot::demo_fixture();
    snapshot.engine_kind = ScriptEngineKind::JavascriptEngine;
    snapshot.engine_capabilities = ScriptEngineCapabilities {
        supports_javascript_syntax: true,
        supports_directive_dsl: false,
        ..ScriptEngineCapabilities::directive_dsl()
    };
    assert!(snapshot.engine_kind_matches_capabilities());

    let mut projection = editor_options_page_projection("mode: rule\n", None);
    projection.script_sandbox = Some(snapshot);
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(projection));
    app.update();

    assert!(
        subtree_has_text(app.world(), root, "JavaScript 引擎"),
        "the injected engine kind renders"
    );
    assert!(
        subtree_has_text(app.world(), root, "引擎能力: 支持 JavaScript 语法"),
        "the negotiated JS capability renders"
    );
    assert!(
        !subtree_has_text(app.world(), root, "不支持 JavaScript 语法"),
        "the no-JS limit is not shown for a JS engine"
    );
}

//! DUAL-10-15: the shared scripting-sandbox regression matrix execution.
//!
//! Every covered row runs the real domain engine, the real
//! [`ScriptApplication`] service and the real Mixin reductions, and reports
//! what it measured. Items without a runnable backend (the QuickJS engine
//! claim, the three-column editor) are registered as explicitly *not covered*
//! with the honest reason.

#[cfg(test)]
#[path = "script_sandbox_matrix_application_test.rs"]
mod script_sandbox_matrix_application_test;

use infiltrator_contract::script_sandbox::{ScriptEngineKind, ScriptSandboxStatus};
use infiltrator_contract::script_sandbox_matrix::{
    ScriptSandboxMatrixReport, ScriptSandboxMatrixScenario,
};
use infiltrator_domain::mixin::MixinConfig;
use infiltrator_domain::mixin_studio;
use infiltrator_domain::script_engine::{HookStage, ScriptCircuitBreaker, ScriptEngine};
use std::time::Duration;

use crate::script_application::ScriptApplication;
use crate::script_export_application::ScriptExportApplication;

/// The one shared executor both surfaces call.
pub struct ScriptSandboxMatrixApplication;

type Check = (bool, String);

fn closed(id: &str, item: &str, (passed, detail): Check) -> ScriptSandboxMatrixScenario {
    ScriptSandboxMatrixScenario {
        id: id.to_string(),
        item: item.to_string(),
        covered: true,
        passed,
        detail,
    }
}

#[cfg_attr(feature = "script-engine-boa", allow(dead_code))]
fn planned(id: &str, item: &str, reason: &str) -> ScriptSandboxMatrixScenario {
    ScriptSandboxMatrixScenario {
        id: id.to_string(),
        item: item.to_string(),
        covered: false,
        passed: false,
        detail: reason.to_string(),
    }
}

const COUNTRY_SCRIPT: &str = r#"function main(config, profile) {
    console.log("matrix grouping start");
    console.info("matrix info line");
    auto_country_groups(config);
    return config;
}"#;

const SAMPLE_YAML: &str =
    "proxies:\n  - name: 🇭🇰 HK 01\n    type: ss\n  - name: 🇯🇵 JP 01\n    type: ss\n";

fn check_hook_stages() -> Check {
    let engine = ScriptEngine::new();
    let result =
        engine.execute_transform_detailed(COUNTRY_SCRIPT, SAMPLE_YAML, HookStage::PostMerge);
    match result {
        Ok(result) => (
            result.stage == HookStage::PostMerge && result.success,
            format!("stage={}", result.stage.as_str()),
        ),
        Err(error) => (false, error.to_string()),
    }
}

fn check_resource_limits() -> Check {
    let app = ScriptApplication::new();
    let timeout = app.run_sandbox(
        "function main(config) { while(true) {} }",
        "port: 7890",
        None,
    );
    if timeout.status != ScriptSandboxStatus::Timeout {
        return (false, format!("timeout status={:?}", timeout.status));
    }
    let memory = ScriptEngine::new()
        .with_max_memory(16)
        .execute_transform_detailed(
            "function main(config) { return config; }",
            "port: 7890",
            HookStage::PreMerge,
        );
    match memory {
        Err(infiltrator_domain::script_engine::ScriptError::MemoryExceeded(bytes)) => {
            (bytes > 16, format!("memory limit reported {bytes} bytes"))
        }
        other => (false, format!("memory guard did not trip: {other:?}")),
    }
}

fn check_presets() -> Check {
    let presets = ScriptEngine::builtin_presets();
    let ids: Vec<&str> = presets.iter().map(|preset| preset.id).collect();
    let expected = [
        "remove-ads",
        "auto-country-groups",
        "streaming-groups",
        "direct-china",
    ];
    (
        expected.iter().all(|id| ids.contains(id)),
        format!("{ids:?}"),
    )
}

fn check_console_capture() -> Check {
    let app = ScriptApplication::new();
    let snapshot = app.run_sandbox(COUNTRY_SCRIPT, SAMPLE_YAML, None);
    let messages: Vec<&str> = snapshot
        .console_logs
        .iter()
        .map(|entry| entry.message.as_str())
        .collect();
    (
        messages
            .iter()
            .any(|message| message.contains("matrix grouping start"))
            && messages
                .iter()
                .any(|message| message.contains("matrix info line")),
        format!("{} logs", snapshot.console_logs.len()),
    )
}

fn check_ast_diff() -> Check {
    let app = ScriptApplication::new();
    let snapshot = app.run_sandbox(COUNTRY_SCRIPT, SAMPLE_YAML, None);
    match snapshot.diff.as_ref() {
        Some(diff) => (
            diff.has_differences(),
            format!("diff lines={}", diff.unified_lines.len()),
        ),
        None => (false, "no diff produced".to_string()),
    }
}

fn check_cascade_pipeline() -> Check {
    let pre = MixinConfig {
        mode: Some("script".to_string()),
        ..Default::default()
    };
    let post = MixinConfig {
        mode: Some("global".to_string()),
        ..Default::default()
    };
    let report = mixin_studio::preview_cascade(
        "mode: rule\nport: 7890\n",
        Some("port: 8080\n"),
        None,
        Some(&pre),
        Some(&post),
    );
    if report.blocked {
        return (false, report.error.unwrap_or_default());
    }
    let applied: Vec<&str> = report
        .stages
        .iter()
        .filter(|stage| stage.applied)
        .map(|stage| stage.id)
        .collect();
    (
        report.stages.len() == 5
            && applied == ["base", "subscription", "pre_mixin", "post_mixin"]
            && report.merged_yaml.as_deref().is_some_and(|merged| {
                merged.contains("mode: global") && merged.contains("port: 8080")
            }),
        format!(
            "{} stages, {} lines",
            report.stages.len(),
            report.merged_line_count()
        ),
    )
}

fn check_mixin_preflight() -> Check {
    let blocked = mixin_studio::preflight_mixin("mode: rule\n", "mode: [unterminated\n");
    let merged = mixin_studio::preflight_mixin("mode: rule\nport: 7890\n", "mode: global\n");
    (
        blocked.is_blocking()
            && blocked.error.is_some()
            && merged.valid
            && merged
                .merged_preview
                .as_deref()
                .is_some_and(|preview| preview.contains("port: 7890")),
        format!("blocked={} valid={}", blocked.is_blocking(), merged.valid),
    )
}

fn check_mixin_toggles() -> Check {
    let enabled = match mixin_studio::set_toggle("{}", "ipv6", true) {
        Ok(text) => text,
        Err(error) => return (false, error),
    };
    let with_dns = match mixin_studio::set_toggle(&enabled, "dns-fake-ip", true) {
        Ok(text) => text,
        Err(error) => return (false, error),
    };
    let read_back = mixin_studio::toggle_enabled(&with_dns, "ipv6").unwrap_or(false)
        && mixin_studio::toggle_enabled(&with_dns, "dns-fake-ip").unwrap_or(false);
    (
        read_back && with_dns.contains("fake-ip"),
        format!("{} toggles", mixin_studio::MIXIN_PRESET_TOGGLES.len()),
    )
}

fn check_safe_degradation() -> Check {
    let app = ScriptApplication::new();
    let input = "port: 7890\nrules:\n  - MATCH,DIRECT\n";
    // The malformed regex directive fails inside the transform; the engine
    // must surface a typed error and leave the input document untouched.
    let snapshot = app.run_sandbox(
        "function main(config) { remove_rules(config, \"([\"); return config; }",
        input,
        None,
    );
    (
        snapshot.has_error()
            && snapshot.transformed_yaml.is_none()
            && snapshot.input_yaml == input
            && snapshot.error_detail.is_some(),
        format!("status={:?}", snapshot.status),
    )
}

fn check_circuit_breaker() -> Check {
    // Domain-level trip/reset unit.
    let mut breaker = ScriptCircuitBreaker::new(3, Duration::from_secs(30));
    breaker.record_failure();
    breaker.record_failure();
    let not_yet = !breaker.is_tripped();
    breaker.record_failure();
    let tripped = breaker.is_tripped();
    breaker.record_success();
    let reset = !breaker.is_tripped();

    // Application-level trip through the real sandbox service.
    let app = ScriptApplication::new();
    let failing = "function main(config) { missing_symbol(); }";
    for _ in 0..3 {
        let _ = app.run_sandbox(failing, "port: [bad\n", None);
    }
    let blocked = app.run_sandbox(failing, "port: [bad\n", None);
    let blocked_message = blocked
        .console_logs
        .iter()
        .any(|entry| entry.message.contains("熔断"));
    (
        not_yet
            && tripped
            && reset
            && blocked.status == ScriptSandboxStatus::RuntimeError
            && blocked_message,
        format!("domain trip={tripped} reset={reset} app blocked={blocked_message}"),
    )
}

fn check_console_read_model() -> Check {
    let app = ScriptApplication::new();
    let snapshot = app.run_sandbox(COUNTRY_SCRIPT, SAMPLE_YAML, Some("auto-country-groups"));
    let ids: Vec<&str> = snapshot
        .matched_directives
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    (
        snapshot.engine_kind == ScriptEngineKind::DirectiveDsl
            && !snapshot.engine_kind.is_real_javascript()
            // DUAL-10-01: the capability negotiation is published next to the
            // kind, and the bundled default is honest about having no JS syntax.
            && !snapshot.engine_capabilities.supports_javascript_syntax
            && snapshot.engine_capabilities.supports_directive_dsl
            && snapshot.engine_capabilities.captures_console
            && snapshot.engine_kind_matches_capabilities()
            && snapshot.hook_stage == "pre_merge"
            && !snapshot.hook_stage_label.is_empty()
            && ids == vec!["auto_country_groups"]
            && !snapshot.input_yaml.is_empty()
            && snapshot.transformed_yaml.is_some()
            && snapshot.diff.is_some()
            && snapshot.circuit_breaker.failure_threshold == 3
            && snapshot.console_logs.len() >= 2,
        format!(
            "directives={ids:?} stage={} engine={}",
            snapshot.hook_stage,
            snapshot.engine_capability_label_zh()
        ),
    )
}

/// DUAL-10-01 migration step 6: when the non-default `script-engine-boa`
/// feature is on, the shared matrix runs the **real** Boa adapter and reports
/// the row as covered; the default build keeps the honest `planned` gap.
fn dual_10_01_scenario() -> ScriptSandboxMatrixScenario {
    #[cfg(feature = "script-engine-boa")]
    {
        closed(
            "DUAL-10-01",
            "QuickJS/ECMAScript embedded engine",
            check_javascript_engine(),
        )
    }
    #[cfg(not(feature = "script-engine-boa"))]
    {
        planned(
            "DUAL-10-01",
            "QuickJS embedded engine",
            "无真实 QuickJS 引擎：默认构建仅交付可插拔引擎接缝（ScriptEnginePort + 能力协商，shared-ready），识别已知指令的正则 DSL 仍是默认实现；真实 ECMAScript 适配器由非默认特性 `script-engine-boa` 显式 opt-in，默认矩阵不对 JS 引擎执行宣称覆盖",
        )
    }
}

#[cfg(feature = "script-engine-boa")]
fn check_javascript_engine() -> Check {
    use crate::script_engine_boa::BoaScriptEngine;
    use infiltrator_contract::script_sandbox::ScriptEngineCapabilities;
    use std::sync::Arc;

    let engine = BoaScriptEngine::new(
        ScriptEngineCapabilities::DEFAULT_TIMEOUT_MS,
        ScriptEngineCapabilities::DEFAULT_MAX_MEMORY_BYTES,
    );
    let app = ScriptApplication::with_engine(Arc::new(engine));
    let snapshot = app.run_sandbox(
        "function main(config) { config.port = 8080; return config; }",
        "port: 7890\nmode: rule\n",
        None,
    );
    let transformed = snapshot.transformed_yaml.as_deref().unwrap_or_default();
    (
        snapshot.status == ScriptSandboxStatus::Success
            && snapshot.engine_kind == ScriptEngineKind::JavascriptEngine
            && snapshot.engine_kind.is_real_javascript()
            && snapshot.engine_capabilities.supports_javascript_syntax
            && snapshot.engine_kind_matches_capabilities()
            && transformed.contains("port: 8080")
            && transformed.contains("mode: rule"),
        format!(
            "engine={} transformed={}",
            snapshot.engine_kind.label_en(),
            transformed.lines().count()
        ),
    )
}

fn check_dual_surface_alignment() -> Check {
    let app = ScriptApplication::new();
    let snapshot = app.run_sandbox(COUNTRY_SCRIPT, SAMPLE_YAML, Some("auto-country-groups"));
    // The cached projection is exactly what the surface reader republishes to
    // the Bevy surface, so both consoles render the same bytes.
    let cached = crate::script_application::last_script_sandbox();
    (
        cached.as_ref() == Some(&snapshot)
            && snapshot.matched_directive_count() == 1
            && snapshot.is_success(),
        format!("cached={} logs={}", cached.is_some(), snapshot.log_count()),
    )
}

fn check_extension_round_trip() -> Check {
    let app = ScriptApplication::new();
    let package = infiltrator_domain::script_engine::ExtensionPackage {
        name: "matrix-ext".to_string(),
        version: "1.0.0".to_string(),
        author: "matrix".to_string(),
        description: "matrix".to_string(),
        stage: HookStage::PostMerge,
        script_code: COUNTRY_SCRIPT.to_string(),
        mixin_yaml: Some("mode: global\n".to_string()),
        tags: vec!["matrix".to_string()],
    };
    let json = match app.export_extension(&package) {
        Ok(json) => json,
        Err(failure) => return (false, failure.message),
    };
    match app.import_extension(&json) {
        Ok(restored) => (
            restored == package && restored.verify_checksum(&package.calculate_checksum()),
            format!("{} bytes", json.len()),
        ),
        Err(failure) => (false, failure.message),
    }
}

/// DUAL-10-09: the three-column editor model carries the real base, the real
/// overlay buffer and the real composed pipeline output (never a mock).
fn check_three_column_editor() -> Check {
    let base = "mode: rule\nport: 7890\n";
    let columns = mixin_studio::mixin_editor_columns(base, "mode: global\n");
    let blocked = mixin_studio::mixin_editor_columns("mode: rule\n", "mode: [bad\n");
    let composed_is_real = columns.composed.content.contains("port: 7890")
        && columns.composed.content.contains("mode: global");
    (
        columns.base.content == base
            && !columns.base.editable
            && columns.overlay.content == "mode: global\n"
            && columns.overlay.editable
            && columns.is_composed()
            && composed_is_real
            && !columns.composed.editable
            && blocked.is_blocked()
            && blocked.composed.content.is_empty()
            && !blocked.is_composed(),
        format!(
            "composed={} lines, blocked={}",
            columns.composed_line_count(),
            blocked.is_blocked()
        ),
    )
}

/// DUAL-10-12: the per-surface export artifacts are real files — a `.yaml`
/// overlay that re-parses through the shared preflight, a `.js`-named
/// directive-DSL script whose header states it is not JavaScript, and a JSON
/// package with its real SHA-256. A host without a save port reports typed
/// unsupported and keeps the composed bytes.
fn check_extension_export() -> Check {
    let (round_trip, round_trip_detail) = check_extension_round_trip();

    let js = match infiltrator_domain::script_export::compose_directive_dsl_export(
        Some("matrix-country"),
        COUNTRY_SCRIPT,
        Some("auto-country-groups"),
    ) {
        Ok(artifact) => artifact,
        Err(error) => return (false, error),
    };
    let js_is_honest = js.file_name.ends_with(".js")
        && !js.kind.is_javascript()
        && js.content.contains("不是 JavaScript")
        && js.content.contains("auto_country_groups");

    let base = "mode: rule\nport: 7890\n";
    let overlay = match infiltrator_domain::script_export::compose_mixin_overlay_export(
        "matrix",
        base,
        "ipv6: true\n",
    ) {
        Ok(artifact) => artifact,
        Err(error) => return (false, error),
    };
    let overlay_round_trip = mixin_studio::preflight_mixin(base, &overlay.content);

    let package = infiltrator_domain::script_engine::ExtensionPackage {
        name: "matrix-export".to_string(),
        version: "1.0.0".to_string(),
        author: "matrix".to_string(),
        description: "export".to_string(),
        stage: HookStage::PreMerge,
        script_code: COUNTRY_SCRIPT.to_string(),
        mixin_yaml: Some("ipv6: true\n".to_string()),
        tags: vec!["matrix".to_string()],
    };
    let package_artifact =
        match infiltrator_domain::script_export::compose_extension_package_export(&package) {
            Ok(artifact) => artifact,
            Err(error) => return (false, error),
        };
    let checksum_is_real = package_artifact
        .checksum
        .as_deref()
        .is_some_and(|checksum| checksum == package.calculate_checksum() && checksum.len() == 64);

    // A host without a save-file port reports a typed unsupported outcome and
    // still carries the composed content; nothing is fabricated.
    let hostless = match ScriptExportApplication::without_host_port().export_directive_dsl(
        Some("matrix-country"),
        COUNTRY_SCRIPT,
        Some("auto-country-groups"),
    ) {
        Ok(snapshot) => snapshot,
        Err(failure) => return (false, failure.message),
    };
    let unsupported_keeps_bytes = hostless.outcome.is_unsupported()
        && hostless.content.contains("不是 JavaScript")
        && hostless.byte_len() > 0;

    (
        round_trip
            && js_is_honest
            && overlay_round_trip.valid
            && overlay_round_trip
                .merged_preview
                .as_deref()
                .is_some_and(|preview| preview.contains("ipv6: true"))
            && checksum_is_real
            && unsupported_keeps_bytes,
        format!(
            "round_trip={round_trip_detail}; js={}; overlay_lines={}; unsupported={unsupported_keeps_bytes}",
            js.file_name,
            overlay.content.lines().count()
        ),
    )
}

impl ScriptSandboxMatrixApplication {
    /// Run every scenario deterministically (no time, no I/O, no randomness).
    pub fn run_deterministic_matrix() -> ScriptSandboxMatrixReport {
        ScriptSandboxMatrixReport {
            scenarios: vec![
                dual_10_01_scenario(),
                closed(
                    "DUAL-10-02",
                    "Pre/Post-Process hook stages",
                    check_hook_stages(),
                ),
                closed(
                    "DUAL-10-03",
                    "Sandbox resource circuit breaker (64MB/500ms)",
                    check_resource_limits(),
                ),
                closed("DUAL-10-04", "Built-in script presets", check_presets()),
                closed(
                    "DUAL-10-05",
                    "Live code-debug console viewport",
                    check_console_read_model(),
                ),
                closed(
                    "DUAL-10-06",
                    "console.log streaming capture",
                    check_console_capture(),
                ),
                closed("DUAL-10-07", "AST before/after diff", check_ast_diff()),
                closed(
                    "DUAL-10-08",
                    "Cascade overlay pipeline",
                    check_cascade_pipeline(),
                ),
                closed(
                    "DUAL-10-09",
                    "Three-column Mixin editor",
                    check_three_column_editor(),
                ),
                closed(
                    "DUAL-10-10",
                    "Mixin syntax check + error blocking",
                    check_mixin_preflight(),
                ),
                closed(
                    "DUAL-10-11",
                    "Common Mixin preset toggles",
                    check_mixin_toggles(),
                ),
                closed(
                    "DUAL-10-12",
                    "Extension export & community sharing",
                    check_extension_export(),
                ),
                closed(
                    "DUAL-10-13",
                    "Exception handling safe degradation",
                    check_safe_degradation(),
                ),
                closed(
                    "DUAL-10-14",
                    "Dual-surface console + Mixin viewport parity",
                    check_dual_surface_alignment(),
                ),
                closed(
                    "DUAL-10-15",
                    "Sandbox circuit-breaker unit matrix",
                    check_circuit_breaker(),
                ),
            ],
        }
    }
}

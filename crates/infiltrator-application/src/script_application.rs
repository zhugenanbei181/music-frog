//! Application service for sandboxed directive-DSL execution, validation,
//! presets, diffing, and the shared script-sandbox read model.
//!
//! Runtime truth: no JavaScript engine is bundled. `ScriptEngine` recognises
//! known directives with regexes; this service projects what really ran into
//! the shared [`ScriptSandboxSnapshot`] both surfaces render, and caches the
//! latest projection so the Bevy surface (which owns no engine) reads exactly
//! what Iced computed.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::script_sandbox::{
    ScriptCircuitBreakerSnapshot, ScriptDirectiveMatch, ScriptEngineCapabilities, ScriptEngineKind,
    ScriptLogEntry, ScriptLogLevel, ScriptPresetSummary, ScriptSandboxSnapshot,
    ScriptSandboxStatus,
};
use infiltrator_domain::myers_diff;
use infiltrator_domain::script_engine::{
    ExtensionPackage, HookStage, ScriptCircuitBreaker, ScriptEngine, ScriptError,
    ScriptExecutionResult, ScriptValidationResult,
};
use infiltrator_ports::script_engine::ScriptEnginePort;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use crate::script_engine_direct::DirectiveDslScriptEngine;

/// Thread-safe application service for managing directive-DSL scripts and sandboxes.
#[derive(Clone)]
pub struct ScriptApplication {
    engine: Arc<dyn ScriptEnginePort>,
    circuit_breaker: Arc<Mutex<ScriptCircuitBreaker>>,
}

impl Default for ScriptApplication {
    fn default() -> Self {
        Self::new()
    }
}

/// DUAL-10-05/14: process-wide cache of the last sandbox projection. Iced runs
/// the shared service and publishes; the surface reader republishes the same
/// snapshot so Bevy renders one source of truth.
fn sandbox_cache() -> &'static Mutex<Option<ScriptSandboxSnapshot>> {
    static SANDBOX: OnceLock<Mutex<Option<ScriptSandboxSnapshot>>> = OnceLock::new();
    SANDBOX.get_or_init(|| Mutex::new(None))
}

/// The last script-sandbox projection computed in this process, if any.
pub fn last_script_sandbox() -> Option<ScriptSandboxSnapshot> {
    sandbox_cache().lock().ok().and_then(|cache| cache.clone())
}

/// Replace the process-wide script-sandbox projection.
pub fn publish_script_sandbox(snapshot: ScriptSandboxSnapshot) {
    if let Ok(mut cache) = sandbox_cache().lock() {
        *cache = Some(snapshot);
    }
}

/// Drop the cached projection (a fresh clear makes it stale).
pub fn clear_script_sandbox() {
    if let Ok(mut cache) = sandbox_cache().lock() {
        *cache = None;
    }
}

impl ScriptApplication {
    pub const DEFAULT_MAX_MEMORY_BYTES: usize = 64 * 1024 * 1024; // 64MB
    pub const DEFAULT_TIMEOUT_MS: u64 = 500; // 500ms

    pub fn new() -> Self {
        let engine =
            DirectiveDslScriptEngine::new(Self::DEFAULT_TIMEOUT_MS, Self::DEFAULT_MAX_MEMORY_BYTES);

        Self {
            engine: Arc::new(engine),
            circuit_breaker: Arc::new(Mutex::new(ScriptCircuitBreaker::default())),
        }
    }

    /// DUAL-10-01: build the service over an injected engine. This is the seam
    /// a future real engine (or a test double) drops into; the read model then
    /// reports that engine's kind and negotiated capabilities.
    pub fn with_engine(engine: Arc<dyn ScriptEnginePort>) -> Self {
        Self {
            engine,
            circuit_breaker: Arc::new(Mutex::new(ScriptCircuitBreaker::default())),
        }
    }

    /// The engine kind the shared read model will report.
    pub fn engine_kind(&self) -> ScriptEngineKind {
        self.engine.kind()
    }

    /// The capability negotiation the shared read model will report.
    pub fn engine_capabilities(&self) -> ScriptEngineCapabilities {
        self.engine.capabilities()
    }

    /// The lifecycle stage the selected preset declares, defaulting to
    /// `PreMerge` for ad-hoc scripts.
    fn stage_for_preset(preset: Option<&str>) -> HookStage {
        preset
            .and_then(ScriptEngine::find_preset)
            .map(|preset| preset.stage)
            .unwrap_or(HookStage::PreMerge)
    }

    fn breaker_snapshot(&self) -> ScriptCircuitBreakerSnapshot {
        match self.circuit_breaker.lock() {
            Ok(breaker) => ScriptCircuitBreakerSnapshot {
                tripped: breaker.is_tripped(),
                consecutive_failures: breaker.consecutive_failures(),
                failure_threshold: breaker.failure_threshold(),
                cooldown_ms: breaker.cooldown().as_millis() as u64,
                remaining_cooldown_ms: breaker.remaining_cooldown().as_millis() as u64,
            },
            Err(_) => ScriptCircuitBreakerSnapshot::default(),
        }
    }

    /// Execute a test run of `script` against `input_yaml` and assemble, cache
    /// and return a complete [`ScriptSandboxSnapshot`]. The stage comes from the
    /// selected shared preset.
    pub fn run_sandbox(
        &self,
        script: &str,
        input_yaml: &str,
        preset: Option<&str>,
    ) -> ScriptSandboxSnapshot {
        let stage = Self::stage_for_preset(preset);
        let snapshot = self.run_sandbox_at_stage(script, input_yaml, preset, stage);
        publish_script_sandbox(snapshot.clone());
        snapshot
    }

    /// Execute a test run at an explicit lifecycle stage (DUAL-10-02).
    pub fn run_sandbox_at_stage(
        &self,
        script: &str,
        input_yaml: &str,
        preset: Option<&str>,
        stage: HookStage,
    ) -> ScriptSandboxSnapshot {
        let presets = self.builtin_presets();
        let selected_preset = preset.map(ToString::to_string);
        let start = Instant::now();
        // DUAL-10-01: negotiate once so every branch of this projection reports
        // the same engine kind and capability limits.
        let engine_kind = self.engine.kind();
        let engine_capabilities = self.engine.capabilities();

        // Check circuit breaker first
        {
            let breaker = self.circuit_breaker.lock().unwrap();
            if breaker.is_tripped() {
                drop(breaker);
                return ScriptSandboxSnapshot {
                    engine_kind,
                    engine_capabilities,
                    status: ScriptSandboxStatus::RuntimeError,
                    hook_stage: stage.as_str().to_string(),
                    hook_stage_label: stage.display_name().to_string(),
                    matched_directives: Vec::new(),
                    circuit_breaker: self.breaker_snapshot(),
                    selected_preset,
                    script_code: script.to_string(),
                    input_yaml: input_yaml.to_string(),
                    transformed_yaml: None,
                    execution_time_ms: 0,
                    memory_used_bytes: 0,
                    max_memory_limit_bytes: Self::DEFAULT_MAX_MEMORY_BYTES,
                    timeout_limit_ms: Self::DEFAULT_TIMEOUT_MS,
                    console_logs: vec![ScriptLogEntry::new(
                        0,
                        ScriptLogLevel::Error,
                        "沙箱熔断保护激活 (Circuit Breaker Tripped): 近期连续失败过多，冷却中",
                    )],
                    diff: None,
                    error_detail: Some("熔断保护激活，请稍后再试".to_string()),
                    presets,
                };
            }
        }

        let mut console_logs = extract_console_logs(script, 0);

        let run_result = self.engine.execute(script, input_yaml, stage);

        let elapsed_ms = start.elapsed().as_millis() as u64;
        let estimated_memory = script.len() + input_yaml.len() * 4 + 1024 * 1024; // baseline + buffers

        match run_result {
            Ok(res) => {
                // Record success on circuit breaker
                if let Ok(mut breaker) = self.circuit_breaker.lock() {
                    breaker.record_success();
                }

                // Add additional parsed logs if not already captured
                for log_msg in &res.console_logs {
                    if !console_logs.iter().any(|l| &l.message == log_msg) {
                        console_logs.push(ScriptLogEntry::new(
                            elapsed_ms,
                            ScriptLogLevel::Log,
                            log_msg.clone(),
                        ));
                    }
                }

                let diff = myers_diff::compute_diff(
                    input_yaml,
                    &res.transformed_yaml,
                    "input.yaml",
                    "transformed.yaml",
                );

                ScriptSandboxSnapshot {
                    engine_kind,
                    engine_capabilities,
                    status: ScriptSandboxStatus::Success,
                    hook_stage: stage.as_str().to_string(),
                    hook_stage_label: stage.display_name().to_string(),
                    matched_directives: project_directives(&res),
                    circuit_breaker: self.breaker_snapshot(),
                    selected_preset,
                    script_code: script.to_string(),
                    input_yaml: input_yaml.to_string(),
                    transformed_yaml: Some(res.transformed_yaml),
                    execution_time_ms: elapsed_ms,
                    memory_used_bytes: estimated_memory,
                    max_memory_limit_bytes: Self::DEFAULT_MAX_MEMORY_BYTES,
                    timeout_limit_ms: Self::DEFAULT_TIMEOUT_MS,
                    console_logs,
                    diff: Some(diff),
                    error_detail: None,
                    presets,
                }
            }
            Err(err) => {
                // Record failure on circuit breaker
                if let Ok(mut breaker) = self.circuit_breaker.lock() {
                    breaker.record_failure();
                }

                let (status, err_msg) = match err {
                    ScriptError::Timeout(ms) => (
                        ScriptSandboxStatus::Timeout,
                        format!("脚本执行超时: 耗时超过 {ms}ms 限制"),
                    ),
                    ScriptError::MemoryExceeded(bytes) => (
                        ScriptSandboxStatus::MemoryExceeded,
                        format!("沙箱内存熔断: 内存申请超标 ({bytes} 字节)"),
                    ),
                    ScriptError::Syntax(details) => (
                        ScriptSandboxStatus::SyntaxError,
                        format!("语法解析错误: {details}"),
                    ),
                    ScriptError::Runtime(details) => (
                        ScriptSandboxStatus::RuntimeError,
                        format!("运行时异常: {details}"),
                    ),
                };

                console_logs.push(ScriptLogEntry::new(
                    elapsed_ms,
                    ScriptLogLevel::Error,
                    &err_msg,
                ));

                ScriptSandboxSnapshot {
                    engine_kind,
                    engine_capabilities,
                    status,
                    hook_stage: stage.as_str().to_string(),
                    hook_stage_label: stage.display_name().to_string(),
                    // A failed run applied no directive the caller can trust;
                    // reporting none is the honest safe-degradation record.
                    matched_directives: Vec::new(),
                    circuit_breaker: self.breaker_snapshot(),
                    selected_preset,
                    script_code: script.to_string(),
                    input_yaml: input_yaml.to_string(),
                    transformed_yaml: None,
                    execution_time_ms: elapsed_ms,
                    memory_used_bytes: estimated_memory,
                    max_memory_limit_bytes: Self::DEFAULT_MAX_MEMORY_BYTES,
                    timeout_limit_ms: Self::DEFAULT_TIMEOUT_MS,
                    console_logs,
                    diff: None,
                    error_detail: Some(err_msg),
                    presets,
                }
            }
        }
    }

    /// Validate syntax and entrypoints of the given script.
    pub fn validate_script(&self, script: &str) -> ScriptValidationResult {
        ScriptEngine::validate_script(script)
    }

    /// List all built-in extension presets with descriptive metadata.
    pub fn builtin_presets(&self) -> Vec<ScriptPresetSummary> {
        ScriptEngine::builtin_presets()
            .into_iter()
            .map(|p| ScriptPresetSummary {
                id: p.id.to_string(),
                name: p.name.to_string(),
                description: p.description.to_string(),
                stage: p.stage.as_str().to_string(),
            })
            .collect()
    }

    /// Export an extension package to a JSON string.
    pub fn export_extension(&self, pkg: &ExtensionPackage) -> Result<String, Failure> {
        ScriptEngine::export_extension_package(pkg).map_err(|e| {
            Failure::new(
                ErrorCode::Internal,
                format!("Failed to export extension package: {e}"),
                false,
            )
        })
    }

    /// Import and parse an extension package from a JSON string.
    pub fn import_extension(&self, json: &str) -> Result<ExtensionPackage, Failure> {
        ScriptEngine::import_extension_package(json).map_err(|e| {
            Failure::new(
                ErrorCode::Internal,
                format!("Failed to import extension package: {e}"),
                false,
            )
        })
    }
}

fn project_directives(result: &ScriptExecutionResult) -> Vec<ScriptDirectiveMatch> {
    result
        .matched_directives
        .iter()
        .map(|audit| {
            ScriptDirectiveMatch::new(audit.id.clone(), audit.label.clone(), audit.affected)
        })
        .collect()
}

fn extract_console_logs(script: &str, elapsed_ms: u64) -> Vec<ScriptLogEntry> {
    let mut logs = Vec::new();
    for line in script.lines() {
        let trimmed = line.trim();
        for (prefix, level) in [
            ("console.log(", ScriptLogLevel::Log),
            ("console.info(", ScriptLogLevel::Info),
            ("console.warn(", ScriptLogLevel::Warn),
            ("console.error(", ScriptLogLevel::Error),
        ] {
            if let Some(idx) = trimmed.find(prefix) {
                let rest = &trimmed[idx + prefix.len()..];
                if let Some(end) = rest.rfind(')') {
                    let arg = rest[..end].trim();
                    let clean = arg.trim_matches(|c| c == '"' || c == '\'');
                    logs.push(ScriptLogEntry::new(elapsed_ms, level, clean));
                }
            }
        }
    }
    logs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_application_initialization_and_presets() {
        let app = ScriptApplication::new();
        let presets = app.builtin_presets();
        assert!(!presets.is_empty());
        assert!(presets.iter().any(|p| p.id == "auto-country-groups"));
    }

    #[test]
    fn run_sandbox_successful_transformation_and_diff_generation() {
        let app = ScriptApplication::new();
        let script = r#"function main(config, profile) {
            console.log("Starting auto-grouping");
            auto_country_groups(config);
            console.info("Done grouping");
            return config;
        }"#;

        let input_yaml =
            "proxies:\n  - name: 🇭🇰 HK 01\n    type: ss\n  - name: 🇯🇵 JP 01\n    type: ss\n";
        let snapshot = app.run_sandbox(script, input_yaml, Some("auto-country-groups"));

        assert_eq!(snapshot.status, ScriptSandboxStatus::Success);
        assert!(snapshot.transformed_yaml.is_some());
        assert!(snapshot.diff.is_some());
        assert!(snapshot.diff.as_ref().unwrap().has_differences());
        assert_eq!(
            snapshot.selected_preset.as_deref(),
            Some("auto-country-groups")
        );
        assert!(
            snapshot
                .console_logs
                .iter()
                .any(|l| l.message.contains("Starting auto-grouping"))
        );
    }

    #[test]
    fn run_sandbox_projects_only_matched_directives_and_real_stage() {
        let app = ScriptApplication::new();
        let script =
            "function main(config, profile) {\n  auto_country_groups(config);\n  return config;\n}";
        let snapshot = app.run_sandbox(
            script,
            "proxies:\n  - name: 🇭🇰 HK 01\n    type: ss\n",
            Some("auto-country-groups"),
        );
        // The preset declares `pre_merge`; the projection must carry it.
        assert_eq!(snapshot.hook_stage, "pre_merge");
        assert!(snapshot.hook_stage_label.contains("Pre-Merge"));
        // Only the directive that appeared is listed; the others never ran.
        let ids: Vec<&str> = snapshot
            .matched_directives
            .iter()
            .map(|m| m.id.as_str())
            .collect();
        assert_eq!(ids, vec!["auto_country_groups"]);
        assert!(!ids.contains(&"direct_china"));
        assert!(!ids.contains(&"streaming_groups"));
        assert_eq!(snapshot.engine_kind, ScriptEngineKind::DirectiveDsl);
        assert!(!snapshot.engine_kind.is_real_javascript());
        // Published for the other surface.
        assert_eq!(last_script_sandbox().as_ref(), Some(&snapshot));
    }

    #[test]
    fn run_sandbox_at_stage_reports_the_requested_hook() {
        let app = ScriptApplication::new();
        let snapshot = app.run_sandbox_at_stage(
            "function main(config) { return config; }",
            "port: 7890\n",
            None,
            HookStage::PostDownload,
        );
        assert_eq!(snapshot.status, ScriptSandboxStatus::Success);
        assert_eq!(snapshot.hook_stage, "post_download");
        assert!(snapshot.hook_stage_label.contains("Post-Download"));
    }

    #[test]
    fn run_sandbox_catches_infinite_loop_timeout() {
        let app = ScriptApplication::new();
        let script = "function main(config) { while(true) {} }";
        let input_yaml = "port: 7890";

        let snapshot = app.run_sandbox(script, input_yaml, None);
        assert_eq!(snapshot.status, ScriptSandboxStatus::Timeout);
        assert!(snapshot.error_detail.is_some());
        assert!(snapshot.has_error());
    }

    #[test]
    fn run_sandbox_catches_syntax_error() {
        let app = ScriptApplication::new();
        let script = "let x = 42;"; // Missing function main
        let input_yaml = "port: 7890";

        let snapshot = app.run_sandbox(script, input_yaml, None);
        assert_eq!(snapshot.status, ScriptSandboxStatus::SyntaxError);
        assert!(snapshot.has_error());
    }

    #[test]
    fn safe_degradation_preserves_the_input_and_reports_no_directive() {
        let app = ScriptApplication::new();
        let input = "port: 7890\nrules:\n  - MATCH,DIRECT\n";
        let snapshot = app.run_sandbox(
            "function main(config) { remove_rules(config, \"([\"); return config; }",
            input,
            None,
        );
        assert!(snapshot.has_error());
        assert!(snapshot.transformed_yaml.is_none());
        assert_eq!(snapshot.input_yaml, input);
        assert!(snapshot.matched_directives.is_empty());
        assert!(snapshot.error_detail.is_some());
    }

    /// DUAL-10-01: a fake second engine that reports the JavaScript
    /// negotiation slot. It proves an engine swap changes the reported kind and
    /// capability limits with no surface edit; it is test-only and never on the
    /// shipped path.
    struct FakeJavascriptScriptEngine;

    impl ScriptEnginePort for FakeJavascriptScriptEngine {
        fn kind(&self) -> ScriptEngineKind {
            ScriptEngineKind::JavascriptEngine
        }

        fn capabilities(&self) -> ScriptEngineCapabilities {
            ScriptEngineCapabilities {
                supports_javascript_syntax: true,
                supports_directive_dsl: false,
                ..ScriptEngineCapabilities::directive_dsl()
            }
        }

        fn execute(
            &self,
            _script: &str,
            input_yaml: &str,
            stage: HookStage,
        ) -> Result<ScriptExecutionResult, ScriptError> {
            Ok(ScriptExecutionResult {
                transformed_yaml: input_yaml.to_string(),
                console_logs: vec!["fake js engine ran".to_string()],
                execution_time_ms: 3,
                success: true,
                stage,
                matched_directives: Vec::new(),
            })
        }
    }

    #[test]
    fn engine_seam_switches_the_reported_kind_and_capabilities() {
        // Default: the bundled directive DSL, honest about having no JS syntax.
        let default_app = ScriptApplication::new();
        assert_eq!(default_app.engine_kind(), ScriptEngineKind::DirectiveDsl);
        let default_capabilities = default_app.engine_capabilities();
        assert!(!default_capabilities.supports_javascript_syntax);
        assert!(default_capabilities.supports_directive_dsl);

        // Inject the fake second engine: the same application surfaces now
        // report the new kind and the negotiated JS capability.
        let app = ScriptApplication::with_engine(Arc::new(FakeJavascriptScriptEngine));
        let snapshot = app.run_sandbox(
            "function main(config) { return config; }",
            "port: 7890\n",
            None,
        );
        assert_eq!(snapshot.engine_kind, ScriptEngineKind::JavascriptEngine);
        assert!(snapshot.engine_kind.is_real_javascript());
        assert!(snapshot.engine_capabilities.supports_javascript_syntax);
        assert!(!snapshot.engine_capabilities.supports_directive_dsl);
        assert!(snapshot.engine_kind_matches_capabilities());
        // The label methods both surfaces call already render the new engine,
        // so the swap needs no surface change.
        assert!(snapshot.engine_label_zh().contains("JavaScript 引擎"));
        assert!(snapshot.engine_label_en().contains("ECMAScript"));
        assert!(
            snapshot
                .engine_capability_label_zh()
                .contains("支持 JavaScript")
        );
        assert!(
            snapshot
                .engine_capability_label_en()
                .contains("JavaScript syntax supported")
        );
        // The projection published for the Bevy surface carries the same fact.
        assert_eq!(last_script_sandbox().as_ref(), Some(&snapshot));
    }
}

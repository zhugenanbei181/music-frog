//! Application service for sandboxed script execution, validation, presets and diffing.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::script_sandbox::{
    ScriptLogEntry, ScriptLogLevel, ScriptPresetSummary, ScriptSandboxSnapshot, ScriptSandboxStatus,
};
use infiltrator_domain::myers_diff;
use infiltrator_domain::script_engine::{
    ExtensionPackage, HookStage, ScriptCircuitBreaker, ScriptEngine, ScriptError,
    ScriptValidationResult,
};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Thread-safe application service for managing QuickJS scripts and sandboxes.
#[derive(Clone)]
pub struct ScriptApplication {
    engine: Arc<ScriptEngine>,
    circuit_breaker: Arc<Mutex<ScriptCircuitBreaker>>,
}

impl Default for ScriptApplication {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptApplication {
    pub const DEFAULT_MAX_MEMORY_BYTES: usize = 64 * 1024 * 1024; // 64MB
    pub const DEFAULT_TIMEOUT_MS: u64 = 500; // 500ms

    pub fn new() -> Self {
        let engine = ScriptEngine::new()
            .with_timeout(Duration::from_millis(Self::DEFAULT_TIMEOUT_MS))
            .with_max_memory(Self::DEFAULT_MAX_MEMORY_BYTES);

        Self {
            engine: Arc::new(engine),
            circuit_breaker: Arc::new(Mutex::new(ScriptCircuitBreaker::default())),
        }
    }

    /// Execute a test run of `script` against `input_yaml` and assemble a complete [`ScriptSandboxSnapshot`].
    pub fn run_sandbox(
        &self,
        script: &str,
        input_yaml: &str,
        preset: Option<&str>,
    ) -> ScriptSandboxSnapshot {
        let presets = self.builtin_presets();
        let selected_preset = preset.map(ToString::to_string);
        let start = Instant::now();

        // Check circuit breaker first
        {
            let breaker = self.circuit_breaker.lock().unwrap();
            if breaker.is_tripped() {
                return ScriptSandboxSnapshot {
                    status: ScriptSandboxStatus::RuntimeError,
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

        let run_result = self.engine.execute_transform_detailed(
            script,
            input_yaml,
            HookStage::PreMerge,
        );

        let elapsed_ms = start.elapsed().as_millis() as u64;
        let estimated_memory = script.len() + input_yaml.len() * 4 + 1024 * 1024; // baseline + buffers

        match run_result {
            Ok(res) => {
                // Record success on circuit breaker
                if let Ok(mut breaker) = self.circuit_breaker.lock() {
                    breaker.record_success();
                }

                // Add additional parsed logs if not already captured
                for log_msg in res.console_logs {
                    if !console_logs.iter().any(|l| l.message == log_msg) {
                        console_logs.push(ScriptLogEntry::new(elapsed_ms, ScriptLogLevel::Log, log_msg));
                    }
                }

                let diff = myers_diff::compute_diff(
                    input_yaml,
                    &res.transformed_yaml,
                    "input.yaml",
                    "transformed.yaml",
                );

                ScriptSandboxSnapshot {
                    status: ScriptSandboxStatus::Success,
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
                    status,
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
        ScriptEngine::export_extension_package(pkg)
            .map_err(|e| Failure::new(ErrorCode::Internal, format!("Failed to export extension package: {e}"), false))
    }

    /// Import and parse an extension package from a JSON string.
    pub fn import_extension(&self, json: &str) -> Result<ExtensionPackage, Failure> {
        ScriptEngine::import_extension_package(json)
            .map_err(|e| Failure::new(ErrorCode::Internal, format!("Failed to import extension package: {e}"), false))
    }
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

        let input_yaml = "proxies:\n  - name: 🇭🇰 HK 01\n    type: ss\n  - name: 🇯🇵 JP 01\n    type: ss\n";
        let snapshot = app.run_sandbox(script, input_yaml, Some("auto-country-groups"));

        assert_eq!(snapshot.status, ScriptSandboxStatus::Success);
        assert!(snapshot.transformed_yaml.is_some());
        assert!(snapshot.diff.is_some());
        assert!(snapshot.diff.as_ref().unwrap().has_differences());
        assert_eq!(snapshot.selected_preset.as_deref(), Some("auto-country-groups"));
        assert!(snapshot.console_logs.iter().any(|l| l.message.contains("Starting auto-grouping")));
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
}

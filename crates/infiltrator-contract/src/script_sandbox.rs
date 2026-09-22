//! Shared script-sandbox console read model, execution metrics and streaming log capture.
//!
//! Runtime truth: this workspace bundles **no JavaScript / QuickJS engine**.
//! `infiltrator_domain::script_engine` recognises a fixed set of known
//! directives (`auto_country_groups`, `filter_nodes_by_regex`, …) with regexes
//! and rewrites the YAML AST. The read model therefore reports
//! [`ScriptEngineKind::DirectiveDsl`] and only lists directives that really
//! matched ([`ScriptDirectiveMatch`]); nothing pretends to have executed
//! arbitrary JavaScript.

use crate::yaml_ast_diff::YamlAstDiffSnapshot;
use serde::{Deserialize, Serialize};

/// Current execution state of the sandboxed script.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptSandboxStatus {
    #[default]
    Idle,
    Running,
    Success,
    Timeout,
    MemoryExceeded,
    SyntaxError,
    RuntimeError,
}

impl ScriptSandboxStatus {
    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::Idle => "就绪",
            Self::Running => "执行中...",
            Self::Success => "执行成功",
            Self::Timeout => "超时中断 (>500ms)",
            Self::MemoryExceeded => "内存熔断 (>64MB)",
            Self::SyntaxError => "语法错误",
            Self::RuntimeError => "运行时异常",
        }
    }

    pub const fn is_error(self) -> bool {
        matches!(
            self,
            Self::Timeout | Self::MemoryExceeded | Self::SyntaxError | Self::RuntimeError
        )
    }
}

/// Log level of an intercepted script console message.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptLogLevel {
    #[default]
    Log,
    Info,
    Warn,
    Error,
}

impl ScriptLogLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Log => "LOG",
            Self::Info => "INF",
            Self::Warn => "WRN",
            Self::Error => "ERR",
        }
    }
}

/// A captured console log line emitted by the script sandbox during execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptLogEntry {
    /// Relative elapsed milliseconds from script start.
    pub timestamp_ms: u64,
    /// Severity level.
    pub level: ScriptLogLevel,
    /// Emitted message body.
    pub message: String,
}

impl ScriptLogEntry {
    pub fn new(timestamp_ms: u64, level: ScriptLogLevel, message: impl Into<String>) -> Self {
        Self {
            timestamp_ms,
            level,
            message: message.into(),
        }
    }
}

/// Summary description of an available script preset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptPresetSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub stage: String,
}

/// What actually produced a sandbox projection.
///
/// The workspace bundles no JavaScript runtime, so the only honest value today
/// is [`ScriptEngineKind::DirectiveDsl`]. The enum exists (rather than a bare
/// `bool`) so a future real engine could be reported without changing the
/// surface contract, and so both surfaces can render the honest label.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptEngineKind {
    /// Regex-matched known directives that rewrite the YAML AST — *not* a JS
    /// interpreter.
    #[default]
    DirectiveDsl,
}

impl ScriptEngineKind {
    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::DirectiveDsl => "指令 DSL（正则识别，非 JavaScript 引擎）",
        }
    }

    pub const fn is_real_javascript(self) -> bool {
        false
    }
}

/// One directive the DSL actually matched and applied to the YAML AST.
///
/// A directive whose pattern did not appear in the script never gets a row, so
/// the console can never claim that un-run work executed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptDirectiveMatch {
    /// Stable directive identifier, e.g. `auto_country_groups`.
    pub id: String,
    /// Human label rendered by both surfaces.
    pub label: String,
    /// Items the directive changed. `0` when the directive reports no count.
    pub affected: usize,
}

impl ScriptDirectiveMatch {
    pub fn new(id: impl Into<String>, label: impl Into<String>, affected: usize) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            affected,
        }
    }
}

/// Circuit-breaker state of the sandbox at projection time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptCircuitBreakerSnapshot {
    /// `true` when repeated failures have opened the breaker.
    pub tripped: bool,
    /// Consecutive failures observed so far.
    pub consecutive_failures: usize,
    /// Failures required before the breaker opens.
    pub failure_threshold: usize,
    /// Cooldown window in milliseconds once tripped.
    pub cooldown_ms: u64,
    /// Remaining cooldown in milliseconds (`0` when closed/expired).
    pub remaining_cooldown_ms: u64,
}

impl ScriptCircuitBreakerSnapshot {
    pub const fn label_zh(self) -> &'static str {
        if self.tripped {
            "熔断（冷却中）"
        } else {
            "闭合（可用）"
        }
    }
}

/// Read model snapshot representing the active state and execution results of the script sandbox.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ScriptSandboxSnapshot {
    /// What produced this projection (always the directive DSL today).
    #[serde(default)]
    pub engine_kind: ScriptEngineKind,
    /// Sandbox operational status.
    pub status: ScriptSandboxStatus,
    /// Lifecycle hook stage the transform ran at, e.g. `pre_merge`.
    #[serde(default)]
    pub hook_stage: String,
    /// Human label for [`Self::hook_stage`].
    #[serde(default)]
    pub hook_stage_label: String,
    /// Directives that really matched and were applied, in execution order.
    #[serde(default)]
    pub matched_directives: Vec<ScriptDirectiveMatch>,
    /// Circuit-breaker state at projection time.
    #[serde(default)]
    pub circuit_breaker: ScriptCircuitBreakerSnapshot,
    /// Currently selected preset identifier if loaded from templates.
    pub selected_preset: Option<String>,
    /// Active script source (the directive DSL, not JavaScript).
    pub script_code: String,
    /// Current input YAML payload before transformation.
    pub input_yaml: String,
    /// Resulting YAML payload after transformation (None if failed or not run).
    pub transformed_yaml: Option<String>,
    /// Actual wall-clock execution time in milliseconds.
    pub execution_time_ms: u64,
    /// Estimated memory usage during execution in bytes.
    pub memory_used_bytes: usize,
    /// Hard memory allocation limit in bytes (64MB = 67,108,864).
    pub max_memory_limit_bytes: usize,
    /// Hard timeout limit in milliseconds (500ms).
    pub timeout_limit_ms: u64,
    /// Intercepted console.log / console.error message stream.
    pub console_logs: Vec<ScriptLogEntry>,
    /// Visual diff comparison between input_yaml and transformed_yaml.
    pub diff: Option<YamlAstDiffSnapshot>,
    /// Detailed error explanation when status is in error.
    pub error_detail: Option<String>,
    /// List of available built-in script presets.
    pub presets: Vec<ScriptPresetSummary>,
}

impl ScriptSandboxSnapshot {
    pub const DEFAULT_MAX_MEMORY_BYTES: usize = 64 * 1024 * 1024;
    pub const DEFAULT_TIMEOUT_MS: u64 = 500;

    /// Deterministic fixture for headless UI tests. It mirrors what the real
    /// directive-DSL run of `auto_country_groups` produces; it never claims a
    /// JavaScript engine ran.
    pub fn demo_fixture() -> Self {
        let diff = YamlAstDiffSnapshot::demo_fixture();
        let console_logs = vec![
            ScriptLogEntry::new(
                2,
                ScriptLogLevel::Info,
                "指令 DSL 沙箱就绪 [内存限额: 64MB, 超时限额: 500ms]",
            ),
            ScriptLogEntry::new(
                14,
                ScriptLogLevel::Log,
                "加载输入配置: 2 个节点, 0 条分流规则",
            ),
            ScriptLogEntry::new(52, ScriptLogLevel::Log, "AST 变换完成，生成最终目标配置"),
        ];
        let matched_directives = vec![ScriptDirectiveMatch::new(
            "auto_country_groups",
            "自动生成国家地区策略组",
            2,
        )];

        let presets = vec![
            ScriptPresetSummary {
                id: "auto-country-groups".to_string(),
                name: "国家地区自动分组".to_string(),
                description: "按节点名称关键词与国旗 Emoji 自动聚合分流策略组".to_string(),
                stage: "pre_merge".to_string(),
            },
            ScriptPresetSummary {
                id: "streaming-groups".to_string(),
                name: "流媒体分流拦截".to_string(),
                description: "生成 Netflix, Disney+, YouTube, OpenAI 专用出站组".to_string(),
                stage: "post_merge".to_string(),
            },
            ScriptPresetSummary {
                id: "direct-china".to_string(),
                name: "国内直连与私网分流".to_string(),
                description: "自动追加 GeoIP CN 与内网直连穿透分流规则".to_string(),
                stage: "post_merge".to_string(),
            },
            ScriptPresetSummary {
                id: "remove-ads".to_string(),
                name: "广告与残留节点清理".to_string(),
                description: "移除节点名称包含过期/流量/官网广告的垃圾节点".to_string(),
                stage: "pre_merge".to_string(),
            },
        ];

        Self {
            engine_kind: ScriptEngineKind::DirectiveDsl,
            status: ScriptSandboxStatus::Success,
            hook_stage: "pre_merge".to_string(),
            hook_stage_label: "Pre-Merge (合并前)".to_string(),
            matched_directives,
            circuit_breaker: ScriptCircuitBreakerSnapshot {
                tripped: false,
                consecutive_failures: 0,
                failure_threshold: 3,
                cooldown_ms: 30_000,
                remaining_cooldown_ms: 0,
            },
            selected_preset: Some("auto-country-groups".to_string()),
            script_code: "function main(config, profile) {\n  auto_country_groups(config);\n  return config;\n}".to_string(),
            input_yaml: "proxies:\n  - name: 🇭🇰 香港 01\n    type: ss\n  - name: 🇯🇵 日本 01\n    type: ss\n".to_string(),
            transformed_yaml: Some("proxies:\n  - name: 🇭🇰 香港 01\n    type: ss\n  - name: 🇯🇵 日本 01\n    type: ss\nproxy-groups:\n  - name: 🇭🇰 香港\n    type: select\n".to_string()),
            execution_time_ms: 54,
            memory_used_bytes: 4 * 1024 * 1024,
            max_memory_limit_bytes: Self::DEFAULT_MAX_MEMORY_BYTES,
            timeout_limit_ms: Self::DEFAULT_TIMEOUT_MS,
            console_logs,
            diff: Some(diff),
            error_detail: None,
            presets,
        }
    }

    pub fn is_success(&self) -> bool {
        self.status == ScriptSandboxStatus::Success
    }

    pub fn is_running(&self) -> bool {
        self.status == ScriptSandboxStatus::Running
    }

    pub fn has_error(&self) -> bool {
        self.status.is_error()
    }

    pub fn log_count(&self) -> usize {
        self.console_logs.len()
    }

    /// Number of directives that really matched and executed.
    pub fn matched_directive_count(&self) -> usize {
        self.matched_directives.len()
    }

    /// `true` while the breaker is open and cooling down.
    pub fn is_circuit_tripped(&self) -> bool {
        self.circuit_breaker.tripped
    }

    /// The honest engine label both surfaces must render.
    pub fn engine_label_zh(&self) -> &'static str {
        self.engine_kind.label_zh()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_sandbox_status_error_predicate() {
        assert!(ScriptSandboxStatus::Timeout.is_error());
        assert!(ScriptSandboxStatus::MemoryExceeded.is_error());
        assert!(ScriptSandboxStatus::SyntaxError.is_error());
        assert!(ScriptSandboxStatus::RuntimeError.is_error());
        assert!(!ScriptSandboxStatus::Success.is_error());
        assert!(!ScriptSandboxStatus::Idle.is_error());
    }

    #[test]
    fn demo_fixture_integrity_and_serde() {
        let fixture = ScriptSandboxSnapshot::demo_fixture();
        assert!(fixture.is_success());
        assert!(!fixture.has_error());
        assert_eq!(fixture.max_memory_limit_bytes, 64 * 1024 * 1024);
        assert_eq!(fixture.timeout_limit_ms, 500);
        assert_eq!(fixture.log_count(), 3);
        assert_eq!(fixture.presets.len(), 4);
        assert!(fixture.diff.is_some());
        assert_eq!(fixture.engine_kind, ScriptEngineKind::DirectiveDsl);
        assert!(!fixture.engine_kind.is_real_javascript());
        assert_eq!(fixture.matched_directive_count(), 1);
        assert_eq!(fixture.hook_stage, "pre_merge");
        assert!(!fixture.is_circuit_tripped());
        assert_eq!(fixture.circuit_breaker.failure_threshold, 3);

        let serialized =
            serde_json::to_string(&fixture).expect("serialize script sandbox snapshot");
        let deserialized: ScriptSandboxSnapshot =
            serde_json::from_str(&serialized).expect("deserialize script sandbox snapshot");
        assert_eq!(fixture, deserialized);
    }

    #[test]
    fn engine_kind_never_claims_javascript() {
        assert!(!ScriptEngineKind::DirectiveDsl.is_real_javascript());
        assert!(
            ScriptEngineKind::DirectiveDsl
                .label_zh()
                .contains("非 JavaScript")
        );
        assert!(
            ScriptCircuitBreakerSnapshot::default()
                .label_zh()
                .contains("闭合")
        );
        let tripped = ScriptCircuitBreakerSnapshot {
            tripped: true,
            ..Default::default()
        };
        assert!(tripped.label_zh().contains("熔断"));
    }
}

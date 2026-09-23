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
/// DUAL-10-01: the workspace bundles no JavaScript runtime, so the only value
/// the shipped default ever produces is [`ScriptEngineKind::DirectiveDsl`].
/// [`ScriptEngineKind::JavascriptEngine`] is the negotiation slot for a future
/// real engine: a host that injects one reports it here and both surfaces
/// render the new label without any surface change. The enum exists (rather
/// than a bare `bool`) so an engine swap is a read-model fact, not a UI edit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptEngineKind {
    /// Regex-matched known directives that rewrite the YAML AST — *not* a JS
    /// interpreter.
    #[default]
    DirectiveDsl,
    /// A real ECMAScript interpreter. No such engine is bundled today; the
    /// variant is only ever produced when a host injects one.
    JavascriptEngine,
}

impl ScriptEngineKind {
    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::DirectiveDsl => "指令 DSL（正则识别，非 JavaScript 引擎）",
            Self::JavascriptEngine => "JavaScript 引擎（真实解析 ECMAScript）",
        }
    }

    pub const fn label_en(self) -> &'static str {
        match self {
            Self::DirectiveDsl => "Directive DSL (regex-matched, not a JavaScript engine)",
            Self::JavascriptEngine => "JavaScript engine (real ECMAScript parser)",
        }
    }

    /// `true` only for a real ECMAScript interpreter. The bundled directive DSL
    /// is always `false`.
    pub const fn is_real_javascript(self) -> bool {
        matches!(self, Self::JavascriptEngine)
    }
}

/// The capability negotiation reported next to [`ScriptEngineKind`].
///
/// DUAL-10-01: the read model must state the *limits* of whatever produced a
/// result, not just its name. The bundled directive DSL always negotiates
/// `supports_javascript_syntax = false`; a future engine would flip exactly
/// that flag. Both surfaces render [`Self::bottom_line_zh`] / [`Self::bottom_line_en`]
/// so "no JS syntax" is never implied away.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptEngineCapabilities {
    /// `true` only for a real ECMAScript interpreter.
    pub supports_javascript_syntax: bool,
    /// `true` when the engine recognises the known regex directives.
    pub supports_directive_dsl: bool,
    /// `true` when `console.*` output is captured into the read model.
    pub captures_console: bool,
    /// `true` when the engine enforces a hard wall-clock timeout.
    pub enforces_timeout: bool,
    /// `true` when the engine enforces a memory ceiling.
    pub enforces_memory_limit: bool,
    /// Hard timeout ceiling in milliseconds.
    pub timeout_ms: u64,
    /// Hard memory ceiling in bytes.
    pub max_memory_bytes: usize,
}

impl Default for ScriptEngineCapabilities {
    fn default() -> Self {
        Self::directive_dsl()
    }
}

impl ScriptEngineCapabilities {
    pub const DEFAULT_MAX_MEMORY_BYTES: usize = 64 * 1024 * 1024;
    pub const DEFAULT_TIMEOUT_MS: u64 = 500;

    /// The honest negotiation of the bundled default engine.
    pub const fn directive_dsl() -> Self {
        Self {
            supports_javascript_syntax: false,
            supports_directive_dsl: true,
            captures_console: true,
            enforces_timeout: true,
            enforces_memory_limit: true,
            timeout_ms: Self::DEFAULT_TIMEOUT_MS,
            max_memory_bytes: Self::DEFAULT_MAX_MEMORY_BYTES,
        }
    }

    /// The one capability limit the UI must never hide: JS syntax support.
    pub const fn bottom_line_zh(self) -> &'static str {
        if self.supports_javascript_syntax {
            "支持 JavaScript 语法"
        } else {
            "不支持 JavaScript 语法（仅指令 DSL）"
        }
    }

    pub const fn bottom_line_en(self) -> &'static str {
        if self.supports_javascript_syntax {
            "JavaScript syntax supported"
        } else {
            "No JavaScript syntax (directive DSL only)"
        }
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
    /// DUAL-10-01: capability negotiation for [`Self::engine_kind`]. A future
    /// engine swap flips `supports_javascript_syntax` without a surface change.
    #[serde(default)]
    pub engine_capabilities: ScriptEngineCapabilities,
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
            engine_capabilities: ScriptEngineCapabilities::directive_dsl(),
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

    /// The same label in English, for the Iced `en-US` locale.
    pub fn engine_label_en(&self) -> &'static str {
        self.engine_kind.label_en()
    }

    /// The negotiated capability bottom line both surfaces must render.
    pub fn engine_capability_label_zh(&self) -> &'static str {
        self.engine_capabilities.bottom_line_zh()
    }

    /// The same capability bottom line in English.
    pub fn engine_capability_label_en(&self) -> &'static str {
        self.engine_capabilities.bottom_line_en()
    }

    /// `true` when the reported kind agrees with the negotiated capabilities.
    ///
    /// The port seam sets both from one engine; a caller that hand-builds a
    /// snapshot can use this to assert it did not contradict itself.
    pub fn engine_kind_matches_capabilities(&self) -> bool {
        self.engine_kind.is_real_javascript() == self.engine_capabilities.supports_javascript_syntax
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
        assert!(fixture.engine_kind_matches_capabilities());
        assert!(!fixture.engine_capabilities.supports_javascript_syntax);
        assert!(fixture.engine_capabilities.supports_directive_dsl);
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
        // The negotiation slot is real but distinct: only a host-injected JS
        // engine may report it, and its label and capability agree.
        assert!(ScriptEngineKind::JavascriptEngine.is_real_javascript());
        assert!(
            ScriptEngineKind::JavascriptEngine
                .label_zh()
                .contains("JavaScript")
        );
        assert!(
            ScriptEngineKind::JavascriptEngine
                .label_en()
                .contains("ECMAScript")
        );
        let js_capabilities = ScriptEngineCapabilities {
            supports_javascript_syntax: true,
            ..ScriptEngineCapabilities::directive_dsl()
        };
        assert!(js_capabilities.bottom_line_zh().contains("支持 JavaScript"));
        assert_eq!(
            ScriptEngineCapabilities::directive_dsl().bottom_line_zh(),
            "不支持 JavaScript 语法（仅指令 DSL）"
        );
        assert!(
            ScriptEngineCapabilities::directive_dsl()
                .bottom_line_en()
                .contains("No JavaScript syntax")
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

    #[test]
    fn capability_defaults_state_the_no_js_boundary() {
        let capabilities = ScriptEngineCapabilities::default();
        assert!(!capabilities.supports_javascript_syntax);
        assert!(capabilities.supports_directive_dsl);
        assert!(capabilities.captures_console);
        assert!(capabilities.enforces_timeout);
        assert!(capabilities.enforces_memory_limit);
        assert_eq!(capabilities.timeout_ms, 500);
        assert_eq!(capabilities.max_memory_bytes, 64 * 1024 * 1024);
        // A snapshot whose kind and capabilities contradict each other is
        // detectable, so no surface can quietly hide the real boundary.
        let mut snapshot = ScriptSandboxSnapshot::demo_fixture();
        snapshot.engine_kind = ScriptEngineKind::JavascriptEngine;
        assert!(!snapshot.engine_kind_matches_capabilities());
        snapshot.engine_capabilities.supports_javascript_syntax = true;
        assert!(snapshot.engine_kind_matches_capabilities());
    }
}

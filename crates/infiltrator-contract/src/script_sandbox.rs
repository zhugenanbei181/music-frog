//! Shared QuickJS script sandbox console, execution metrics, and streaming log read model.

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

/// Read model snapshot representing the active state and execution results of the script sandbox.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ScriptSandboxSnapshot {
    /// Sandbox operational status.
    pub status: ScriptSandboxStatus,
    /// Currently selected preset identifier if loaded from templates.
    pub selected_preset: Option<String>,
    /// Active JavaScript transform source code.
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

    /// Deterministic fixture for headless UI tests.
    pub fn demo_fixture() -> Self {
        let diff = YamlAstDiffSnapshot::demo_fixture();
        let console_logs = vec![
            ScriptLogEntry::new(2, ScriptLogLevel::Info, "QuickJS 沙箱初始化完毕 [内存限额: 64MB, 超时限额: 500ms]"),
            ScriptLogEntry::new(14, ScriptLogLevel::Log, "加载输入配置: 32 个节点, 14 条分流规则"),
            ScriptLogEntry::new(28, ScriptLogLevel::Log, "自动聚合国家地区策略组: 香港, 日本, 美国, 新加坡"),
            ScriptLogEntry::new(45, ScriptLogLevel::Info, "注入中国大陆直连分流规则: 4 条"),
            ScriptLogEntry::new(52, ScriptLogLevel::Log, "AST 变换完成，生成最终目标配置"),
        ];

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
                stage: "pre_merge".to_string(),
            },
            ScriptPresetSummary {
                id: "direct-china".to_string(),
                name: "开发与内网穿透".to_string(),
                description: "自动追加 GeoIP CN 与内网直连穿透分流规则".to_string(),
                stage: "post_download".to_string(),
            },
            ScriptPresetSummary {
                id: "remove-ads".to_string(),
                name: "广告与残留节点清理".to_string(),
                description: "移除节点名称包含过期/流量/官网广告的垃圾节点".to_string(),
                stage: "pre_merge".to_string(),
            },
        ];

        Self {
            status: ScriptSandboxStatus::Success,
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
        assert_eq!(fixture.log_count(), 5);
        assert_eq!(fixture.presets.len(), 4);
        assert!(fixture.diff.is_some());

        let serialized = serde_json::to_string(&fixture).expect("serialize script sandbox snapshot");
        let deserialized: ScriptSandboxSnapshot =
            serde_json::from_str(&serialized).expect("deserialize script sandbox snapshot");
        assert_eq!(fixture, deserialized);
    }
}

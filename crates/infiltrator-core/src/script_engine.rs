//! Sandboxed scripting engine, YAML AST transformers, plugin manifests, and Web API shims.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_yaml_ng::Value;
use std::collections::HashSet;
use std::time::{Duration, Instant};

#[path = "script_engine_directives.rs"]
mod script_engine_directives;
#[path = "script_engine_presets.rs"]
mod script_engine_presets;
#[path = "script_engine_runtime.rs"]
mod script_engine_runtime;
#[path = "script_engine_shims.rs"]
mod script_engine_shims;
#[path = "script_engine_validation.rs"]
mod script_engine_validation;
#[path = "script_engine_yaml.rs"]
mod script_engine_yaml;
pub const DEFAULT_SCRIPT_TIMEOUT_MS: u64 = 500;
pub const DEFAULT_MAX_MEMORY_BYTES: usize = 64 * 1024 * 1024;

/// Lifecycle hook execution stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HookStage {
    PreDownload,
    PostDownload,
    #[default]
    PreMerge,
    PostMerge,
}

impl HookStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreDownload => "pre_download",
            Self::PostDownload => "post_download",
            Self::PreMerge => "pre_merge",
            Self::PostMerge => "post_merge",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::PreDownload => "Pre-Download (下载前)",
            Self::PostDownload => "Post-Download (下载后)",
            Self::PreMerge => "Pre-Merge (合并前)",
            Self::PostMerge => "Post-Merge (合并后)",
        }
    }
}

/// A built-in or custom extension script preset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub stage: HookStage,
    pub script_code: &'static str,
}

/// Result of executing a transform script.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptExecutionResult {
    pub transformed_yaml: String,
    pub console_logs: Vec<String>,
    pub execution_time_ms: u64,
    pub success: bool,
    pub stage: HookStage,
}

/// Script execution errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScriptError {
    #[error("Script execution timed out after {0}ms")]
    Timeout(u64),
    #[error("Script syntax error: {0}")]
    Syntax(String),
    #[error("Runtime error: {0}")]
    Runtime(String),
    #[error("Memory guard limit exceeded ({0} bytes)")]
    MemoryExceeded(usize),
}

/// An exportable/importable script package bundle (`.infiltrator-ext` / JSON).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionPackage {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    #[serde(default)]
    pub stage: HookStage,
    pub script_code: String,
    pub mixin_yaml: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl ExtensionPackage {
    /// Calculate SHA-256 integrity checksum of the extension package.
    pub fn calculate_checksum(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.name.as_bytes());
        hasher.update(b":");
        hasher.update(self.version.as_bytes());
        hasher.update(b":");
        hasher.update(self.script_code.as_bytes());
        if let Some(ref mixin) = self.mixin_yaml {
            hasher.update(b":");
            hasher.update(mixin.as_bytes());
        }
        let result = hasher.finalize();
        let mut hex = String::with_capacity(result.len() * 2);
        for byte in result {
            use std::fmt::Write;
            let _ = write!(hex, "{:02x}", byte);
        }
        hex
    }

    /// Verify the integrity checksum against expected value.
    pub fn verify_checksum(&self, expected: &str) -> bool {
        let actual = self.calculate_checksum();
        CryptoSubtleShim::timing_safe_equal(actual.as_bytes(), expected.as_bytes())
    }
}

/// Plugin permission boundaries declared in plugin manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginPermission {
    NetworkAccess,
    FileSystemRead,
    Notification,
    ModifyRules,
}

impl PluginPermission {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NetworkAccess => "network_access",
            Self::FileSystemRead => "file_system_read",
            Self::Notification => "notification",
            Self::ModifyRules => "modify_rules",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::NetworkAccess => "Network Access (网络访问)",
            Self::FileSystemRead => "File System Read (文件读取)",
            Self::Notification => "Notification (系统通知)",
            Self::ModifyRules => "Modify Rules (修改规则)",
        }
    }
}

/// Type definition for a plugin setting field in settings schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingFieldType {
    Boolean,
    String,
    Number,
    Select { options: Vec<String> },
    Textarea,
}

impl Serialize for SettingFieldType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "snake_case", tag = "type")]
        enum Helper<'a> {
            Boolean,
            String,
            Number,
            Select { options: &'a [String] },
            Textarea,
        }
        match self {
            Self::Boolean => Helper::Boolean.serialize(serializer),
            Self::String => Helper::String.serialize(serializer),
            Self::Number => Helper::Number.serialize(serializer),
            Self::Select { options } => Helper::Select { options }.serialize(serializer),
            Self::Textarea => Helper::Textarea.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for SettingFieldType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let val = serde_json::Value::deserialize(deserializer)?;
        if let Some(s) = val.as_str() {
            match s.to_ascii_lowercase().as_str() {
                "boolean" | "bool" => Ok(Self::Boolean),
                "string" | "text" => Ok(Self::String),
                "number" | "int" | "float" => Ok(Self::Number),
                "textarea" => Ok(Self::Textarea),
                "select" => Ok(Self::Select {
                    options: Vec::new(),
                }),
                other => Err(serde::de::Error::custom(format!(
                    "Unknown field type: {other}"
                ))),
            }
        } else if let Some(map) = val.as_object() {
            let type_str = map.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match type_str.to_ascii_lowercase().as_str() {
                "boolean" | "bool" => Ok(Self::Boolean),
                "string" | "text" => Ok(Self::String),
                "number" | "int" | "float" => Ok(Self::Number),
                "textarea" => Ok(Self::Textarea),
                _ if map.contains_key("options") => {
                    let opts = map
                        .get("options")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(ToString::to_string))
                                .collect()
                        })
                        .unwrap_or_default();
                    Ok(Self::Select { options: opts })
                }
                other => Err(serde::de::Error::custom(format!(
                    "Unknown field type object: {other}"
                ))),
            }
        } else {
            Err(serde::de::Error::custom("Invalid setting field type value"))
        }
    }
}

impl SettingFieldType {
    pub fn is_valid_value(&self, val: &serde_json::Value) -> bool {
        match self {
            Self::Boolean => val.is_boolean(),
            Self::String | Self::Textarea => val.is_string(),
            Self::Number => val.is_number(),
            Self::Select { options } => {
                options.is_empty() || val.as_str().is_some_and(|s| options.iter().any(|o| o == s))
            }
        }
    }
}

/// A configurable setting field definition for a plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginSettingField {
    pub key: String,
    pub label: String,
    pub field_type: SettingFieldType,
    #[serde(default)]
    pub default_value: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl PluginSettingField {
    pub fn new(
        key: impl Into<String>,
        label: impl Into<String>,
        field_type: SettingFieldType,
        default_value: serde_json::Value,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            field_type,
            default_value,
            description: None,
        }
    }
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
    pub fn validate_value(&self, val: &serde_json::Value) -> bool {
        self.field_type.is_valid_value(val)
    }
    pub fn validate_default(&self) -> bool {
        self.default_value.is_null() || self.validate_value(&self.default_value)
    }
}

/// Plugin manifest schema definition (`plugin.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(default)]
    pub permissions: Vec<PluginPermission>,
    #[serde(default)]
    pub settings_schema: Vec<PluginSettingField>,
}

impl PluginManifest {
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
    pub fn has_permission(&self, permission: PluginPermission) -> bool {
        self.permissions.contains(&permission)
    }
    pub fn get_setting_field(&self, key: &str) -> Option<&PluginSettingField> {
        self.settings_schema.iter().find(|f| f.key == key)
    }

    pub fn validate(&self) -> Result<(), ScriptError> {
        if self.id.trim().is_empty()
            || self.name.trim().is_empty()
            || self.version.trim().is_empty()
        {
            return Err(ScriptError::Runtime(
                "Plugin id, name, and version must not be empty".to_string(),
            ));
        }
        let mut keys = HashSet::new();
        for field in &self.settings_schema {
            if field.key.trim().is_empty() {
                return Err(ScriptError::Runtime(
                    "Setting key cannot be empty".to_string(),
                ));
            }
            if !keys.insert(&field.key) {
                return Err(ScriptError::Runtime(format!(
                    "Duplicate setting key: {}",
                    field.key
                )));
            }
            if !field.validate_default() {
                return Err(ScriptError::Runtime(format!(
                    "Default value for `{}` is invalid",
                    field.key
                )));
            }
        }
        Ok(())
    }
}

// --- Web API Shims Simulator for Script Runtime ---

/// Web Fetch `Headers` API shim for sandboxed script environment.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct HeadersShim {
    entries: Vec<(String, String)>,
}

/// Web `URL` API shim for sandboxed script environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlShim {
    inner: url::Url,
}

/// Web Crypto `crypto.subtle` API shim for cryptographic operations in scripts.
pub struct CryptoSubtleShim;

/// Base64 and URL-safe Base64 encoding/decoding shim.
pub struct Base64Shim;

/// Fetch permission guard for script sandbox.
pub struct FetchPermissionShim;

// --- Country Group Definitions ---

/// Country definition for dynamic policy group generation.
#[derive(Debug, Clone, Copy)]
pub struct CountryGroupDef {
    pub flag: &'static str,
    pub name_zh: &'static str,
    pub code: &'static str,
    pub match_keywords: &'static [&'static str],
}

pub const COUNTRY_GROUP_DEFS: &[CountryGroupDef] = &[
    CountryGroupDef {
        flag: "🇭🇰",
        name_zh: "香港",
        code: "HK",
        match_keywords: &["🇭🇰", "香港", "HK", "Hong Kong", "HongKong", "HKG"],
    },
    CountryGroupDef {
        flag: "🇯🇵",
        name_zh: "日本",
        code: "JP",
        match_keywords: &["🇯🇵", "日本", "JP", "Japan", "Tokyo", "Osaka", "NRT", "HND"],
    },
    CountryGroupDef {
        flag: "🇺🇸",
        name_zh: "美国",
        code: "US",
        match_keywords: &[
            "🇺🇸",
            "美国",
            "美國",
            "US",
            "USA",
            "America",
            "United States",
            "Los Angeles",
            "Silicon Valley",
        ],
    },
    CountryGroupDef {
        flag: "🇸🇬",
        name_zh: "新加坡",
        code: "SG",
        match_keywords: &["🇸🇬", "新加坡", "SG", "Singapore", "狮城", "SIN"],
    },
    CountryGroupDef {
        flag: "🇹🇼",
        name_zh: "台湾",
        code: "TW",
        match_keywords: &["🇹🇼", "台湾", "台灣", "TW", "Taiwan", "Taipei", "TPE"],
    },
    CountryGroupDef {
        flag: "🇰🇷",
        name_zh: "韩国",
        code: "KR",
        match_keywords: &["🇰🇷", "韩国", "韓國", "KR", "Korea", "Seoul", "ICN"],
    },
    CountryGroupDef {
        flag: "🇬🇧",
        name_zh: "英国",
        code: "GB",
        match_keywords: &[
            "🇬🇧",
            "英国",
            "英國",
            "UK",
            "GB",
            "United Kingdom",
            "London",
            "LHR",
        ],
    },
    CountryGroupDef {
        flag: "🇩🇪",
        name_zh: "德国",
        code: "DE",
        match_keywords: &["🇩🇪", "德国", "德國", "DE", "Germany", "Frankfurt", "FRA"],
    },
];

// --- YAML AST Helpers ---

/// Add or update a proxy group in the YAML configuration AST.
pub fn add_proxy_group(
    yaml: &mut Value,
    name: &str,
    group_type: &str,
    proxies: &[String],
    url: Option<&str>,
    interval: Option<u64>,
) -> Result<(), ScriptError> {
    script_engine_yaml::add_proxy_group(yaml, name, group_type, proxies, url, interval)
}

/// Remove rules matching a regular expression or substring from the rules sequence.
pub fn remove_rules(yaml: &mut Value, pattern: &str) -> Result<usize, ScriptError> {
    script_engine_yaml::remove_rules(yaml, pattern)
}

/// Filter proxy nodes matching regex. If `invert` is true, remove matching nodes.
pub fn filter_nodes_by_regex(
    yaml: &mut Value,
    pattern: &str,
    invert: bool,
) -> Result<usize, ScriptError> {
    script_engine_yaml::filter_nodes_by_regex(yaml, pattern, invert)
}

/// Set DNS mode and enable state in YAML AST.
pub fn set_dns_mode(
    yaml: &mut Value,
    enhanced_mode: &str,
    enable: bool,
) -> Result<(), ScriptError> {
    script_engine_yaml::set_dns_mode(yaml, enhanced_mode, enable)
}

/// Dynamically generate country-specific policy groups from the node list.
pub fn generate_country_proxy_groups(
    yaml: &mut Value,
    create_auto_select: bool,
) -> Result<Vec<String>, ScriptError> {
    script_engine_yaml::generate_country_proxy_groups(yaml, create_auto_select)
}

/// Dynamically generate a low-latency auto-test group covering all nodes.
pub fn generate_auto_latency_group(
    yaml: &mut Value,
    group_name: &str,
    url: Option<&str>,
    interval: Option<u64>,
) -> Result<Option<String>, ScriptError> {
    script_engine_yaml::generate_auto_latency_group(yaml, group_name, url, interval)
}

/// Dynamically generate dedicated streaming service policy groups and routing rules.
pub fn generate_streaming_proxy_groups(yaml: &mut Value) -> Result<Vec<String>, ScriptError> {
    script_engine_yaml::generate_streaming_proxy_groups(yaml)
}

/// Inject direct China LAN and GeoIP routing rules into the configuration.
pub fn generate_china_direct_rules(yaml: &mut Value) -> Result<(), ScriptError> {
    script_engine_yaml::generate_china_direct_rules(yaml)
}

/// Rename proxy nodes matching regex pattern using replacement string.
pub fn rename_nodes_by_regex(
    yaml: &mut Value,
    pattern: &str,
    replacement: &str,
) -> Result<usize, ScriptError> {
    script_engine_yaml::rename_nodes_by_regex(yaml, pattern, replacement)
}

/// Remove a proxy group by name.
pub fn remove_proxy_group(yaml: &mut Value, name: &str) -> Result<bool, ScriptError> {
    script_engine_yaml::remove_proxy_group(yaml, name)
}

/// Prepend a routing rule to the rules list.
pub fn prepend_rule(yaml: &mut Value, rule: &str) -> Result<(), ScriptError> {
    script_engine_yaml::prepend_rule(yaml, rule)
}

/// Append a routing rule to the rules list.
pub fn append_rule(yaml: &mut Value, rule: &str) -> Result<(), ScriptError> {
    script_engine_yaml::append_rule(yaml, rule)
}

/// Circuit breaker protecting against repetitive script crashes or timeouts.
#[derive(Debug, Clone)]
pub struct ScriptCircuitBreaker {
    failure_threshold: usize,
    cooldown: Duration,
    consecutive_failures: usize,
    tripped_at: Option<Instant>,
}

impl Default for ScriptCircuitBreaker {
    fn default() -> Self {
        Self::new(3, Duration::from_secs(30))
    }
}

impl ScriptCircuitBreaker {
    pub fn new(failure_threshold: usize, cooldown: Duration) -> Self {
        Self {
            failure_threshold,
            cooldown,
            consecutive_failures: 0,
            tripped_at: None,
        }
    }

    pub fn is_tripped(&self) -> bool {
        if let Some(tripped_at) = self.tripped_at
            && tripped_at.elapsed() < self.cooldown {
                return true;
            }
        false
    }

    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.tripped_at = None;
    }

    pub fn record_failure(&mut self) {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        if self.consecutive_failures >= self.failure_threshold {
            self.tripped_at = Some(Instant::now());
        }
    }

    pub fn reset(&mut self) {
        self.consecutive_failures = 0;
        self.tripped_at = None;
    }

    pub fn trip(&mut self) {
        self.consecutive_failures = self.failure_threshold;
        self.tripped_at = Some(Instant::now());
    }

    pub fn consecutive_failures(&self) -> usize {
        self.consecutive_failures
    }
}

/// Execution context passed into script transforms.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ScriptContext {
    pub stage: HookStage,
    pub profile_name: Option<String>,
    #[serde(default)]
    pub environment: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub permissions: HashSet<PluginPermission>,
    #[serde(default)]
    pub dry_run: bool,
}

impl ScriptContext {
    pub fn new(stage: HookStage) -> Self {
        Self {
            stage,
            profile_name: None,
            environment: std::collections::HashMap::new(),
            permissions: HashSet::new(),
            dry_run: false,
        }
    }

    pub fn with_profile(mut self, profile_name: impl Into<String>) -> Self {
        self.profile_name = Some(profile_name.into());
        self
    }

    pub fn with_permission(mut self, permission: PluginPermission) -> Self {
        self.permissions.insert(permission);
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment.insert(key.into(), value.into());
        self
    }

    pub fn has_permission(&self, permission: PluginPermission) -> bool {
        self.permissions.contains(&permission)
    }
}

/// Script syntax and entrypoint validation result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptValidationResult {
    pub valid: bool,
    pub error: Option<String>,
    pub entry_point_found: bool,
    pub directives_count: usize,
}

// --- Script Engine ---

/// Sandboxed script execution engine with timeout and memory protection.
#[derive(Debug, Clone)]
pub struct ScriptEngine {
    timeout: Duration,
    max_memory_bytes: usize,
}

#[cfg(test)]
#[path = "script_engine_test.rs"]
mod tests;

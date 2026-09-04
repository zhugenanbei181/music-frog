use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use log::warn;
use serde::{Deserialize, Serialize};
use serde_json::json;

use infiltrator_core::{
    doctor::{DoctorFixAction, DoctorReport},
    profiles::ProfileInfo,
    settings::WebDavConfig,
};
use infiltrator_domain::script_engine::{ExtensionPackage, HookStage, PluginManifest, ScriptPreset};

#[derive(Serialize, Deserialize)]
pub struct SwitchProfilePayload {
    pub name: String,
}

#[derive(Serialize, Deserialize)]
pub struct ImportProfilePayload {
    pub name: String,
    pub url: String,
    pub activate: Option<bool>,
}

#[derive(Serialize, Deserialize)]
pub struct SaveProfilePayload {
    pub name: String,
    pub content: String,
    pub activate: Option<bool>,
}

#[derive(Serialize, Deserialize)]
pub struct OpenProfilePayload {
    pub name: String,
}

#[derive(Serialize, Deserialize)]
pub struct SubscriptionConfigPayload {
    pub url: String,
    pub auto_update_enabled: bool,
    pub update_interval_hours: Option<u32>,
}

#[derive(Serialize, Deserialize)]
pub struct EditorConfigPayload {
    pub editor: Option<String>,
}

#[derive(Serialize)]
pub struct EditorConfigResponse {
    pub editor: Option<String>,
}

#[derive(Serialize)]
pub struct CoreVersionsResponse {
    pub current: Option<String>,
    pub versions: Vec<String>,
}

#[derive(Serialize)]
pub struct CoreLatestStableResponse {
    pub version: String,
    pub release_date: String,
}

#[derive(Deserialize)]
pub struct CoreDownloadPayload {
    pub version: String,
}

#[derive(Serialize)]
pub struct CoreDownloadResponse {
    pub version: String,
    pub downloaded: bool,
    pub already_installed: bool,
}

#[derive(Serialize)]
pub struct CoreUpdateStableResponse {
    pub version: String,
    pub downloaded: bool,
    pub already_installed: bool,
    pub rebuild_scheduled: bool,
}

#[derive(Serialize)]
pub struct RebuildStatusResponse {
    pub in_progress: bool,
    pub last_error: Option<String>,
    pub last_reason: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct RuntimeLogsQuery {
    pub level: Option<String>,
}

#[derive(Serialize)]
pub struct RuntimeLogEvent {
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeTrafficSnapshotResponse {
    pub up_rate: u64,
    pub down_rate: u64,
    pub up_total: u64,
    pub down_total: u64,
    pub up_peak: u64,
    pub down_peak: u64,
    pub connections: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeIpCheckResponse {
    pub ip: String,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeProxyDelayNode {
    pub name: String,
    pub proxy_type: String,
    pub delay_ms: Option<u32>,
    pub tested_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeProxyDelayNodesResponse {
    pub nodes: Vec<RuntimeProxyDelayNode>,
    pub default_test_url: String,
    pub default_timeout_ms: u32,
}

#[derive(Debug, Deserialize)]
pub struct RuntimeDelayTestPayload {
    pub proxy: String,
    pub test_url: Option<String>,
    pub timeout_ms: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
pub struct RuntimeDelayBatchPayload {
    pub proxies: Option<Vec<String>>,
    pub test_url: Option<String>,
    pub timeout_ms: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ProxyDelayPayload {
    pub proxy: Option<String>,
    pub proxies: Option<Vec<String>>,
    pub test_url: Option<String>,
    pub timeout_ms: Option<u32>,
    pub all: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeDelayTestResponse {
    pub proxy: String,
    pub delay_ms: u32,
    pub tested_at: String,
    pub test_url: String,
    pub timeout_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeDelayBatchResult {
    pub proxy: String,
    pub delay_ms: Option<u32>,
    pub tested_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeDelayBatchResponse {
    pub results: Vec<RuntimeDelayBatchResult>,
    pub success_count: usize,
    pub failed_count: usize,
    pub test_url: String,
    pub timeout_ms: u32,
}

#[derive(Serialize)]
pub struct CacheFlushResponse {
    pub removed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunConfigPayload {
    pub enable: Option<bool>,
    pub stack: Option<String>,
    pub dns_hijack: Option<Vec<String>>,
    pub auto_route: Option<bool>,
    pub auto_detect_interface: Option<bool>,
    pub mtu: Option<u32>,
    pub strict_route: Option<bool>,
}

#[derive(Serialize)]
pub struct ProfileActionResponse {
    pub profile: ProfileInfo,
    pub rebuild_scheduled: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProfilesUpdateAllResponse {
    pub total: usize,
    pub updated: usize,
    pub failed: usize,
    pub skipped: usize,
}

impl From<crate::scheduler::subscription::SubscriptionUpdateSummary> for ProfilesUpdateAllResponse {
    fn from(s: crate::scheduler::subscription::SubscriptionUpdateSummary) -> Self {
        Self {
            total: s.total,
            updated: s.updated,
            failed: s.failed,
            skipped: s.skipped,
        }
    }
}

#[derive(Deserialize)]
pub struct CoreActivatePayload {
    pub version: String,
}

#[derive(Serialize, Deserialize)]
pub struct AppSettingsPayload {
    pub editor_path: Option<String>,
    pub use_bundled_core: Option<bool>,
    pub language: Option<String>,
    pub theme: Option<String>,
    /// Mirrors `AppSettings.notifications_enabled` (0.20 OS system
    /// notifications); omitted values keep the persisted one untouched.
    pub notifications_enabled: Option<bool>,
    pub webdav: Option<WebDavConfig>,
    /// Mirrors `AppSettings.configs_dir`; blank values are stored as `None`.
    pub configs_dir: Option<String>,
    pub autostart_enabled: Option<bool>,
    pub system_proxy_enabled: Option<bool>,
    pub runtime_running: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
pub struct DoctorRunQuery {
    pub only: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct DoctorFixPayload {
    pub only: Option<String>,
    pub stream: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
pub struct DoctorFixQuery {
    pub only: Option<String>,
    pub stream: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorFixProgressEvent {
    pub stage: String,
    pub task: Option<String>,
    pub summary: Option<String>,
    pub progress_pct: Option<u8>,
    pub actions: Option<Vec<DoctorFixAction>>,
}

#[derive(Serialize)]
pub struct DoctorRunResponse {
    #[serde(flatten)]
    pub report: DoctorReport,
    pub exit_code: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdminCapabilities {
    pub schema_version: u32,
    pub platform: String,
    pub runtime: RuntimeCapabilitySet,
    pub proxy: ProxyCapabilitySet,
    pub settings: SettingsCapabilitySet,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeCapabilitySet {
    pub status: bool,
    pub lifecycle: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProxyCapabilitySet {
    pub list: bool,
    pub mode_switch: bool,
    pub select: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingsCapabilitySet {
    pub autostart: bool,
    pub system_proxy: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeProxyGroupEntry {
    pub name: String,
    pub proxy_type: String,
    pub current: Option<String>,
    pub all: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeProxiesResponse {
    pub mode: String,
    pub groups: Vec<RuntimeProxyGroupEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProxyModePayload {
    pub mode: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProxySelectPayload {
    pub group: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeStatusResponse {
    pub running: bool,
    pub controller: Option<String>,
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyLeakIssue {
    pub id: String,
    pub severity: String,
    pub category: String,
    pub title: String,
    pub detail: String,
    pub affected_target: Option<String>,
    pub process_name: Option<String>,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditTrafficSummary {
    pub upload_total: u64,
    pub download_total: u64,
    pub active_connections: usize,
    pub proxied_bytes: u64,
    pub direct_bytes: u64,
    pub direct_bypass_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditProcessTraffic {
    pub process_name: String,
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub total_bytes: u64,
    pub connections_count: usize,
    pub direct_bypass_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditResponse {
    pub leak_detected: bool,
    pub leaks: Vec<PrivacyLeakIssue>,
    pub traffic_summary: AuditTrafficSummary,
    pub top_processes: Vec<AuditProcessTraffic>,
    pub audited_connections_count: usize,
    pub timestamp: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct WebhookPayload {
    pub action: Option<String>,
    pub intent: Option<String>,
    pub command: Option<String>,
    pub mode: Option<String>,
    pub profile: Option<String>,
    pub proxy: Option<String>,
    pub group: Option<String>,
    pub enabled: Option<bool>,
    pub test_url: Option<String>,
    pub timeout_ms: Option<u32>,
    pub payload: Option<serde_json::Value>,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookResponse {
    pub success: bool,
    pub action: String,
    pub message: Option<String>,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptExecutePayload {
    pub script: String,
    pub yaml_content: String,
    pub stage: Option<HookStage>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptValidatePayload {
    pub script: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptPresetItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub stage: HookStage,
    pub script_code: String,
}

impl From<ScriptPreset> for ScriptPresetItem {
    fn from(p: ScriptPreset) -> Self {
        Self {
            id: p.id.to_string(),
            name: p.name.to_string(),
            description: p.description.to_string(),
            stage: p.stage,
            script_code: p.script_code.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptPresetsResponse {
    pub presets: Vec<ScriptPresetItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionExportPayload {
    pub package: ExtensionPackage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionExportResponse {
    pub json: String,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionImportPayload {
    pub json: String,
    pub expected_checksum: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionImportResponse {
    pub package: ExtensionPackage,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionManifestValidatePayload {
    pub manifest: PluginManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionManifestValidateResponse {
    pub valid: bool,
    pub error: Option<String>,
}

pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::internal(err.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if self.status.is_client_error() || self.status.is_server_error() {
            warn!("admin api error: {}", self.message);
        }
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

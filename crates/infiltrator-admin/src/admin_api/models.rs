use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use log::warn;
use serde::{Deserialize, Serialize};
use serde_json::json;

use infiltrator_core::{ProfileInfo, settings::WebDavConfig};

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

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeDelayTestResponse {
    pub proxy: String,
    pub delay_ms: u32,
    pub tested_at: String,
    pub test_url: String,
    pub timeout_ms: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeDelayBatchResult {
    pub proxy: String,
    pub delay_ms: Option<u32>,
    pub tested_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Deserialize)]
pub struct CoreActivatePayload {
    pub version: String,
}

#[derive(Serialize, Deserialize)]
pub struct AppSettingsPayload {
    pub open_webui_on_startup: Option<bool>,
    pub editor_path: Option<String>,
    pub use_bundled_core: Option<bool>,
    pub language: Option<String>,
    pub theme: Option<String>,
    pub webdav: Option<WebDavConfig>,
    pub autostart_enabled: Option<bool>,
    pub system_proxy_enabled: Option<bool>,
    pub runtime_running: Option<bool>,
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

#[derive(Debug, Clone, Deserialize)]
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

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
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

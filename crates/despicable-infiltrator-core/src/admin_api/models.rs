use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use log::warn;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ProfileInfo;

#[derive(Deserialize)]
pub struct SwitchProfilePayload {
    pub name: String,
}

#[derive(Deserialize)]
pub struct ImportProfilePayload {
    pub name: String,
    pub url: String,
    pub activate: Option<bool>,
}

#[derive(Deserialize)]
pub struct SaveProfilePayload {
    pub name: String,
    pub content: String,
    pub activate: Option<bool>,
}

#[derive(Deserialize)]
pub struct OpenProfilePayload {
    pub name: String,
}

#[derive(Deserialize)]
pub struct SubscriptionConfigPayload {
    pub url: String,
    pub auto_update_enabled: bool,
    pub update_interval_hours: Option<u32>,
}

#[derive(Deserialize)]
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
pub struct RebuildStatusResponse {
    pub in_progress: bool,
    pub last_error: Option<String>,
    pub last_reason: Option<String>,
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

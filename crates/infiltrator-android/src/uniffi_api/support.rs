//! Shared plumbing for the FFI surface: the global tokio runtime, generic
//! string/list normalization helpers, error mapping into [`FfiStatus`], and
//! the mihomo controller client used by every live-query passthrough.

use std::sync::OnceLock;

use mihomo_api::client::MihomoClient;
use mihomo_api::error::MihomoError;
use mihomo_config::manager::ConfigManager;
use mihomo_platform::android_bridge::get_android_bridge;
use tokio::runtime::Runtime;

use super::session::shared_core;
use crate::ffi::{FfiErrorCode, FfiStatus};

pub(super) fn get_runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| Runtime::new().expect("failed to create tokio runtime"))
}

pub(super) fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

pub(super) fn sanitize_list(value: Option<Vec<String>>) -> Option<Vec<String>> {
    value.map(|items| {
        items
            .into_iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect()
    })
}

pub(super) async fn build_controller_client() -> Result<MihomoClient, FfiStatus> {
    // Prefer the shared session: endpoint and secret are re-resolved from the
    // current profile on every call (port rotation and secret aware).
    if let Ok(core) = shared_core() {
        match core.session.client().await {
            Ok(client) => return Ok(client),
            Err(err) => {
                log::debug!("session client unavailable, using legacy resolution: {err}");
            }
        }
    }
    let manager = ConfigManager::new().map_err(map_mihomo_error)?;
    let controller_url = match manager.get_external_controller().await {
        Ok(url) => url,
        Err(err) => {
            if let Some(bridge) = get_android_bridge()
                && let Some(url) = bridge.core_controller_url()
            {
                return MihomoClient::new(&url, None).map_err(map_mihomo_error);
            }
            return Err(map_mihomo_error(err));
        }
    };
    MihomoClient::new(&controller_url, None).map_err(map_mihomo_error)
}

pub(super) fn map_anyhow_error(err: anyhow::Error) -> FfiStatus {
    if let Some(source) = err.downcast_ref::<MihomoError>() {
        return map_mihomo_error_ref(source);
    }
    FfiStatus::err(FfiErrorCode::Unknown, err.to_string())
}

pub(super) fn map_mihomo_error(err: MihomoError) -> FfiStatus {
    map_mihomo_error_ref(&err)
}

fn map_mihomo_error_ref(err: &MihomoError) -> FfiStatus {
    match err {
        MihomoError::Http(_) => FfiStatus::err(FfiErrorCode::Network, err.to_string()),
        MihomoError::Io(_) => FfiStatus::err(FfiErrorCode::Io, err.to_string()),
        MihomoError::Json(_) | MihomoError::Yaml(_) | MihomoError::YamlEmit(_) => {
            FfiStatus::err(FfiErrorCode::InvalidState, err.to_string())
        }
        MihomoError::UrlParse(_) => FfiStatus::err(FfiErrorCode::InvalidInput, err.to_string()),
        MihomoError::WebSocket(_) => FfiStatus::err(FfiErrorCode::Network, err.to_string()),
        MihomoError::Config(_) | MihomoError::Service(_) | MihomoError::Version(_) => {
            FfiStatus::err(FfiErrorCode::InvalidState, err.to_string())
        }
        MihomoError::Proxy(_) | MihomoError::NotFound(_) => {
            FfiStatus::err(FfiErrorCode::NotReady, err.to_string())
        }
    }
}

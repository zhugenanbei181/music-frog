//! Dynamic Scripting and Extension management endpoints (`/admin/api/scripts/*`, `/admin/api/extensions/*`).

use axum::Json;
use std::time::Duration;

use infiltrator_core::script_engine::{
    HookStage, ScriptEngine, ScriptExecutionResult, ScriptValidationResult,
    DEFAULT_SCRIPT_TIMEOUT_MS,
};

use crate::admin_api::models::{
    ApiError, ExtensionExportPayload, ExtensionExportResponse, ExtensionImportPayload,
    ExtensionImportResponse, ExtensionManifestValidatePayload, ExtensionManifestValidateResponse,
    ScriptExecutePayload, ScriptPresetItem, ScriptPresetsResponse, ScriptValidatePayload,
};
use crate::admin_api::state::{AdminApiContext, AdminApiState};

pub async fn list_script_presets_http<C: AdminApiContext>(
    axum::extract::State(_state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<ScriptPresetsResponse>, ApiError> {
    let presets = ScriptEngine::builtin_presets()
        .into_iter()
        .map(ScriptPresetItem::from)
        .collect();
    Ok(Json(ScriptPresetsResponse { presets }))
}

pub async fn execute_script_http<C: AdminApiContext>(
    axum::extract::State(_state): axum::extract::State<AdminApiState<C>>,
    Json(payload): Json<ScriptExecutePayload>,
) -> Result<Json<ScriptExecutionResult>, ApiError> {
    let timeout_ms = payload.timeout_ms.unwrap_or(DEFAULT_SCRIPT_TIMEOUT_MS);
    let stage = payload.stage.unwrap_or(HookStage::PreMerge);
    let engine = ScriptEngine::new().with_timeout(Duration::from_millis(timeout_ms));

    let result = engine
        .execute_transform_detailed(&payload.script, &payload.yaml_content, stage)
        .map_err(|e| ApiError::bad_request(format!("Script execution failed: {e}")))?;

    Ok(Json(result))
}

pub async fn validate_script_http<C: AdminApiContext>(
    axum::extract::State(_state): axum::extract::State<AdminApiState<C>>,
    Json(payload): Json<ScriptValidatePayload>,
) -> Result<Json<ScriptValidationResult>, ApiError> {
    let result = ScriptEngine::validate_script(&payload.script);
    Ok(Json(result))
}

pub async fn export_extension_package_http<C: AdminApiContext>(
    axum::extract::State(_state): axum::extract::State<AdminApiState<C>>,
    Json(payload): Json<ExtensionExportPayload>,
) -> Result<Json<ExtensionExportResponse>, ApiError> {
    let checksum = payload.package.calculate_checksum();
    let json = ScriptEngine::export_extension_package(&payload.package)
        .map_err(|e| ApiError::bad_request(format!("Failed to export extension package: {e}")))?;
    Ok(Json(ExtensionExportResponse { json, checksum }))
}

pub async fn import_extension_package_http<C: AdminApiContext>(
    axum::extract::State(_state): axum::extract::State<AdminApiState<C>>,
    Json(payload): Json<ExtensionImportPayload>,
) -> Result<Json<ExtensionImportResponse>, ApiError> {
    let package = ScriptEngine::import_extension_package(&payload.json)
        .map_err(|e| ApiError::bad_request(format!("Failed to import extension package: {e}")))?;

    let checksum = package.calculate_checksum();
    if let Some(expected) = payload.expected_checksum {
        if !package.verify_checksum(&expected) {
            return Err(ApiError::bad_request(format!(
                "Checksum mismatch: expected `{expected}`, got `{checksum}`"
            )));
        }
    }

    Ok(Json(ExtensionImportResponse { package, checksum }))
}

pub async fn validate_manifest_http<C: AdminApiContext>(
    axum::extract::State(_state): axum::extract::State<AdminApiState<C>>,
    Json(payload): Json<ExtensionManifestValidatePayload>,
) -> Result<Json<ExtensionManifestValidateResponse>, ApiError> {
    match payload.manifest.validate() {
        Ok(()) => Ok(Json(ExtensionManifestValidateResponse {
            valid: true,
            error: None,
        })),
        Err(e) => Ok(Json(ExtensionManifestValidateResponse {
            valid: false,
            error: Some(e.to_string()),
        })),
    }
}

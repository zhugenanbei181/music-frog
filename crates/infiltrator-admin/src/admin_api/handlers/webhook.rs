//! Typed automation endpoint for Alfred / Raycast / Apple Shortcuts
//! (`POST /admin/api/webhook`).

use crate::admin_api::events::{
    AdminEvent, EVENT_DOCTOR_FIX, EVENT_PROFILES_CHANGED, EVENT_RUNTIME_CHANGED,
    EVENT_SETTINGS_CHANGED,
};
use crate::admin_api::handlers::profiles::ensure_valid_profile_name;
use crate::admin_api::handlers::schedule_core_restart;
use crate::admin_api::models::{
    ApiError, ProfilesUpdateAllResponse, WebhookAction, WebhookPayload, WebhookResponse,
};
use crate::admin_api::state::{AdminApiContext, AdminApiState};
use crate::scheduler::subscription::update_all_subscriptions;
use axum::{Json, extract};
use infiltrator_contract::routing_intent::RoutingIntentSnapshot;
use log::info;

pub async fn handle_webhook_http<C: AdminApiContext>(
    extract::State(state): extract::State<AdminApiState<C>>,
    Json(payload): Json<WebhookPayload>,
) -> Result<Json<WebhookResponse>, ApiError> {
    let WebhookPayload { action, profile } = payload;

    match action {
        WebhookAction::SwitchProfile => {
            let profile_candidate = profile
                .as_deref()
                .ok_or_else(|| ApiError::bad_request("Profile name is required"))?;
            let name = ensure_valid_profile_name(profile_candidate)?;
            let application = state
                .ctx
                .profile_application()
                .await
                .map_err(|error| ApiError::internal(error.to_string()))?;
            application
                .select_profile(&name)
                .await
                .map_err(|failure| ApiError::bad_request(failure.message))?;
            schedule_core_restart(&state.ctx, &state.rebuild_status, "webhook-switch-profile");
            state
                .events
                .publish(AdminEvent::new(EVENT_PROFILES_CHANGED));
            Ok(Json(WebhookResponse {
                success: true,
                action: "switch_profile".to_string(),
                message: Some(format!("Switched to profile '{name}'")),
                data: Some(serde_json::json!({ "profile": name })),
            }))
        }
        WebhookAction::StartCore => {
            state
                .ctx
                .rebuild_runtime()
                .await
                .map_err(|error| ApiError::internal(error.to_string()))?;
            state.events.publish(AdminEvent::new(EVENT_RUNTIME_CHANGED));
            Ok(Json(WebhookResponse {
                success: true,
                action: "start_core".to_string(),
                message: Some("Core started".to_string()),
                data: Some(serde_json::json!({ "running": true })),
            }))
        }
        WebhookAction::StopCore => {
            state
                .ctx
                .stop_runtime()
                .await
                .map_err(|error| ApiError::internal(error.to_string()))?;
            state.events.publish(AdminEvent::new(EVENT_RUNTIME_CHANGED));
            Ok(Json(WebhookResponse {
                success: true,
                action: "stop_core".to_string(),
                message: Some("Core stopped".to_string()),
                data: Some(serde_json::json!({ "running": false })),
            }))
        }
        WebhookAction::UpdateAll => {
            let summary = update_all_subscriptions(&state.ctx)
                .await
                .map_err(|error| ApiError::internal(error.to_string()))?;
            state
                .events
                .publish(AdminEvent::new(EVENT_PROFILES_CHANGED));
            Ok(Json(WebhookResponse {
                success: true,
                action: "update_all".to_string(),
                message: Some(format!(
                    "Updated {}/{} subscriptions ({} failed, {} skipped)",
                    summary.updated, summary.total, summary.failed, summary.skipped
                )),
                data: Some(
                    serde_json::to_value(ProfilesUpdateAllResponse::from(summary))
                        .unwrap_or_default(),
                ),
            }))
        }
        WebhookAction::Status => {
            let running = state.ctx.runtime_running().await;
            let controller = state.ctx.runtime_controller_url().await;
            let routing_intent = if running {
                state
                    .ctx
                    .routing_intent_snapshot()
                    .await
                    .ok()
                    .map(|(_, snapshot)| snapshot)
                    .unwrap_or_default()
            } else {
                RoutingIntentSnapshot::default()
            };
            Ok(Json(WebhookResponse {
                success: true,
                action: "status".to_string(),
                message: Some(format!(
                    "Runtime is {}",
                    if running { "running" } else { "stopped" }
                )),
                data: Some(serde_json::json!({
                    "running": running,
                    "controller": controller,
                    "routing_intent": routing_intent,
                })),
            }))
        }
        WebhookAction::DoctorFix => {
            let application = state
                .ctx
                .doctor_application()
                .await
                .map_err(|error| ApiError::internal(error.to_string()))?;
            let report = application
                .fix(None)
                .await
                .map_err(|failure| ApiError::internal(failure.message))?;
            state.events.publish(AdminEvent::new(EVENT_DOCTOR_FIX));
            Ok(Json(WebhookResponse {
                success: true,
                action: "doctor_fix".to_string(),
                message: Some(format!(
                    "Doctor fix executed ({} actions applied)",
                    report.actions.len()
                )),
                data: Some(serde_json::to_value(&report).unwrap_or_default()),
            }))
        }
        WebhookAction::Doctor => {
            let application = state
                .ctx
                .doctor_application()
                .await
                .map_err(|error| ApiError::internal(error.to_string()))?;
            let report = application
                .run(None)
                .await
                .map_err(|failure| ApiError::internal(failure.message))?;
            let exit_code = if report.has_failures() { 1 } else { 0 };
            Ok(Json(WebhookResponse {
                success: exit_code == 0,
                action: "doctor".to_string(),
                message: Some(format!(
                    "Doctor diagnostics completed with exit code {exit_code}"
                )),
                data: Some(serde_json::to_value(&report).unwrap_or_default()),
            }))
        }
        action @ (WebhookAction::EnableSystemProxy | WebhookAction::DisableSystemProxy) => {
            let enabled = matches!(action, WebhookAction::EnableSystemProxy);
            state
                .ctx
                .set_system_proxy_enabled(enabled)
                .await
                .map_err(|error| ApiError::internal(error.to_string()))?;
            state
                .events
                .publish(AdminEvent::new(EVENT_SETTINGS_CHANGED));
            Ok(Json(WebhookResponse {
                success: true,
                action: if enabled {
                    "enable_system_proxy"
                } else {
                    "disable_system_proxy"
                }
                .to_string(),
                message: Some(format!("System proxy set to {enabled}")),
                data: Some(serde_json::json!({ "system_proxy_enabled": enabled })),
            }))
        }
        WebhookAction::Unsupported(raw_action) => {
            info!("rejected unsupported webhook action: '{raw_action}'");
            Err(ApiError::bad_request(format!(
                "Unsupported webhook action: '{raw_action}'"
            )))
        }
    }
}

//! Generic automation endpoint for Alfred / Raycast / Apple Shortcuts
//! (`POST /admin/api/webhook`).

use std::time::Duration;

use axum::Json;
use chrono::Utc;
use infiltrator_core::doctor;
use infiltrator_core::flow_control::BatchDelayTester;
use log::info;
use tokio::sync::watch;

use crate::admin_api::events::{
    AdminEvent, EVENT_DOCTOR_FIX, EVENT_PROFILES_CHANGED, EVENT_PROXY_CHANGED,
    EVENT_RUNTIME_CHANGED, EVENT_SETTINGS_CHANGED,
};
use crate::admin_api::handlers::doctor::detect_doctor_env;
use crate::admin_api::handlers::profiles::ensure_valid_profile_name;
use crate::admin_api::handlers::proxies::{
    DEFAULT_DELAY_TEST_URL, DEFAULT_DELAY_TIMEOUT_MS, collect_delay_test_candidates,
    normalize_proxy_mode, normalize_proxy_mode_candidate,
};
use crate::admin_api::handlers::schedule_core_restart;
use crate::admin_api::models::{
    ApiError, ProfilesUpdateAllResponse, WebhookPayload, WebhookResponse,
};
use crate::admin_api::state::{AdminApiContext, AdminApiState};
use crate::support::app_config_manager;

pub async fn handle_webhook_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    Json(payload): Json<WebhookPayload>,
) -> Result<Json<WebhookResponse>, ApiError> {
    let raw_action = payload
        .action
        .as_deref()
        .or(payload.intent.as_deref())
        .or(payload.command.as_deref())
        .unwrap_or_default()
        .trim();

    let normalized = raw_action.to_ascii_lowercase();

    // 1. Mode switch
    if normalized == "set_mode"
        || normalized == "mode"
        || raw_action == "SetMode"
        || (normalized.is_empty() && payload.mode.is_some())
    {
        let mode_candidate = payload
            .mode
            .as_deref()
            .or_else(|| {
                payload
                    .params
                    .as_ref()
                    .and_then(|p| p.get("mode").and_then(|v| v.as_str()))
            })
            .or_else(|| {
                payload
                    .payload
                    .as_ref()
                    .and_then(|p| p.get("mode").and_then(|v| v.as_str()))
            })
            .unwrap_or(raw_action);

        let mode = normalize_proxy_mode_candidate(mode_candidate)?;
        let client = state
            .ctx
            .runtime_gateway()
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        client
            .patch_config(serde_json::json!({ "mode": mode }))
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        state.events.publish(AdminEvent::new(EVENT_PROXY_CHANGED));
        return Ok(Json(WebhookResponse {
            success: true,
            action: "set_mode".to_string(),
            message: Some(format!("Proxy mode switched to '{mode}'")),
            data: Some(serde_json::json!({ "mode": mode })),
        }));
    }

    // 2. Switch Profile
    if normalized == "switch_profile"
        || normalized == "profile"
        || raw_action == "SwitchProfile"
        || (normalized.is_empty() && payload.profile.is_some())
    {
        let profile_candidate = payload
            .profile
            .as_deref()
            .or_else(|| {
                payload.params.as_ref().and_then(|p| {
                    p.get("name")
                        .or_else(|| p.get("profile"))
                        .and_then(|v| v.as_str())
                })
            })
            .or_else(|| {
                payload.payload.as_ref().and_then(|p| {
                    p.get("name")
                        .or_else(|| p.get("profile"))
                        .and_then(|v| v.as_str())
                })
            })
            .ok_or_else(|| ApiError::bad_request("Profile name is required"))?;

        let name = ensure_valid_profile_name(profile_candidate)?;
        let manager = app_config_manager()
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        manager
            .set_current(&name)
            .await
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        schedule_core_restart(&state.ctx, &state.rebuild_status, "webhook-switch-profile");
        state
            .events
            .publish(AdminEvent::new(EVENT_PROFILES_CHANGED));
        return Ok(Json(WebhookResponse {
            success: true,
            action: "switch_profile".to_string(),
            message: Some(format!("Switched to profile '{name}'")),
            data: Some(serde_json::json!({ "profile": name })),
        }));
    }

    // 3. Start proxy runtime
    if normalized == "start"
        || normalized == "start_proxy"
        || normalized == "start_runtime"
        || raw_action == "StartProxy"
    {
        state
            .ctx
            .rebuild_runtime()
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        state.events.publish(AdminEvent::new(EVENT_RUNTIME_CHANGED));
        return Ok(Json(WebhookResponse {
            success: true,
            action: "start".to_string(),
            message: Some("Proxy runtime started".to_string()),
            data: Some(serde_json::json!({ "running": true })),
        }));
    }

    // 4. Stop proxy runtime
    if normalized == "stop"
        || normalized == "stop_proxy"
        || normalized == "stop_runtime"
        || raw_action == "StopProxy"
    {
        state
            .ctx
            .stop_runtime()
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        state.events.publish(AdminEvent::new(EVENT_RUNTIME_CHANGED));
        return Ok(Json(WebhookResponse {
            success: true,
            action: "stop".to_string(),
            message: Some("Proxy runtime stopped".to_string()),
            data: Some(serde_json::json!({ "running": false })),
        }));
    }

    // 5. Toggle proxy runtime
    if normalized == "toggle"
        || normalized == "toggle_proxy"
        || normalized == "toggle_runtime"
        || raw_action == "ToggleProxy"
    {
        let is_running = state.ctx.runtime_running().await;
        if is_running {
            state
                .ctx
                .stop_runtime()
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
        } else {
            state
                .ctx
                .rebuild_runtime()
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
        }
        state.events.publish(AdminEvent::new(EVENT_RUNTIME_CHANGED));
        let new_state = !is_running;
        return Ok(Json(WebhookResponse {
            success: true,
            action: "toggle".to_string(),
            message: Some(format!(
                "Proxy runtime toggled to {}",
                if new_state { "running" } else { "stopped" }
            )),
            data: Some(serde_json::json!({ "running": new_state })),
        }));
    }

    // 6. Update all subscription profiles
    if normalized == "update_all"
        || normalized == "update_subscriptions"
        || normalized == "update_all_subscriptions"
        || raw_action == "UpdateSubscriptions"
        || raw_action == "UpdateAll"
    {
        let summary = crate::scheduler::subscription::update_all_subscriptions(
            &state.ctx,
            &state.http_client,
            &state.raw_http_client,
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
        state
            .events
            .publish(AdminEvent::new(EVENT_PROFILES_CHANGED));
        return Ok(Json(WebhookResponse {
            success: true,
            action: "update_all".to_string(),
            message: Some(format!(
                "Updated {}/{} subscriptions ({} failed, {} skipped)",
                summary.updated, summary.total, summary.failed, summary.skipped
            )),
            data: Some(
                serde_json::to_value(ProfilesUpdateAllResponse::from(summary)).unwrap_or_default(),
            ),
        }));
    }

    // 7. Delay test
    if normalized == "delay_test"
        || normalized == "delay"
        || raw_action == "DelayTest"
    {
        let test_url = payload
            .test_url
            .as_deref()
            .unwrap_or(DEFAULT_DELAY_TEST_URL);
        let timeout_ms = payload.timeout_ms.unwrap_or(DEFAULT_DELAY_TIMEOUT_MS);
        let client = state
            .ctx
            .runtime_gateway()
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;

        if let Some(ref proxy_name) = payload.proxy {
            let delay = client
                .test_delay(proxy_name, test_url, timeout_ms)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
            return Ok(Json(WebhookResponse {
                success: true,
                action: "delay_test".to_string(),
                message: Some(format!("Delay for '{proxy_name}': {delay}ms")),
                data: Some(serde_json::json!({
                    "proxy": proxy_name,
                    "delay_ms": delay,
                    "tested_at": Utc::now().to_rfc3339()
                })),
            }));
        } else {
            let proxies = client
                .get_proxies()
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
            let mut results = Vec::new();
            let candidates = collect_delay_test_candidates(None, &proxies, &mut results);
            let tester = BatchDelayTester::new(
                30,
                test_url.to_string(),
                Duration::from_millis(timeout_ms as u64),
            );
            let (_cancel_tx, cancel_rx) = watch::channel(false);
            let gateway = client;
            let outcomes = tester
                .test_proxies(
                    candidates,
                    move |proxy, url| {
                        let gateway = gateway.clone();
                        async move {
                            gateway
                                .test_delay(&proxy, &url, timeout_ms)
                                .await
                                .map(|d| d as u64)
                                .map_err(|e| e.to_string())
                        }
                    },
                    cancel_rx,
                )
                .await;
            let success_count = outcomes.iter().filter(|o| o.result.is_ok()).count();
            return Ok(Json(WebhookResponse {
                success: true,
                action: "delay_test".to_string(),
                message: Some(format!(
                    "Tested {} proxies ({} successful)",
                    outcomes.len(),
                    success_count
                )),
                data: Some(serde_json::json!({
                    "total": outcomes.len(),
                    "success": success_count
                })),
            }));
        }
    }

    // 8. Select proxy in a group
    if normalized == "select_proxy" || raw_action == "SelectProxy" {
        let group = payload.group.as_deref().unwrap_or("").trim();
        let name = payload
            .proxy
            .as_deref()
            .or_else(|| {
                payload
                    .params
                    .as_ref()
                    .and_then(|p| p.get("name").and_then(|v| v.as_str()))
            })
            .unwrap_or("")
            .trim();
        if group.is_empty() || name.is_empty() {
            return Err(ApiError::bad_request("group and proxy name are required"));
        }
        let client = state
            .ctx
            .runtime_gateway()
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        client
            .switch_proxy(group, name)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        state.events.publish(AdminEvent::new(EVENT_PROXY_CHANGED));
        return Ok(Json(WebhookResponse {
            success: true,
            action: "select_proxy".to_string(),
            message: Some(format!("Selected proxy '{name}' in group '{group}'")),
            data: Some(serde_json::json!({ "group": group, "name": name })),
        }));
    }

    // 9. Runtime status
    if normalized == "status" || raw_action == "GetStatus" {
        let running = state.ctx.runtime_running().await;
        let controller = state.ctx.runtime_controller_url().await;
        let mode = if running {
            match state.ctx.runtime_gateway().await {
                Ok(client) => client
                    .get_config()
                    .await
                    .ok()
                    .map(|cfg| normalize_proxy_mode(&cfg.mode)),
                Err(_) => None,
            }
        } else {
            None
        };
        return Ok(Json(WebhookResponse {
            success: true,
            action: "status".to_string(),
            message: Some(format!(
                "Runtime is {}",
                if running { "running" } else { "stopped" }
            )),
            data: Some(serde_json::json!({
                "running": running,
                "controller": controller,
                "mode": mode,
            })),
        }));
    }

    // 10. Doctor auto-repair
    if normalized == "doctor_fix" || raw_action == "DoctorFix" {
        let env = detect_doctor_env()?;
        let report = doctor::fix_with(&env, None)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        state.events.publish(AdminEvent::new(EVENT_DOCTOR_FIX));
        return Ok(Json(WebhookResponse {
            success: true,
            action: "doctor_fix".to_string(),
            message: Some(format!(
                "Doctor fix executed ({} actions applied)",
                report.actions.len()
            )),
            data: Some(serde_json::to_value(&report).unwrap_or_default()),
        }));
    }

    // 11. Doctor diagnostics inspection
    if normalized == "doctor" || raw_action == "InspectDiagnostics" {
        let env = detect_doctor_env()?;
        let report = doctor::run_with(&env, None).await;
        let exit_code = doctor::exit_code(&report);
        return Ok(Json(WebhookResponse {
            success: exit_code == 0,
            action: "doctor".to_string(),
            message: Some(format!(
                "Doctor diagnostics completed with exit code {exit_code}"
            )),
            data: Some(serde_json::to_value(&report).unwrap_or_default()),
        }));
    }

    // 12. System proxy settings
    if normalized == "set_system_proxy" || normalized == "system_proxy" {
        let enabled = payload.enabled.unwrap_or(true);
        state
            .ctx
            .set_system_proxy_enabled(enabled)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        state
            .events
            .publish(AdminEvent::new(EVENT_SETTINGS_CHANGED));
        return Ok(Json(WebhookResponse {
            success: true,
            action: "set_system_proxy".to_string(),
            message: Some(format!("System proxy set to {enabled}")),
            data: Some(serde_json::json!({ "system_proxy_enabled": enabled })),
        }));
    }

    if normalized == "toggle_system_proxy" {
        let current = state.ctx.system_proxy_enabled().await;
        let target = !current;
        state
            .ctx
            .set_system_proxy_enabled(target)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        state
            .events
            .publish(AdminEvent::new(EVENT_SETTINGS_CHANGED));
        return Ok(Json(WebhookResponse {
            success: true,
            action: "toggle_system_proxy".to_string(),
            message: Some(format!("System proxy toggled to {target}")),
            data: Some(serde_json::json!({ "system_proxy_enabled": target })),
        }));
    }

    // Fallback: AdminSharedBridge intent resolution
    info!("delegating unknown webhook action to bridge: '{raw_action}'");
    let bridge_req = crate::shared_bridge::BridgeRequest {
        intent: raw_action.to_string(),
        payload: payload.payload.or(payload.params),
    };
    let bridge_resp =
        crate::shared_bridge::AdminSharedBridge::handle_intent(&bridge_req, "en-US");
    if bridge_resp.success {
        Ok(Json(WebhookResponse {
            success: true,
            action: raw_action.to_string(),
            message: Some("Intent executed successfully".to_string()),
            data: bridge_resp.data,
        }))
    } else {
        let err_msg = bridge_resp
            .error
            .map(|e| e.message)
            .unwrap_or_else(|| format!("Unknown webhook action: '{raw_action}'"));
        Err(ApiError::bad_request(err_msg))
    }
}

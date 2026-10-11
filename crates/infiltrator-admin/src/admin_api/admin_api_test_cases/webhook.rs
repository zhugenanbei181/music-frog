//! Behavior cases for the typed webhook contract.
//! test-intent: behavior

use super::*;
use crate::admin_api::models::{WebhookAction, WebhookPayload};
use axum::body::to_bytes;

#[test]
fn webhook_action_deserializes_only_canonical_snake_case_names() {
    let canonical_actions = [
        ("switch_profile", WebhookAction::SwitchProfile),
        ("start_core", WebhookAction::StartCore),
        ("stop_core", WebhookAction::StopCore),
        ("update_all", WebhookAction::UpdateAll),
        ("status", WebhookAction::Status),
        ("doctor_fix", WebhookAction::DoctorFix),
        ("doctor", WebhookAction::Doctor),
        ("enable_system_proxy", WebhookAction::EnableSystemProxy),
        ("disable_system_proxy", WebhookAction::DisableSystemProxy),
    ];

    for (name, expected) in canonical_actions {
        let decoded: WebhookAction = serde_json::from_value(serde_json::json!(name)).unwrap();
        assert_eq!(decoded, expected);
        assert_eq!(decoded.canonical_name(), Some(name));
    }
}

#[test]
fn webhook_action_preserves_unknown_names_as_unsupported_and_requires_action() {
    for unknown in [
        "CheckUpdates",
        "StartProxy",
        "StopProxy",
        "start_proxy",
        "stop_proxy",
        "start_runtime",
        "toggle_proxy",
        "ToggleProxy",
        "delay_test",
        "select_proxy",
        "toggle_system_proxy",
        "set_system_proxy",
    ] {
        let decoded: WebhookAction = serde_json::from_value(serde_json::json!(unknown)).unwrap();
        assert_eq!(decoded, WebhookAction::Unsupported(unknown.to_string()));
        assert_eq!(decoded.canonical_name(), None);
    }

    assert!(serde_json::from_str::<WebhookPayload>(r#"{"profile":"home"}"#).is_err());
    assert!(
        serde_json::from_str::<WebhookPayload>(r#"{"action":"status","intent":"status"}"#).is_err()
    );
    assert!(
        serde_json::from_str::<WebhookPayload>(r#"{"action":"status","proxy":"Proxy-A"}"#).is_err()
    );
}

#[tokio::test]
async fn unknown_webhook_actions_return_bad_request_without_bridge_success() {
    let _guard = TEST_LOCK.lock().await;
    let app = setup_app();

    for action in [
        "CheckUpdates",
        "StartProxy",
        "toggle_proxy",
        "delay_test",
        "select_proxy",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/webhook")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({ "action": action }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{action}");
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("Unsupported webhook action"), "{body}");
        assert!(!body.contains("\"success\":true"), "{body}");
    }
}

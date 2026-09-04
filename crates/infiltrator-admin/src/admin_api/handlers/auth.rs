//! Admin REST API Token authentication and isolation middleware (`verify_admin_token`).

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::admin_api::state::{AdminApiContext, AdminApiState};

/// Verify the admin API token when `state.auth_token` is configured.
///
/// Supports:
/// 1. `Authorization: Bearer <token>`
/// 2. `x-admin-token: <token>`
/// 3. Query string `?token=<token>` or `?auth_token=<token>`
///
/// If `auth_token` in state is `None`, authentication is bypassed (default development mode).
pub async fn verify_admin_token<C: AdminApiContext>(
    State(state): State<AdminApiState<C>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    if let Some(ref expected_token) = state.auth_token {
        let auth_header = req
            .headers()
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok());

        let token_from_header = auth_header.map(|h| {
            if let Some(bearer) = h.strip_prefix("Bearer ") {
                bearer.trim()
            } else if let Some(bearer) = h.strip_prefix("bearer ") {
                bearer.trim()
            } else {
                h.trim()
            }
        });

        let x_token = req
            .headers()
            .get("x-admin-token")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim());

        let query_token = req
            .uri()
            .query()
            .and_then(|q| {
                url::form_urlencoded::parse(q.as_bytes())
                    .find(|(k, _)| k == "token" || k == "auth_token")
                    .map(|(_, v)| v.into_owned())
            });

        let candidate = token_from_header
            .or(x_token)
            .or(query_token.as_deref());

        let valid = match candidate {
            Some(token) => infiltrator_domain::script_engine::CryptoSubtleShim::timing_safe_equal(
                token.as_bytes(),
                expected_token.as_bytes(),
            ),
            None => false,
        };

        if !valid {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Unauthorized: invalid or missing admin token" })),
            )
                .into_response();
        }
    }

    next.run(req).await
}

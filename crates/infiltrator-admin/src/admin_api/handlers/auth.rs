//! Admin REST API Token authentication and isolation middleware (`verify_admin_token`).

use crate::admin_api::state::{AdminApiContext, AdminApiState};
use axum::Json;
use axum::body::Body;
use axum::extract::State;
use axum::http::header::AUTHORIZATION;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use infiltrator_domain::script_engine::CryptoSubtleShim;
use serde_json::json;
use url::form_urlencoded::parse;

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
            .get(AUTHORIZATION)
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

        let query_token = req.uri().query().and_then(|q| {
            parse(q.as_bytes())
                .find(|(k, _)| k == "token" || k == "auth_token")
                .map(|(_, v)| v.into_owned())
        });

        let candidate = token_from_header.or(x_token).or(query_token.as_deref());

        let valid = match candidate {
            Some(token) => {
                CryptoSubtleShim::timing_safe_equal(token.as_bytes(), expected_token.as_bytes())
            }
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

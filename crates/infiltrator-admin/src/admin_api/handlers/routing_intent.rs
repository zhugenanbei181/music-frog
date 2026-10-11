//! Typed routing intent REST ingress over the existing scoped Core owner.

use super::api_failure;
use crate::admin_api::events::{AdminEvent, EVENT_ROUTING_INTENT_CHANGED};
use crate::admin_api::models::{ApiError, RoutingIntentPayload, RoutingIntentStateResponse};
use crate::admin_api::state::{AdminApiContext, AdminApiState};
use axum::{Json, extract};

pub async fn get_routing_intent_http<C: AdminApiContext>(
    extract::State(state): extract::State<AdminApiState<C>>,
) -> Result<Json<RoutingIntentStateResponse>, ApiError> {
    let (scope, snapshot) = state
        .ctx
        .routing_intent_snapshot()
        .await
        .map_err(api_failure)?;
    Ok(Json(RoutingIntentStateResponse { scope, snapshot }))
}

pub async fn apply_routing_intent_http<C: AdminApiContext>(
    extract::State(state): extract::State<AdminApiState<C>>,
    Json(payload): Json<RoutingIntentPayload>,
) -> Result<Json<RoutingIntentStateResponse>, ApiError> {
    let (scope, snapshot) = state
        .ctx
        .apply_routing_intent(payload.scope, payload.intent)
        .await
        .map_err(api_failure)?;
    state
        .events
        .publish(AdminEvent::new(EVENT_ROUTING_INTENT_CHANGED));
    Ok(Json(RoutingIntentStateResponse { scope, snapshot }))
}

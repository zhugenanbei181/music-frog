//! Typed outbound inventory, selection, and probe endpoints.

use super::api_failure;
use crate::admin_api::events::{AdminEvent, EVENT_OUTBOUND_CHANGED};
use crate::admin_api::models::ApiError;
use crate::admin_api::state::{AdminApiContext, AdminApiState};
use axum::{Json, extract};
use infiltrator_contract::outbound::inventory::{
    OutboundInventorySnapshot, OutboundObservationSnapshot, OutboundProbeSnapshot,
};
use infiltrator_contract::outbound::probe::{
    OutboundProbeBatchSnapshot, OutboundSelectionRequest, ProbeOutboundBatchRequest,
    ProbeOutboundRequest,
};

/// Read one coherent typed inventory/details observation. Its inventory
/// revision is the only revision accepted by the selection and probe actions.
pub async fn get_outbounds_http<C: AdminApiContext>(
    extract::State(state): extract::State<AdminApiState<C>>,
) -> Result<Json<OutboundObservationSnapshot>, ApiError> {
    let owner = state
        .ctx
        .outbound_owner()
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let (_, observation) = owner.observe().await.map_err(api_failure)?;
    Ok(Json(observation))
}

/// Select using the exact typed IDs and inventory revision supplied by the
/// caller. The returned snapshot is the owner's write-after-readback result.
pub async fn select_outbound_http<C: AdminApiContext>(
    extract::State(state): extract::State<AdminApiState<C>>,
    Json(request): Json<OutboundSelectionRequest>,
) -> Result<Json<OutboundInventorySnapshot>, ApiError> {
    let owner = state
        .ctx
        .outbound_owner()
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let (scope, _) = owner.inventory().await.map_err(api_failure)?;
    let snapshot = owner.select(&scope, &request).await.map_err(api_failure)?;
    state
        .events
        .publish(AdminEvent::new(EVENT_OUTBOUND_CHANGED));
    Ok(Json(snapshot))
}

/// Probe one typed outbound from the caller's accepted inventory revision.
pub async fn probe_outbound_http<C: AdminApiContext>(
    extract::State(state): extract::State<AdminApiState<C>>,
    Json(request): Json<ProbeOutboundRequest>,
) -> Result<Json<OutboundProbeSnapshot>, ApiError> {
    let owner = state
        .ctx
        .outbound_owner()
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let (scope, _) = owner.inventory().await.map_err(api_failure)?;
    let snapshot = owner.probe(&scope, &request).await.map_err(api_failure)?;
    Ok(Json(snapshot))
}

/// Probe a typed set of outbounds. The shared contract validates target shape,
/// URL, timeout, and concurrency before the owner performs any probe.
pub async fn probe_outbounds_batch_http<C: AdminApiContext>(
    extract::State(state): extract::State<AdminApiState<C>>,
    Json(request): Json<ProbeOutboundBatchRequest>,
) -> Result<Json<OutboundProbeBatchSnapshot>, ApiError> {
    let owner = state
        .ctx
        .outbound_owner()
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let (scope, _) = owner.inventory().await.map_err(api_failure)?;
    let snapshot = owner
        .probe_batch(&scope, &request)
        .await
        .map_err(api_failure)?;
    Ok(Json(snapshot))
}

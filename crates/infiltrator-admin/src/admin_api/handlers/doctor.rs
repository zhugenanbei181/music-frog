//! Doctor self-diagnostics and one-shot bootstrap endpoints
//! (`/admin/api/doctor*`, `/admin/api/bootstrap`).

use std::convert::Infallible;
use std::time::Duration;

use axum::{
    Json,
    http::{HeaderMap, StatusCode, header},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use infiltrator_contract::doctor::{BootstrapReport, DoctorCheckMeta, DoctorFixAction};
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::admin_api::events::{AdminEvent, EVENT_DOCTOR_FIX};
use crate::admin_api::models::{
    ApiError, DoctorFixPayload, DoctorFixProgressEvent, DoctorFixQuery, DoctorRunQuery,
    DoctorRunResponse,
};
use crate::admin_api::state::{AdminApiContext, AdminApiState};

pub async fn run_doctor_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    axum::extract::Query(query): axum::extract::Query<DoctorRunQuery>,
) -> Result<Json<DoctorRunResponse>, ApiError> {
    let application = state
        .ctx
        .doctor_application()
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let report = application
        .run(query.only)
        .await
        .map_err(|failure| ApiError::internal(failure.message))?;
    let exit_code = if report.has_failures() { 1 } else { 0 };
    Ok(Json(DoctorRunResponse { report, exit_code }))
}

pub async fn list_doctor_checks_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<Vec<DoctorCheckMeta>>, ApiError> {
    let application = state
        .ctx
        .doctor_application()
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(Json(application.list_checks()))
}

pub async fn explain_doctor_check_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    axum::extract::Path(check_id): axum::extract::Path<String>,
) -> Result<Json<DoctorCheckMeta>, ApiError> {
    let application = state
        .ctx
        .doctor_application()
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;
    match application.explain(&check_id) {
        Ok(meta) => Ok(Json(meta)),
        Err(failure) => Err(ApiError::not_found(failure.message)),
    }
}

pub async fn fix_doctor_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    headers: HeaderMap,
    axum::extract::Query(query): axum::extract::Query<DoctorFixQuery>,
    payload: Option<Json<DoctorFixPayload>>,
) -> Result<Response, ApiError> {
    let is_stream = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.contains("text/event-stream"))
        .unwrap_or(false)
        || query.stream.unwrap_or(false)
        || payload.as_ref().and_then(|p| p.stream).unwrap_or(false);

    let only = payload
        .and_then(|Json(p)| p.only)
        .or(query.only);

    let application = state
        .ctx
        .doctor_application()
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;

    if !is_stream {
        let report = application
            .fix(only.clone())
            .await
            .map_err(|failure| ApiError::internal(failure.message))?;
        let detail = if report.actions.is_empty() {
            "nothing to repair".to_string()
        } else {
            report
                .actions
                .iter()
                .map(|action| action.id.as_str())
                .collect::<Vec<_>>()
                .join(",")
        };
        state
            .events
            .publish(AdminEvent::new(EVENT_DOCTOR_FIX).with_detail(detail));
        return Ok((StatusCode::OK, Json(report)).into_response());
    }

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Result<Event, Infallible>>();
    let state_events = state.events.clone();
    let application_for_stream = application.clone();

    tokio::spawn(async move {
        let repair_tasks: &[(&str, &str, u8)] = &[
            ("config.configs_dir", "config", 25),
            ("config.current_yaml", "config", 50),
            ("controller.external_controller", "controller", 75),
            ("service.stale_pid", "service", 90),
        ];

        let _ = tx.send(Ok(Event::default().event("start").data(
            serde_json::to_string(&DoctorFixProgressEvent {
                stage: "start".to_string(),
                task: None,
                summary: Some("Starting automated doctor repair tasks...".to_string()),
                progress_pct: Some(0),
                actions: None,
            })
            .unwrap_or_default(),
        )));

        let mut all_actions: Vec<DoctorFixAction> = Vec::new();

        for &(task_id, category, progress_pct) in repair_tasks {
            let filter_matches = match only.as_deref() {
                Some(filter) => {
                    let tokens: Vec<&str> = filter.split(',').map(str::trim).collect();
                    tokens.iter().any(|t| *t == category || task_id.starts_with(t))
                }
                None => true,
            };

            if !filter_matches {
                continue;
            }

            let _ = tx.send(Ok(Event::default().event("progress").data(
                serde_json::to_string(&DoctorFixProgressEvent {
                    stage: "checking".to_string(),
                    task: Some(task_id.to_string()),
                    summary: Some(format!("Checking and repairing {task_id}...")),
                    progress_pct: Some(progress_pct),
                    actions: None,
                })
                .unwrap_or_default(),
            )));

            match application_for_stream
                .fix(Some(task_id.to_string()))
                .await
            {
                Ok(report) => {
                    for action in report.actions {
                        let _ = tx.send(Ok(Event::default().event("action").data(
                            serde_json::to_string(&DoctorFixProgressEvent {
                                stage: "action".to_string(),
                                task: Some(action.id.clone()),
                                summary: Some(action.summary.clone()),
                                progress_pct: Some(progress_pct),
                                actions: None,
                            })
                            .unwrap_or_default(),
                        )));
                        all_actions.push(action);
                    }
                }
                Err(err) => {
                    let _ = tx.send(Ok(Event::default().event("error").data(
                        serde_json::to_string(&DoctorFixProgressEvent {
                            stage: "error".to_string(),
                            task: Some(task_id.to_string()),
                            summary: Some(format!("Failed to repair {task_id}: {}", err.message)),
                            progress_pct: Some(progress_pct),
                            actions: None,
                        })
                        .unwrap_or_default(),
                    )));
                }
            }
        }

        let detail = if all_actions.is_empty() {
            "nothing to repair".to_string()
        } else {
            all_actions
                .iter()
                .map(|action| action.id.as_str())
                .collect::<Vec<_>>()
                .join(",")
        };
        state_events.publish(AdminEvent::new(EVENT_DOCTOR_FIX).with_detail(detail));

        let _ = tx.send(Ok(Event::default().event("complete").data(
            serde_json::to_string(&DoctorFixProgressEvent {
                stage: "complete".to_string(),
                task: None,
                summary: Some(format!(
                    "Doctor repair completed with {} actions applied",
                    all_actions.len()
                )),
                progress_pct: Some(100),
                actions: Some(all_actions),
            })
            .unwrap_or_default(),
        )));
    });

    let stream = UnboundedReceiverStream::new(rx);
    Ok(Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keepalive"),
        )
        .into_response())
}

pub async fn run_bootstrap_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<BootstrapReport>, ApiError> {
    let application = state
        .ctx
        .doctor_application()
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let report = application
        .bootstrap()
        .await
        .map_err(|failure| ApiError::internal(failure.message))?;
    Ok(Json(report))
}

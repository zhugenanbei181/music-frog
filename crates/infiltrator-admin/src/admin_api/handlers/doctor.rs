//! Doctor self-diagnostics and one-shot bootstrap endpoints
//! (`/admin/api/doctor*`, `/admin/api/bootstrap`).

use axum::Json;
use infiltrator_core::bootstrap::{self, BootstrapReport};
use infiltrator_core::doctor::{self, DoctorCheckMeta, DoctorEnv, DoctorFixReport};
use mihomo_platform::paths::get_home_dir;

use crate::admin_api::events::{AdminEvent, EVENT_DOCTOR_FIX};
use crate::admin_api::models::{ApiError, DoctorFixPayload, DoctorRunQuery, DoctorRunResponse};
use crate::admin_api::state::{AdminApiContext, AdminApiState};

/// The settings check must inspect the exact file the `/admin/api/settings`
/// handlers read and write: both sides derive it through
/// `settings::settings_path(get_home_dir())`, never through a second
/// derivation.
fn detect_doctor_env() -> Result<DoctorEnv, ApiError> {
    let home = get_home_dir().map_err(|e| ApiError::internal(e.to_string()))?;
    let settings_file = infiltrator_core::settings::settings_path(&home)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(DoctorEnv::with_home(home).with_settings_file(settings_file))
}

pub async fn run_doctor_http<C: AdminApiContext>(
    axum::extract::State(_state): axum::extract::State<AdminApiState<C>>,
    axum::extract::Query(query): axum::extract::Query<DoctorRunQuery>,
) -> Result<Json<DoctorRunResponse>, ApiError> {
    let env = detect_doctor_env()?;
    let report = doctor::run_with(&env, query.only.as_deref()).await;
    let exit_code = doctor::exit_code(&report);
    Ok(Json(DoctorRunResponse { report, exit_code }))
}

pub async fn list_doctor_checks_http<C: AdminApiContext>(
    axum::extract::State(_state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<Vec<DoctorCheckMeta>>, ApiError> {
    Ok(Json(doctor::list_checks().to_vec()))
}

pub async fn explain_doctor_check_http<C: AdminApiContext>(
    axum::extract::State(_state): axum::extract::State<AdminApiState<C>>,
    axum::extract::Path(check_id): axum::extract::Path<String>,
) -> Result<Json<DoctorCheckMeta>, ApiError> {
    match doctor::explain_check(&check_id) {
        Ok(meta) => Ok(Json(*meta)),
        Err(err) => Err(ApiError::not_found(err.to_string())),
    }
}

pub async fn fix_doctor_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    payload: Option<Json<DoctorFixPayload>>,
) -> Result<Json<DoctorFixReport>, ApiError> {
    let only = payload.and_then(|Json(payload)| payload.only);
    let env = detect_doctor_env()?;
    let report = doctor::fix_with(&env, only.as_deref()).await?;
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
    Ok(Json(report))
}

pub async fn run_bootstrap_http<C: AdminApiContext>(
    axum::extract::State(_state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<BootstrapReport>, ApiError> {
    let home = get_home_dir().map_err(|e| ApiError::internal(e.to_string()))?;
    let report = bootstrap::ensure_bootstrap_at(&home).await?;
    Ok(Json(report))
}

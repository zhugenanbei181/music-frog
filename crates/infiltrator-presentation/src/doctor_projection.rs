//! Pure diagnostic page projection and localized copy over accepted observations.
use chrono::{DateTime, Utc};
use infiltrator_contract::doctor::{
    DoctorCheckKind, DoctorCheckMeta, DoctorObservation, DoctorReport, DoctorStatus,
};
use infiltrator_contract::snapshot::{CoreWatchdogSnapshot, CoreWatchdogState};
use infiltrator_contract::surface_snapshot::{
    DoctorCheckSnapshot, DoctorPageSnapshot, PageData, PageStatus,
};
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

pub const fn status_key(status: DoctorStatus) -> &'static str {
    match status {
        DoctorStatus::Pass => "doctor_status_pass",
        DoctorStatus::Warn => "doctor_status_warn",
        DoctorStatus::Fail => "doctor_status_fail",
        DoctorStatus::Skip => "doctor_status_skip",
    }
}
pub fn status_counts(statuses: impl IntoIterator<Item = DoctorStatus>) -> [usize; 4] {
    let mut counts = [0; 4];
    for status in statuses {
        counts[match status {
            DoctorStatus::Pass => 0,
            DoctorStatus::Warn => 1,
            DoctorStatus::Fail => 2,
            DoctorStatus::Skip => 3,
        }] += 1;
    }
    counts
}
pub fn summary(
    statuses: impl IntoIterator<Item = DoctorStatus>,
    finished_at: Option<u64>,
    code: &str,
) -> String {
    let counts = status_counts(statuses);
    if counts.iter().sum::<usize>() == 0 {
        return Lang(code)
            .tr(if finished_at.is_some() {
                "doctor_no_checks"
            } else {
                "doctor_not_observed"
            })
            .into_owned();
    }
    localize(
        code,
        "doctor_observed_summary",
        &[
            ("pass", counts[0].to_string()),
            ("warn", counts[1].to_string()),
            ("fail", counts[2].to_string()),
            ("skip", counts[3].to_string()),
        ],
    )
}
pub fn watchdog_status_text(snapshot: &CoreWatchdogSnapshot, code: &str) -> String {
    let (key, values) = match snapshot.state {
        CoreWatchdogState::Idle => ("watchdog_idle", vec![]),
        CoreWatchdogState::Waiting {
            attempt,
            retry_in_ms,
        } => (
            "watchdog_waiting",
            vec![
                ("attempt", attempt.to_string()),
                ("delay", retry_in_ms.to_string()),
            ],
        ),
        CoreWatchdogState::Restarting { attempt } => (
            "watchdog_restarting",
            vec![("attempt", attempt.to_string())],
        ),
        CoreWatchdogState::Recovered { attempts } => (
            "watchdog_recovered",
            vec![("attempts", attempts.to_string())],
        ),
        CoreWatchdogState::Tripped { attempts } => {
            ("watchdog_tripped", vec![("attempts", attempts.to_string())])
        }
    };
    localize(code, key, &values)
}
pub const fn watchdog_badge(state: &CoreWatchdogState) -> (&'static str, DoctorStatus) {
    match state {
        CoreWatchdogState::Idle => ("watchdog_badge_idle", DoctorStatus::Skip),
        CoreWatchdogState::Waiting { .. } => ("watchdog_badge_waiting", DoctorStatus::Warn),
        CoreWatchdogState::Restarting { .. } => ("watchdog_badge_restarting", DoctorStatus::Warn),
        CoreWatchdogState::Recovered { .. } => ("watchdog_badge_recovered", DoctorStatus::Pass),
        CoreWatchdogState::Tripped { .. } => ("watchdog_badge_tripped", DoctorStatus::Fail),
    }
}

pub fn project_observation(
    observation: &DoctorObservation,
    metadata: &[DoctorCheckMeta],
) -> PageData<DoctorPageSnapshot> {
    let (status, report) = match observation {
        DoctorObservation::Unobserved => (PageStatus::Empty, None),
        DoctorObservation::Loading => (PageStatus::Loading, None),
        DoctorObservation::LoadingWithReport(report) => (PageStatus::Loading, Some(report)),
        DoctorObservation::Failed(failure) => (
            PageStatus::Failed {
                failure: failure.clone(),
            },
            None,
        ),
        DoctorObservation::FailedWithReport { report, failure } => (
            PageStatus::Failed {
                failure: failure.clone(),
            },
            Some(report),
        ),
        DoctorObservation::Ready(report) => (PageStatus::Ready, Some(report)),
    };
    PageData {
        status,
        data: Some(project_ready_report(report, metadata)),
    }
}

fn project_ready_report(
    report: Option<&DoctorReport>,
    metadata: &[DoctorCheckMeta],
) -> DoctorPageSnapshot {
    let Some(report) = report else {
        return DoctorPageSnapshot {
            overall_healthy: false,
            last_run: String::new(),
            checks: Vec::new(),
            report_started_at: None,
            report_finished_at: None,
        };
    };
    DoctorPageSnapshot {
        overall_healthy: !report.checks.is_empty()
            && report
                .checks
                .iter()
                .all(|check| check.status == DoctorStatus::Pass),
        last_run: i64::try_from(report.finished_at)
            .ok()
            .and_then(|seconds| DateTime::<Utc>::from_timestamp(seconds, 0))
            .map_or_else(|| report.finished_at.to_string(), |time| time.to_rfc3339()),
        checks: report
            .checks
            .iter()
            .map(|check| DoctorCheckSnapshot {
                kind: None,
                detail_copy_key: None,
                id: check.id.clone(),
                name: check.summary.clone(),
                category: check.category.clone(),
                state: check.status,
                detail: check.detail.clone().unwrap_or_default(),
                hint: check.hint.clone(),
                fix_available: check.status != DoctorStatus::Pass
                    && check.status != DoctorStatus::Skip
                    && metadata
                        .iter()
                        .any(|meta| meta.id == check.id && meta.fixable),
            })
            .collect(),
        report_started_at: Some(report.started_at),
        report_finished_at: Some(report.finished_at),
    }
}

/// Built-in descriptors localize by typed identity; externally observed text stays opaque.
pub fn check_name(kind: Option<DoctorCheckKind>, name: &str, category: &str, code: &str) -> String {
    let (name, category) = kind.map_or_else(
        || (name.to_owned(), category.to_owned()),
        |kind| {
            let keys = kind.copy_keys();
            let lang = Lang(code);
            (lang.tr(keys.0).into_owned(), lang.tr(keys.1).into_owned())
        },
    );
    if category.is_empty() {
        name
    } else {
        format!("[{category}] {name}")
    }
}
pub fn check_detail(key: Option<&str>, observed: &str, code: &str) -> String {
    key.map_or_else(
        || observed.to_owned(),
        |key| Lang(code).tr(key).into_owned(),
    )
}

#[cfg(test)]
#[path = "doctor_projection_tests.rs"]
mod tests;

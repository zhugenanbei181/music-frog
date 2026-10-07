//! One fold from real diagnostic reports to peer-product rows and status semantics.
use chrono::{DateTime, Utc};
use infiltrator_contract::doctor::{DoctorCheckKind, DoctorCheckMeta, DoctorReport, DoctorStatus};
use infiltrator_contract::snapshot::{CoreWatchdogSnapshot, CoreWatchdogState};
use infiltrator_contract::surface_snapshot::{DoctorCheckSnapshot, DoctorPageSnapshot};
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

pub fn project_report(
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
mod tests {
    use super::*;
    use infiltrator_contract::doctor::DoctorCheckResult;
    #[test]
    fn skipped_and_unobserved_are_never_healthy_and_external_copy_remains_opaque() {
        assert_eq!(
            status_counts([
                DoctorStatus::Pass,
                DoctorStatus::Warn,
                DoctorStatus::Fail,
                DoctorStatus::Skip
            ]),
            [1, 1, 1, 1]
        );
        assert_eq!(summary([], None, "en-US"), "Diagnostics have not run yet");
        assert_eq!(
            watchdog_badge(&CoreWatchdogState::Idle).1,
            DoctorStatus::Skip
        );
        assert_eq!(
            watchdog_badge(&CoreWatchdogState::Recovered { attempts: 1 }).1,
            DoctorStatus::Pass
        );
        let report = DoctorReport {
            started_at: 100,
            finished_at: 200,
            checks: vec![DoctorCheckResult {
                id: "probe".into(),
                category: "network".into(),
                status: DoctorStatus::Skip,
                summary: "reported {count}".into(),
                detail: Some("{pass}".into()),
                hint: Some("install permission".into()),
            }],
        };
        let page = project_report(Some(&report), &[]);
        assert!(!page.overall_healthy);
        assert_eq!(page.report_started_at, Some(100));
        assert_eq!(page.report_finished_at, Some(200));
        assert_eq!(page.checks[0].state, DoctorStatus::Skip);
        assert!(!page.checks[0].fix_available);
        assert_eq!(
            check_name(
                None,
                &page.checks[0].name,
                &page.checks[0].category,
                "en-US"
            ),
            "[network] reported {count}"
        );
        assert_eq!(
            check_detail(None, &page.checks[0].detail, "en-US"),
            "{pass}"
        );
        assert_eq!(page.checks[0].hint.as_deref(), Some("install permission"));
        assert!(serde_json::from_str::<DoctorStatus>("\"unexpected\"").is_err());
        assert_eq!(
            serde_json::from_str::<DoctorStatus>("\"skip\"").unwrap(),
            DoctorStatus::Skip
        );
        assert_eq!(
            serde_json::from_str::<DoctorStatus>("\"warning\"").unwrap(),
            DoctorStatus::Warn
        );
    }
    #[test]
    fn completed_empty_report_preserves_observation_and_never_claims_healthy_or_not_run() {
        let report = DoctorReport {
            started_at: 10,
            finished_at: 20,
            checks: Vec::new(),
        };
        let page = project_report(Some(&report), &[]);
        assert!(!page.overall_healthy);
        assert_eq!(page.report_finished_at, Some(20));
        assert_eq!(
            summary([], page.report_finished_at, "en-US"),
            "Diagnostics completed without check results"
        );
        assert_eq!(summary([], None, "en-US"), "Diagnostics have not run yet");
        assert!(!page.last_run.is_empty());
    }
}

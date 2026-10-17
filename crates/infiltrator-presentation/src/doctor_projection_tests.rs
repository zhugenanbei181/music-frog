//! Behavior tests for Doctor presentation state and report projection.
use super::*;
use infiltrator_contract::doctor::DoctorCheckResult;
use lurker_core_contract::error::{ErrorCode, Failure};
use std::slice::from_ref;

fn report() -> DoctorReport {
    DoctorReport {
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
    }
}

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
    let page = project_observation(&DoctorObservation::Ready(report()), &[]);
    assert_eq!(page.status, PageStatus::Ready);
    let page = page.data.unwrap();
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
    let page = project_observation(&DoctorObservation::Ready(report), &[]);
    assert_eq!(page.status, PageStatus::Ready);
    let page = page.data.unwrap();
    assert!(!page.overall_healthy);
    assert_eq!(page.report_finished_at, Some(20));
    assert_eq!(
        summary([], page.report_finished_at, "en-US"),
        "Diagnostics completed without check results"
    );
    assert_eq!(summary([], None, "en-US"), "Diagnostics have not run yet");
    assert!(!page.last_run.is_empty());
}

#[test]
fn fix_availability_requires_a_matching_fixable_diagnostic_descriptor() {
    let mut report = report();
    report.checks[0].status = DoctorStatus::Fail;
    let metadata = DoctorCheckMeta {
        id: "probe".into(),
        category: "network".into(),
        summary: "probe check".into(),
        why: "probe network state".into(),
        fail_means: "probe failed".into(),
        hint: "retry".into(),
        fixable: true,
        default_enabled: true,
    };
    let available = project_observation(
        &DoctorObservation::Ready(report.clone()),
        from_ref(&metadata),
    );
    assert!(available.data.unwrap().checks[0].fix_available);

    let mut read_only = metadata.clone();
    read_only.fixable = false;
    let unavailable = project_observation(&DoctorObservation::Ready(report.clone()), &[read_only]);
    assert!(!unavailable.data.unwrap().checks[0].fix_available);

    let mut foreign = metadata;
    foreign.id = "another-check".into();
    let unavailable = project_observation(&DoctorObservation::Ready(report), &[foreign]);
    assert!(!unavailable.data.unwrap().checks[0].fix_available);
}

#[test]
fn page_projection_preserves_explicit_empty_loading_failure_and_stale_report_states() {
    let failure = Failure::new(ErrorCode::Permission, "doctor access denied", false);
    let unobserved = project_observation(&DoctorObservation::Unobserved, &[]);
    assert_eq!(unobserved.status, PageStatus::Empty);
    assert!(unobserved.data.unwrap().checks.is_empty());

    let loading = project_observation(&DoctorObservation::Loading, &[]);
    assert_eq!(loading.status, PageStatus::Loading);
    assert!(loading.data.unwrap().report_finished_at.is_none());

    let previous = report();
    let loading_previous =
        project_observation(&DoctorObservation::LoadingWithReport(previous.clone()), &[]);
    assert_eq!(loading_previous.status, PageStatus::Loading);
    assert_eq!(
        loading_previous.data.unwrap().report_finished_at,
        Some(previous.finished_at)
    );

    let failed = project_observation(&DoctorObservation::Failed(failure.clone()), &[]);
    assert_eq!(
        failed.status,
        PageStatus::Failed {
            failure: failure.clone()
        }
    );
    assert!(failed.data.unwrap().report_finished_at.is_none());

    let failed_previous = project_observation(
        &DoctorObservation::FailedWithReport {
            report: previous.clone(),
            failure: failure.clone(),
        },
        &[],
    );
    assert_eq!(failed_previous.status, PageStatus::Failed { failure });
    assert_eq!(
        failed_previous.data.unwrap().report_finished_at,
        Some(previous.finished_at)
    );
}

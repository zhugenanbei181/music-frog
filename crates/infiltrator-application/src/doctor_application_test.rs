//! test-intent: behavior
use super::*;
use crate::command_application::CommandApplication;
use async_trait::async_trait;
use futures_util::{future, poll};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::doctor::{DoctorCheckResult, DoctorStatus};
use infiltrator_ports::error::PortError;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Default)]
struct DiagnosticPort {
    fixed: AtomicBool,
    fail: AtomicBool,
    blocked: AtomicBool,
    runs: AtomicUsize,
    fixes: AtomicUsize,
}
#[async_trait]
impl DoctorPort for DiagnosticPort {
    async fn run(&self, _: Option<String>) -> Result<DoctorReport, PortError> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        if self.blocked.load(Ordering::SeqCst) {
            future::pending::<()>().await
        }
        if self.fail.load(Ordering::SeqCst) {
            return Err(PortError::PermissionDenied("diagnostic read denied".into()));
        }
        Ok(DoctorReport {
            started_at: 10,
            finished_at: 20,
            checks: vec![DoctorCheckResult {
                id: "config.check".into(),
                category: "configuration".into(),
                status: if self.fixed.load(Ordering::SeqCst) {
                    DoctorStatus::Pass
                } else {
                    DoctorStatus::Fail
                },
                summary: "configuration integrity".into(),
                detail: Some("observed finding {count}".into()),
                hint: Some("repair it".into()),
            }],
        })
    }
    async fn fix(&self, _: Option<String>) -> Result<DoctorFixReport, PortError> {
        self.fixes.fetch_add(1, Ordering::SeqCst);
        self.fixed.store(true, Ordering::SeqCst);
        Ok(DoctorFixReport::default())
    }
    fn list_checks(&self) -> Vec<DoctorCheckMeta> {
        vec![DoctorCheckMeta {
            id: "config.check".into(),
            category: "configuration".into(),
            summary: String::new(),
            why: String::new(),
            fail_means: String::new(),
            hint: String::new(),
            fixable: true,
            default_enabled: true,
        }]
    }
    fn explain(&self, _: &str) -> Result<DoctorCheckMeta, PortError> {
        Ok(self.list_checks().remove(0))
    }
    async fn bootstrap(&self) -> Result<BootstrapReport, PortError> {
        Ok(BootstrapReport::default())
    }
}

#[tokio::test]
async fn real_command_and_reader_clone_share_reports_fix_refresh_and_permission_failure_keeps_the_last_observation()
 {
    let port = Arc::new(DiagnosticPort::default());
    let doctor = DoctorApplication::new(port.clone());
    let reader = doctor.clone();
    assert_eq!(reader.page().status, PageStatus::Empty);
    assert!(!reader.page().data.unwrap().overall_healthy);
    let commands = CommandApplication::new().with_doctor(doctor);
    commands
        .execute(CommandIntent::RunDoctorDiagnostics)
        .await
        .unwrap();
    let first = reader.page();
    assert_eq!(first.status, PageStatus::Ready);
    assert_eq!(
        first.data.as_ref().unwrap().checks[0].state,
        DoctorStatus::Fail
    );
    assert!(first.data.as_ref().unwrap().checks[0].fix_available);
    port.fail.store(true, Ordering::SeqCst);
    let failure = commands
        .execute(CommandIntent::RunDoctorDiagnostics)
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::Permission);
    let failed = reader.page();
    assert_eq!(failed.data, first.data);
    assert_eq!(failed.status, PageStatus::Failed { failure });
    port.fail.store(false, Ordering::SeqCst);
    commands
        .execute(CommandIntent::RepairDoctorIssue {
            check_id: "config.check".into(),
        })
        .await
        .unwrap();
    let repaired = reader.page();
    assert_eq!(repaired.status, PageStatus::Ready);
    assert!(repaired.data.as_ref().unwrap().overall_healthy);
    assert_eq!(
        repaired.data.as_ref().unwrap().checks[0].state,
        DoctorStatus::Pass
    );
    assert!(!repaired.data.as_ref().unwrap().checks[0].fix_available);
    assert_eq!(port.runs.load(Ordering::SeqCst), 3);
    assert_eq!(port.fixes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn dropping_a_pending_operation_preserves_report_and_clears_busy_with_canceled_failure_then_retry_works()
 {
    let port = Arc::new(DiagnosticPort::default());
    let doctor = DoctorApplication::new(port.clone());
    doctor.run(None).await.unwrap();
    let previous = doctor.page().data;
    port.blocked.store(true, Ordering::SeqCst);
    let mut request = Box::pin(doctor.run(None));
    assert!(poll!(request.as_mut()).is_pending());
    assert_eq!(doctor.page().status, PageStatus::Loading);
    assert_eq!(doctor.page().data, previous);
    drop(request);
    let PageStatus::Failed { failure } = doctor.page().status else {
        panic!("cancel outcome")
    };
    assert_eq!(failure.code, ErrorCode::Canceled);
    assert_eq!(doctor.page().data, previous);
    port.blocked.store(false, Ordering::SeqCst);
    doctor.run(None).await.unwrap();
    assert_eq!(doctor.page().status, PageStatus::Ready);
}

#[tokio::test]
async fn malformed_report_is_rejected_without_replacing_the_last_observation() {
    let port = Arc::new(DiagnosticPort::default());
    let doctor = DoctorApplication::new(port);
    doctor.run(None).await.unwrap();
    let previous = doctor.page().data;
    let mut malformed = doctor.observation.lock().unwrap().report.clone().unwrap();
    malformed.checks.push(malformed.checks[0].clone());
    let failure = DoctorApplication::validate_report(malformed).unwrap_err();
    assert_eq!(failure.code, ErrorCode::InvalidState);
    doctor.finish(&Err(failure.clone())).unwrap();
    assert_eq!(doctor.page().data, previous);
    assert_eq!(doctor.page().status, PageStatus::Failed { failure });
}

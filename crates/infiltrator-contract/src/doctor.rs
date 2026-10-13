//! Cross-surface doctor and bootstrap results.

use crate::command::CommandIntent;
use crate::error::{ErrorCode, Failure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DoctorAction {
    Diagnose,
    RepairAll,
    RepairOne(String),
    Bootstrap,
}

/// Canonical descriptions for built-in checks; arbitrary host findings remain report data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DoctorCheckKind {
    TunHealth,
    SystemProxy,
    Ports,
    DnsPrivacy,
    Privileges,
    Configuration,
}
impl DoctorCheckKind {
    pub const fn copy_keys(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::TunHealth => (
                "doctor_check_tun_name",
                "doctor_category_network",
                "doctor_check_tun_fixture",
            ),
            Self::SystemProxy => (
                "doctor_check_proxy_name",
                "doctor_category_system",
                "doctor_check_proxy_fixture",
            ),
            Self::Ports => (
                "doctor_check_ports_name",
                "doctor_category_ports",
                "doctor_check_ports_fixture",
            ),
            Self::DnsPrivacy => (
                "doctor_check_dns_name",
                "doctor_category_dns",
                "doctor_check_dns_fixture",
            ),
            Self::Privileges => (
                "doctor_check_privilege_name",
                "doctor_category_privilege",
                "doctor_check_privilege_fixture",
            ),
            Self::Configuration => (
                "doctor_check_config_name",
                "doctor_category_config",
                "doctor_check_config_fixture",
            ),
        }
    }
}
impl DoctorAction {
    pub fn intent(&self) -> CommandIntent {
        match self {
            Self::Diagnose => CommandIntent::RunDoctorDiagnostics,
            Self::RepairAll => CommandIntent::RepairAllDoctorIssues,
            Self::RepairOne(check_id) => CommandIntent::RepairDoctorIssue {
                check_id: check_id.clone(),
            },
            Self::Bootstrap => CommandIntent::BootstrapDoctor,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorStatus {
    Pass,
    #[serde(alias = "warning")]
    Warn,
    Fail,
    Skip,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorCheckResult {
    pub id: String,
    pub category: String,
    pub status: DoctorStatus,
    pub summary: String,
    pub detail: Option<String>,
    pub hint: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub started_at: u64,
    pub finished_at: u64,
    pub checks: Vec<DoctorCheckResult>,
}

impl DoctorReport {
    pub fn has_failures(&self) -> bool {
        self.count_by_status(DoctorStatus::Fail) > 0
    }

    pub fn count_by_status(&self, status: DoctorStatus) -> usize {
        self.checks
            .iter()
            .filter(|check| check.status == status)
            .count()
    }
}

/// Authoritative application observation state. A prior report is an explicit
/// variant payload so loading/failure never erases the last accepted facts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "value", rename_all = "snake_case")]
pub enum DoctorObservation {
    Unobserved,
    Loading,
    LoadingWithReport(DoctorReport),
    Failed(Failure),
    FailedWithReport {
        report: DoctorReport,
        failure: Failure,
    },
    Ready(DoctorReport),
}

impl DoctorObservation {
    /// Start a read while retaining only an accepted report from the prior state.
    pub fn begin(&self) -> Result<Self, Failure> {
        match self {
            Self::Unobserved | Self::Failed(_) => Ok(Self::Loading),
            Self::Ready(report) | Self::FailedWithReport { report, .. } => {
                Ok(Self::LoadingWithReport(report.clone()))
            }
            Self::Loading | Self::LoadingWithReport(_) => Err(Self::invalid_transition()),
        }
    }

    /// Complete the active read with its validated report or typed failure.
    pub fn finish(self, result: Result<DoctorReport, Failure>) -> Result<Self, Failure> {
        match self {
            Self::Loading => match result {
                Ok(report) => Ok(Self::Ready(report)),
                Err(failure) => Ok(Self::Failed(failure)),
            },
            Self::LoadingWithReport(previous) => match result {
                Ok(report) => Ok(Self::Ready(report)),
                Err(failure) => Ok(Self::FailedWithReport {
                    report: previous,
                    failure,
                }),
            },
            Self::Unobserved | Self::Failed(_) | Self::FailedWithReport { .. } | Self::Ready(_) => {
                Err(Self::invalid_transition())
            }
        }
    }

    /// Record cancellation without discarding any last accepted report.
    pub fn cancel(self) -> Result<Self, Failure> {
        let failure = Failure::new(
            ErrorCode::Canceled,
            "diagnostic operation was canceled",
            true,
        );
        match self {
            Self::Loading => Ok(Self::Failed(failure)),
            Self::LoadingWithReport(report) => Ok(Self::FailedWithReport { report, failure }),
            Self::Unobserved | Self::Failed(_) | Self::FailedWithReport { .. } | Self::Ready(_) => {
                Err(Self::invalid_transition())
            }
        }
    }

    fn invalid_transition() -> Failure {
        Failure::new(
            ErrorCode::InvalidState,
            "diagnostic observation transition is invalid",
            false,
        )
    }
}

#[cfg(test)]
mod observation_tests {
    use super::{DoctorObservation, DoctorReport};
    use crate::doctor::DoctorCheckResult;
    use crate::error::{ErrorCode, Failure};

    fn report() -> DoctorReport {
        DoctorReport {
            started_at: 10,
            finished_at: 20,
            checks: vec![DoctorCheckResult {
                id: "core.ready".into(),
                category: "core".into(),
                status: super::DoctorStatus::Pass,
                summary: "Core is ready".into(),
                detail: None,
                hint: None,
            }],
        }
    }

    #[test]
    fn retry_failure_and_cancellation_retain_only_the_last_accepted_report() {
        let previous = report();
        let loading = DoctorObservation::Ready(previous.clone())
            .begin()
            .expect("ready can begin a refresh");
        assert_eq!(
            loading,
            DoctorObservation::LoadingWithReport(previous.clone())
        );

        let failure = Failure::new(ErrorCode::Permission, "doctor read denied", false);
        let failed = loading
            .finish(Err(failure.clone()))
            .expect("loading can finish as failed");
        assert_eq!(
            failed,
            DoctorObservation::FailedWithReport {
                report: previous.clone(),
                failure
            }
        );

        let cancelled = DoctorObservation::Unobserved
            .begin()
            .expect("unobserved can begin")
            .cancel()
            .expect("loading can be cancelled");
        assert!(matches!(cancelled, DoctorObservation::Failed(_)));

        let retried = failed.begin().expect("failed can retry");
        assert_eq!(retried, DoctorObservation::LoadingWithReport(previous));
    }

    #[test]
    fn only_loading_observations_accept_terminal_transitions() {
        assert_eq!(
            DoctorObservation::Unobserved
                .finish(Ok(report()))
                .unwrap_err()
                .code,
            ErrorCode::InvalidState
        );
        assert_eq!(
            DoctorObservation::Ready(report())
                .cancel()
                .unwrap_err()
                .code,
            ErrorCode::InvalidState
        );
        assert_eq!(
            DoctorObservation::Loading.begin().unwrap_err().code,
            ErrorCode::InvalidState
        );
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorCheckMeta {
    pub id: String,
    pub category: String,
    pub summary: String,
    pub why: String,
    pub fail_means: String,
    pub hint: String,
    pub fixable: bool,
    pub default_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorFixAction {
    pub id: String,
    pub summary: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorFixReport {
    pub actions: Vec<DoctorFixAction>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapStep {
    pub id: String,
    pub executed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapReport {
    pub steps: Vec<BootstrapStep>,
}

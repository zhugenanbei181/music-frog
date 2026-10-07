//! Cross-surface doctor and bootstrap results.

use crate::command::CommandIntent;
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

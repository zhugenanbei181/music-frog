//! Cross-surface doctor and bootstrap results.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorStatus {
    Pass,
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

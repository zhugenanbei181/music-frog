//! Doctor panel interaction state; all diagnostic result types belong to the contract.

use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_contract::doctor::DoctorReport;
use infiltrator_contract::snapshot::CoreWatchdogSnapshot;
use infiltrator_presentation::doctor_actions::{DoctorActionPhase, DoctorActionState};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DoctorPanelOperation {
    #[default]
    Idle,
    Checking,
    Fixing,
    Bootstrapping,
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoctorPanelRequest {
    Check,
    Fix,
    Bootstrap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoctorPanelStart {
    Started,
    Busy,
}

/// 体检面板的 UI 状态（诊断域子状态）。
#[derive(Debug, Clone, Default)]
pub struct DoctorPanelState {
    pub action: DoctorActionState,
    pub capture_observation: Option<DoctorApplication>,
    pub report: Option<DoctorReport>,
    pub operation: DoctorPanelOperation,
}

impl DoctorPanelState {
    pub fn begin(&mut self, request: DoctorPanelRequest) -> DoctorPanelStart {
        match (&self.operation, request) {
            (
                DoctorPanelOperation::Checking
                | DoctorPanelOperation::Fixing
                | DoctorPanelOperation::Bootstrapping,
                _,
            ) => DoctorPanelStart::Busy,
            (DoctorPanelOperation::Idle | DoctorPanelOperation::Failed(_), request) => {
                self.operation = match request {
                    DoctorPanelRequest::Check => DoctorPanelOperation::Checking,
                    DoctorPanelRequest::Fix => DoctorPanelOperation::Fixing,
                    DoctorPanelRequest::Bootstrap => DoctorPanelOperation::Bootstrapping,
                };
                DoctorPanelStart::Started
            }
        }
    }

    pub fn finish(&mut self, result: Result<(), String>) {
        self.operation = match result {
            Ok(()) => DoctorPanelOperation::Idle,
            Err(failure) => DoctorPanelOperation::Failed(failure),
        };
    }

    pub fn clear_local_failure(&mut self) {
        match &self.operation {
            DoctorPanelOperation::Failed(_) => self.operation = DoctorPanelOperation::Idle,
            DoctorPanelOperation::Idle
            | DoctorPanelOperation::Checking
            | DoctorPanelOperation::Fixing
            | DoctorPanelOperation::Bootstrapping => {}
        }
    }

    pub fn failure_message(&self) -> Option<&str> {
        match &self.operation {
            DoctorPanelOperation::Failed(failure) => Some(failure),
            DoctorPanelOperation::Checking
            | DoctorPanelOperation::Fixing
            | DoctorPanelOperation::Bootstrapping => None,
            DoctorPanelOperation::Idle => match &self.action.phase {
                DoctorActionPhase::Failed { failure, .. } => Some(failure.message.as_str()),
                DoctorActionPhase::Idle | DoctorActionPhase::Pending(_) => None,
            },
        }
    }
}

/// State for the Crash Watchdog and Sanitized Forensics Reporter.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CrashWatchdogState {
    pub is_orphaned_detected: bool,
    pub last_crash_summary: Option<String>,
    pub recovery_status: Option<String>,
    pub exported_log_path: Option<String>,
    /// Canonical crash-recovery state from the shared Core snapshot.
    pub shared: CoreWatchdogSnapshot,
}

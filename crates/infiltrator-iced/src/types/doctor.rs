//! Doctor panel interaction state; all diagnostic result types belong to the contract.

use infiltrator_application::doctor_actions::DoctorActionState;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_contract::doctor::DoctorReport;
use infiltrator_contract::snapshot::CoreWatchdogSnapshot;

/// 体检面板的 UI 状态（诊断域子状态）。
#[derive(Debug, Clone, Default)]
pub struct DoctorPanelState {
    pub action: DoctorActionState,
    pub capture_observation: Option<DoctorApplication>,
    pub report: Option<DoctorReport>,
    pub is_running: bool,
    pub is_fixing: bool,
    pub is_bootstrapping: bool,
    pub error: Option<String>,
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

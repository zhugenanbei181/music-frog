//! Doctor panel interaction state; all diagnostic result types belong to the contract.

use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_contract::doctor::{DoctorAction, DoctorReport};
use infiltrator_contract::snapshot::CoreWatchdogSnapshot;
use infiltrator_contract::surface_snapshot::{DoctorCheckSnapshot, DoctorPageSnapshot};
use infiltrator_presentation::doctor_actions::{
    DoctorActionFinish, DoctorActionPhase, DoctorActionState, PendingDoctorAction,
};
use lurker_core_contract::error::Failure;

/// 体检面板的 UI 状态（诊断域子状态）。
///
/// The panel keeps no phase of its own: the diagnostic stage is the shared
/// [`DoctorActionState`], so the surface can never answer "is the panel
/// busy" differently from the owner. The parallel `DoctorPanelOperation`
/// machine that used to sit next to it is gone.
#[derive(Debug, Clone, Default)]
pub struct DoctorPanelState {
    /// The shared action state of the panel: the one typed stage machine.
    pub action: DoctorActionState,
    /// The doctor page snapshot the owner published.
    pub page: Option<DoctorPageSnapshot>,
    /// The last diagnostic report the admin server published.
    pub report: Option<DoctorReport>,
    /// The demo capture application the panel projects from.
    pub capture_observation: Option<DoctorApplication>,
}

impl DoctorPanelState {
    /// Begin a diagnostic operation.
    ///
    /// A second operation is refused while one is pending: the panel runs
    /// one diagnostic at a time.
    pub fn begin(&mut self, action: DoctorAction) -> Result<PendingDoctorAction, Failure> {
        self.action.begin(action)
    }

    /// Retry the operation that failed.
    pub fn begin_retry(&mut self) -> Result<PendingDoctorAction, Failure> {
        self.action.begin_retry()
    }

    /// Finish the running operation.
    ///
    /// An answer for a superseded token is stale and changes nothing.
    pub fn finish(&mut self, token: u64, result: Result<(), Failure>) -> DoctorActionFinish {
        self.action.finish(token, result)
    }

    /// Whether the panel is running a diagnostic operation.
    pub fn is_busy(&self) -> bool {
        matches!(&self.action.phase, DoctorActionPhase::Pending(_))
    }

    /// The failure the panel shows, when the last operation failed.
    pub fn failure(&self) -> Option<&Failure> {
        match &self.action.phase {
            DoctorActionPhase::Failed { failure, .. } => Some(failure),
            DoctorActionPhase::Idle | DoctorActionPhase::Pending(_) => None,
        }
    }

    /// The action the panel is running, or the action that failed.
    pub fn current_action(&self) -> Option<&DoctorAction> {
        match &self.action.phase {
            DoctorActionPhase::Pending(pending) => Some(&pending.action),
            DoctorActionPhase::Failed { retry_action, .. } => Some(retry_action),
            DoctorActionPhase::Idle => None,
        }
    }

    /// Whether the user may retry the failed operation.
    pub fn can_retry(&self) -> bool {
        self.action.can_retry()
    }

    /// Settle the running diagnostic operation with the observed result.
    ///
    /// The token is read from the shared phase, so an answer that arrives
    /// after the operation was superseded changes nothing.
    pub fn settle(&mut self, result: Result<(), Failure>) -> DoctorActionFinish {
        let token = match &self.action.phase {
            DoctorActionPhase::Pending(pending) => pending.token,
            DoctorActionPhase::Idle | DoctorActionPhase::Failed { .. } => {
                return DoctorActionFinish::Stale;
            }
        };
        self.action.finish(token, result)
    }

    /// The checks the owner reported.
    pub fn checks(&self) -> &[DoctorCheckSnapshot] {
        match &self.page {
            Some(page) => &page.checks,
            None => &[],
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

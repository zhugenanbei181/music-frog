//! The one typed page model of the doctor page, shared by every surface.
//!
//! The doctor panel used to run two stage machines for one operation: the
//! shared `DoctorActionState` and a private `DoctorPanelOperation` that
//! repeated "idle / running / failed" next to it. Two machines for one
//! process can disagree, and Iced's copy could say "checking" while the
//! shared state said "idle".
//!
//! This module owns the page state and composes the shared
//! [`crate::doctor_actions`] machine instead of repeating it. The panel's
//! busy flag, its failure and the action it is running all come from the one
//! shared phase, so every surface answers the same way.

use crate::doctor_actions::{
    DoctorActionFinish, DoctorActionPhase, DoctorActionState, PendingDoctorAction,
};
use infiltrator_contract::doctor::{DoctorAction, DoctorReport};
use infiltrator_contract::surface_snapshot::{DoctorCheckSnapshot, DoctorPageSnapshot};
use lurker_core_contract::error::Failure;

/// The overall health the doctor page shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DoctorOverallHealth {
    /// Every reported check passed.
    Healthy,
    /// At least one reported check warned or failed.
    Unhealthy,
    /// The owner has not published a report yet.
    #[default]
    Unobserved,
}

/// The one page model of the doctor page.
///
/// A surface renders the doctor page from this model alone: the busy state,
/// the failure, the report and the checks come from here, and the surface
/// only dispatches typed intents back.
#[derive(Clone, Debug, Default)]
pub struct DoctorPageModel {
    action: DoctorActionState,
    page: Option<DoctorPageSnapshot>,
    report: Option<DoctorReport>,
}

impl DoctorPageModel {
    /// The shared action state of the panel.
    pub fn action(&self) -> &DoctorActionState {
        &self.action
    }

    /// The shared action state of the panel, for the fold that changes it.
    pub fn action_mut(&mut self) -> &mut DoctorActionState {
        &mut self.action
    }

    /// Begin a diagnostic operation.
    ///
    /// A second operation is refused while one is pending: the panel runs one
    /// diagnostic at a time.
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

    /// Fold the owner's doctor page snapshot into the model.
    pub fn observe_page(&mut self, page: DoctorPageSnapshot) {
        self.page = Some(page);
    }

    /// The owner's doctor page snapshot, when it has been observed.
    pub fn page(&self) -> Option<&DoctorPageSnapshot> {
        self.page.as_ref()
    }

    /// The checks the owner reported.
    pub fn checks(&self) -> &[DoctorCheckSnapshot] {
        match &self.page {
            Some(page) => &page.checks,
            None => &[],
        }
    }

    /// The overall health the owner reported.
    pub fn overall_health(&self) -> DoctorOverallHealth {
        match &self.page {
            Some(page) => {
                if page.overall_healthy {
                    DoctorOverallHealth::Healthy
                } else {
                    DoctorOverallHealth::Unhealthy
                }
            }
            None => DoctorOverallHealth::Unobserved,
        }
    }

    /// Fold the owner's diagnostic report into the model.
    pub fn observe_report(&mut self, report: DoctorReport) {
        self.report = Some(report);
    }

    /// The diagnostic report the owner published.
    pub fn report(&self) -> Option<&DoctorReport> {
        self.report.as_ref()
    }
}

#[cfg(test)]
#[path = "doctor_page_model_tests.rs"]
mod tests;

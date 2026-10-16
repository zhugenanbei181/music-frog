use super::*;

impl SpeedtestOwnerState {
    pub(super) fn run_id(&self) -> Option<SpeedtestRunId> {
        match &self.execution {
            SpeedtestExecution::Idle => None,
            SpeedtestExecution::Probing { run_id, .. }
            | SpeedtestExecution::CancelRequested { run_id, .. }
            | SpeedtestExecution::Completed { run_id, .. }
            | SpeedtestExecution::Cancelled { run_id, .. }
            | SpeedtestExecution::Failed { run_id, .. } => Some(*run_id),
        }
    }

    pub(super) fn is_active(&self) -> bool {
        matches!(
            &self.execution,
            SpeedtestExecution::Probing { .. } | SpeedtestExecution::CancelRequested { .. }
        )
    }

    pub(super) fn snapshot(&self) -> SpeedtestSnapshot {
        let state = match &self.execution {
            SpeedtestExecution::Idle => SpeedtestRunState::Idle,
            SpeedtestExecution::Probing {
                run_id,
                target,
                progress,
                ..
            } => SpeedtestRunState::Probing {
                run_id: *run_id,
                target: target.clone(),
                progress: progress.clone(),
            },
            SpeedtestExecution::CancelRequested {
                run_id,
                target,
                progress,
            } => SpeedtestRunState::CancelRequested {
                run_id: *run_id,
                target: target.clone(),
                progress: progress.clone(),
            },
            SpeedtestExecution::Completed {
                run_id,
                target,
                progress,
            } => SpeedtestRunState::Completed {
                run_id: *run_id,
                target: target.clone(),
                progress: progress.clone(),
            },
            SpeedtestExecution::Cancelled {
                run_id,
                target,
                progress,
            } => SpeedtestRunState::Cancelled {
                run_id: *run_id,
                target: target.clone(),
                progress: progress.clone(),
            },
            SpeedtestExecution::Failed {
                run_id,
                target,
                failure,
            } => SpeedtestRunState::Failed {
                run_id: *run_id,
                target: target.clone(),
                failure: failure.clone(),
            },
        };
        SpeedtestSnapshot {
            revision: self.revision,
            state,
            concurrency: self.concurrency,
            results: self.results.clone(),
            recent_history: self.recent_history.clone(),
        }
    }
}

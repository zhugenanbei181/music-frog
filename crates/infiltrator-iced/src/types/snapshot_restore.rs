//! Native restoration actions keep every completion correlated with the shared transaction.
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::Failure;
use infiltrator_contract::snapshot_restore::SnapshotRestoreTarget;
#[derive(Clone, Debug)]
pub enum RestoreAction {
    Open(SnapshotRestoreTarget),
    Confirm,
    Cancel,
    Retry,
    Finished {
        operation: u64,
        result: Box<Result<CommandOutput, Failure>>,
    },
}

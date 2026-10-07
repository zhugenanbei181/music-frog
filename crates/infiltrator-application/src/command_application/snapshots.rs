//! Source-scoped snapshot operations and typed terminal receipts.
use super::CommandApplication;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::Failure;
use infiltrator_contract::snapshot_history::{
    SNAPSHOT_DEFAULT_KEEP, SnapshotCreatedReceipt, SnapshotPruneSource, SnapshotPrunedReceipt,
};
use std::path::Path;
impl CommandApplication {
    pub(super) async fn execute_snapshot_output(
        &self,
        intent: CommandIntent,
    ) -> Result<CommandOutput, Failure> {
        match intent {
            CommandIntent::CreateBackupSnapshot { profile } => {
                let profile = self.snapshot_profile(profile).await?;
                let meta = self.snapshots()?.create(&profile).await?;
                Ok(CommandOutput::SnapshotCreated(SnapshotCreatedReceipt {
                    profile: meta.profile,
                    id: meta.path.to_string_lossy().into_owned(),
                    sha256: meta.sha256,
                }))
            }
            CommandIntent::LoadSnapshotHistory { profile, keep } => {
                let profile = self.snapshot_profile(profile).await?;
                self.snapshots()?
                    .history(&profile, keep)
                    .await
                    .map(CommandOutput::SnapshotHistoryLoaded)
            }
            CommandIntent::LoadSnapshotDiff {
                profile,
                snapshot_id,
            } => {
                let profile = self.snapshot_profile(profile).await?;
                let snapshots = self.snapshots()?;
                let diff = match snapshot_id {
                    Some(id) => Some(snapshots.diff_snapshot(&profile, Path::new(&id)).await?),
                    None => snapshots.diff_newest(&profile).await?,
                };
                Ok(CommandOutput::SnapshotDiffLoaded(diff.map(Box::new)))
            }
            CommandIntent::PruneSnapshots { profile, keep } => {
                let profile = self.snapshot_profile(profile).await?;
                self.snapshots()?
                    .prune(
                        &profile,
                        keep.unwrap_or(SNAPSHOT_DEFAULT_KEEP),
                        SnapshotPruneSource::Manual,
                    )
                    .await
                    .map(|report| {
                        CommandOutput::SnapshotsPruned(SnapshotPrunedReceipt { profile, report })
                    })
            }
            CommandIntent::PrepareSnapshotRestore { target } => self
                .snapshots()?
                .prepare_restore(&target)
                .await
                .map(|review| CommandOutput::SnapshotRestorePrepared(Box::new(review))),
            CommandIntent::ConfirmSnapshotRestore { identity } => self
                .snapshots()?
                .confirm_restore(self.managed_runtime.clone(), &identity)
                .await
                .map(CommandOutput::SnapshotRestored),
            CommandIntent::CancelSnapshotRestore { identity } => self
                .snapshots()?
                .cancel_restore(&identity)
                .map(CommandOutput::SnapshotRestoreCancelled),
            _ => Err(Failure::unsupported("Not a snapshot operation")),
        }
    }
    async fn snapshot_profile(&self, requested: Option<String>) -> Result<String, Failure> {
        match requested {
            Some(profile) => Ok(profile),
            None => self.profile()?.current_profile().await,
        }
    }
}

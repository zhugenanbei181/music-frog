//! The WebDAV conflict-resolution use case over the sync and profile owners.
//!
//! The sync owner owns the remote conflict copy; the profile owner owns the
//! local configuration. This use case composes them: keeping the local
//! configuration discards the remote copy, and taking the remote configuration
//! writes the remote copy into the local profile before the copy is discarded.
//! The owner's own readback decides whether the resolution landed, so a
//! surface never infers the outcome from the absence of an error.

use crate::profile_application::{ProfileApplication, ProfileRuntimeAccess};
use crate::sync_application::SyncApplication;
use infiltrator_contract::sync::{
    SyncConflict, SyncConflictResolution, SyncConflictResolutionReceipt,
};
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::config::validate_yaml;
use infiltrator_ports::active_profile_reference_applier::ActiveProfileReferenceApplier;
use lurker_core_contract::error::{ErrorCode, Failure};

/// The conflict-resolution use case over the sync owner and the profile owner.
pub struct SyncConflictApplication {
    sync: SyncApplication,
    profile: ProfileApplication,
    runtime: ProfileRuntimeAccess<dyn ActiveProfileReferenceApplier>,
    configs_dir: Option<String>,
}

impl SyncConflictApplication {
    pub fn new(
        sync: SyncApplication,
        profile: ProfileApplication,
        runtime: ProfileRuntimeAccess<dyn ActiveProfileReferenceApplier>,
        configs_dir: Option<String>,
    ) -> Self {
        Self {
            sync,
            profile,
            runtime,
            configs_dir,
        }
    }

    /// Resolve the conflict the sync owner currently holds.
    ///
    /// The owner names the conflict it holds, so the resolution always acts on
    /// a real conflict instead of on a guess made by the caller.
    pub async fn resolve(
        &self,
        resolution: SyncConflictResolution,
    ) -> Result<SyncConflictResolutionReceipt, Failure> {
        let conflict = self.sync.current_conflict().await?.ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidState,
                "no synchronization conflict is pending",
                false,
            )
        })?;
        match resolution {
            SyncConflictResolution::KeepLocal => {
                // The local configuration stays the truth; the remote copy is
                // discarded so the same conflict cannot be resolved twice.
                self.sync
                    .discard_conflict(self.configs_dir.as_deref(), &conflict)
                    .await?;
            }
            SyncConflictResolution::TakeRemote => {
                let content = self
                    .sync
                    .read_conflict_content(self.configs_dir.as_deref(), &conflict)
                    .await?;
                validate_yaml(&content).map_err(|error| {
                    Failure::new(ErrorCode::Configuration, error.to_string(), false)
                })?;
                self.profile
                    .save_profile_content(
                        self.runtime.clone(),
                        conflict.conflict.profile.clone(),
                        content,
                        ApplyStrategy::PreferReload,
                    )
                    .await?;
                // The remote copy is discarded only after the local profile
                // holds the remote content, so a failed write keeps it.
                self.sync
                    .discard_conflict(self.configs_dir.as_deref(), &conflict)
                    .await?;
            }
        }
        self.read_back(conflict.conflict.clone(), resolution).await
    }

    /// Read the owner's state back and decide whether the resolution landed.
    ///
    /// An owner that still holds the resolved conflict did not accept the
    /// resolution, and an owner that moved on to another conflict means this
    /// answer is stale: neither case may report a resolution.
    async fn read_back(
        &self,
        conflict: SyncConflict,
        resolution: SyncConflictResolution,
    ) -> Result<SyncConflictResolutionReceipt, Failure> {
        match self.sync.current_conflict().await? {
            None => Ok(SyncConflictResolutionReceipt {
                conflict,
                resolution,
            }),
            Some(observed) if observed.conflict == conflict => Err(Failure::new(
                ErrorCode::InvalidState,
                "the synchronization conflict was not cleared by the resolution",
                false,
            )),
            Some(_) => Err(Failure::new(
                ErrorCode::NotReady,
                "the synchronization conflict changed while it was being resolved",
                true,
            )),
        }
    }
}

#[cfg(test)]
#[path = "sync_conflict_application_tests.rs"]
mod tests;

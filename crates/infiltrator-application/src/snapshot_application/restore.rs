//! Only a live reviewed identity can commit; cancellation never touches a storage write port.
use super::SnapshotApplication;
use crate::profile_application::ProfileRuntimeAccess;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceState;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_contract::snapshot_restore::{
    SnapshotRestoreCancelled, SnapshotRestoreIdentity, SnapshotRestoreReceipt,
    SnapshotRestoreReview, SnapshotRestoreTarget,
};
use infiltrator_domain::config::validate_yaml;
use infiltrator_domain::myers_diff::compute_diff;
use infiltrator_domain::profile_source::hash_document_bytes;
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use infiltrator_ports::runtime_gateway::ActiveProfileReferenceApplier;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) fn allocate_owner() -> u64 {
    static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);
    NEXT_OWNER
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .expect("snapshot restoration owner exhausted")
}
#[derive(Clone)]
struct FrozenRestore {
    review: SnapshotRestoreReview,
    workspace: ProfileWorkspace,
    active_reference: ActiveProfileReferenceState,
}
#[derive(Clone)]
enum RestorePhase {
    Idle { sequence: u64 },
    Preparing { sequence: u64 },
    Prepared(FrozenRestore),
    Committing(FrozenRestore),
}
pub(super) struct RestoreState {
    phase: RestorePhase,
}
impl Default for RestoreState {
    fn default() -> Self {
        Self {
            phase: RestorePhase::Idle { sequence: 0 },
        }
    }
}
struct RestoreTransitionGuard<'a> {
    state: &'a Mutex<RestoreState>,
    sequence: u64,
    fallback: RestorePhase,
    kind: RestoreTransitionKind,
}
#[derive(Clone, Copy)]
enum RestoreTransitionKind {
    Preparing,
    Committing,
}
impl Drop for RestoreTransitionGuard<'_> {
    fn drop(&mut self) {
        let mut state = self.state.lock().expect("snapshot restore state");
        let matches_transition = match &state.phase {
            RestorePhase::Preparing { sequence } => {
                matches!(self.kind, RestoreTransitionKind::Preparing) && *sequence == self.sequence
            }
            RestorePhase::Committing(frozen) => {
                matches!(self.kind, RestoreTransitionKind::Committing)
                    && frozen.review.identity.sequence == self.sequence
            }
            RestorePhase::Idle { .. } | RestorePhase::Prepared(_) => false,
        };
        if matches_transition {
            state.phase = self.fallback.clone();
        }
    }
}
fn invalid(reason: &str) -> Failure {
    Failure::new(ErrorCode::InvalidState, reason, false)
}
impl SnapshotApplication {
    pub async fn prepare_restore(
        &self,
        target: &SnapshotRestoreTarget,
    ) -> Result<SnapshotRestoreReview, Failure> {
        self.validate_namespace()?;
        if target.profile.is_empty() || target.snapshot_id.is_empty() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "No snapshot restoration target",
                false,
            ));
        }
        let sequence = {
            let mut state = self.restores.lock().expect("snapshot restore state");
            let RestorePhase::Idle { sequence: current } = &state.phase else {
                return Err(invalid("A snapshot restoration review is already active"));
            };
            let sequence = current
                .checked_add(1)
                .ok_or_else(|| invalid("Snapshot restoration sequence exhausted"))?;
            state.phase = RestorePhase::Preparing { sequence };
            sequence
        };
        let _admission = RestoreTransitionGuard {
            state: &self.restores,
            sequence,
            fallback: RestorePhase::Idle { sequence },
            kind: RestoreTransitionKind::Preparing,
        };
        let active_reference = self
            .profiles
            .active_profile_reference()
            .await
            .map_err(Failure::from)?;
        let target_profile = ProfileId::new(target.profile.clone())
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let workspace = self.profiles.load_workspace(&target_profile).await?;
        if self
            .profiles
            .active_profile_reference()
            .await
            .map_err(Failure::from)?
            != active_reference
        {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The active profile selection changed while the restore review was prepared",
                true,
            ));
        }
        if workspace.document.identity.profile_id.as_str() != target.profile {
            return Err(invalid("Snapshot workspace source is inconsistent"));
        }
        let restored_yaml = self
            .read(&target.profile, Path::new(&target.snapshot_id))
            .await?;
        validate_yaml(&restored_yaml)
            .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
        let snapshot_hash = hash_document_bytes(&restored_yaml);
        let before_yaml = workspace
            .document
            .utf8_text()
            .map_err(|error| Failure::new(ErrorCode::Storage, error.to_string(), false))?
            .to_owned();
        let difference = compute_diff(
            &before_yaml,
            &restored_yaml,
            &target.profile,
            &target.profile,
        )
        .with_source_path(target.snapshot_id.clone());
        let mut state = self.restores.lock().expect("snapshot restore state");
        let review = SnapshotRestoreReview {
            identity: SnapshotRestoreIdentity {
                owner: self.restore_owner,
                sequence,
                snapshot_hash,
                workspace_identity: workspace.identity(),
            },
            target: target.clone(),
            before_yaml,
            restored_yaml,
            difference,
        };
        review.validate()?;
        state.phase = RestorePhase::Prepared(FrozenRestore {
            review: review.clone(),
            workspace,
            active_reference,
        });
        Ok(review)
    }
    pub fn cancel_restore(
        &self,
        identity: &SnapshotRestoreIdentity,
    ) -> Result<SnapshotRestoreCancelled, Failure> {
        let mut state = self.restores.lock().expect("snapshot restore state");
        match &state.phase {
            RestorePhase::Prepared(frozen) if frozen.review.identity == *identity => {}
            RestorePhase::Preparing { .. } | RestorePhase::Committing(_) => {
                return Err(invalid("Snapshot restoration is pending"));
            }
            RestorePhase::Idle { .. } | RestorePhase::Prepared(_) => {
                return Err(invalid("Snapshot restoration review expired"));
            }
        }
        state.phase = RestorePhase::Idle {
            sequence: identity.sequence,
        };
        Ok(SnapshotRestoreCancelled {
            identity: identity.clone(),
        })
    }
    pub async fn confirm_restore<R: ActiveProfileReferenceApplier + ?Sized>(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        identity: &SnapshotRestoreIdentity,
    ) -> Result<SnapshotRestoreReceipt, Failure> {
        self.validate_namespace()?;
        let frozen = {
            let mut state = self.restores.lock().expect("snapshot restore state");
            let frozen = match &state.phase {
                RestorePhase::Prepared(frozen) if frozen.review.identity == *identity => {
                    frozen.clone()
                }
                RestorePhase::Preparing { .. } | RestorePhase::Committing(_) => {
                    return Err(invalid("Snapshot restoration is pending"));
                }
                RestorePhase::Idle { .. } | RestorePhase::Prepared(_) => {
                    return Err(invalid("Snapshot restoration review expired"));
                }
            };
            state.phase = RestorePhase::Committing(frozen.clone());
            frozen
        };
        let _admission = RestoreTransitionGuard {
            state: &self.restores,
            sequence: identity.sequence,
            fallback: RestorePhase::Prepared(frozen.clone()),
            kind: RestoreTransitionKind::Committing,
        };
        let snapshot = self
            .read(
                &frozen.review.target.profile,
                Path::new(&frozen.review.target.snapshot_id),
            )
            .await?;
        if hash_document_bytes(&snapshot) != identity.snapshot_hash {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The reviewed snapshot changed; cancel and prepare a new review",
                false,
            ));
        }
        let current = self
            .profiles
            .load_workspace(&identity.workspace_identity.document.profile_id)
            .await?;
        if current.identity() != identity.workspace_identity {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The profile or options changed; cancel and prepare a new review",
                false,
            ));
        }
        if self
            .profiles
            .active_profile_reference()
            .await
            .map_err(Failure::from)?
            != frozen.active_reference
        {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The active profile selection changed; cancel and prepare a new review",
                false,
            ));
        }
        let update = ProfileWorkspaceUpdate {
            purpose: ProfileWorkspacePurpose::Derived,
            document_bytes: frozen.review.restored_yaml.into_bytes(),
            sidecar_bytes: frozen.workspace.sidecar_bytes,
        };
        let committed = self
            .profiles
            .commit_workspace_at_reference(
                runtime,
                frozen.active_reference.clone(),
                &identity.workspace_identity,
                &update,
            )
            .await?;
        let receipt = SnapshotRestoreReceipt {
            identity: identity.clone(),
            committed: committed.identity(),
        };
        receipt.validate(identity)?;
        self.clear_diff(identity.workspace_identity.document.profile_id.as_str());
        self.restores.lock().expect("snapshot restore state").phase = RestorePhase::Idle {
            sequence: identity.sequence,
        };
        Ok(receipt)
    }
}

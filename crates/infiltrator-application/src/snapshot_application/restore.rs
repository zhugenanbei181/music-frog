//! Only a live reviewed identity can commit; cancellation never touches a storage write port.
use super::SnapshotApplication;
use infiltrator_contract::error::{ErrorCode, Failure};
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
use infiltrator_ports::runtime_gateway::ManagedRuntime;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

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
}
#[derive(Default)]
pub(super) struct RestoreState {
    sequence: u64,
    review: Option<FrozenRestore>,
    busy: bool,
}
struct AdmissionGuard<'a>(&'a Mutex<RestoreState>);
impl Drop for AdmissionGuard<'_> {
    fn drop(&mut self) {
        self.0.lock().expect("snapshot restore state").busy = false;
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
        {
            let mut state = self.restores.lock().expect("snapshot restore state");
            if state.busy || state.review.is_some() {
                return Err(invalid("A snapshot restoration review is already active"));
            }
            state.busy = true;
        }
        let _admission = AdmissionGuard(&self.restores);
        let workspace = self.profiles.load_workspace(&target.profile).await?;
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
        let sequence = state
            .sequence
            .checked_add(1)
            .ok_or_else(|| invalid("Snapshot restoration sequence exhausted"))?;
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
        state.sequence = sequence;
        state.review = Some(FrozenRestore {
            review: review.clone(),
            workspace,
        });
        Ok(review)
    }
    pub fn cancel_restore(
        &self,
        identity: &SnapshotRestoreIdentity,
    ) -> Result<SnapshotRestoreCancelled, Failure> {
        let mut state = self.restores.lock().expect("snapshot restore state");
        if state.busy {
            return Err(invalid("Snapshot restoration is pending"));
        }
        if state
            .review
            .as_ref()
            .is_none_or(|review| review.review.identity != *identity)
        {
            return Err(invalid("Snapshot restoration review expired"));
        }
        state.review = None;
        Ok(SnapshotRestoreCancelled {
            identity: identity.clone(),
        })
    }
    pub async fn confirm_restore<R: ManagedRuntime + ?Sized>(
        &self,
        runtime: Option<Arc<R>>,
        identity: &SnapshotRestoreIdentity,
    ) -> Result<SnapshotRestoreReceipt, Failure> {
        self.validate_namespace()?;
        let frozen = {
            let mut state = self.restores.lock().expect("snapshot restore state");
            if state.busy {
                return Err(invalid("Snapshot restoration is pending"));
            }
            let frozen = state
                .review
                .as_ref()
                .filter(|review| review.review.identity == *identity)
                .ok_or_else(|| invalid("Snapshot restoration review expired"))?
                .clone();
            state.busy = true;
            frozen
        };
        let _admission = AdmissionGuard(&self.restores);
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
            .load_workspace_id(&identity.workspace_identity.document.profile_id)
            .await?;
        if current.identity() != identity.workspace_identity {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The profile or options changed; cancel and prepare a new review",
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
            .commit_workspace(runtime, &identity.workspace_identity, &update)
            .await?;
        let receipt = SnapshotRestoreReceipt {
            identity: identity.clone(),
            committed: committed.identity(),
        };
        receipt.validate(identity)?;
        self.clear_diff(identity.workspace_identity.document.profile_id.as_str());
        self.restores.lock().expect("snapshot restore state").review = None;
        Ok(receipt)
    }
}

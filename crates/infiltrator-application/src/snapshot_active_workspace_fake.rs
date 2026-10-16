use tokio::sync::OwnedMutexGuard;
use super::FakeProfileStore;
use async_trait::async_trait;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceState,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::{
    ActiveProfileWorkspacePublishOutcome, ActiveProfileWorkspacePublishedLease,
    ActiveProfileWorkspaceRestoreOutcome, ActiveProfileWorkspaceRestoredLease,
    ActiveProfileWorkspaceWriteLease, ProfileStore,
};
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};

pub(super) struct FakeActiveWorkspaceWriteLease<'a> {
    pub(super) store: &'a FakeProfileStore,
    pub(super) expected_reference: ActiveProfileReference,
    pub(super) observed: ProfileWorkspace,
    pub(super) _guard: OwnedMutexGuard<()>,
}

struct FakeActiveWorkspacePublishedLease<'a> {
    store: &'a FakeProfileStore,
    previous: ProfileWorkspace,
    published: ProfileWorkspace,
    published_state: ProfileWorkspace,
    _guard: OwnedMutexGuard<()>,
}

struct FakeActiveWorkspaceRestoredLease {
    workspace: ProfileWorkspace,
    _guard: OwnedMutexGuard<()>,
}

#[async_trait]
impl ActiveProfileWorkspaceWriteLease for FakeActiveWorkspaceWriteLease<'_> {
    fn observed_workspace(&self) -> &ProfileWorkspace {
        &self.observed
    }

    async fn publish(
        self: Box<Self>,
        update: ProfileWorkspaceUpdate,
    ) -> ActiveProfileWorkspacePublishOutcome<'async_trait> {
        match self.store.read_active_profile_reference().await {
            Ok(ActiveProfileReferenceState::Selected(reference))
                if reference == self.expected_reference => {}
            Ok(_) => {
                return ActiveProfileWorkspacePublishOutcome::Unpublished(PortError::Rejected(
                    Failure::new(ErrorCode::NotReady, "active reference changed", true),
                ));
            }
            Err(error) => return ActiveProfileWorkspacePublishOutcome::Unpublished(error),
        }
        let expected = self.observed.identity();
        let published = match self.store.publish_workspace_locked(&expected, &update) {
            Ok(workspace) => workspace,
            Err(error) => return ActiveProfileWorkspacePublishOutcome::Unpublished(error),
        };
        let profile = expected.document.profile_id.clone();
        let published_state = match ProfileStore::load_workspace(self.store, &profile).await {
            Ok(workspace) => workspace,
            Err(error) => return ActiveProfileWorkspacePublishOutcome::Unpublished(error),
        };
        ActiveProfileWorkspacePublishOutcome::Published(Box::new(
            FakeActiveWorkspacePublishedLease {
                store: self.store,
                previous: self.observed.clone(),
                published,
                published_state,
                _guard: self._guard,
            },
        ))
    }
}

#[async_trait]
impl ActiveProfileWorkspacePublishedLease for FakeActiveWorkspacePublishedLease<'_> {
    fn previous_workspace(&self) -> &ProfileWorkspace {
        &self.previous
    }

    fn published_workspace(&self) -> &ProfileWorkspace {
        &self.published
    }

    fn commit(self: Box<Self>) -> ProfileWorkspace {
        self.published.clone()
    }

    async fn restore_if_current(
        self: Box<Self>,
    ) -> Result<ActiveProfileWorkspaceRestoreOutcome<'async_trait>, PortError> {
        let profile = self.published_state.document.identity.profile_id.clone();
        let current = ProfileStore::load_workspace(self.store, &profile).await?;
        if current.identity() != self.published_state.identity() {
            return Ok(ActiveProfileWorkspaceRestoreOutcome::Superseded {
                observed: current.identity(),
            });
        }
        let update = ProfileWorkspaceUpdate {
            purpose: ProfileWorkspacePurpose::Derived,
            document_bytes: self.previous.document.raw_bytes.clone(),
            sidecar_bytes: self.previous.sidecar_bytes.clone(),
        };
        let workspace = self
            .store
            .publish_workspace_locked(&self.published_state.identity(), &update)?;
        Ok(ActiveProfileWorkspaceRestoreOutcome::Restored(Box::new(
            FakeActiveWorkspaceRestoredLease {
                workspace,
                _guard: self._guard,
            },
        )))
    }
}

#[async_trait]
impl ActiveProfileWorkspaceRestoredLease for FakeActiveWorkspaceRestoredLease {
    fn restored_workspace(&self) -> &ProfileWorkspace {
        &self.workspace
    }

    fn finish(self: Box<Self>) -> ProfileWorkspace {
        self.workspace.clone()
    }
}

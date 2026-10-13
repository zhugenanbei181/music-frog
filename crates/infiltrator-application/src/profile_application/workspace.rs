//! Coherent document and sidecar reads and source-bound workspace commits.
use super::ProfileApplication;
use super::{ProfileRuntimeAccess, ProfileRuntimeAccessRef};
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceState, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::{
    ProfileId, ProfileSidecarRevision, ProfileWorkspaceIdentity,
};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::subscription_quota::QuotaProfileIdentity;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::profile_source::hash_document_bytes;
use infiltrator_ports::active_profile_reference_applier::ActiveProfileReferenceApplier;
use infiltrator_ports::profile_store::{
    ActiveProfileWorkspacePublishOutcome, ActiveProfileWorkspacePublishedLease,
    ActiveProfileWorkspaceRecoveryLease, ActiveProfileWorkspaceRecoveryResolution,
    ActiveProfileWorkspaceRestoreOutcome,
};
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};

enum WorkspaceReferenceTarget<'a> {
    Selected(&'a ActiveProfileReference),
    Other,
}

/// How a selected workspace commit is admitted against the observed Core
/// lifecycle. Storing for the next start is an explicit choice, never an
/// implicit fallback after a failed native apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileWorkspaceEffect {
    Apply(ApplyStrategy),
    StoreForNextStart(ProfileWorkspaceStoreReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileWorkspaceStoreReason {
    Offline,
    AutoReloadDisabled,
    CoreStopped,
    CoreFailed,
}

impl ProfileApplication {
    pub async fn load_workspace(&self, profile: &ProfileId) -> Result<ProfileWorkspace, Failure> {
        let store_engine = self.store.engine_id();
        let workspace = self
            .store
            .load_workspace(profile)
            .await
            .map_err(Failure::from)?;
        validate_workspace(&workspace, profile, &store_engine)?;
        Ok(workspace)
    }

    pub(crate) fn validate_expected_engine(
        &self,
        expected: &ProfileWorkspaceIdentity,
    ) -> Result<(), Failure> {
        let store_engine = self.store.engine_id();
        if expected.document.engine_id != store_engine {
            return Err(engine_mismatch(&expected.document.engine_id, &store_engine));
        }
        Ok(())
    }

    pub async fn active_quota_identity(&self) -> Result<QuotaProfileIdentity, Failure> {
        let profile = self.current_profile().await?;
        if profile.is_empty() {
            return Ok(QuotaProfileIdentity {
                profile,
                provider_hash: None,
            });
        }
        let metadata = self.load_metadata(&profile).await?;
        let provider_hash = metadata
            .subscription_url
            .as_deref()
            .filter(|url| !url.trim().is_empty())
            .map(hash_document_bytes);
        Ok(QuotaProfileIdentity {
            profile,
            provider_hash,
        })
    }

    pub async fn commit_workspace<R: ActiveProfileReferenceApplier + ?Sized>(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, Failure> {
        self.validate_expected_engine(expected)?;
        let expected_reference = self
            .store
            .read_active_profile_reference()
            .await
            .map_err(Failure::from)?;
        self.commit_workspace_at_reference(runtime, expected_reference, expected, update)
            .await
    }

    /// Commit a workspace against both the editor's observed workspace
    /// identity and the complete active-reference state captured when the
    /// edit began. The workspace identity fences later content edits even
    /// when the selected-profile revision itself has not changed.
    pub async fn commit_workspace_at_reference<R: ActiveProfileReferenceApplier + ?Sized>(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        expected_reference: ActiveProfileReferenceState,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, Failure> {
        self.validate_expected_engine(expected)?;
        let effect = match runtime.as_ref() {
            ProfileRuntimeAccessRef::Offline => {
                ProfileWorkspaceEffect::StoreForNextStart(ProfileWorkspaceStoreReason::Offline)
            }
            ProfileRuntimeAccessRef::CoreOwner(runtime) => match runtime.core_lifecycle() {
                CoreLifecycle::Ready | CoreLifecycle::Running => {
                    ProfileWorkspaceEffect::Apply(ApplyStrategy::PreferReload)
                }
                CoreLifecycle::Stopped => ProfileWorkspaceEffect::StoreForNextStart(
                    ProfileWorkspaceStoreReason::CoreStopped,
                ),
                CoreLifecycle::Failed => ProfileWorkspaceEffect::StoreForNextStart(
                    ProfileWorkspaceStoreReason::CoreFailed,
                ),
                CoreLifecycle::Starting | CoreLifecycle::Stopping => {
                    return Err(Failure::new(
                        ErrorCode::InvalidState,
                        "Core lifecycle is transitioning; profile content was not changed",
                        true,
                    ));
                }
            },
        };
        self.commit_workspace_at_reference_with_effect(
            runtime,
            expected_reference,
            expected,
            update,
            effect,
        )
        .await
    }

    pub async fn commit_workspace_at_reference_with_effect<
        R: ActiveProfileReferenceApplier + ?Sized,
    >(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        expected_reference: ActiveProfileReferenceState,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
        effect: ProfileWorkspaceEffect,
    ) -> Result<ProfileWorkspace, Failure> {
        self.validate_expected_engine(expected)?;
        let store_engine = self.store.engine_id();
        let previous = self
            .store
            .load_workspace(&expected.document.profile_id)
            .await
            .map_err(Failure::from)?;
        validate_workspace(&previous, &expected.document.profile_id, &store_engine)?;
        if previous.identity() != *expected {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The profile workspace changed before the commit began",
                true,
            ));
        }
        let observed_reference = self
            .store
            .read_active_profile_reference()
            .await
            .map_err(Failure::from)?;
        if observed_reference != expected_reference {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The active profile selection changed before the workspace commit began",
                true,
            ));
        }
        let reference_target = match &expected_reference {
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == expected.document.engine_id
                    && reference.profile_id == expected.document.profile_id =>
            {
                WorkspaceReferenceTarget::Selected(reference)
            }
            ActiveProfileReferenceState::Unselected { .. }
            | ActiveProfileReferenceState::Selected(_) => WorkspaceReferenceTarget::Other,
        };
        let committed = match runtime.as_ref() {
            ProfileRuntimeAccessRef::Offline => match reference_target {
                WorkspaceReferenceTarget::Selected(reference) => {
                    match effect {
                        ProfileWorkspaceEffect::Apply(_) => {
                            return Err(Failure::unsupported(
                                "selected profile apply requires a managed Core runtime",
                            ));
                        }
                        ProfileWorkspaceEffect::StoreForNextStart(
                            ProfileWorkspaceStoreReason::Offline,
                        ) => {}
                        ProfileWorkspaceEffect::StoreForNextStart(_) => {
                            return Err(Failure::new(
                                ErrorCode::InvalidState,
                                "offline workspace storage cannot claim a managed Core lifecycle",
                                false,
                            ));
                        }
                    }
                    self.persist_active_workspace(
                        reference,
                        expected,
                        update,
                        ActiveProfileReferenceWriteAuthorization::Unowned,
                    )
                    .await?
                }
                WorkspaceReferenceTarget::Other => self
                    .store
                    .compare_and_save_inactive_workspace(expected, update)
                    .await
                    .map_err(Failure::from)?,
            },
            ProfileRuntimeAccessRef::CoreOwner(runtime) => match reference_target {
                WorkspaceReferenceTarget::Selected(reference) => {
                    let runtime_engine = runtime.engine_id().ok_or_else(|| {
                        Failure::new(
                            ErrorCode::NotReady,
                            "Core owner has no authoritative engine identity",
                            false,
                        )
                    })?;
                    if runtime_engine != expected.document.engine_id {
                        return Err(engine_mismatch(
                            &expected.document.engine_id,
                            &runtime_engine,
                        ));
                    }
                    let authorization = runtime.write_authorization().map_err(Failure::from)?;
                    super::ensure_bound_owner_authorization(
                        authorization.clone(),
                        &expected.document.engine_id,
                    )?;
                    match (runtime.core_lifecycle(), effect) {
                        (
                            CoreLifecycle::Ready | CoreLifecycle::Running,
                            ProfileWorkspaceEffect::Apply(strategy),
                        ) => runtime
                            .apply_active_profile_workspace(
                                reference,
                                expected,
                                update,
                                authorization,
                                strategy,
                            )
                            .await
                            .map_err(Failure::from)?,
                        (
                            CoreLifecycle::Ready | CoreLifecycle::Running,
                            ProfileWorkspaceEffect::StoreForNextStart(
                                ProfileWorkspaceStoreReason::AutoReloadDisabled,
                            ),
                        )
                        | (
                            CoreLifecycle::Stopped,
                            ProfileWorkspaceEffect::StoreForNextStart(
                                ProfileWorkspaceStoreReason::CoreStopped,
                            ),
                        )
                        | (
                            CoreLifecycle::Failed,
                            ProfileWorkspaceEffect::StoreForNextStart(
                                ProfileWorkspaceStoreReason::CoreFailed,
                            ),
                        ) => {
                            self.persist_active_workspace(
                                reference,
                                expected,
                                update,
                                authorization,
                            )
                            .await?
                        }
                        (CoreLifecycle::Stopped, ProfileWorkspaceEffect::Apply(_)) => {
                            return Err(Failure::new(
                                ErrorCode::InvalidState,
                                "profile apply requires a ready or running Core; the Core is stopped and was not started",
                                true,
                            ));
                        }
                        (CoreLifecycle::Failed, ProfileWorkspaceEffect::Apply(_)) => {
                            return Err(Failure::new(
                                ErrorCode::InvalidState,
                                "profile apply requires a ready or running Core; the Core is failed and was not started",
                                true,
                            ));
                        }
                        (CoreLifecycle::Starting | CoreLifecycle::Stopping, _) => {
                            return Err(Failure::new(
                                ErrorCode::InvalidState,
                                "Core lifecycle is transitioning; profile content was not changed",
                                true,
                            ));
                        }
                        (_, ProfileWorkspaceEffect::StoreForNextStart(_)) => {
                            return Err(Failure::new(
                                ErrorCode::NotReady,
                                "the observed Core lifecycle changed before the selected workspace could be stored",
                                true,
                            ));
                        }
                    }
                }
                WorkspaceReferenceTarget::Other => self
                    .store
                    .compare_and_save_inactive_workspace(expected, update)
                    .await
                    .map_err(Failure::from)?,
            },
        };
        validate_workspace(&committed, &expected.document.profile_id, &store_engine)?;
        if !same_document_namespace(expected, &committed.identity()) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Workspace receipt changed engine, profile, source, format, or schema",
                false,
            ));
        }
        if committed.document.raw_bytes != update.document_bytes
            || committed.sidecar_bytes != update.sidecar_bytes
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Workspace receipt does not match the submitted documents",
                false,
            ));
        }
        if previous.document.raw_bytes != update.document_bytes
            && committed.document.identity.revision == previous.document.identity.revision
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Workspace receipt changed document bytes without changing its revision",
                false,
            ));
        }
        if previous.sidecar_bytes != update.sidecar_bytes
            && committed.sidecar_revision == previous.sidecar_revision
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Workspace receipt changed sidecar bytes without changing its revision",
                false,
            ));
        }
        Ok(committed)
    }

    pub(super) async fn persist_active_workspace(
        &self,
        reference: &ActiveProfileReference,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<ProfileWorkspace, Failure> {
        self.persist_active_workspace_owned(reference, expected, update, authorization)
            .await
    }

    async fn persist_active_workspace_owned(
        &self,
        reference: &ActiveProfileReference,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<ProfileWorkspace, Failure> {
        if reference.engine_id != expected.document.engine_id
            || reference.profile_id != expected.document.profile_id
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "active reference does not identify the submitted profile workspace",
                false,
            ));
        }
        let lease = self
            .store
            .acquire_active_workspace_write_lease(
                reference.clone(),
                expected.clone(),
                authorization,
            )
            .await
            .map_err(Failure::from)?;
        if lease.observed_workspace().identity() != *expected {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The selected profile workspace changed before the write began",
                true,
            ));
        }
        let previous = lease.observed_workspace().clone();
        let published = match lease.publish(update.clone()).await {
            ActiveProfileWorkspacePublishOutcome::Published(published) => published,
            ActiveProfileWorkspacePublishOutcome::Unpublished(error) => {
                return Err(Failure::from(error));
            }
            ActiveProfileWorkspacePublishOutcome::Superseded { observed } => {
                return Err(workspace_superseded_failure(observed));
            }
            ActiveProfileWorkspacePublishOutcome::RecoveryRequired(recovery) => {
                resolve_active_workspace_publish(recovery).await?
            }
        };
        let committed = published.published_workspace().clone();
        if workspace_matches_update(&previous, &committed, expected, update) {
            return Ok(published.commit());
        }
        let restored = restore_active_workspace(published).await?;
        if restored != previous {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "active workspace recovery did not restore the exact prior documents",
                false,
            ));
        }
        Err(Failure::new(
            ErrorCode::InvalidState,
            "active workspace readback differed from the submitted documents",
            false,
        ))
    }

    /// Read and validate the active profile's exact typed workspace identity
    /// under the profile-store owner boundary.
    pub async fn load_active_workspace(&self) -> Result<ProfileWorkspace, Failure> {
        let engine = self.store.engine_id();
        let workspace = self
            .store
            .load_active_workspace()
            .await
            .map_err(Failure::from)?;
        validate_workspace(&workspace, &workspace.document.identity.profile_id, &engine)?;
        Ok(workspace)
    }
}

async fn resolve_active_workspace_publish<'a>(
    mut recovery: Box<dyn ActiveProfileWorkspaceRecoveryLease + 'a>,
) -> Result<Box<dyn ActiveProfileWorkspacePublishedLease + 'a>, Failure> {
    loop {
        match recovery.resolve().await {
            ActiveProfileWorkspaceRecoveryResolution::Published(published) => {
                return Ok(published);
            }
            ActiveProfileWorkspaceRecoveryResolution::Restored(restored) => {
                let _ = restored.finish();
                return Err(Failure::new(
                    ErrorCode::NotReady,
                    "active workspace publication was recovered to its previous content",
                    true,
                ));
            }
            ActiveProfileWorkspaceRecoveryResolution::Unpublished(error) => {
                return Err(Failure::from(error));
            }
            ActiveProfileWorkspaceRecoveryResolution::Superseded { observed } => {
                return Err(workspace_superseded_failure(observed));
            }
            ActiveProfileWorkspaceRecoveryResolution::RecoveryRequired(next) => {
                recovery = next;
            }
        }
    }
}

async fn restore_active_workspace<'a>(
    published: Box<dyn ActiveProfileWorkspacePublishedLease + 'a>,
) -> Result<ProfileWorkspace, Failure> {
    let mut restore = published
        .restore_if_current()
        .await
        .map_err(Failure::from)?;
    loop {
        match restore {
            ActiveProfileWorkspaceRestoreOutcome::Restored(restored) => {
                return Ok(restored.finish());
            }
            ActiveProfileWorkspaceRestoreOutcome::Superseded { observed } => {
                return Err(workspace_superseded_failure(observed));
            }
            ActiveProfileWorkspaceRestoreOutcome::RecoveryRequired(recovery) => {
                match recovery.resolve().await {
                    ActiveProfileWorkspaceRecoveryResolution::Restored(restored) => {
                        return Ok(restored.finish());
                    }
                    ActiveProfileWorkspaceRecoveryResolution::Published(published) => {
                        restore = published
                            .restore_if_current()
                            .await
                            .map_err(Failure::from)?;
                    }
                    ActiveProfileWorkspaceRecoveryResolution::Unpublished(error) => {
                        return Err(Failure::from(error));
                    }
                    ActiveProfileWorkspaceRecoveryResolution::Superseded { observed } => {
                        return Err(workspace_superseded_failure(observed));
                    }
                    ActiveProfileWorkspaceRecoveryResolution::RecoveryRequired(next) => {
                        restore = ActiveProfileWorkspaceRestoreOutcome::RecoveryRequired(next);
                    }
                }
            }
        }
    }
}

fn workspace_matches_update(
    previous: &ProfileWorkspace,
    current: &ProfileWorkspace,
    expected: &ProfileWorkspaceIdentity,
    update: &ProfileWorkspaceUpdate,
) -> bool {
    let identity = current.identity();
    current.document.raw_bytes == update.document_bytes
        && current.sidecar_bytes == update.sidecar_bytes
        && identity.document.engine_id == expected.document.engine_id
        && identity.document.profile_id == expected.document.profile_id
        && identity.document.source == expected.document.source
        && identity.document.format_id == expected.document.format_id
        && identity.document.schema_version == expected.document.schema_version
        && (current.document.raw_bytes == previous.document.raw_bytes
            || identity.document.revision != previous.document.identity.revision)
        && (current.sidecar_bytes == previous.sidecar_bytes
            || identity.sidecar_revision != previous.sidecar_revision)
}

fn workspace_superseded_failure(observed: ProfileWorkspaceIdentity) -> Failure {
    Failure::new(
        ErrorCode::NotReady,
        format!("active profile workspace was superseded: {observed:?}"),
        true,
    )
}

fn validate_workspace(
    workspace: &ProfileWorkspace,
    profile: &ProfileId,
    engine: &EngineId,
) -> Result<(), Failure> {
    let workspace_identity = workspace.identity();
    let sidecar_present = matches!(
        workspace_identity.sidecar_revision,
        ProfileSidecarRevision::Present(_)
    );
    if workspace.document.identity.profile_id != *profile
        || workspace.document.identity.engine_id != *engine
        || workspace.sidecar_bytes.is_some() != sidecar_present
    {
        return Err(Failure::new(
            ErrorCode::InvalidState,
            "Workspace identity does not match its engine, profile, or sidecar bytes",
            false,
        ));
    }
    Ok(())
}

fn engine_mismatch(expected: &EngineId, actual: &EngineId) -> Failure {
    Failure::new(
        ErrorCode::Unsupported,
        format!(
            "profile engine `{}` does not match active engine `{}`",
            expected.as_str(),
            actual.as_str()
        ),
        false,
    )
}

fn same_document_namespace(
    expected: &ProfileWorkspaceIdentity,
    actual: &ProfileWorkspaceIdentity,
) -> bool {
    let expected = &expected.document;
    let actual = &actual.document;
    expected.engine_id == actual.engine_id
        && expected.profile_id == actual.profile_id
        && expected.source == actual.source
        && expected.format_id == actual.format_id
        && expected.schema_version == actual.schema_version
}

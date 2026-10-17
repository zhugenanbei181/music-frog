//! Fenced active-workspace transaction and recovery for engine profile stores.

use crate::active_reference_file::ActiveProfileReferenceFile;
use crate::active_reference_owner_coordinator::{
    ActiveProfileOwnerCoordinator, validate_reference_write_authorization,
};
use crate::reset_fence::{
    ensure_no_pending_reset_in_engine_namespaces, initialize_namespace_with_reset_fence,
};
use crate::store::EngineProfileStore;
use crate::workspace::workspace_for_update;
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::profile_store::{
    ActiveProfileWorkspacePublishOutcome, ActiveProfileWorkspacePublishedLease,
    ActiveProfileWorkspaceRecoveryLease, ActiveProfileWorkspaceRecoveryResolution,
    ActiveProfileWorkspaceRecoveryState, ActiveProfileWorkspaceRestoreOutcome,
    ActiveProfileWorkspaceRestoredLease, ActiveProfileWorkspaceWriteLease,
};
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceState, ActiveProfileReferenceWriteAuthorization,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::profile_document_identity::ProfileWorkspaceIdentity;
use lurker_core_ports::error::PortError;
use lurker_profile_store::document_store::{remove_optional, write_document_bytes_durable};
use lurker_profile_store::file_lock::ProfileWriteGuard;
use lurker_profile_store::instance_lock::ThreadedInstanceLockGuard;
use lurker_profile_store::owner_switch::reset::ActiveProfileResetMutationAccess;
use lurker_profile_store::workspace::validate_workspace_identity;
use lurker_profile_store::workspace_store::ProfileWorkspacePublicationTarget;

struct ActiveWorkspaceWriteGuards {
    _root: ThreadedInstanceLockGuard,
    _owner_state: ThreadedInstanceLockGuard,
    _profile: ProfileWriteGuard,
}

struct EngineProfileObservedWorkspaceLease<'a, S: SecureStore> {
    store: &'a EngineProfileStore<S>,
    observed: ProfileWorkspace,
    guards: ActiveWorkspaceWriteGuards,
}

struct EngineProfilePublishedWorkspaceLease<'a, S: SecureStore> {
    store: &'a EngineProfileStore<S>,
    previous: ProfileWorkspace,
    published: ProfileWorkspace,
    guards: ActiveWorkspaceWriteGuards,
}

struct EngineProfileRestoredWorkspaceLease {
    restored: ProfileWorkspace,
    _guards: ActiveWorkspaceWriteGuards,
}

enum ActiveWorkspaceRecoveryPurpose {
    ConfirmPublished { target: ProfileWorkspace },
    RestorePrevious { published: ProfileWorkspace },
}

struct EngineProfileWorkspaceRecoveryLease<'a, S: SecureStore> {
    store: &'a EngineProfileStore<S>,
    previous: ProfileWorkspace,
    purpose: ActiveWorkspaceRecoveryPurpose,
    guards: ActiveWorkspaceWriteGuards,
    last_error: String,
}

impl<S: SecureStore> EngineProfileStore<S> {
    pub async fn compare_and_save_inactive_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        validate_workspace_identity(self.document_codec_scope(), expected)?;
        self.validate_document_bytes(&update.document_bytes)?;
        initialize_namespace_with_reset_fence(self).await?;
        let selection = ActiveProfileReferenceStore::acquire_selection_lease(self).await?;
        let _profile_guard = self.lock_profile_writes().await?;
        self.publish_profile_workspace_locked(
            expected,
            update,
            ProfileWorkspacePublicationTarget::InactiveSelection(selection.state()),
        )
        .await
    }

    pub async fn restore_inactive_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        previous: &ProfileWorkspace,
    ) -> Result<ProfileWorkspace, PortError> {
        self.validate_profile_workspace_recovery(expected, previous)?;
        initialize_namespace_with_reset_fence(self).await?;
        let selection = ActiveProfileReferenceStore::acquire_selection_lease(self).await?;
        let _profile_guard = self.lock_profile_writes().await?;
        self.restore_profile_workspace_locked(
            expected,
            previous,
            ProfileWorkspacePublicationTarget::InactiveSelection(selection.state()),
        )
        .await
    }

    /// Persist one derived active-profile document as a single atomic file
    /// replacement while the reference is selected and no Core owner is
    /// bound. This path deliberately cannot change sidecar bytes or perform a
    /// runtime apply, so caller cancellation leaves either the previous file
    /// or the complete replacement.
    pub async fn compare_and_save_unowned_active_derived_document(
        &self,
        expected_reference: ActiveProfileReference,
        expected_workspace: ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        if expected_reference.engine_id != self.engine_id()
            || expected_workspace.document.engine_id != expected_reference.engine_id
            || expected_workspace.document.profile_id != expected_reference.profile_id
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "derived document identity differs from the active profile reference",
                false,
            )));
        }
        if update.purpose != ProfileWorkspacePurpose::Derived {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidInput,
                "unowned active-document persistence accepts derived updates only",
                false,
            )));
        }
        self.validate_document_bytes(&update.document_bytes)?;

        initialize_namespace_with_reset_fence(self).await?;
        let (config_root, _root_guard) =
            ActiveProfileReferenceFile::new(self.config_root().to_path_buf())
                .acquire_root_lock()
                .await?;
        ensure_no_pending_reset_in_engine_namespaces(
            &config_root,
            ActiveProfileResetMutationAccess::OrdinaryMutation,
        )
        .await?;
        let mut record = ActiveProfileReferenceFile::new(config_root.clone())
            .read_active_profile_record_or_pending()
            .await?;
        let expected_state = ActiveProfileReferenceState::Selected(expected_reference.clone());
        if record.selection != expected_state {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active profile reference changed before derived document persistence",
                true,
            )));
        }
        let (owner_state, _owner_state_guard) =
            ActiveProfileOwnerCoordinator::new(config_root.clone())
                .normalize_owner_state_unlocked(&mut record)
                .await?;
        validate_reference_write_authorization(
            &owner_state,
            &ActiveProfileReferenceWriteAuthorization::Unowned,
            &config_root,
        )?;
        let _profile_guard = self.lock_profile_writes().await?;
        let observed = self
            .read_profile_workspace_locked(&expected_reference.profile_id)
            .await?;
        if observed.identity() != expected_workspace {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active workspace changed before derived document persistence",
                true,
            )));
        }
        if observed.sidecar_bytes != update.sidecar_bytes {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "derived document persistence cannot modify profile sidecar bytes",
                false,
            )));
        }
        if observed.document.raw_bytes == update.document_bytes {
            return Ok(observed);
        }

        let path = self
            .existing_profile_document_path(&expected_reference.profile_id)
            .await?;
        write_document_bytes_durable(&path, &update.document_bytes).await?;
        if let Err(error) = remove_optional(&self.layout().document_backup_path(&path)).await {
            log::warn!("committed derived workspace backup cleanup failed: {error}");
        }
        Ok(workspace_for_update(
            self.document_codec_scope(),
            &observed,
            update,
        ))
    }

    /// Acquire a typed active-workspace transaction lease in root -> owner
    /// state -> profile order. No content bytes are written until the caller
    /// consumes the observed lease through `publish`.
    pub async fn acquire_active_workspace_write_lease<'a>(
        &'a self,
        expected_reference: ActiveProfileReference,
        expected_workspace: ProfileWorkspaceIdentity,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<Box<dyn ActiveProfileWorkspaceWriteLease + 'a>, PortError> {
        if expected_reference.engine_id != self.engine_id()
            || expected_workspace.document.profile_id != expected_reference.profile_id
            || expected_workspace.document.engine_id != expected_reference.engine_id
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active workspace identity differs from its expected reference",
                false,
            )));
        }
        initialize_namespace_with_reset_fence(self).await?;
        let (config_root, root_guard) =
            ActiveProfileReferenceFile::new(self.config_root().to_path_buf())
                .acquire_root_lock()
                .await?;
        ensure_no_pending_reset_in_engine_namespaces(
            &config_root,
            ActiveProfileResetMutationAccess::OrdinaryMutation,
        )
        .await?;
        let mut record = ActiveProfileReferenceFile::new(config_root.clone())
            .read_active_profile_record_sync_or_pending()?;
        let expected_state = ActiveProfileReferenceState::Selected(expected_reference.clone());
        if record.selection != expected_state {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active profile reference changed before workspace publication",
                true,
            )));
        }
        let (owner_state, owner_state_guard) =
            ActiveProfileOwnerCoordinator::new(config_root.clone())
                .normalize_owner_state_unlocked(&mut record)
                .await?;
        validate_reference_write_authorization(&owner_state, &authorization, &config_root)?;
        let profile_guard = self.lock_profile_writes().await?;
        let observed = self
            .read_profile_workspace_locked(&expected_reference.profile_id)
            .await?;
        if observed.identity() != expected_workspace {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active workspace identity changed before transaction lease acquisition",
                true,
            )));
        }
        let guards = ActiveWorkspaceWriteGuards {
            _root: root_guard,
            _owner_state: owner_state_guard,
            _profile: profile_guard,
        };
        Ok(Box::new(EngineProfileObservedWorkspaceLease {
            store: self,
            observed,
            guards,
        }))
    }
}

#[async_trait::async_trait]
impl<S: SecureStore> ActiveProfileWorkspaceWriteLease
    for EngineProfileObservedWorkspaceLease<'_, S>
{
    fn observed_workspace(&self) -> &ProfileWorkspace {
        &self.observed
    }

    async fn publish(
        self: Box<Self>,
        update: ProfileWorkspaceUpdate,
    ) -> ActiveProfileWorkspacePublishOutcome<'async_trait> {
        let EngineProfileObservedWorkspaceLease {
            store,
            observed,
            guards,
        } = *self;
        let target = workspace_for_update(store.document_codec_scope(), &observed, &update);
        let write_result = store
            .publish_profile_workspace_locked(
                &observed.identity(),
                &update,
                ProfileWorkspacePublicationTarget::AuthorizedActive,
            )
            .await;
        let (target, last_error) = match write_result {
            Ok(published) => (
                published,
                "publication returned success; verifying readback".into(),
            ),
            Err(error) if is_definitely_unpublished_workspace_error(&error) => {
                return ActiveProfileWorkspacePublishOutcome::Unpublished(error);
            }
            Err(error) => (target, error.to_string()),
        };
        let recovery = Box::new(EngineProfileWorkspaceRecoveryLease {
            store,
            previous: observed,
            purpose: ActiveWorkspaceRecoveryPurpose::ConfirmPublished { target },
            guards,
            last_error,
        });
        match recovery.resolve().await {
            ActiveProfileWorkspaceRecoveryResolution::Published(published) => {
                ActiveProfileWorkspacePublishOutcome::Published(published)
            }
            ActiveProfileWorkspaceRecoveryResolution::Unpublished(error) => {
                ActiveProfileWorkspacePublishOutcome::Unpublished(error)
            }
            ActiveProfileWorkspaceRecoveryResolution::Superseded { observed } => {
                ActiveProfileWorkspacePublishOutcome::Superseded { observed }
            }
            ActiveProfileWorkspaceRecoveryResolution::RecoveryRequired(recovery) => {
                ActiveProfileWorkspacePublishOutcome::RecoveryRequired(recovery)
            }
            ActiveProfileWorkspaceRecoveryResolution::Restored(restored) => {
                restored.finish();
                ActiveProfileWorkspacePublishOutcome::Unpublished(PortError::Failed(
                    "active workspace publication was restored to its previous bytes".into(),
                ))
            }
        }
    }
}

#[async_trait::async_trait]
impl<S: SecureStore> ActiveProfileWorkspaceRecoveryLease
    for EngineProfileWorkspaceRecoveryLease<'_, S>
{
    fn state(&self) -> ActiveProfileWorkspaceRecoveryState {
        match &self.purpose {
            ActiveWorkspaceRecoveryPurpose::ConfirmPublished { target } => {
                ActiveProfileWorkspaceRecoveryState::ConfirmPublished {
                    previous: self.previous.identity(),
                    target: target.identity(),
                }
            }
            ActiveWorkspaceRecoveryPurpose::RestorePrevious { published } => {
                ActiveProfileWorkspaceRecoveryState::RestorePrevious {
                    previous: self.previous.identity(),
                    published: published.identity(),
                }
            }
        }
    }

    fn last_error(&self) -> &str {
        &self.last_error
    }

    async fn resolve(self: Box<Self>) -> ActiveProfileWorkspaceRecoveryResolution<'async_trait> {
        let EngineProfileWorkspaceRecoveryLease {
            store,
            previous,
            purpose,
            guards,
            last_error,
        } = *self;
        let profile_id = previous.document.identity.profile_id.clone();
        let observed = match store.read_profile_workspace_locked(&profile_id).await {
            Ok(observed) => observed,
            Err(error) => {
                return ActiveProfileWorkspaceRecoveryResolution::RecoveryRequired(Box::new(
                    EngineProfileWorkspaceRecoveryLease {
                        store,
                        previous,
                        purpose,
                        guards,
                        last_error: format!("{last_error}; readback failed: {error}"),
                    },
                ));
            }
        };

        match purpose {
            ActiveWorkspaceRecoveryPurpose::ConfirmPublished { target } => {
                if same_workspace_bytes(&observed, &target) {
                    return ActiveProfileWorkspaceRecoveryResolution::Published(Box::new(
                        EngineProfilePublishedWorkspaceLease {
                            store,
                            previous,
                            published: observed,
                            guards,
                        },
                    ));
                }
                if same_workspace_bytes(&observed, &previous) {
                    return ActiveProfileWorkspaceRecoveryResolution::Unpublished(
                        PortError::Failed(format!(
                            "active workspace publication did not commit: {last_error}"
                        )),
                    );
                }

                let recovery_result = store
                    .restore_profile_workspace_locked(
                        &observed.identity(),
                        &previous,
                        ProfileWorkspacePublicationTarget::AuthorizedActive,
                    )
                    .await;
                let recovered = match store.read_profile_workspace_locked(&profile_id).await {
                    Ok(current) => current,
                    Err(error) => {
                        let detail = match recovery_result {
                            Ok(_) => format!(
                                "{last_error}; conditional rollback returned success but readback failed: {error}"
                            ),
                            Err(rollback_error) => format!(
                                "{last_error}; conditional rollback failed: {rollback_error}; readback failed: {error}"
                            ),
                        };
                        return ActiveProfileWorkspaceRecoveryResolution::RecoveryRequired(
                            Box::new(EngineProfileWorkspaceRecoveryLease {
                                store,
                                previous,
                                purpose: ActiveWorkspaceRecoveryPurpose::ConfirmPublished {
                                    target,
                                },
                                guards,
                                last_error: detail,
                            }),
                        );
                    }
                };
                if same_workspace_bytes(&recovered, &previous) {
                    return ActiveProfileWorkspaceRecoveryResolution::Unpublished(
                        PortError::Failed(format!(
                            "active workspace publication was conditionally rolled back: {last_error}"
                        )),
                    );
                }
                if same_workspace_bytes(&recovered, &target) {
                    return ActiveProfileWorkspaceRecoveryResolution::Published(Box::new(
                        EngineProfilePublishedWorkspaceLease {
                            store,
                            previous,
                            published: recovered,
                            guards,
                        },
                    ));
                }
                ActiveProfileWorkspaceRecoveryResolution::Superseded {
                    observed: recovered.identity(),
                }
            }
            ActiveWorkspaceRecoveryPurpose::RestorePrevious { published } => {
                if same_workspace_bytes(&observed, &previous) {
                    return ActiveProfileWorkspaceRecoveryResolution::Restored(Box::new(
                        EngineProfileRestoredWorkspaceLease {
                            restored: observed,
                            _guards: guards,
                        },
                    ));
                }
                if !same_workspace_bytes(&observed, &published) {
                    return ActiveProfileWorkspaceRecoveryResolution::Superseded {
                        observed: observed.identity(),
                    };
                }

                let recovery_result = store
                    .restore_profile_workspace_locked(
                        &published.identity(),
                        &previous,
                        ProfileWorkspacePublicationTarget::AuthorizedActive,
                    )
                    .await;
                let recovered = match store.read_profile_workspace_locked(&profile_id).await {
                    Ok(current) => current,
                    Err(error) => {
                        let detail = match recovery_result {
                            Ok(_) => format!(
                                "conditional workspace restore returned success but readback failed: {error}"
                            ),
                            Err(rollback_error) => format!(
                                "conditional workspace restore failed: {rollback_error}; readback failed: {error}"
                            ),
                        };
                        return ActiveProfileWorkspaceRecoveryResolution::RecoveryRequired(
                            Box::new(EngineProfileWorkspaceRecoveryLease {
                                store,
                                previous,
                                purpose: ActiveWorkspaceRecoveryPurpose::RestorePrevious {
                                    published,
                                },
                                guards,
                                last_error: detail,
                            }),
                        );
                    }
                };
                if same_workspace_bytes(&recovered, &previous) {
                    return ActiveProfileWorkspaceRecoveryResolution::Restored(Box::new(
                        EngineProfileRestoredWorkspaceLease {
                            restored: recovered,
                            _guards: guards,
                        },
                    ));
                }
                if same_workspace_bytes(&recovered, &published) {
                    let detail = match recovery_result {
                        Ok(_) => format!(
                            "conditional workspace restore did not change published bytes: {last_error}"
                        ),
                        Err(error) => format!("{last_error}; restore failed: {error}"),
                    };
                    return ActiveProfileWorkspaceRecoveryResolution::RecoveryRequired(Box::new(
                        EngineProfileWorkspaceRecoveryLease {
                            store,
                            previous,
                            purpose: ActiveWorkspaceRecoveryPurpose::RestorePrevious { published },
                            guards,
                            last_error: detail,
                        },
                    ));
                }
                ActiveProfileWorkspaceRecoveryResolution::Superseded {
                    observed: recovered.identity(),
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl<S: SecureStore> ActiveProfileWorkspacePublishedLease
    for EngineProfilePublishedWorkspaceLease<'_, S>
{
    fn previous_workspace(&self) -> &ProfileWorkspace {
        &self.previous
    }

    fn published_workspace(&self) -> &ProfileWorkspace {
        &self.published
    }

    fn commit(self: Box<Self>) -> ProfileWorkspace {
        let lease = *self;
        lease.published
    }

    async fn restore_if_current(
        self: Box<Self>,
    ) -> Result<ActiveProfileWorkspaceRestoreOutcome<'async_trait>, PortError> {
        let EngineProfilePublishedWorkspaceLease {
            store,
            previous,
            published,
            guards,
        } = *self;
        let recovery = Box::new(EngineProfileWorkspaceRecoveryLease {
            store,
            previous,
            purpose: ActiveWorkspaceRecoveryPurpose::RestorePrevious { published },
            guards,
            last_error: "restore requested after managed apply failure".into(),
        });
        match recovery.resolve().await {
            ActiveProfileWorkspaceRecoveryResolution::Restored(restored) => {
                Ok(ActiveProfileWorkspaceRestoreOutcome::Restored(restored))
            }
            ActiveProfileWorkspaceRecoveryResolution::Superseded { observed } => {
                Ok(ActiveProfileWorkspaceRestoreOutcome::Superseded { observed })
            }
            ActiveProfileWorkspaceRecoveryResolution::RecoveryRequired(recovery) => Ok(
                ActiveProfileWorkspaceRestoreOutcome::RecoveryRequired(recovery),
            ),
            ActiveProfileWorkspaceRecoveryResolution::Published(published) => {
                published.restore_if_current().await
            }
            ActiveProfileWorkspaceRecoveryResolution::Unpublished(error) => Err(error),
        }
    }
}

#[async_trait::async_trait]
impl ActiveProfileWorkspaceRestoredLease for EngineProfileRestoredWorkspaceLease {
    fn restored_workspace(&self) -> &ProfileWorkspace {
        &self.restored
    }

    fn finish(self: Box<Self>) -> ProfileWorkspace {
        let lease = *self;
        lease.restored
    }
}

fn is_definitely_unpublished_workspace_error(error: &PortError) -> bool {
    match error {
        PortError::Rejected(failure) => {
            failure.code == ErrorCode::NotReady
                || failure.code == ErrorCode::Configuration
                || failure.code == ErrorCode::InvalidInput
        }
        PortError::Unsupported { .. } => true,
        PortError::PermissionDenied(_)
        | PortError::NotFound(_)
        | PortError::Io(_)
        | PortError::Network(_)
        | PortError::Failed(_) => false,
    }
}

fn same_workspace_bytes(left: &ProfileWorkspace, right: &ProfileWorkspace) -> bool {
    left.identity() == right.identity()
        && left.document.raw_bytes == right.document.raw_bytes
        && left.sidecar_bytes == right.sidecar_bytes
        && left.write_protection == right.write_protection
}

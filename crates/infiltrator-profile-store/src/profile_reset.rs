//! Engine-neutral durable profile reset transaction.

use crate::active_reference_file::{ActiveProfileReferenceFile, is_durability_uncertain};
use crate::active_reference_owner::{
    ActiveCoreOwnerState, ActiveProfileReferenceRecord, LegacySelectionMigration,
    transition_rejected,
};
use crate::active_reference_owner_coordinator::{
    ActiveProfileOwnerCoordinator, validate_reference_write_authorization,
};
use crate::document_store::{
    read_optional_bytes, remove_optional_durable, write_document_bytes_durable,
};
use crate::file_lock::lock_profile_writes_for_dir;
use crate::reset_fence::{
    ACTIVE_PROFILE_RESET_COMPLETE_FILE, ACTIVE_PROFILE_RESET_DEFAULT_ABSENT_FILE,
    ACTIVE_PROFILE_RESET_DEFAULT_BACKUP_FILE, ACTIVE_PROFILE_RESET_INTENT_FILE,
    ActiveProfileResetIntentRecord, ActiveProfileResetMutationAccess,
    ParsedActiveProfileResetIntentRecord, ensure_no_pending_reset_in_engine_namespaces,
    initialize_namespace_with_reset_fence, parse_reset_intent_record, reset_pending_for_dir,
};
use crate::store::EngineProfileStore;
use crate::workspace::sha256_hex;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceState, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_platform::instance_lock::ThreadedInstanceLockGuard;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_ports::error::PortError;
use serde_json::to_vec_pretty;
use std::fmt::Display;
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActiveProfileReferenceResetOutcome {
    NoPendingIntent,
    Committed {
        previous: ActiveProfileReferenceState,
        current: ActiveProfileReferenceState,
    },
    Stale {
        expected: ActiveProfileReferenceState,
        observed: ActiveProfileReferenceState,
    },
    CleanupPending {
        previous: ActiveProfileReferenceState,
        current: ActiveProfileReferenceState,
        error: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActiveProfileReferenceResetRequest {
    ResumePendingIntent,
    Start {
        expected: ActiveProfileReferenceState,
        default_document: String,
    },
}

enum ActiveProfileResetAttempt {
    Publish,
    FinishCommittedCleanup,
}

fn lock_error(error: impl Display) -> PortError {
    PortError::Io(error.to_string())
}

async fn reset_default_document_path<S: SecureStore>(
    store: &EngineProfileStore<S>,
) -> Result<PathBuf, PortError> {
    let profile =
        ProfileId::new("default").map_err(|error| PortError::Failed(error.to_string()))?;
    store.profile_document_path(&profile).await
}

fn sync_read_active_profile_record(
    config_root: &Path,
) -> Result<ActiveProfileReferenceRecord, PortError> {
    Ok(ActiveProfileReferenceFile::new(config_root.to_path_buf())
        .read_active_profile_record_sync()?
        .unwrap_or_else(ActiveProfileReferenceRecord::pending_legacy_migration))
}

fn sync_publish_active_profile_record(
    config_root: &Path,
    record: &ActiveProfileReferenceRecord,
) -> Result<(), PortError> {
    ActiveProfileReferenceFile::new(config_root.to_path_buf())
        .publish_active_profile_record_sync_reconciled(record)
}

async fn restore_reset_default(
    default_path: &Path,
    backup_path: &Path,
    absent_marker_path: &Path,
    intent_path: &Path,
    previous_default: Option<&[u8]>,
    write_attempted: bool,
) -> Result<(), PortError> {
    if write_attempted {
        match previous_default {
            Some(bytes) => write_document_bytes_durable(default_path, bytes).await?,
            None => remove_optional_durable(default_path)?,
        }
    }
    remove_optional_durable(backup_path)?;
    remove_optional_durable(absent_marker_path)?;
    remove_optional_durable(intent_path)?;
    Ok(())
}

impl<S: SecureStore> EngineProfileStore<S> {
    /// Reset this engine's profile files and select its default profile as
    /// one unowned reference transaction. The default is written before CAS;
    /// old files remain available until the CAS commits, and cleanup can be
    /// retried safely if the process exits after commit.
    pub async fn reset_profiles_to_default(
        &self,
        request: ActiveProfileReferenceResetRequest,
    ) -> Result<ActiveProfileReferenceResetOutcome, PortError> {
        let (expected, default_document, config_root, root_guard) = match request {
            ActiveProfileReferenceResetRequest::Start {
                expected,
                default_document,
            } => {
                self.validate_document_bytes(default_document.as_bytes())?;
                if read_optional_bytes(&self.config_dir().join(ACTIVE_PROFILE_RESET_INTENT_FILE))
                    .await?
                    .is_none()
                {
                    initialize_namespace_with_reset_fence(self).await?;
                }
                let (config_root, root_guard) =
                    ActiveProfileReferenceFile::new(self.config_root().to_path_buf())
                        .acquire_root_lock()
                        .await?;
                (expected, default_document, config_root, root_guard)
            }
            ActiveProfileReferenceResetRequest::ResumePendingIntent => {
                let (config_root, root_guard) =
                    ActiveProfileReferenceFile::new(self.config_root().to_path_buf())
                        .acquire_root_lock()
                        .await?;
                if !reset_pending_for_dir(self.config_dir()).await? {
                    return Ok(ActiveProfileReferenceResetOutcome::NoPendingIntent);
                }
                let intent_path = self.config_dir().join(ACTIVE_PROFILE_RESET_INTENT_FILE);
                let intent_bytes = read_optional_bytes(&intent_path).await?.ok_or_else(|| {
                    PortError::Failed(
                        "pending profile reset disappeared while its root lock was held".into(),
                    )
                })?;
                let intent = parse_reset_intent_record(&intent_bytes)?;
                let expected = intent.expected_reference().clone();
                let next_revision = expected
                    .revision()
                    .next()
                    .ok_or_else(|| PortError::Failed("active-profile revision exhausted".into()))?;
                let target = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
                    self.engine_id(),
                    ProfileId::new("default")
                        .map_err(|error| PortError::Failed(error.to_string()))?,
                    next_revision,
                ));
                if intent.target_reference() != &target {
                    return Err(transition_rejected(
                        "pending reset intent has an invalid expected-to-default transition",
                    ));
                }
                let default_document = match &intent {
                    ParsedActiveProfileResetIntentRecord::CurrentV2(record) => {
                        record.default_document.clone()
                    }
                    ParsedActiveProfileResetIntentRecord::LegacyV1(record) => {
                        let default_path = reset_default_document_path(self).await?;
                        let backup_path = self
                            .config_dir()
                            .join(ACTIVE_PROFILE_RESET_DEFAULT_BACKUP_FILE);
                        let current = read_optional_bytes(&default_path).await?;
                        let backup = read_optional_bytes(&backup_path).await?;
                        current
                            .as_deref()
                            .filter(|bytes| sha256_hex(bytes) == record.default_document_sha256)
                            .or_else(|| {
                                backup.as_deref().filter(|bytes| {
                                    sha256_hex(bytes) == record.default_document_sha256
                                })
                            })
                            .ok_or_else(|| {
                                PortError::unsupported(
                                    Capability::Profiles,
                                    "legacy reset intent does not contain recoverable default bytes",
                                )
                            })?
                            .to_vec()
                    }
                };
                let default_document = String::from_utf8(default_document).map_err(|error| {
                    PortError::Io(format!(
                        "pending reset default document is not UTF-8: {error}"
                    ))
                })?;
                (expected, default_document, config_root, root_guard)
            }
        };
        self.reset_profiles_to_default_locked(expected, &default_document, config_root, root_guard)
            .await
    }

    async fn reset_profiles_to_default_locked(
        &self,
        expected: ActiveProfileReferenceState,
        default_content: &str,
        config_root: PathBuf,
        _root_guard: ThreadedInstanceLockGuard,
    ) -> Result<ActiveProfileReferenceResetOutcome, PortError> {
        self.validate_document_bytes(default_content.as_bytes())?;
        ensure_no_pending_reset_in_engine_namespaces(
            &config_root,
            ActiveProfileResetMutationAccess::RetryEngine(self.engine_id()),
        )
        .await?;
        let mut record = sync_read_active_profile_record(&config_root)?;
        let revision = expected
            .revision()
            .next()
            .ok_or_else(|| PortError::Failed("active-profile revision exhausted".into()))?;
        let default_id =
            ProfileId::new("default").map_err(|error| PortError::Failed(error.to_string()))?;
        let current = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
            self.engine_id(),
            default_id.clone(),
            revision,
        ));
        let attempt = if record.selection == expected {
            ActiveProfileResetAttempt::Publish
        } else if record.selection == current {
            ActiveProfileResetAttempt::FinishCommittedCleanup
        } else {
            return Ok(ActiveProfileReferenceResetOutcome::Stale {
                expected: expected.clone(),
                observed: record.selection,
            });
        };
        let (owner_state, _owner_state_guard) =
            ActiveProfileOwnerCoordinator::new(config_root.clone())
                .normalize_owner_state_unlocked(&mut record)
                .await?;
        validate_reference_write_authorization(
            &owner_state,
            &ActiveProfileReferenceWriteAuthorization::Unowned,
            &config_root,
        )?;

        let _profile_guard = lock_profile_writes_for_dir(self.config_dir()).await?;
        let default_path = reset_default_document_path(self).await?;
        let backup_path = self
            .config_dir()
            .join(ACTIVE_PROFILE_RESET_DEFAULT_BACKUP_FILE);
        let absent_marker_path = self
            .config_dir()
            .join(ACTIVE_PROFILE_RESET_DEFAULT_ABSENT_FILE);
        let intent_path = self.config_dir().join(ACTIVE_PROFILE_RESET_INTENT_FILE);
        let complete_path = self.config_dir().join(ACTIVE_PROFILE_RESET_COMPLETE_FILE);

        let previous_default = read_optional_bytes(&default_path).await?;
        let mut backup_bytes = read_optional_bytes(&backup_path).await?;
        let mut absent_marker = read_optional_bytes(&absent_marker_path).await?;
        let stored_intent_bytes = read_optional_bytes(&intent_path).await?;
        let complete_bytes = read_optional_bytes(&complete_path).await?;
        if backup_bytes.is_some() && absent_marker.is_some() {
            return Err(transition_rejected(
                "profile reset has conflicting default recovery markers",
            ));
        }
        let mut stored_intent = stored_intent_bytes
            .as_deref()
            .map(parse_reset_intent_record)
            .transpose()?;
        let complete_intent = complete_bytes
            .as_deref()
            .map(parse_reset_intent_record)
            .transpose()?;
        if let Some(ParsedActiveProfileResetIntentRecord::LegacyV1(legacy)) = stored_intent.as_ref()
        {
            let recoverable_default = previous_default
                .as_deref()
                .filter(|bytes| sha256_hex(bytes) == legacy.default_document_sha256)
                .or_else(|| {
                    let bytes = default_content.as_bytes();
                    (sha256_hex(bytes) == legacy.default_document_sha256).then_some(bytes)
                });
            let Some(recoverable_default) = recoverable_default else {
                return Err(PortError::unsupported(
                    Capability::Profiles,
                    "legacy reset intent contains only a digest and its exact default document cannot be recovered",
                ));
            };
            let migrated = ActiveProfileResetIntentRecord::new(
                legacy.expected_reference.clone(),
                legacy.target_reference.clone(),
                recoverable_default,
            );
            let bytes =
                to_vec_pretty(&migrated).map_err(|error| PortError::Io(error.to_string()))?;
            write_document_bytes_durable(&intent_path, &bytes).await?;
            stored_intent = Some(ParsedActiveProfileResetIntentRecord::CurrentV2(migrated));
        }
        let reset_default_document = stored_intent
            .as_ref()
            .and_then(ParsedActiveProfileResetIntentRecord::current_record)
            .filter(|stored| stored.matches_transition(&expected, &current))
            .map(|stored| stored.default_document.as_slice())
            .unwrap_or_else(|| default_content.as_bytes())
            .to_vec();
        self.validate_document_bytes(&reset_default_document)?;
        let desired_intent = ActiveProfileResetIntentRecord::new(
            expected.clone(),
            current.clone(),
            &reset_default_document,
        );
        let desired_intent_state =
            ParsedActiveProfileResetIntentRecord::CurrentV2(desired_intent.clone());
        if let ActiveProfileResetAttempt::FinishCommittedCleanup = attempt {
            if complete_intent.as_ref() == Some(&desired_intent_state)
                && stored_intent
                    .as_ref()
                    .is_none_or(|stored| stored == &desired_intent_state)
            {
                remove_optional_durable(&intent_path)?;
                return Ok(ActiveProfileReferenceResetOutcome::Committed {
                    previous: expected,
                    current,
                });
            }
            if stored_intent.as_ref() != Some(&desired_intent_state) {
                return Ok(ActiveProfileReferenceResetOutcome::Stale {
                    expected,
                    observed: record.selection,
                });
            }
            if previous_default.as_deref() != Some(reset_default_document.as_slice()) {
                write_document_bytes_durable(&default_path, &reset_default_document).await?;
            }
            let workspace = self.read_profile_workspace_locked(&default_id).await?;
            if workspace.document.identity.engine_id != self.engine_id()
                || workspace.document.identity.profile_id != default_id
                || workspace.document.raw_bytes.as_slice() != reset_default_document.as_slice()
            {
                return Err(PortError::Failed(
                    "committed reset target could not be re-established from its durable intent"
                        .into(),
                ));
            }
            let cleanup = self
                .cleanup_profiles_after_reset(
                    &default_path,
                    &intent_path,
                    &backup_path,
                    &absent_marker_path,
                    &complete_path,
                )
                .await;
            return match cleanup {
                Ok(()) => match self
                    .complete_profile_reset(&desired_intent, &intent_path, &complete_path)
                    .await
                {
                    Ok(()) => Ok(ActiveProfileReferenceResetOutcome::Committed {
                        previous: expected,
                        current,
                    }),
                    Err(error) => Ok(ActiveProfileReferenceResetOutcome::CleanupPending {
                        previous: expected,
                        current,
                        error: error.to_string(),
                    }),
                },
                Err(error) => Ok(ActiveProfileReferenceResetOutcome::CleanupPending {
                    previous: expected,
                    current,
                    error: error.to_string(),
                }),
            };
        }
        if let Some(previous_intent) = stored_intent
            .as_ref()
            .filter(|stored| stored.target_reference() == &expected)
            .cloned()
        {
            if complete_intent.as_ref() != Some(&previous_intent) {
                let previous_record = previous_intent.current_record().ok_or_else(|| {
                    PortError::unsupported(
                        Capability::Profiles,
                        "legacy reset intent was not upgraded before cleanup",
                    )
                })?;
                self.cleanup_profiles_after_reset(
                    &default_path,
                    &intent_path,
                    &backup_path,
                    &absent_marker_path,
                    &complete_path,
                )
                .await?;
                self.complete_profile_reset(previous_record, &intent_path, &complete_path)
                    .await?;
            } else {
                remove_optional_durable(&intent_path)?;
            }
            stored_intent = None;
            backup_bytes = None;
            absent_marker = None;
        }
        if stored_intent
            .as_ref()
            .is_some_and(|stored| stored != &desired_intent_state)
        {
            return Err(transition_rejected(
                "profile reset already has an intent for a different source or target",
            ));
        }
        let rollback_default = if let Some(bytes) = &backup_bytes {
            Some(bytes.clone())
        } else if absent_marker.is_some() {
            None
        } else {
            previous_default.clone()
        };
        let mut default_write_attempted = false;
        if previous_default.as_deref() != Some(reset_default_document.as_slice())
            && backup_bytes.is_none()
            && absent_marker.is_none()
        {
            match previous_default.as_deref() {
                Some(bytes) => write_document_bytes_durable(&backup_path, bytes).await?,
                None => write_document_bytes_durable(&absent_marker_path, b"absent").await?,
            }
        }
        if stored_intent.is_none() {
            let bytes =
                to_vec_pretty(&desired_intent).map_err(|error| PortError::Io(error.to_string()))?;
            write_document_bytes_durable(&intent_path, &bytes).await?;
        }
        if previous_default.as_deref() != Some(reset_default_document.as_slice()) {
            default_write_attempted = true;
            if let Err(error) =
                write_document_bytes_durable(&default_path, &reset_default_document).await
            {
                restore_reset_default(
                    &default_path,
                    &backup_path,
                    &absent_marker_path,
                    &intent_path,
                    rollback_default.as_deref(),
                    default_write_attempted,
                )
                .await?;
                return Err(error);
            }
        }

        let workspace = match self.read_profile_workspace_locked(&default_id).await {
            Ok(workspace) => workspace,
            Err(error) => {
                restore_reset_default(
                    &default_path,
                    &backup_path,
                    &absent_marker_path,
                    &intent_path,
                    rollback_default.as_deref(),
                    default_write_attempted,
                )
                .await?;
                return Err(error);
            }
        };
        if workspace.document.identity.engine_id != self.engine_id()
            || workspace.document.identity.profile_id != default_id
            || workspace.document.raw_bytes.as_slice() != reset_default_document.as_slice()
        {
            restore_reset_default(
                &default_path,
                &backup_path,
                &absent_marker_path,
                &intent_path,
                rollback_default.as_deref(),
                default_write_attempted,
            )
            .await?;
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "default workspace identity or bytes changed before reference CAS",
                true,
            )));
        }

        record.selection = current.clone();
        record.legacy_migration = LegacySelectionMigration::ExplicitSelection;
        record.owner = ActiveCoreOwnerState::Unowned;
        let (previous, current) = match sync_publish_active_profile_record(&config_root, &record) {
            Ok(()) => (expected.clone(), current),
            Err(commit_error) => {
                if is_durability_uncertain(&commit_error) {
                    return Err(PortError::Failed(format!(
                        "{commit_error}; reset recovery bytes were retained"
                    )));
                }
                match sync_read_active_profile_record(&config_root) {
                    Ok(observed)
                        if observed.selection == current
                            && matches!(observed.owner, ActiveCoreOwnerState::Unowned) =>
                    {
                        (expected.clone(), current)
                    }
                    Ok(observed)
                        if observed.selection == expected
                            && matches!(observed.owner, ActiveCoreOwnerState::Unowned) =>
                    {
                        restore_reset_default(
                            &default_path,
                            &backup_path,
                            &absent_marker_path,
                            &intent_path,
                            rollback_default.as_deref(),
                            default_write_attempted,
                        )
                        .await?;
                        return Err(commit_error);
                    }
                    Ok(_) | Err(_) => {
                        return Err(PortError::Failed(format!(
                            "active-reference reset outcome is uncertain; retained old profile files and default recovery data ({commit_error})"
                        )));
                    }
                }
            }
        };

        match self
            .cleanup_profiles_after_reset(
                &default_path,
                &intent_path,
                &backup_path,
                &absent_marker_path,
                &complete_path,
            )
            .await
        {
            Ok(()) => {
                if previous != current {
                    if let Err(error) = self
                        .complete_profile_reset(&desired_intent, &intent_path, &complete_path)
                        .await
                    {
                        return Ok(ActiveProfileReferenceResetOutcome::CleanupPending {
                            previous,
                            current,
                            error: error.to_string(),
                        });
                    }
                }
                Ok(ActiveProfileReferenceResetOutcome::Committed { previous, current })
            }
            Err(error) => Ok(ActiveProfileReferenceResetOutcome::CleanupPending {
                previous,
                current,
                error: error.to_string(),
            }),
        }
    }

    async fn cleanup_profiles_after_reset(
        &self,
        default_path: &Path,
        intent_path: &Path,
        backup_path: &Path,
        absent_marker_path: &Path,
        complete_path: &Path,
    ) -> Result<(), PortError> {
        self.reset_engine_scoped_namespace_marker_unlocked().await?;
        let mut entries = fs::read_dir(self.config_dir()).await.map_err(lock_error)?;
        let default_name = default_path
            .file_name()
            .ok_or_else(|| PortError::Failed("default profile path has no filename".into()))?;
        let intent_name = intent_path
            .file_name()
            .ok_or_else(|| PortError::Failed("reset intent path has no filename".into()))?;
        let backup_name = backup_path
            .file_name()
            .ok_or_else(|| PortError::Failed("reset backup path has no filename".into()))?;
        let absent_marker_name = absent_marker_path
            .file_name()
            .ok_or_else(|| PortError::Failed("reset absent marker path has no filename".into()))?;
        let complete_name = complete_path
            .file_name()
            .ok_or_else(|| PortError::Failed("reset completion path has no filename".into()))?;
        while let Some(entry) = entries.next_entry().await.map_err(lock_error)? {
            let name = entry.file_name();
            if name == default_name
                || name == "profile-state.toml"
                || name == intent_name
                || name == backup_name
                || name == absent_marker_name
                || name == complete_name
            {
                continue;
            }
            let path = entry.path();
            if entry.file_type().await.map_err(lock_error)?.is_dir() {
                fs::remove_dir_all(path).await.map_err(lock_error)?;
            } else {
                fs::remove_file(path).await.map_err(lock_error)?;
            }
        }
        remove_optional_durable(backup_path)?;
        remove_optional_durable(absent_marker_path)?;
        Ok(())
    }

    async fn complete_profile_reset(
        &self,
        intent: &ActiveProfileResetIntentRecord,
        intent_path: &Path,
        complete_path: &Path,
    ) -> Result<(), PortError> {
        let bytes = to_vec_pretty(intent).map_err(|error| PortError::Io(error.to_string()))?;
        write_document_bytes_durable(complete_path, &bytes).await?;
        remove_optional_durable(intent_path)
    }
}

//! Product-wide active-profile reference storage and the explicit legacy
//! Mihomo selection migration.

use super::ConfigManager;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceState,
};
#[cfg(test)]
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceRevision, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::capability::Capability;
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_platform::instance_lock::ThreadedInstanceLockGuard;
#[cfg(test)]
use infiltrator_ports::active_profile_reference_store::{
    ActiveProfileOwnerRecoveryEvidence, ActiveProfileOwnerRecoveryOutcome,
    ActiveProfileOwnerSwitchRecovery, ActiveProfileReferenceStore,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
#[cfg(test)]
use infiltrator_profile_store::active_owner_registry::remove_local_owner_registration;
use infiltrator_profile_store::active_reference_file::ActiveProfileReferenceFile;
#[cfg(test)]
use infiltrator_profile_store::active_reference_owner::{
    ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION, ACTIVE_CORE_OWNER_STATE_FILE,
    LEGACY_ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION,
};
#[cfg(test)]
use infiltrator_profile_store::active_reference_owner::{
    ActiveCoreOwnerProof, ActiveCoreOwnerProofRecord,
};
use infiltrator_profile_store::active_reference_owner::{
    ActiveCoreOwnerState, ActiveProfileReferenceRecord, LegacySelectionMigration,
    transition_rejected,
};
use infiltrator_profile_store::active_reference_owner_coordinator::ActiveProfileOwnerCoordinator;
use infiltrator_profile_store::profile_reset::{
    ActiveProfileReferenceResetOutcome, ActiveProfileReferenceResetRequest,
};
#[cfg(test)]
use infiltrator_profile_store::reset_fence::{
    ACTIVE_PROFILE_RESET_COMPLETE_FILE, ACTIVE_PROFILE_RESET_DEFAULT_BACKUP_FILE,
    ACTIVE_PROFILE_RESET_INTENT_FILE, ActiveProfileResetIntentRecord,
    LEGACY_ACTIVE_PROFILE_RESET_INTENT_SCHEMA_VERSION, ParsedActiveProfileResetIntentRecord,
    parse_reset_intent_record,
};
use infiltrator_profile_store::reset_fence::{
    ActiveProfileResetMutationAccess, PROFILE_RESET_PENDING_MUTATION_MESSAGE,
    ensure_no_pending_reset_in_engine_namespaces, reset_pending_for_dir,
};
#[cfg(test)]
use infiltrator_profile_store::workspace::sha256_hex;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;
#[cfg(test)]
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity};
use mihomo_api::error::MihomoError;
use std::fmt::Display;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::result;

// Cross-process lock order is fixed: root reference lock -> owner-state lock
// -> owner admission lock. Terminal owner release takes only owner-state and
// local-registration locks, never the root lock; no path acquires root after
// owner-state.

fn lock_error(error: impl Display) -> PortError {
    PortError::Io(error.to_string())
}

fn profile_namespace_initialization_error(error: MihomoError) -> PortError {
    match error {
        MihomoError::Config(message) if message == PROFILE_RESET_PENDING_MUTATION_MESSAGE => {
            PortError::Rejected(Failure::new(ErrorCode::NotReady, message, true))
        }
        error => lock_error(error),
    }
}

#[cfg(test)]
fn sync_read_active_profile_record(
    config_root: &Path,
) -> Result<ActiveProfileReferenceRecord, PortError> {
    Ok(ActiveProfileReferenceFile::new(config_root)
        .read_active_profile_record_sync()?
        .unwrap_or_else(ActiveProfileReferenceRecord::pending_legacy_migration))
}

#[cfg(test)]
fn sync_read_active_core_owner_proof(
    config_root: &Path,
) -> Result<ActiveCoreOwnerProofRecord, PortError> {
    ActiveProfileReferenceFile::new(config_root).read_owner_proof_sync()
}

impl<S: SecureStore> ConfigManager<S> {
    fn active_profile_reference_file(&self) -> ActiveProfileReferenceFile {
        ActiveProfileReferenceFile::new(self.config_root().to_path_buf())
    }

    async fn acquire_active_profile_reference_lock(
        &self,
    ) -> Result<(PathBuf, ThreadedInstanceLockGuard), PortError> {
        self.active_profile_reference_file()
            .acquire_root_lock()
            .await
    }

    async fn read_active_profile_record_unlocked(
        &self,
    ) -> Result<ActiveProfileReferenceRecord, PortError> {
        let Some(record) = self
            .active_profile_reference_file()
            .read_active_profile_record()
            .await?
        else {
            return Ok(ActiveProfileReferenceRecord::pending_legacy_migration());
        };
        Ok(record)
    }

    async fn publish_active_profile_record_unlocked(
        &self,
        record: &ActiveProfileReferenceRecord,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        self.active_profile_reference_file()
            .publish_active_profile_record(record)
            .await
    }

    /// Explicitly migrate the legacy Mihomo provider choice once. The current
    /// profile is accepted only when its persisted document
    /// identity proves both Mihomo engine and exact profile ID. A rejected
    /// migration is recorded as Unselected and will never silently retry as a
    /// Mihomo fallback.
    pub async fn migrate_legacy_mihomo_selection(
        &self,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        if self.engine_id() != crate::engine_id() {
            return Err(PortError::unsupported(
                Capability::Profiles,
                "legacy desktop selection migration is Mihomo-only",
            ));
        }
        self.initialize_profile_namespace()
            .await
            .map_err(profile_namespace_initialization_error)?;
        let (_config_root, _guard) = self.acquire_active_profile_reference_lock().await?;
        ensure_no_pending_reset_in_engine_namespaces(
            &_config_root,
            ActiveProfileResetMutationAccess::OrdinaryMutation,
        )
        .await?;
        let mut record = self.read_active_profile_record_unlocked().await?;
        let (owner_state, owner_state_guard) =
            ActiveProfileOwnerCoordinator::new(_config_root.clone())
                .normalize_owner_state_unlocked(&mut record)
                .await?;
        if !matches!(owner_state, ActiveCoreOwnerState::Unowned) {
            return Err(transition_rejected(
                "legacy active-profile migration cannot run while a Core owner is bound",
            ));
        }
        drop(owner_state_guard);
        if !matches!(
            record.legacy_migration,
            LegacySelectionMigration::PendingDesktopMihomoV0
        ) {
            return Ok(record.selection);
        }

        let migrated = self.legacy_mihomo_reference().await;
        match migrated {
            Ok((engine_id, profile_id)) => {
                let revision =
                    record.selection.revision().next().ok_or_else(|| {
                        PortError::Failed("active-profile revision exhausted".into())
                    })?;
                record.selection = ActiveProfileReferenceState::Selected(
                    ActiveProfileReference::new(engine_id, profile_id, revision),
                );
                record.legacy_migration = LegacySelectionMigration::MigratedDesktopMihomoV0;
            }
            Err(reason) => {
                let revision =
                    record.selection.revision().next().ok_or_else(|| {
                        PortError::Failed("active-profile revision exhausted".into())
                    })?;
                record.selection = ActiveProfileReferenceState::Unselected { revision };
                record.legacy_migration = LegacySelectionMigration::RejectedDesktopMihomoV0;
                self.publish_active_profile_record_unlocked(&record).await?;
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    format!("legacy Mihomo selection could not be migrated: {reason}"),
                    false,
                )));
            }
        }
        self.publish_active_profile_record_unlocked(&record).await
    }

    async fn legacy_mihomo_reference(&self) -> Result<(EngineId, ProfileId), String> {
        let profile_name = self
            .get_current_unlocked()
            .await
            .map_err(|error| error.to_string())?;
        let profile_id = ProfileId::new(profile_name)
            .map_err(|error| format!("legacy profile ID is invalid: {error}"))?;
        match self
            .existing_profile_document_path(profile_id.as_str())
            .await
        {
            Ok(_) => {}
            Err(MihomoError::NotFound(_)) if profile_id.as_str() == "default" => {
                self.ensure_profile_document(profile_id.as_str())
                    .await
                    .map_err(|error| {
                        format!("new-install Mihomo default profile could not be created: {error}")
                    })?;
            }
            Err(error) => {
                return Err(format!("legacy profile document is unavailable: {error}"));
            }
        }
        let workspace = self
            .read_workspace(&profile_id)
            .await
            .map_err(|error| error.to_string())?;
        let document = workspace.document.identity;
        let engine_id = crate::engine_id();
        if document.engine_id != engine_id || document.profile_id != profile_id {
            return Err(format!(
                "legacy active pointer resolved to {}/{} instead of {}/{}",
                document.engine_id.as_str(),
                document.profile_id.as_str(),
                engine_id.as_str(),
                profile_id.as_str()
            ));
        }
        Ok((engine_id, profile_id))
    }
}

impl<S: SecureStore> ConfigManager<S> {
    pub async fn initialize_profile_namespace_for_reset_recovery(
        &self,
    ) -> result::Result<(), MihomoError> {
        if !self.namespace_is_initialized()
            && self
                .has_pending_profile_reset_for_namespace_init()
                .await
                .map_err(|error| MihomoError::Config(error.to_string()))?
        {
            return Ok(());
        }
        self.initialize_profile_namespace().await
    }

    pub(crate) async fn has_pending_profile_reset_for_namespace_init(
        &self,
    ) -> Result<bool, PortError> {
        let (_config_root, _root_guard) = self.acquire_active_profile_reference_lock().await?;
        reset_pending_for_dir(self.config_dir()).await
    }

    /// Delegate the product reset transaction to the engine-neutral profile owner.
    pub async fn reset_profiles_to_default(
        &self,
        request: ActiveProfileReferenceResetRequest,
    ) -> Result<ActiveProfileReferenceResetOutcome, PortError> {
        self.profile_store.reset_profiles_to_default(request).await
    }
}

#[cfg(test)]
#[path = "active_profile_reference_tests.rs"]
mod tests;

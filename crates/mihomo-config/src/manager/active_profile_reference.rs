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
use infiltrator_contract::core_identity::EngineId;
#[cfg(test)]
use infiltrator_contract::core_identity::{CoreGeneration, CoreIdentity};
use infiltrator_contract::error::{ErrorCode, Failure};
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
use mihomo_api::error::MihomoError;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;

// Cross-process lock order is fixed: root reference lock -> owner-state lock
// -> owner admission lock. Terminal owner release takes only owner-state and
// local-registration locks, never the root lock; no path acquires root after
// owner-state.

fn lock_error(error: impl std::fmt::Display) -> PortError {
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
                infiltrator_contract::capability::Capability::Profiles,
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
    ) -> std::result::Result<(), MihomoError> {
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
mod tests {
    use super::*;
    use infiltrator_contract::core_identity::CoreVersion;
    use infiltrator_contract::core_process::CoreProcessStartContext;
    use infiltrator_contract::session::SessionToken;
    use infiltrator_ports::active_profile_reference_store::ActiveProfileOwnerSwitchState;
    use infiltrator_profile_store::active_reference_owner::ActiveCoreOwnerLaunchProof;
    use std::process::{Child, Command};
    use std::sync::Arc;
    use std::time::Duration;
    use tempfile::TempDir;

    struct TestStore;

    #[async_trait::async_trait]
    impl SecureStore for TestStore {
        async fn get(&self, _namespace: &str, _key: &str) -> Result<Option<String>, PortError> {
            Ok(None)
        }

        async fn set(&self, _namespace: &str, _key: &str, _value: &str) -> Result<(), PortError> {
            Ok(())
        }

        async fn delete(&self, _namespace: &str, _key: &str) -> Result<(), PortError> {
            Ok(())
        }
    }

    #[test]
    fn v1_bound_owner_proof_migrates_as_legacy_unverified() {
        let directory = TempDir::new().unwrap();
        let proof_path = directory.path().join(ACTIVE_CORE_OWNER_STATE_FILE);
        let legacy_identity = identity("mihomo", "legacy-owner");
        let legacy = serde_json::json!({
            "schema_version": LEGACY_ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION,
            "state": {
                "state": "bound",
                "owner": {
                    "identity": legacy_identity,
                    "generation": 1,
                    "token": "legacy-token"
                }
            }
        });
        std::fs::write(&proof_path, serde_json::to_vec(&legacy).unwrap()).unwrap();

        let migrated = sync_read_active_core_owner_proof(directory.path()).unwrap();

        assert_eq!(
            migrated.schema_version,
            ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION
        );
        assert!(matches!(migrated.state, ActiveCoreOwnerProof::Bound { .. }));
        assert_eq!(
            migrated.launch,
            ActiveCoreOwnerLaunchProof::LegacyUnverified
        );
    }

    #[tokio::test]
    async fn managed_start_context_is_persisted_and_advances_by_generation() {
        let directory = TempDir::new().unwrap();
        let manager = manager(&directory);
        manager.migrate_legacy_mihomo_selection().await.unwrap();
        let owner_identity = identity(crate::engine_id().as_str(), "managed-session-owner");
        let owner = manager
            .engine_profile_store()
            .acquire_provider_selection_lease()
            .await
            .unwrap()
            .into_owner_binding(owner_identity.clone(), CoreGeneration::new(1))
            .await
            .unwrap();
        let first = CoreProcessStartContext::managed(
            owner_identity.clone(),
            CoreGeneration::new(2),
            SessionToken::new(42),
        );
        owner.record_start_context(&first).await.unwrap();

        let second_same_generation = CoreProcessStartContext::managed(
            owner_identity.clone(),
            CoreGeneration::new(2),
            SessionToken::new(43),
        );
        assert!(
            owner
                .record_start_context(&second_same_generation)
                .await
                .is_err()
        );
        let next = CoreProcessStartContext::managed(
            owner_identity,
            CoreGeneration::new(3),
            SessionToken::new(43),
        );
        owner.record_start_context(&next).await.unwrap();

        let restarted_store = manager_at(directory.path());
        assert_eq!(
            sync_read_active_core_owner_proof(restarted_store.config_root())
                .unwrap()
                .launch,
            ActiveCoreOwnerLaunchProof::Managed(next)
        );
    }

    fn manager(directory: &TempDir) -> ConfigManager<TestStore> {
        manager_at(directory.path())
    }

    fn manager_at(home: &Path) -> ConfigManager<TestStore> {
        ConfigManager::with_home_and_store(home.to_path_buf(), TestStore)
            .expect("isolated config manager")
    }

    #[tokio::test]
    async fn pending_reset_in_another_engine_blocks_reference_and_sidecar_mutations() {
        let directory = TempDir::new().unwrap();
        let manager = manager(&directory);
        let engines_root = manager.config_root().join("engines");
        let other_engine_dir = engines_root.join("sing-box");
        std::fs::create_dir_all(&other_engine_dir).unwrap();
        let source = ActiveProfileReferenceState::Unselected {
            revision: ActiveProfileReferenceRevision::new(0),
        };
        let target = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
            EngineId::new("sing-box").unwrap(),
            ProfileId::new("default").unwrap(),
            ActiveProfileReferenceRevision::new(1),
        ));
        let intent = ActiveProfileResetIntentRecord::new(source, target, b"mode: direct\n");
        let intent_path = other_engine_dir.join(ACTIVE_PROFILE_RESET_INTENT_FILE);
        std::fs::write(&intent_path, serde_json::to_vec(&intent).unwrap()).unwrap();

        assert!(matches!(
            manager
                .engine_profile_store()
                .compare_and_set(
                    ActiveProfileReferenceRevision::new(0),
                    ActiveProfileReferenceTarget::selected(
                        manager.engine_id(),
                        ProfileId::new("default").unwrap(),
                    ),
                    ActiveProfileReferenceWriteAuthorization::Unowned,
                )
                .await,
            Err(PortError::Rejected(Failure {
                code: ErrorCode::NotReady,
                retryable: true,
                ..
            }))
        ));
        assert!(matches!(
            manager
                .engine_profile_store()
                .acquire_selection_lease()
                .await,
            Err(PortError::Rejected(Failure {
                code: ErrorCode::NotReady,
                retryable: true,
                ..
            }))
        ));
        assert!(matches!(
            crate::profile_option_store::write_options_bytes(
                manager.config_dir(),
                "default",
                Some(b"{}"),
            )
            .await,
            Err(PortError::Rejected(Failure {
                code: ErrorCode::NotReady,
                retryable: true,
                ..
            }))
        ));
        assert!(
            !infiltrator_domain::profile_options::options_path(manager.config_dir(), "default")
                .exists()
        );
    }

    #[tokio::test]
    async fn reset_resume_pending_v2_needs_no_new_default_or_port_discovery() {
        let directory = TempDir::new().unwrap();
        let initial_manager = manager(&directory);
        let expected = initial_manager
            .migrate_legacy_mihomo_selection()
            .await
            .unwrap();
        let current = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
            initial_manager.engine_id(),
            ProfileId::new("default").unwrap(),
            expected.revision().next().unwrap(),
        ));
        let default_path = initial_manager
            .existing_profile_document_path("default")
            .await
            .unwrap();
        let previous_default = tokio::fs::read(&default_path).await.unwrap();
        let persisted_default = b"mode: rule\nmixed-port: 7890\n";
        tokio::fs::write(
            initial_manager
                .config_dir()
                .join(ACTIVE_PROFILE_RESET_DEFAULT_BACKUP_FILE),
            &previous_default,
        )
        .await
        .unwrap();
        let intent = ActiveProfileResetIntentRecord::new(
            expected.clone(),
            current.clone(),
            persisted_default,
        );
        let intent_path = initial_manager
            .config_dir()
            .join(ACTIVE_PROFILE_RESET_INTENT_FILE);
        tokio::fs::write(&intent_path, serde_json::to_vec(&intent).unwrap())
            .await
            .unwrap();

        let restarted = manager(&directory);
        assert!(restarted.initialize_profile_namespace().await.is_err());
        restarted
            .initialize_profile_namespace_for_reset_recovery()
            .await
            .unwrap();
        let outcome = restarted
            .reset_profiles_to_default(ActiveProfileReferenceResetRequest::ResumePendingIntent)
            .await
            .unwrap();

        assert_eq!(
            outcome,
            ActiveProfileReferenceResetOutcome::Committed {
                previous: expected.clone(),
                current: current.clone(),
            }
        );
        assert_eq!(
            tokio::fs::read(default_path).await.unwrap().as_slice(),
            persisted_default
        );
        assert!(!intent_path.exists());
        let restarted_profile_store = restarted.engine_profile_store();
        assert_eq!(
            ActiveProfileReferenceStore::read(restarted_profile_store)
                .await
                .unwrap(),
            current
        );
    }

    #[tokio::test]
    async fn reset_resume_without_intent_allows_normal_typed_start() {
        let directory = TempDir::new().unwrap();
        let manager = manager(&directory);
        let expected = manager.migrate_legacy_mihomo_selection().await.unwrap();
        assert_eq!(
            manager
                .reset_profiles_to_default(ActiveProfileReferenceResetRequest::ResumePendingIntent)
                .await
                .unwrap(),
            ActiveProfileReferenceResetOutcome::NoPendingIntent
        );

        let outcome = manager
            .reset_profiles_to_default(ActiveProfileReferenceResetRequest::Start {
                expected: expected.clone(),
                default_document: "mode: direct\n".to_owned(),
            })
            .await
            .unwrap();
        assert!(matches!(
            outcome,
            ActiveProfileReferenceResetOutcome::Committed { previous, current }
                if previous == expected && current.revision() == previous.revision().next().unwrap()
        ));
    }

    #[tokio::test]
    async fn legacy_reset_intent_upgrades_when_default_file_proves_exact_bytes() {
        let directory = TempDir::new().unwrap();
        let initial_manager = manager(&directory);
        let expected = initial_manager
            .migrate_legacy_mihomo_selection()
            .await
            .unwrap();
        let current = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
            initial_manager.engine_id(),
            ProfileId::new("default").unwrap(),
            expected.revision().next().unwrap(),
        ));
        let default_path = initial_manager
            .existing_profile_document_path("default")
            .await
            .unwrap();
        let previous_default = tokio::fs::read(&default_path).await.unwrap();
        let recovered_default = b"mode: rule\nmixed-port: 7890\n";
        tokio::fs::write(
            initial_manager
                .config_dir()
                .join(ACTIVE_PROFILE_RESET_DEFAULT_BACKUP_FILE),
            previous_default,
        )
        .await
        .unwrap();
        let legacy_intent = serde_json::json!({
            "schema_version": LEGACY_ACTIVE_PROFILE_RESET_INTENT_SCHEMA_VERSION,
            "expected_reference": expected.clone(),
            "target_reference": current.clone(),
            "default_document_sha256": sha256_hex(recovered_default),
        });
        let intent_path = initial_manager
            .config_dir()
            .join(ACTIVE_PROFILE_RESET_INTENT_FILE);
        tokio::fs::write(&intent_path, serde_json::to_vec(&legacy_intent).unwrap())
            .await
            .unwrap();
        tokio::fs::write(&default_path, recovered_default)
            .await
            .unwrap();

        let restarted = manager(&directory);
        restarted
            .initialize_profile_namespace_for_reset_recovery()
            .await
            .unwrap();
        let outcome = restarted
            .reset_profiles_to_default(ActiveProfileReferenceResetRequest::ResumePendingIntent)
            .await
            .unwrap();

        assert_eq!(
            outcome,
            ActiveProfileReferenceResetOutcome::Committed {
                previous: expected,
                current,
            }
        );
        assert_eq!(
            tokio::fs::read(default_path).await.unwrap(),
            recovered_default
        );
        let complete = tokio::fs::read(
            restarted
                .config_dir()
                .join(ACTIVE_PROFILE_RESET_COMPLETE_FILE),
        )
        .await
        .unwrap();
        assert!(matches!(
            parse_reset_intent_record(&complete).unwrap(),
            ParsedActiveProfileResetIntentRecord::CurrentV2(_)
        ));
        assert!(!intent_path.exists());
    }

    #[tokio::test]
    async fn reset_restart_finishes_committed_cleanup_with_changed_default_input() {
        let directory = TempDir::new().unwrap();
        let initial_manager = manager(&directory);
        let expected = initial_manager
            .migrate_legacy_mihomo_selection()
            .await
            .unwrap();
        let current = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
            initial_manager.engine_id(),
            ProfileId::new("default").unwrap(),
            expected.revision().next().unwrap(),
        ));
        let persisted_default = "mode: rule\nmixed-port: 7890\n";
        assert!(matches!(
            initial_manager
                .reset_profiles_to_default(ActiveProfileReferenceResetRequest::Start {
                    expected: expected.clone(),
                    default_document: persisted_default.to_owned(),
                })
                .await
                .unwrap(),
            ActiveProfileReferenceResetOutcome::Committed { .. }
        ));

        let intent = ActiveProfileResetIntentRecord::new(
            expected.clone(),
            current.clone(),
            persisted_default.as_bytes(),
        );
        let intent_path = initial_manager
            .config_dir()
            .join(ACTIVE_PROFILE_RESET_INTENT_FILE);
        let complete_path = initial_manager
            .config_dir()
            .join(ACTIVE_PROFILE_RESET_COMPLETE_FILE);
        tokio::fs::write(&intent_path, serde_json::to_vec(&intent).unwrap())
            .await
            .unwrap();
        tokio::fs::remove_file(&complete_path).await.unwrap();
        let obsolete_path = initial_manager.config_dir().join("obsolete.yaml");
        tokio::fs::write(&obsolete_path, b"mode: direct\n")
            .await
            .unwrap();

        let restarted = manager(&directory);
        assert!(restarted.initialize_profile_namespace().await.is_err());
        restarted
            .initialize_profile_namespace_for_reset_recovery()
            .await
            .unwrap();
        let outcome = restarted
            .reset_profiles_to_default(ActiveProfileReferenceResetRequest::ResumePendingIntent)
            .await
            .unwrap();

        assert_eq!(
            outcome,
            ActiveProfileReferenceResetOutcome::Committed {
                previous: expected,
                current,
            }
        );
        assert_eq!(
            tokio::fs::read(
                restarted
                    .existing_profile_document_path("default")
                    .await
                    .unwrap()
            )
            .await
            .unwrap()
            .as_slice(),
            persisted_default.as_bytes()
        );
        assert!(!obsolete_path.exists());
        assert!(!intent_path.exists());
    }

    #[tokio::test]
    async fn legacy_reset_intent_fails_closed_when_its_default_bytes_are_unrecoverable() {
        let directory = TempDir::new().unwrap();
        let initial_manager = manager(&directory);
        let expected = initial_manager
            .migrate_legacy_mihomo_selection()
            .await
            .unwrap();
        let current = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
            initial_manager.engine_id(),
            ProfileId::new("default").unwrap(),
            expected.revision().next().unwrap(),
        ));
        let previous_default = tokio::fs::read(
            initial_manager
                .existing_profile_document_path("default")
                .await
                .unwrap(),
        )
        .await
        .unwrap();
        let legacy_target_bytes = b"mode: legacy-target\n";
        let intent_value = serde_json::json!({
            "schema_version": LEGACY_ACTIVE_PROFILE_RESET_INTENT_SCHEMA_VERSION,
            "expected_reference": expected.clone(),
            "target_reference": current,
            "default_document_sha256": sha256_hex(legacy_target_bytes),
        });
        let intent_path = initial_manager
            .config_dir()
            .join(ACTIVE_PROFILE_RESET_INTENT_FILE);
        let intent_bytes = serde_json::to_vec(&intent_value).unwrap();
        tokio::fs::write(&intent_path, &intent_bytes).await.unwrap();

        let restarted = manager(&directory);
        restarted
            .initialize_profile_namespace_for_reset_recovery()
            .await
            .unwrap();
        assert!(
            restarted
                .reset_profiles_to_default(ActiveProfileReferenceResetRequest::ResumePendingIntent)
                .await
                .is_err()
        );

        assert_eq!(
            tokio::fs::read(
                restarted
                    .existing_profile_document_path("default")
                    .await
                    .unwrap()
            )
            .await
            .unwrap()
            .as_slice(),
            previous_default.as_slice()
        );
        assert_eq!(tokio::fs::read(intent_path).await.unwrap(), intent_bytes);
        let restarted_profile_store = restarted.engine_profile_store();
        assert_eq!(
            ActiveProfileReferenceStore::read(restarted_profile_store)
                .await
                .unwrap(),
            expected
        );
    }

    fn spawn_test_child(test_name: &str, environment: &[(&str, &str)]) -> Child {
        let mut command = Command::new(std::env::current_exe().expect("test binary path"));
        command.arg(test_name).arg("--nocapture");
        for (name, value) in environment {
            command.env(name, value);
        }
        command.spawn().expect("spawn lock protocol test child")
    }

    async fn wait_for_marker(path: &Path) {
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                if path.exists() {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("timed out waiting for subprocess marker");
    }

    async fn wait_child(mut child: Child) -> std::process::ExitStatus {
        tokio::task::spawn_blocking(move || child.wait())
            .await
            .expect("join lock protocol child waiter")
            .expect("wait for lock protocol child")
    }

    fn identity(engine: &str, instance: &str) -> CoreIdentity {
        CoreIdentity::new(
            EngineId::new(engine).expect("valid engine id"),
            infiltrator_contract::core_identity::CoreInstanceId::new(instance)
                .expect("valid core instance id"),
            None,
        )
    }

    fn stopped_snapshot(
        identity: CoreIdentity,
        generation: u64,
    ) -> infiltrator_contract::snapshot::CoreLifecycleSnapshot {
        infiltrator_contract::snapshot::CoreLifecycleSnapshot {
            identity: Some(identity),
            lifecycle: infiltrator_contract::snapshot::CoreLifecycle::Stopped,
            generation,
            session_token: None,
            revision: generation,
            failure: None,
        }
    }

    #[tokio::test]
    async fn migration_persists_the_exact_legacy_mihomo_profile_once() {
        let directory = TempDir::new().unwrap();
        let config = manager(&directory);
        config
            .save_inactive_profile("default", "mixed-port: 7890\n")
            .await
            .unwrap();
        config
            .save_inactive_profile("legacy-choice", "mixed-port: 7890\n")
            .await
            .unwrap();
        tokio::fs::write(
            config.settings_file(),
            "[default]\nprofile = \"legacy-choice\"\n",
        )
        .await
        .unwrap();

        let migrated = config.migrate_legacy_mihomo_selection().await.unwrap();

        assert!(matches!(
            &migrated,
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == crate::engine_id()
                    && reference.profile_id.as_str() == "legacy-choice"
                    && reference.revision.value() == 1
        ));
        let record = config.read_active_profile_record_unlocked().await.unwrap();
        assert_eq!(
            record.legacy_migration,
            LegacySelectionMigration::MigratedDesktopMihomoV0
        );

        let explicit_clear = config
            .engine_profile_store()
            .compare_and_set(
                migrated.revision(),
                ActiveProfileReferenceTarget::Unselected,
                ActiveProfileReferenceWriteAuthorization::Unowned,
            )
            .await
            .unwrap();
        let local_pointer = config.get_current_unlocked().await.unwrap();
        config.migrate_legacy_mihomo_selection().await.unwrap();

        assert_eq!(
            config.engine_profile_store().read().await.unwrap(),
            explicit_clear
        );
        assert_eq!(config.get_current_unlocked().await.unwrap(), local_pointer);
        assert_eq!(
            config
                .read_active_profile_record_unlocked()
                .await
                .unwrap()
                .legacy_migration,
            LegacySelectionMigration::ExplicitSelection
        );

        let restarted = manager(&directory);
        let after_restart = restarted.migrate_legacy_mihomo_selection().await.unwrap();
        assert_eq!(after_restart, explicit_clear);
        assert_eq!(
            restarted.get_current_unlocked().await.unwrap(),
            local_pointer
        );
    }

    #[tokio::test]
    async fn pending_v0_migration_bootstraps_default_for_a_new_install() {
        let directory = TempDir::new().unwrap();
        let manager = manager(&directory);

        let selected = tokio::time::timeout(
            Duration::from_secs(3),
            manager.migrate_legacy_mihomo_selection(),
        )
        .await
        .expect("new-install migration must not reacquire its held reference lock")
        .unwrap();

        assert!(matches!(
            &selected,
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == crate::engine_id()
                    && reference.profile_id.as_str() == "default"
                    && reference.revision.value() == 1
        ));
        let workspace = manager
            .read_workspace(&ProfileId::new("default").unwrap())
            .await
            .unwrap();
        assert_eq!(workspace.document.identity.engine_id, crate::engine_id());
        assert_eq!(workspace.document.identity.profile_id.as_str(), "default");
        assert_eq!(
            manager.engine_profile_store().read().await.unwrap(),
            selected
        );
    }

    #[tokio::test]
    async fn active_reference_cas_updates_only_the_global_reference() {
        let directory = TempDir::new().unwrap();
        let manager = manager(&directory);
        manager
            .save_inactive_profile("default", "mixed-port: 7890\n")
            .await
            .unwrap();
        manager
            .save_inactive_profile("other", "mixed-port: 7891\n")
            .await
            .unwrap();
        let initial = manager.migrate_legacy_mihomo_selection().await.unwrap();
        let legacy_pointer = manager.get_current_unlocked().await.unwrap();

        let selected = manager
            .engine_profile_store()
            .compare_and_set(
                initial.revision(),
                ActiveProfileReferenceTarget::selected(
                    crate::engine_id(),
                    ProfileId::new("other").unwrap(),
                ),
                ActiveProfileReferenceWriteAuthorization::Unowned,
            )
            .await
            .unwrap();

        assert!(matches!(
            &selected,
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == crate::engine_id()
                    && reference.profile_id.as_str() == "other"
                    && reference.revision.value() == 2
        ));
        assert_eq!(
            manager.get_current_unlocked().await.unwrap(),
            legacy_pointer
        );
        assert_eq!(
            manager.migrate_legacy_mihomo_selection().await.unwrap(),
            selected
        );
    }

    #[tokio::test]
    async fn selection_lease_blocks_cas_until_candidate_owner_is_bound() {
        let directory = TempDir::new().unwrap();
        let manager = Arc::new(manager(&directory));
        let selected = manager.migrate_legacy_mihomo_selection().await.unwrap();
        let lease = manager
            .engine_profile_store()
            .acquire_selection_lease()
            .await
            .unwrap();
        assert_eq!(lease.state(), &selected);

        let writer = Arc::clone(&manager);
        let expected = selected.revision();
        let pending = tokio::spawn(async move {
            writer
                .engine_profile_store()
                .compare_and_set(
                    expected,
                    ActiveProfileReferenceTarget::Unselected,
                    ActiveProfileReferenceWriteAuthorization::Unowned,
                )
                .await
        });
        tokio::task::yield_now().await;
        assert!(!pending.is_finished());

        drop(lease);
        let cleared = pending.await.unwrap().unwrap();
        assert_eq!(cleared.revision().value(), 2);
    }

    #[tokio::test]
    async fn owner_binding_allows_same_engine_profile_updates_but_fences_provider_switches() {
        let directory = TempDir::new().unwrap();
        let manager = Arc::new(manager(&directory));
        let selected = manager.migrate_legacy_mihomo_selection().await.unwrap();

        let lease = manager
            .engine_profile_store()
            .acquire_provider_selection_lease()
            .await
            .unwrap();
        let identity = CoreIdentity::new(
            crate::engine_id(),
            infiltrator_contract::core_identity::CoreInstanceId::new("mihomo-owner").unwrap(),
            None,
        );
        let owner_identity = identity.clone();
        let owner = lease
            .into_owner_binding(identity, CoreGeneration::new(1))
            .await
            .unwrap();

        let same_engine_update = manager
            .engine_profile_store()
            .compare_and_set(
                selected.revision(),
                ActiveProfileReferenceTarget::selected(
                    crate::engine_id(),
                    ProfileId::new("default").unwrap(),
                ),
                ActiveProfileReferenceWriteAuthorization::BoundOwner {
                    identity: owner_identity.clone(),
                    generation: CoreGeneration::new(1),
                },
            )
            .await
            .unwrap();
        assert_eq!(same_engine_update.revision().value(), 2);

        let foreign_engine = EngineId::new("sing-box").unwrap();
        let denied = manager
            .engine_profile_store()
            .compare_and_set(
                same_engine_update.revision(),
                ActiveProfileReferenceTarget::selected(
                    foreign_engine,
                    ProfileId::new("default").unwrap(),
                ),
                ActiveProfileReferenceWriteAuthorization::BoundOwner {
                    identity: owner_identity.clone(),
                    generation: CoreGeneration::new(1),
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(denied, PortError::Rejected(_)));

        match manager
            .engine_profile_store()
            .acquire_provider_selection_lease()
            .await
        {
            Err(PortError::Rejected(_)) => {}
            Ok(_) => panic!("a second provider lease bypassed the active owner"),
            Err(other) => panic!("unexpected provider lease failure: {other}"),
        }

        let terminal = infiltrator_contract::snapshot::CoreLifecycleSnapshot {
            identity: Some(owner_identity),
            lifecycle: infiltrator_contract::snapshot::CoreLifecycle::Stopped,
            generation: 1,
            session_token: None,
            revision: 1,
            failure: None,
        };
        owner.release_after_terminal(&terminal).await.unwrap();
        owner.release_after_terminal(&terminal).await.unwrap();
        drop(owner);
        let next_provider = manager
            .engine_profile_store()
            .acquire_provider_selection_lease()
            .await
            .unwrap();
        assert_eq!(next_provider.state(), &same_engine_update);
    }

    #[tokio::test]
    async fn persisted_owner_release_and_recovery_ignore_runtime_version_changes() {
        let directory = TempDir::new().unwrap();
        let manager = manager(&directory);
        let selected = manager.migrate_legacy_mihomo_selection().await.unwrap();
        let bound_identity = identity(crate::engine_id().as_str(), "version-refresh-owner");
        let owner = manager
            .engine_profile_store()
            .acquire_provider_selection_lease()
            .await
            .unwrap()
            .into_owner_binding(bound_identity.clone(), CoreGeneration::new(1))
            .await
            .unwrap();
        let session_context = CoreProcessStartContext::managed(
            bound_identity.clone(),
            CoreGeneration::new(2),
            SessionToken::new(52),
        );
        owner.record_start_context(&session_context).await.unwrap();

        let mut ready_identity = bound_identity.clone();
        ready_identity.version = Some(CoreVersion::new("1.19.12").unwrap());
        let ready_terminal = stopped_snapshot(ready_identity.clone(), 2);
        owner.release_after_terminal(&ready_terminal).await.unwrap();

        // A later observation can refresh capability evidence again while
        // the persisted owner proof remains bound to the same engine instance.
        ready_identity.version = Some(CoreVersion::new("1.19.13").unwrap());
        let refreshed_terminal = stopped_snapshot(ready_identity, 2);
        owner
            .release_after_terminal(&refreshed_terminal)
            .await
            .unwrap();
        drop(owner);

        assert_eq!(
            manager
                .engine_profile_store()
                .recover_bound_owner_after_terminal(
                    ActiveProfileOwnerRecoveryEvidence::ManagedChildExited {
                        context: session_context,
                        terminal: refreshed_terminal,
                    },
                )
                .await
                .unwrap(),
            ActiveProfileOwnerRecoveryOutcome::Recovered(selected.clone())
        );
        let provider_lease = manager
            .engine_profile_store()
            .acquire_provider_selection_lease()
            .await
            .unwrap();
        assert_eq!(provider_lease.state(), &selected);
    }

    #[tokio::test]
    async fn never_started_terminal_evidence_is_a_noop_without_a_persisted_owner() {
        let directory = TempDir::new().unwrap();
        let manager = manager(&directory);

        let outcome = manager
            .engine_profile_store()
            .recover_bound_owner_after_terminal(ActiveProfileOwnerRecoveryEvidence::NeverStarted {
                terminal: stopped_snapshot(identity("mihomo", "android-active"), 0),
            })
            .await
            .unwrap();

        assert_eq!(
            outcome,
            ActiveProfileOwnerRecoveryOutcome::NoRecoveryRequired
        );
    }

    #[tokio::test]
    async fn terminal_release_never_waits_for_the_root_selection_lock() {
        let directory = TempDir::new().unwrap();
        let manager = Arc::new(manager(&directory));
        let selected = manager.migrate_legacy_mihomo_selection().await.unwrap();
        let old_identity = identity(crate::engine_id().as_str(), "close-cas-owner");
        let owner = manager
            .engine_profile_store()
            .acquire_provider_selection_lease()
            .await
            .unwrap()
            .into_owner_binding(old_identity.clone(), CoreGeneration::new(1))
            .await
            .unwrap();
        let terminal = stopped_snapshot(old_identity, 1);

        // A normal selector may hold the root reference lock. Terminal
        // release takes only owner-state, so CoreApplication::close cannot
        // deadlock behind that selector's lease.
        let selection_lease = manager
            .engine_profile_store()
            .acquire_selection_lease()
            .await
            .unwrap();
        let release = tokio::spawn(async move { owner.release_after_terminal(&terminal).await });
        tokio::time::timeout(Duration::from_secs(3), release)
            .await
            .expect("terminal release waited for root selection lock")
            .expect("terminal release task panicked")
            .unwrap();

        let writer = Arc::clone(&manager);
        let pending = tokio::spawn(async move {
            writer
                .engine_profile_store()
                .compare_and_set(
                    selected.revision(),
                    ActiveProfileReferenceTarget::selected(
                        EngineId::new("sing-box").unwrap(),
                        ProfileId::new("default").unwrap(),
                    ),
                    ActiveProfileReferenceWriteAuthorization::Unowned,
                )
                .await
        });
        tokio::task::yield_now().await;
        assert!(!pending.is_finished());
        drop(selection_lease);
        let changed = pending.await.unwrap().unwrap();
        assert_eq!(changed.revision().value(), 2);
    }

    #[tokio::test]
    async fn owner_switch_requires_terminal_then_cas_and_candidate_binding() {
        let directory = TempDir::new().unwrap();
        let manager = manager(&directory);
        let selected = manager.migrate_legacy_mihomo_selection().await.unwrap();
        let old_identity = identity(crate::engine_id().as_str(), "old-owner");
        let owner = manager
            .engine_profile_store()
            .acquire_provider_selection_lease()
            .await
            .unwrap()
            .into_owner_binding(old_identity.clone(), CoreGeneration::new(1))
            .await
            .unwrap();
        let switch = manager
            .engine_profile_store()
            .begin_owner_switch(
                selected.revision(),
                old_identity.clone(),
                CoreGeneration::new(1),
            )
            .await
            .unwrap();
        assert!(matches!(
            switch.state(),
            ActiveProfileOwnerSwitchState::ClosingOld { .. }
        ));

        let blocked_cas = manager
            .engine_profile_store()
            .compare_and_set(
                selected.revision(),
                ActiveProfileReferenceTarget::selected(
                    EngineId::new("sing-box").unwrap(),
                    ProfileId::new("default").unwrap(),
                ),
                ActiveProfileReferenceWriteAuthorization::Unowned,
            )
            .await;
        assert!(matches!(blocked_cas, Err(PortError::Rejected(_))));
        assert!(matches!(
            manager
                .engine_profile_store()
                .acquire_provider_selection_lease()
                .await,
            Err(PortError::Rejected(_))
        ));

        let old_terminal = stopped_snapshot(old_identity.clone(), 1);
        owner.release_after_terminal(&old_terminal).await.unwrap();
        let recovery = manager
            .engine_profile_store()
            .recover_owner_switch()
            .await
            .unwrap();
        let old_stopped = match recovery {
            ActiveProfileOwnerSwitchRecovery::ResumeOldStopped(lease) => lease,
            _ => panic!("expected resumable OldStopped stage"),
        };
        assert!(matches!(
            old_stopped.state(),
            ActiveProfileOwnerSwitchState::OldStopped {
                terminal_generation,
                ..
            } if *terminal_generation == CoreGeneration::new(1)
        ));
        let target_engine = EngineId::new("sing-box").unwrap();
        let changed = old_stopped
            .compare_and_set_target(ActiveProfileReferenceTarget::selected(
                target_engine.clone(),
                ProfileId::new("default").unwrap(),
            ))
            .await
            .unwrap();
        drop(changed);
        let changed = match manager
            .engine_profile_store()
            .recover_owner_switch()
            .await
            .unwrap()
        {
            ActiveProfileOwnerSwitchRecovery::ResumeReferenceChanged(lease) => lease,
            _ => panic!("expected resumable ReferenceChanged stage"),
        };
        let candidate = changed
            .reserve_new_owner(target_engine.clone())
            .await
            .unwrap();
        let candidate_identity = identity(target_engine.as_str(), "new-owner");
        let started_candidate_receipt = stopped_snapshot(candidate_identity.clone(), 1);
        assert!(
            candidate
                .discard_unstarted_candidate(&started_candidate_receipt)
                .await
                .is_err()
        );
        let candidate = match manager
            .engine_profile_store()
            .recover_owner_switch()
            .await
            .unwrap()
        {
            ActiveProfileOwnerSwitchRecovery::ResumeBindingNew(lease) => lease,
            _ => panic!("expected resumable BindingNew stage"),
        };
        let new_owner = candidate
            .finalize(candidate_identity.clone(), CoreGeneration::new(0))
            .await
            .unwrap();
        let committed = manager.engine_profile_store().read().await.unwrap();
        assert!(matches!(
            &committed,
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == target_engine
                    && reference.profile_id.as_str() == "default"
                    && reference.revision.value() == 2
        ));
        new_owner
            .release_after_terminal(&stopped_snapshot(candidate_identity, 0))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn crashed_closing_old_requires_explicit_terminal_evidence_and_can_resume() {
        let directory = TempDir::new().unwrap();
        let manager = manager(&directory);
        let selected = manager.migrate_legacy_mihomo_selection().await.unwrap();
        let old_identity = identity(crate::engine_id().as_str(), "crashed-old-owner");
        let mut observed_identity = old_identity.clone();
        observed_identity.version = Some(CoreVersion::new("1.19.12").unwrap());
        let owner = manager
            .engine_profile_store()
            .acquire_provider_selection_lease()
            .await
            .unwrap()
            .into_owner_binding(old_identity.clone(), CoreGeneration::new(1))
            .await
            .unwrap();
        let switch = manager
            .engine_profile_store()
            .begin_owner_switch(
                selected.revision(),
                observed_identity.clone(),
                CoreGeneration::new(1),
            )
            .await
            .unwrap();
        drop(switch);

        // Simulate process death: release the kernel owner guard without
        // publishing terminal state. Recovery must ask the platform seam.
        let config_root = tokio::fs::canonicalize(manager.config_root())
            .await
            .unwrap();
        let record = sync_read_active_profile_record(&config_root).unwrap();
        let owner_token = match record.owner {
            ActiveCoreOwnerState::Switching { owner_token, .. } => owner_token,
            other => panic!("expected durable switch reservation, got {other:?}"),
        };
        remove_local_owner_registration(&config_root, &owner_token).unwrap();
        drop(owner);
        assert!(matches!(
            manager.engine_profile_store().recover_owner_switch().await.unwrap(),
            ActiveProfileOwnerSwitchRecovery::OwnerTerminalEvidenceRequired {
                identity,
                bound_generation,
            } if identity == old_identity && bound_generation == CoreGeneration::new(1)
        ));

        let old_terminal = stopped_snapshot(observed_identity, 2);
        let old_stopped = manager
            .engine_profile_store()
            .recover_old_owner_after_terminal(&old_terminal)
            .await
            .unwrap();
        assert!(matches!(
            old_stopped.state(),
            ActiveProfileOwnerSwitchState::OldStopped {
                terminal_generation,
                ..
            } if *terminal_generation == CoreGeneration::new(2)
        ));
        let target_engine = EngineId::new("sing-box").unwrap();
        let changed = old_stopped
            .compare_and_set_target(ActiveProfileReferenceTarget::selected(
                target_engine.clone(),
                ProfileId::new("after-crash").unwrap(),
            ))
            .await
            .unwrap();
        let candidate = changed
            .reserve_new_owner(target_engine.clone())
            .await
            .unwrap();
        let selected_after_abort = candidate.abort_before_core_application().await.unwrap();
        assert!(matches!(
            &selected_after_abort,
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == target_engine
                    && reference.profile_id.as_str() == "after-crash"
                    && reference.revision.value() == 2
        ));
        assert!(matches!(
            manager
                .engine_profile_store()
                .recover_owner_switch()
                .await
                .unwrap(),
            ActiveProfileOwnerSwitchRecovery::NoPendingSwitch
        ));
    }

    #[tokio::test]
    async fn subprocess_compare_and_set_entrypoint() {
        let Ok(home) = std::env::var("MUSIC_FROG_ACTIVE_REFERENCE_CAS_HOME") else {
            return;
        };
        let ready = std::env::var("MUSIC_FROG_ACTIVE_REFERENCE_CAS_READY").unwrap();
        let gate = std::env::var("MUSIC_FROG_ACTIVE_REFERENCE_CAS_GATE").unwrap();
        let result_path = std::env::var("MUSIC_FROG_ACTIVE_REFERENCE_CAS_RESULT").unwrap();
        let engine = std::env::var("MUSIC_FROG_ACTIVE_REFERENCE_CAS_ENGINE").unwrap();
        std::fs::write(ready, "ready").unwrap();
        wait_for_marker(Path::new(&gate)).await;
        let manager = manager_at(Path::new(&home));
        let result = manager
            .engine_profile_store()
            .compare_and_set(
                ActiveProfileReferenceRevision::new(0),
                ActiveProfileReferenceTarget::selected(
                    EngineId::new(engine).unwrap(),
                    ProfileId::new("default").unwrap(),
                ),
                ActiveProfileReferenceWriteAuthorization::Unowned,
            )
            .await;
        let outcome = match result {
            Ok(state) => format!("committed:{}", state.revision().value()),
            Err(PortError::Rejected(_)) => "stale".to_owned(),
            Err(error) => format!("error:{error}"),
        };
        std::fs::write(result_path, outcome).unwrap();
    }

    #[tokio::test]
    async fn compare_and_set_has_one_cross_process_revision_winner() {
        let directory = TempDir::new().unwrap();
        let home = directory.path().to_string_lossy().into_owned();
        let gate = directory.path().join("cas-start");
        let ready_a = directory.path().join("cas-ready-a");
        let ready_b = directory.path().join("cas-ready-b");
        let result_a = directory.path().join("cas-result-a");
        let result_b = directory.path().join("cas-result-b");
        let ready_a_text = ready_a.to_string_lossy().into_owned();
        let ready_b_text = ready_b.to_string_lossy().into_owned();
        let gate_text = gate.to_string_lossy().into_owned();
        let result_a_text = result_a.to_string_lossy().into_owned();
        let result_b_text = result_b.to_string_lossy().into_owned();
        let child_a = spawn_test_child(
            "subprocess_compare_and_set_entrypoint",
            &[
                ("MUSIC_FROG_ACTIVE_REFERENCE_CAS_HOME", &home),
                ("MUSIC_FROG_ACTIVE_REFERENCE_CAS_READY", &ready_a_text),
                ("MUSIC_FROG_ACTIVE_REFERENCE_CAS_GATE", &gate_text),
                ("MUSIC_FROG_ACTIVE_REFERENCE_CAS_RESULT", &result_a_text),
                ("MUSIC_FROG_ACTIVE_REFERENCE_CAS_ENGINE", "engine-a"),
            ],
        );
        let child_b = spawn_test_child(
            "subprocess_compare_and_set_entrypoint",
            &[
                ("MUSIC_FROG_ACTIVE_REFERENCE_CAS_HOME", &home),
                ("MUSIC_FROG_ACTIVE_REFERENCE_CAS_READY", &ready_b_text),
                ("MUSIC_FROG_ACTIVE_REFERENCE_CAS_GATE", &gate_text),
                ("MUSIC_FROG_ACTIVE_REFERENCE_CAS_RESULT", &result_b_text),
                ("MUSIC_FROG_ACTIVE_REFERENCE_CAS_ENGINE", "engine-b"),
            ],
        );
        wait_for_marker(&ready_a).await;
        wait_for_marker(&ready_b).await;
        std::fs::write(&gate, "go").unwrap();
        assert!(wait_child(child_a).await.success());
        assert!(wait_child(child_b).await.success());
        let results = [
            std::fs::read_to_string(result_a).unwrap(),
            std::fs::read_to_string(result_b).unwrap(),
        ];
        assert_eq!(
            results
                .iter()
                .filter(|result| result.starts_with("committed:1"))
                .count(),
            1
        );
        assert_eq!(
            results.iter().filter(|result| *result == "stale").count(),
            1
        );
        assert!(!results.iter().any(|result| result.starts_with("error:")));
        assert_eq!(
            manager_at(directory.path())
                .engine_profile_store()
                .read()
                .await
                .unwrap()
                .revision()
                .value(),
            1
        );
    }

    #[tokio::test]
    async fn subprocess_owner_lock_fences_cas_and_process_exit_releases_it() {
        let directory = TempDir::new().unwrap();
        let home = directory.path().to_string_lossy().into_owned();
        let ready = directory.path().join("owner-ready");
        let release = directory.path().join("owner-release");
        let ready_text = ready.to_string_lossy().into_owned();
        let release_text = release.to_string_lossy().into_owned();
        let child = spawn_test_child(
            "subprocess_owner_fence_entrypoint",
            &[
                ("MUSIC_FROG_OWNER_FENCE_HOME", &home),
                ("MUSIC_FROG_OWNER_FENCE_READY", &ready_text),
                ("MUSIC_FROG_OWNER_FENCE_RELEASE", &release_text),
            ],
        );
        wait_for_marker(&ready).await;
        let manager = manager_at(directory.path());
        let foreign_engine = EngineId::new("engine-after-crash").unwrap();
        let owner_context = CoreProcessStartContext::managed(
            identity("engine-live", "live-owner"),
            CoreGeneration::new(2),
            SessionToken::new(42),
        );
        assert!(matches!(
            manager
                .engine_profile_store()
                .compare_and_set(
                    ActiveProfileReferenceRevision::new(1),
                    ActiveProfileReferenceTarget::selected(
                        foreign_engine.clone(),
                        ProfileId::new("default").unwrap(),
                    ),
                    ActiveProfileReferenceWriteAuthorization::Unowned,
                )
                .await,
            Err(PortError::Rejected(_))
        ));
        assert!(matches!(
            manager
                .engine_profile_store()
                .acquire_provider_selection_lease()
                .await,
            Err(PortError::Rejected(_))
        ));
        assert!(
            manager
                .engine_profile_store()
                .recover_bound_owner_after_terminal(
                    ActiveProfileOwnerRecoveryEvidence::ManagedChildExited {
                        context: owner_context.clone(),
                        terminal: stopped_snapshot(identity("engine-live", "live-owner"), 2),
                    },
                )
                .await
                .is_err()
        );
        std::fs::write(&release, "exit").unwrap();
        assert!(wait_child(child).await.success());

        // The kernel lock disappearing proves process exit, not that a
        // supervised Core child reached Stopped. Keep the persisted Bound
        // proof fenced until the platform supplies exact terminal evidence.
        assert!(matches!(
            manager
                .engine_profile_store()
                .compare_and_set(
                    ActiveProfileReferenceRevision::new(1),
                    ActiveProfileReferenceTarget::selected(
                        foreign_engine.clone(),
                        ProfileId::new("default").unwrap(),
                    ),
                    ActiveProfileReferenceWriteAuthorization::Unowned,
                )
                .await,
            Err(PortError::Rejected(_))
        ));
        let wrong_session = CoreProcessStartContext::managed(
            identity("engine-live", "live-owner"),
            CoreGeneration::new(2),
            SessionToken::new(43),
        );
        assert!(
            manager
                .engine_profile_store()
                .recover_bound_owner_after_terminal(
                    ActiveProfileOwnerRecoveryEvidence::ManagedChildExited {
                        context: wrong_session,
                        terminal: stopped_snapshot(identity("engine-live", "live-owner"), 2),
                    },
                )
                .await
                .is_err()
        );
        let wrong_generation = CoreProcessStartContext::managed(
            identity("engine-live", "live-owner"),
            CoreGeneration::new(3),
            SessionToken::new(42),
        );
        assert!(
            manager
                .engine_profile_store()
                .recover_bound_owner_after_terminal(
                    ActiveProfileOwnerRecoveryEvidence::ManagedChildExited {
                        context: wrong_generation,
                        terminal: stopped_snapshot(identity("engine-live", "live-owner"), 3),
                    },
                )
                .await
                .is_err()
        );
        let owner_terminal = stopped_snapshot(identity("engine-live", "live-owner"), 2);
        assert!(matches!(
            manager
                .engine_profile_store()
                .recover_bound_owner_after_terminal(
                    ActiveProfileOwnerRecoveryEvidence::ManagedChildExited {
                        context: owner_context,
                        terminal: owner_terminal,
                    },
                )
                .await
                .unwrap(),
            ActiveProfileOwnerRecoveryOutcome::Recovered(_)
        ));

        let after_exit = manager
            .engine_profile_store()
            .compare_and_set(
                ActiveProfileReferenceRevision::new(1),
                ActiveProfileReferenceTarget::selected(
                    foreign_engine,
                    ProfileId::new("default").unwrap(),
                ),
                ActiveProfileReferenceWriteAuthorization::Unowned,
            )
            .await
            .unwrap();
        assert_eq!(after_exit.revision().value(), 2);
    }

    #[tokio::test]
    async fn subprocess_owner_fence_entrypoint() {
        let Ok(home) = std::env::var("MUSIC_FROG_OWNER_FENCE_HOME") else {
            return;
        };
        let ready = std::env::var("MUSIC_FROG_OWNER_FENCE_READY").unwrap();
        let release = std::env::var("MUSIC_FROG_OWNER_FENCE_RELEASE").unwrap();
        let manager = manager_at(Path::new(&home));
        let engine = EngineId::new("engine-live").unwrap();
        let selected = manager
            .engine_profile_store()
            .compare_and_set(
                ActiveProfileReferenceRevision::new(0),
                ActiveProfileReferenceTarget::selected(
                    engine.clone(),
                    ProfileId::new("default").unwrap(),
                ),
                ActiveProfileReferenceWriteAuthorization::Unowned,
            )
            .await
            .unwrap();
        let owner = manager
            .engine_profile_store()
            .acquire_provider_selection_lease()
            .await
            .unwrap()
            .into_owner_binding(
                identity(engine.as_str(), "live-owner"),
                CoreGeneration::new(1),
            )
            .await
            .unwrap();
        let owner_context = CoreProcessStartContext::managed(
            identity(engine.as_str(), "live-owner"),
            CoreGeneration::new(2),
            SessionToken::new(42),
        );
        owner.record_start_context(&owner_context).await.unwrap();
        assert_eq!(selected.revision().value(), 1);
        std::fs::write(ready, "owner-bound").unwrap();
        wait_for_marker(Path::new(&release)).await;
        drop(owner);
        // No terminal release: process exit must release only the kernel
        // fence, and the next store instance must prove the stale binding.
    }
}

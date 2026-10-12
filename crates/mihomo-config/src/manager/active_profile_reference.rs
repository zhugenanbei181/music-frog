//! Product-wide active-profile reference storage and the explicit legacy
//! Mihomo selection migration.

use super::{
    ActiveProfileReferenceResetOutcome, ActiveProfileReferenceResetRequest, ConfigManager,
};
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
    ActiveProfileReferenceTarget, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::core_identity::{CoreGeneration, CoreIdentity, EngineId};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::{ProfileId, ProfileWorkspaceIdentity};
use infiltrator_platform::instance_lock::ThreadedInstanceLockGuard;
use infiltrator_ports::active_profile_reference_store::{
    ActiveProfileOwnerRecoveryEvidence, ActiveProfileOwnerRecoveryOutcome,
    ActiveProfileOwnerStoppedSwitchLease, ActiveProfileOwnerSwitchLease,
    ActiveProfileOwnerSwitchRecovery, ActiveProfileOwnerSwitchState, ActiveProfileReferenceLease,
    ActiveProfileReferenceStore,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::{
    ActiveProfileWorkspacePublishOutcome, ActiveProfileWorkspacePublishedLease,
    ActiveProfileWorkspaceRecoveryLease, ActiveProfileWorkspaceRecoveryResolution,
    ActiveProfileWorkspaceRecoveryState, ActiveProfileWorkspaceRestoreOutcome,
    ActiveProfileWorkspaceRestoredLease, ActiveProfileWorkspaceWriteLease, ProfileStore,
};
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use infiltrator_ports::secure_store::SecureStore;
#[cfg(test)]
use infiltrator_profile_store::active_owner_registry::remove_local_owner_registration;
use infiltrator_profile_store::active_owner_registry::{
    LocalOwnerRegistrationState, LocalSwitchRegistrationRecovery,
    local_owner_registration_matches as local_registration_matches, local_owner_registration_state,
    lock_local_owner_registry,
};
use infiltrator_profile_store::active_owner_switch::{
    ActiveOwnerStoppedSwitchLease, ActiveOwnerSwitchLease, OwnerBindingLease,
    ReferenceChangedLease, recover_local_switch_registration, terminal_proof_generation,
};
use infiltrator_profile_store::active_profile_selection_lease::ActiveProfileSelectionLease;
use infiltrator_profile_store::active_reference_cas::ActiveProfileReferenceTargetEvidence;
use infiltrator_profile_store::active_reference_file::{
    ActiveProfileReferenceFile, is_durability_uncertain,
};
#[cfg(test)]
use infiltrator_profile_store::active_reference_owner::{
    ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION, ACTIVE_CORE_OWNER_STATE_FILE,
    LEGACY_ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION,
};
use infiltrator_profile_store::active_reference_owner::{
    ActiveCoreOwnerProof, ActiveCoreOwnerProofRecord, ActiveCoreOwnerState, ActiveOwnerSwitchStage,
    ActiveOwnerToken, ActiveProfileReferenceRecord, LegacySelectionMigration, transition_rejected,
};
use infiltrator_profile_store::active_reference_owner_coordinator::{
    ActiveProfileOwnerCoordinator, validate_reference_write_authorization,
    validate_terminal_for_owner,
};
use infiltrator_profile_store::document_store::{
    read_optional_bytes, remove_optional, remove_optional_durable, write_document_bytes_durable,
};
use infiltrator_profile_store::file_lock::{ProfileWriteGuard, lock_profile_writes_for_dir};
use infiltrator_profile_store::profile_files::profile_document_backup_path;
use infiltrator_profile_store::workspace::{sha256_hex, workspace_for_update};
use mihomo_api::error::MihomoError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const ACTIVE_PROFILE_RESET_INTENT_SCHEMA_VERSION: u32 = 2;
const LEGACY_ACTIVE_PROFILE_RESET_INTENT_SCHEMA_VERSION: u32 = 1;
const ACTIVE_PROFILE_RESET_DEFAULT_BACKUP_FILE: &str = ".active-profile-reset-default.previous";
const ACTIVE_PROFILE_RESET_DEFAULT_ABSENT_FILE: &str = ".active-profile-reset-default.was-absent";
const ACTIVE_PROFILE_RESET_INTENT_FILE: &str = ".active-profile-reset-intent.json";
const ACTIVE_PROFILE_RESET_COMPLETE_FILE: &str = ".active-profile-reset-complete.json";
const PROFILE_RESET_PENDING_MUTATION_MESSAGE: &str =
    "a profile reset is awaiting durable cleanup; retry that reset before other profile mutations";
// Cross-process lock order is fixed: root reference lock -> owner-state lock
// -> owner admission lock. Terminal owner release takes only owner-state and
// local-registration locks, never the root lock; no path acquires root after
// owner-state. These kernel locks fence only processes using this protocol.

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
struct ActiveProfileResetIntentRecord {
    schema_version: u32,
    expected_reference: ActiveProfileReferenceState,
    target_reference: ActiveProfileReferenceState,
    default_document_sha256: String,
    default_document: Vec<u8>,
}

impl ActiveProfileResetIntentRecord {
    fn new(
        expected_reference: ActiveProfileReferenceState,
        target_reference: ActiveProfileReferenceState,
        default_content: &[u8],
    ) -> Self {
        Self {
            schema_version: ACTIVE_PROFILE_RESET_INTENT_SCHEMA_VERSION,
            expected_reference,
            target_reference,
            default_document_sha256: sha256_hex(default_content),
            default_document: default_content.to_vec(),
        }
    }

    fn validate(&self) -> Result<(), PortError> {
        if self.schema_version != ACTIVE_PROFILE_RESET_INTENT_SCHEMA_VERSION {
            return Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Profiles,
                format!(
                    "active profile reset intent schema {} is unsupported",
                    self.schema_version
                ),
            ));
        }
        if self.default_document_sha256.len() != 64
            || !self
                .default_document_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(PortError::Io(
                "active profile reset intent has an invalid default digest".into(),
            ));
        }
        if self.default_document_sha256 != sha256_hex(&self.default_document) {
            return Err(PortError::Io(
                "active profile reset intent default bytes do not match their digest".into(),
            ));
        }
        Ok(())
    }

    fn matches_transition(
        &self,
        expected: &ActiveProfileReferenceState,
        target: &ActiveProfileReferenceState,
    ) -> bool {
        &self.expected_reference == expected && &self.target_reference == target
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LegacyActiveProfileResetIntentRecord {
    expected_reference: ActiveProfileReferenceState,
    target_reference: ActiveProfileReferenceState,
    default_document_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ParsedActiveProfileResetIntentRecord {
    LegacyV1(LegacyActiveProfileResetIntentRecord),
    CurrentV2(ActiveProfileResetIntentRecord),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActiveProfileResetIntentEnvelope {
    schema_version: u32,
    expected_reference: ActiveProfileReferenceState,
    target_reference: ActiveProfileReferenceState,
    default_document_sha256: String,
    default_document: Option<Vec<u8>>,
}

impl ParsedActiveProfileResetIntentRecord {
    fn expected_reference(&self) -> &ActiveProfileReferenceState {
        match self {
            Self::LegacyV1(record) => &record.expected_reference,
            Self::CurrentV2(record) => &record.expected_reference,
        }
    }

    fn target_reference(&self) -> &ActiveProfileReferenceState {
        match self {
            Self::LegacyV1(record) => &record.target_reference,
            Self::CurrentV2(record) => &record.target_reference,
        }
    }

    fn current_record(&self) -> Option<&ActiveProfileResetIntentRecord> {
        match self {
            Self::LegacyV1(_) => None,
            Self::CurrentV2(record) => Some(record),
        }
    }
}

/// Lock bundle owned by a multi-step active-workspace transaction. Managed
/// consumers keep this bundle in an owned continuation until commit or
/// conditional restoration is complete; process shutdown releases the OS
/// locks while the owner proof remains bound unless exact terminal release
/// already committed.
struct ActiveWorkspaceWriteGuards {
    _root: ThreadedInstanceLockGuard,
    _owner_state: ThreadedInstanceLockGuard,
    _profile: ProfileWriteGuard,
}

struct ConfigManagerObservedWorkspaceLease<'a, S: SecureStore> {
    manager: &'a ConfigManager<S>,
    observed: ProfileWorkspace,
    guards: ActiveWorkspaceWriteGuards,
}

struct ConfigManagerPublishedWorkspaceLease<'a, S: SecureStore> {
    manager: &'a ConfigManager<S>,
    previous: ProfileWorkspace,
    published: ProfileWorkspace,
    guards: ActiveWorkspaceWriteGuards,
}

struct ConfigManagerRestoredWorkspaceLease {
    restored: ProfileWorkspace,
    _guards: ActiveWorkspaceWriteGuards,
}

enum ActiveWorkspaceRecoveryPurpose {
    ConfirmPublished { target: ProfileWorkspace },
    RestorePrevious { published: ProfileWorkspace },
}

enum ActiveProfileResetAttempt {
    Publish,
    FinishCommittedCleanup,
}

enum ActiveProfileResetMutationAccess {
    OrdinaryMutation,
    RetryEngine(EngineId),
}

/// Holds the product-wide reset fence before the engine-scoped profile lock.
/// Direct sidecar/metadata writers use this when they do not own a
/// ConfigManager reference lease themselves.
pub(crate) struct ResetFencedProfileMutationGuard {
    _root: ThreadedInstanceLockGuard,
    _profile: ProfileWriteGuard,
}

struct ConfigManagerWorkspaceRecoveryLease<'a, S: SecureStore> {
    manager: &'a ConfigManager<S>,
    previous: ProfileWorkspace,
    purpose: ActiveWorkspaceRecoveryPurpose,
    guards: ActiveWorkspaceWriteGuards,
    last_error: String,
}

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

fn parse_reset_intent_record(
    bytes: &[u8],
) -> Result<ParsedActiveProfileResetIntentRecord, PortError> {
    let record: ActiveProfileResetIntentEnvelope = serde_json::from_slice(bytes)
        .map_err(|error| PortError::Io(format!("invalid profile reset intent record: {error}")))?;
    if record.default_document_sha256.len() != 64
        || !record
            .default_document_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(PortError::Io(
            "active profile reset intent has an invalid default digest".into(),
        ));
    }
    match (record.schema_version, record.default_document) {
        (LEGACY_ACTIVE_PROFILE_RESET_INTENT_SCHEMA_VERSION, None) => Ok(
            ParsedActiveProfileResetIntentRecord::LegacyV1(LegacyActiveProfileResetIntentRecord {
                expected_reference: record.expected_reference,
                target_reference: record.target_reference,
                default_document_sha256: record.default_document_sha256,
            }),
        ),
        (ACTIVE_PROFILE_RESET_INTENT_SCHEMA_VERSION, Some(default_document)) => {
            let current = ActiveProfileResetIntentRecord {
                schema_version: record.schema_version,
                expected_reference: record.expected_reference,
                target_reference: record.target_reference,
                default_document_sha256: record.default_document_sha256,
                default_document,
            };
            current.validate()?;
            Ok(ParsedActiveProfileResetIntentRecord::CurrentV2(current))
        }
        (schema_version, _) => Err(PortError::unsupported(
            infiltrator_contract::capability::Capability::Profiles,
            format!("active profile reset intent schema {schema_version} is unsupported"),
        )),
    }
}

pub(super) async fn ensure_reset_not_pending_for_dir(config_dir: &Path) -> Result<(), PortError> {
    if reset_pending_for_dir(config_dir).await? {
        return Err(PortError::Rejected(Failure::new(
            ErrorCode::NotReady,
            PROFILE_RESET_PENDING_MUTATION_MESSAGE,
            true,
        )));
    }
    Ok(())
}

async fn reset_pending_for_dir(config_dir: &Path) -> Result<bool, PortError> {
    let intent_path = config_dir.join(ACTIVE_PROFILE_RESET_INTENT_FILE);
    let Some(intent_bytes) = read_optional_bytes(&intent_path).await? else {
        return Ok(false);
    };
    let intent = parse_reset_intent_record(&intent_bytes)?;
    let complete_path = config_dir.join(ACTIVE_PROFILE_RESET_COMPLETE_FILE);
    let complete = read_optional_bytes(&complete_path).await?;
    if complete
        .as_deref()
        .map(parse_reset_intent_record)
        .transpose()?
        .as_ref()
        == Some(&intent)
    {
        return Ok(false);
    }
    Ok(true)
}

async fn ensure_no_pending_reset_in_engine_namespaces(
    config_root: &Path,
    access: ActiveProfileResetMutationAccess,
) -> Result<(), PortError> {
    let engines_root = config_root.join("engines");
    let mut entries = match tokio::fs::read_dir(&engines_root).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(lock_error(error)),
    };
    while let Some(entry) = entries.next_entry().await.map_err(lock_error)? {
        let file_type = entry.file_type().await.map_err(lock_error)?;
        if file_type.is_symlink() {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "engine namespace scan encountered a symlink and cannot prove reset state",
                true,
            )));
        }
        if !file_type.is_dir() {
            continue;
        }
        let engine_name = entry.file_name();
        let skip_local_retry_engine = match &access {
            ActiveProfileResetMutationAccess::OrdinaryMutation => false,
            ActiveProfileResetMutationAccess::RetryEngine(engine_id) => {
                engine_name == engine_id.as_str()
            }
        };
        if skip_local_retry_engine {
            continue;
        }
        ensure_reset_not_pending_for_dir(&entry.path()).await?;
    }
    Ok(())
}

/// Acquire the existing root reference OS lock, inspect every engine reset
/// intent, then acquire this engine's profile-write boundary. This preserves
/// root -> profile order and closes the race between reset intent publication
/// and ordinary profile mutations.
pub(crate) async fn acquire_reset_fenced_profile_mutation_for_dir(
    config_dir: &Path,
) -> Result<ResetFencedProfileMutationGuard, PortError> {
    let engines_root = config_dir
        .parent()
        .ok_or_else(|| PortError::Io("engine config directory has no parent".into()))?;
    if engines_root.file_name().and_then(|name| name.to_str()) != Some("engines") {
        return Err(PortError::Io(
            "engine config directory is outside the shared engines namespace".into(),
        ));
    }
    let config_root = engines_root
        .parent()
        .ok_or_else(|| PortError::Io("engine config namespace has no shared config root".into()))?;
    let (config_root, root) = ActiveProfileReferenceFile::new(config_root.to_path_buf())
        .acquire_root_lock()
        .await?;
    ensure_no_pending_reset_in_engine_namespaces(
        &config_root,
        ActiveProfileResetMutationAccess::OrdinaryMutation,
    )
    .await?;
    let profile = lock_profile_writes_for_dir(config_dir)
        .await
        .map_err(lock_error)?;
    Ok(ResetFencedProfileMutationGuard {
        _root: root,
        _profile: profile,
    })
}

fn sync_read_active_profile_record(
    config_root: &Path,
) -> Result<ActiveProfileReferenceRecord, PortError> {
    Ok(ActiveProfileReferenceFile::new(config_root)
        .read_active_profile_record_sync()?
        .unwrap_or_else(ActiveProfileReferenceRecord::pending_legacy_migration))
}

fn sync_read_active_core_owner_proof(
    config_root: &Path,
) -> Result<ActiveCoreOwnerProofRecord, PortError> {
    ActiveProfileReferenceFile::new(config_root).read_owner_proof_sync()
}

fn sync_publish_active_core_owner_proof(
    config_root: &Path,
    record: &ActiveCoreOwnerProofRecord,
) -> Result<(), PortError> {
    ActiveProfileReferenceFile::new(config_root).publish_owner_proof_sync_reconciled(record)
}

fn sync_publish_active_profile_record(
    config_root: &Path,
    record: &ActiveProfileReferenceRecord,
) -> Result<(), PortError> {
    ActiveProfileReferenceFile::new(config_root)
        .publish_active_profile_record_sync_reconciled(record)
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

    async fn compare_and_set_record_unlocked(
        &self,
        record: &mut ActiveProfileReferenceRecord,
        expected_revision: ActiveProfileReferenceRevision,
        target: ActiveProfileReferenceTarget,
        authorization: ActiveProfileReferenceWriteAuthorization,
        config_root: &Path,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        ensure_no_pending_reset_in_engine_namespaces(
            config_root,
            ActiveProfileResetMutationAccess::OrdinaryMutation,
        )
        .await?;
        ActiveProfileOwnerCoordinator::new(config_root.to_path_buf())
            .compare_and_set_reference_unlocked(
                record,
                expected_revision,
                target,
                authorization,
                self.engine_id(),
                |target, local_engine| async move {
                    match target {
                        ActiveProfileReferenceTarget::Unselected => {
                            Ok(ActiveProfileReferenceTargetEvidence::Unselected)
                        }
                        ActiveProfileReferenceTarget::Selected {
                            engine_id,
                            profile_id,
                        } if engine_id == local_engine => {
                            let workspace = ProfileStore::load_workspace(self, &profile_id).await?;
                            Ok(ActiveProfileReferenceTargetEvidence::LocalWorkspace(
                                workspace.identity(),
                            ))
                        }
                        ActiveProfileReferenceTarget::Selected { .. } => {
                            Ok(ActiveProfileReferenceTargetEvidence::OtherEngine)
                        }
                    }
                },
            )
            .await
    }
}

#[async_trait::async_trait]
impl<S: SecureStore> ActiveProfileReferenceStore for ConfigManager<S> {
    async fn read(&self) -> Result<ActiveProfileReferenceState, PortError> {
        Ok(self.read_active_profile_record_unlocked().await?.selection)
    }

    async fn acquire_selection_lease(
        &self,
    ) -> Result<Box<dyn ActiveProfileReferenceLease>, PortError> {
        self.initialize_profile_namespace()
            .await
            .map_err(profile_namespace_initialization_error)?;
        let (config_root, guard) = self.acquire_active_profile_reference_lock().await?;
        ensure_no_pending_reset_in_engine_namespaces(
            &config_root,
            ActiveProfileResetMutationAccess::OrdinaryMutation,
        )
        .await?;
        let mut record = self.read_active_profile_record_unlocked().await?;
        let (_owner_state, _owner_state_guard) =
            ActiveProfileOwnerCoordinator::new(config_root.clone())
                .normalize_owner_state_unlocked(&mut record)
                .await?;
        Ok(Box::new(ActiveProfileSelectionLease::new(
            record.selection,
            config_root,
            guard,
        )))
    }

    async fn acquire_provider_selection_lease(
        &self,
    ) -> Result<Box<dyn ActiveProfileReferenceLease>, PortError> {
        self.initialize_profile_namespace()
            .await
            .map_err(profile_namespace_initialization_error)?;
        let (config_root, guard) = self.acquire_active_profile_reference_lock().await?;
        ensure_no_pending_reset_in_engine_namespaces(
            &config_root,
            ActiveProfileResetMutationAccess::OrdinaryMutation,
        )
        .await?;
        let mut record = self.read_active_profile_record_unlocked().await?;
        let (owner_state, _owner_state_guard) =
            ActiveProfileOwnerCoordinator::new(config_root.clone())
                .normalize_owner_state_unlocked(&mut record)
                .await?;
        if let ActiveCoreOwnerState::Bound {
            identity,
            generation,
            ..
        } = owner_state
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                format!(
                    "active Core {} at generation {} must close before another provider is composed",
                    identity.instance.as_str(),
                    generation.value()
                ),
                true,
            )));
        }
        Ok(Box::new(ActiveProfileSelectionLease::new(
            record.selection,
            config_root,
            guard,
        )))
    }

    async fn recover_bound_owner_after_terminal(
        &self,
        evidence: ActiveProfileOwnerRecoveryEvidence,
    ) -> Result<ActiveProfileOwnerRecoveryOutcome, PortError> {
        ActiveProfileOwnerCoordinator::new(self.config_root().to_path_buf())
            .recover_bound_owner_after_terminal(evidence)
            .await
    }

    async fn compare_and_set(
        &self,
        expected_revision: ActiveProfileReferenceRevision,
        target: ActiveProfileReferenceTarget,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        self.initialize_profile_namespace()
            .await
            .map_err(profile_namespace_initialization_error)?;
        let (config_root, _guard) = self.acquire_active_profile_reference_lock().await?;
        let mut record = self.read_active_profile_record_unlocked().await?;
        self.compare_and_set_record_unlocked(
            &mut record,
            expected_revision,
            target,
            authorization,
            &config_root,
        )
        .await
    }

    async fn begin_owner_switch(
        &self,
        expected_revision: ActiveProfileReferenceRevision,
        expected_owner: CoreIdentity,
        expected_binding_generation: CoreGeneration,
    ) -> Result<Box<dyn ActiveProfileOwnerSwitchLease>, PortError> {
        let (config_root, _guard) = self.acquire_active_profile_reference_lock().await?;
        ensure_no_pending_reset_in_engine_namespaces(
            &config_root,
            ActiveProfileResetMutationAccess::OrdinaryMutation,
        )
        .await?;
        let mut record = self.read_active_profile_record_unlocked().await?;
        // Multi-lock order is always root reference lock, then owner-state
        // lock. Terminal release takes only owner-state and never waits for
        // the root lock.
        let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        if record.selection.revision() != expected_revision {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active profile reference revision is stale",
                true,
            )));
        }
        let (owner_identity, owner_generation, owner_token) = match &record.owner {
            ActiveCoreOwnerState::Bound {
                identity,
                generation,
                token,
            } if identity.same_instance(&expected_owner)
                && *generation == expected_binding_generation =>
            {
                (identity.clone(), *generation, token.clone())
            }
            ActiveCoreOwnerState::Bound {
                identity,
                generation,
                ..
            } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    format!(
                        "active Core owner {}/{} at generation {} does not match switch request",
                        identity.engine.as_str(),
                        identity.instance.as_str(),
                        generation.value()
                    ),
                    true,
                )));
            }
            ActiveCoreOwnerState::Unowned => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "owner switching requires a currently bound Core owner",
                    true,
                )));
            }
            ActiveCoreOwnerState::Switching { .. } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "an active Core owner switch is already in progress",
                    true,
                )));
            }
        };
        let owner_probe =
            ActiveProfileReferenceFile::new(config_root.clone()).try_acquire_owner_lock()?;
        if owner_probe.is_some() {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "persisted owner binding has no live kernel owner lock",
                true,
            )));
        }
        let proof = sync_read_active_core_owner_proof(&config_root)?;
        if !matches!(
            proof.state,
            ActiveCoreOwnerProof::Bound {
                ref identity,
                generation,
                ref token,
            } if identity.same_instance(&owner_identity)
                && generation == owner_generation
                && token == &owner_token
        ) {
            return Err(transition_rejected(
                "persisted owner proof does not match the active owner binding",
            ));
        }
        if !local_registration_matches(
            &config_root,
            &owner_token,
            &LocalOwnerRegistrationState::Bound {
                identity: owner_identity.clone(),
                generation: owner_generation,
            },
        )? {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "only the process holding the old Core owner can reserve its switch",
                true,
            )));
        }
        let transition_id = ActiveOwnerToken::generate()?;
        let previous_reference = record.selection.clone();
        record.owner = ActiveCoreOwnerState::Switching {
            transition_id: transition_id.clone(),
            previous_reference: previous_reference.clone(),
            owner_identity: owner_identity.clone(),
            owner_generation,
            owner_token: owner_token.clone(),
            stage: ActiveOwnerSwitchStage::ClosingOld,
        };
        sync_publish_active_profile_record(&config_root, &record)?;
        {
            let mut registrations = lock_local_owner_registry()?;
            registrations.begin_switching(
                &config_root,
                &owner_token,
                transition_id.clone(),
                owner_identity.clone(),
                owner_generation,
            )?;
        }
        let state = ActiveProfileOwnerSwitchState::ClosingOld {
            previous_reference,
            owner_identity,
            owner_generation,
        };
        Ok(Box::new(ActiveOwnerSwitchLease::new(
            config_root,
            transition_id,
            expected_owner,
            expected_binding_generation,
            state,
        )))
    }

    async fn recover_owner_switch(&self) -> Result<ActiveProfileOwnerSwitchRecovery, PortError> {
        let (config_root, _root_guard) = self.acquire_active_profile_reference_lock().await?;
        let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        let mut record = sync_read_active_profile_record(&config_root)?;
        let (
            transition_id,
            previous_reference,
            owner_identity,
            owner_generation,
            owner_token,
            stage,
        ) = match &record.owner {
            ActiveCoreOwnerState::Switching {
                transition_id,
                previous_reference,
                owner_identity,
                owner_generation,
                owner_token,
                stage,
            } => (
                transition_id.clone(),
                previous_reference.clone(),
                owner_identity.clone(),
                *owner_generation,
                owner_token.clone(),
                stage.clone(),
            ),
            ActiveCoreOwnerState::Unowned | ActiveCoreOwnerState::Bound { .. } => {
                return Ok(ActiveProfileOwnerSwitchRecovery::NoPendingSwitch);
            }
        };
        match stage {
            ActiveOwnerSwitchStage::ClosingOld => {
                match ActiveProfileReferenceFile::new(config_root.clone())
                    .try_acquire_owner_lock()?
                {
                    None => {
                        let local_recovery = recover_local_switch_registration(
                            &config_root,
                            &owner_token,
                            &transition_id,
                            &owner_identity,
                            owner_generation,
                        )?;
                        if !matches!(
                            local_recovery,
                            LocalSwitchRegistrationRecovery::NotHeldByThisOwner
                        ) {
                            let state = ActiveProfileOwnerSwitchState::ClosingOld {
                                previous_reference,
                                owner_identity: owner_identity.clone(),
                                owner_generation,
                            };
                            return Ok(ActiveProfileOwnerSwitchRecovery::ResumeClosingOld(
                                Box::new(ActiveOwnerSwitchLease::new(
                                    config_root,
                                    transition_id,
                                    owner_identity,
                                    owner_generation,
                                    state,
                                )),
                            ));
                        }
                        Ok(ActiveProfileOwnerSwitchRecovery::OwnerStillLive {
                            identity: owner_identity,
                            generation: owner_generation,
                        })
                    }
                    Some(owner_probe) => {
                        drop(owner_probe);
                        let proof = sync_read_active_core_owner_proof(&config_root)?;
                        match terminal_proof_generation(
                            &proof,
                            &owner_identity,
                            owner_generation,
                            &owner_token,
                        ) {
                            Ok(terminal_generation) => {
                                record.owner = ActiveCoreOwnerState::Switching {
                                    transition_id: transition_id.clone(),
                                    previous_reference: previous_reference.clone(),
                                    owner_identity: owner_identity.clone(),
                                    owner_generation,
                                    owner_token,
                                    stage: ActiveOwnerSwitchStage::OldStopped {
                                        terminal_generation,
                                    },
                                };
                                sync_publish_active_profile_record(&config_root, &record)?;
                                let state = ActiveProfileOwnerSwitchState::OldStopped {
                                    previous_reference,
                                    owner_identity,
                                    owner_generation,
                                    terminal_generation,
                                };
                                Ok(ActiveProfileOwnerSwitchRecovery::ResumeOldStopped(
                                    Box::new(ActiveOwnerStoppedSwitchLease::new(
                                        config_root,
                                        transition_id,
                                        state,
                                    )),
                                ))
                            }
                            Err(_) => Ok(
                                ActiveProfileOwnerSwitchRecovery::OwnerTerminalEvidenceRequired {
                                    identity: owner_identity,
                                    bound_generation: owner_generation,
                                },
                            ),
                        }
                    }
                }
            }
            ActiveOwnerSwitchStage::OldStopped {
                terminal_generation,
            } => {
                let proof = sync_read_active_core_owner_proof(&config_root)?;
                let proven_generation = terminal_proof_generation(
                    &proof,
                    &owner_identity,
                    owner_generation,
                    &owner_token,
                )?;
                if proven_generation != terminal_generation {
                    return Err(transition_rejected(
                        "persisted old-owner terminal generation differs from its proof",
                    ));
                }
                if ActiveProfileReferenceFile::new(config_root.clone())
                    .try_acquire_owner_lock()?
                    .is_none()
                {
                    return Ok(ActiveProfileOwnerSwitchRecovery::OwnerStillLive {
                        identity: owner_identity,
                        generation: owner_generation,
                    });
                }
                let state = ActiveProfileOwnerSwitchState::OldStopped {
                    previous_reference,
                    owner_identity,
                    owner_generation,
                    terminal_generation,
                };
                Ok(ActiveProfileOwnerSwitchRecovery::ResumeOldStopped(
                    Box::new(ActiveOwnerStoppedSwitchLease::new(
                        config_root,
                        transition_id,
                        state,
                    )),
                ))
            }
            ActiveOwnerSwitchStage::ReferenceChanged {
                terminal_generation,
            } => {
                let owner_probe = ActiveProfileReferenceFile::new(config_root.clone())
                    .try_acquire_owner_lock()?;
                let Some(owner_probe) = owner_probe else {
                    return Ok(ActiveProfileOwnerSwitchRecovery::OwnerStillLive {
                        identity: owner_identity,
                        generation: owner_generation,
                    });
                };
                drop(owner_probe);
                let mut proof = sync_read_active_core_owner_proof(&config_root)?;
                let proven_generation = match terminal_proof_generation(
                    &proof,
                    &owner_identity,
                    owner_generation,
                    &owner_token,
                ) {
                    Ok(proven_generation) => proven_generation,
                    Err(_) => {
                        // `reserve_new_owner` persists its kernel admission
                        // proof before advancing the root record to
                        // BindingNew. If a process dies between those writes,
                        // this ReferenceChanged stage proves provider
                        // dispatch never received a lease. Restore the old
                        // terminal proof so the durable CAS stage can resume.
                        let selected_engine = match &record.selection {
                            ActiveProfileReferenceState::Selected(reference) => {
                                &reference.engine_id
                            }
                            ActiveProfileReferenceState::Unselected { .. } => {
                                return Err(transition_rejected(
                                    "reference-changed owner switch has no selected engine",
                                ));
                            }
                        };
                        if !matches!(
                            &proof.state,
                            ActiveCoreOwnerProof::Reserved {
                                transition_id: proof_transition,
                                engine_id: proof_engine,
                                ..
                            } if proof_transition == &transition_id
                                && proof_engine == selected_engine
                        ) {
                            return Err(transition_rejected(
                                "persisted owner terminal generation differs from its proof",
                            ));
                        }
                        proof.state = ActiveCoreOwnerProof::Terminal {
                            identity: owner_identity.clone(),
                            bound_generation: owner_generation,
                            terminal_generation,
                            token: owner_token.clone(),
                        };
                        sync_publish_active_core_owner_proof(&config_root, &proof)?;
                        terminal_generation
                    }
                };
                if proven_generation != terminal_generation {
                    return Err(transition_rejected(
                        "persisted owner terminal generation differs from its proof",
                    ));
                }
                let state = ActiveProfileOwnerSwitchState::ReferenceChanged {
                    previous_reference,
                    selection: record.selection,
                    owner_identity,
                    terminal_generation,
                };
                Ok(ActiveProfileOwnerSwitchRecovery::ResumeReferenceChanged(
                    Box::new(ReferenceChangedLease::new(
                        config_root,
                        transition_id,
                        state,
                    )),
                ))
            }
            ActiveOwnerSwitchStage::BindingNew {
                engine_id, token, ..
            } => match ActiveProfileReferenceFile::new(config_root.clone())
                .try_acquire_owner_lock()?
            {
                None => {
                    let local_state = LocalOwnerRegistrationState::Binding {
                        transition_id: transition_id.clone(),
                    };
                    if local_registration_matches(&config_root, &token, &local_state)? {
                        let state = ActiveProfileOwnerSwitchState::BindingNew {
                            selection: record.selection,
                            engine_id,
                        };
                        Ok(ActiveProfileOwnerSwitchRecovery::ResumeBindingNew(
                            Box::new(OwnerBindingLease::new(
                                config_root,
                                transition_id,
                                token,
                                state,
                            )),
                        ))
                    } else {
                        Ok(ActiveProfileOwnerSwitchRecovery::CandidateStillLive { engine_id })
                    }
                }
                Some(owner_probe) => {
                    drop(owner_probe);
                    Ok(
                        ActiveProfileOwnerSwitchRecovery::CandidateTerminalEvidenceRequired {
                            selection: record.selection,
                            engine_id,
                        },
                    )
                }
            },
        }
    }

    async fn recover_old_owner_after_terminal(
        &self,
        terminal: &infiltrator_contract::snapshot::CoreLifecycleSnapshot,
    ) -> Result<Box<dyn ActiveProfileOwnerStoppedSwitchLease>, PortError> {
        let (config_root, _root_guard) = self.acquire_active_profile_reference_lock().await?;
        let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        let mut record = sync_read_active_profile_record(&config_root)?;
        let (transition_id, previous_reference, owner_identity, owner_generation, owner_token) =
            match &record.owner {
                ActiveCoreOwnerState::Switching {
                    transition_id,
                    previous_reference,
                    owner_identity,
                    owner_generation,
                    owner_token,
                    stage: ActiveOwnerSwitchStage::ClosingOld,
                } => (
                    transition_id.clone(),
                    previous_reference.clone(),
                    owner_identity.clone(),
                    *owner_generation,
                    owner_token.clone(),
                ),
                _ => {
                    return Err(transition_rejected(
                        "old-owner terminal recovery requires a ClosingOld reservation",
                    ));
                }
            };
        if record.selection != previous_reference {
            return Err(transition_rejected(
                "active reference changed before old-owner terminal recovery",
            ));
        }
        let terminal_generation =
            validate_terminal_for_owner(terminal, &owner_identity, owner_generation)?;
        if local_owner_registration_state(&config_root, &owner_token)?.is_some() {
            return Err(transition_rejected(
                "old owner still has a local registration; recover its live switch lease",
            ));
        }
        let owner_probe = ActiveProfileReferenceFile::new(config_root.clone())
            .try_acquire_owner_lock()?
            .ok_or_else(|| {
                transition_rejected("old owner still holds the cross-process owner fence")
            })?;
        drop(owner_probe);

        let mut proof = sync_read_active_core_owner_proof(&config_root)?;
        match proof.state.clone() {
            ActiveCoreOwnerProof::Bound {
                identity: proof_identity,
                generation: proof_generation,
                token: proof_token,
            } if proof_identity.same_instance(&owner_identity)
                && proof_generation == owner_generation
                && proof_token == owner_token =>
            {
                proof.state = ActiveCoreOwnerProof::Terminal {
                    identity: owner_identity.clone(),
                    bound_generation: owner_generation,
                    terminal_generation,
                    token: owner_token.clone(),
                };
                sync_publish_active_core_owner_proof(&config_root, &proof)?;
            }
            ActiveCoreOwnerProof::Terminal {
                identity: proof_identity,
                bound_generation: proof_generation,
                terminal_generation: proof_terminal_generation,
                token: proof_token,
            } if proof_identity.same_instance(&owner_identity)
                && proof_generation == owner_generation
                && proof_terminal_generation == terminal_generation
                && proof_token == owner_token => {}
            _ => {
                return Err(transition_rejected(
                    "old-owner terminal snapshot does not match the persisted owner proof",
                ));
            }
        }
        record.owner = ActiveCoreOwnerState::Switching {
            transition_id: transition_id.clone(),
            previous_reference: previous_reference.clone(),
            owner_identity: owner_identity.clone(),
            owner_generation,
            owner_token,
            stage: ActiveOwnerSwitchStage::OldStopped {
                terminal_generation,
            },
        };
        sync_publish_active_profile_record(&config_root, &record)?;
        let state = ActiveProfileOwnerSwitchState::OldStopped {
            previous_reference,
            owner_identity,
            owner_generation,
            terminal_generation,
        };
        Ok(Box::new(ActiveOwnerStoppedSwitchLease::new(
            config_root,
            transition_id,
            state,
        )))
    }

    async fn recover_candidate_after_terminal(
        &self,
        terminal: &infiltrator_contract::snapshot::CoreLifecycleSnapshot,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        let (config_root, _root_guard) = self.acquire_active_profile_reference_lock().await?;
        let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        let mut record = sync_read_active_profile_record(&config_root)?;
        let (
            transition_id,
            engine_id,
            token,
            old_owner_identity,
            old_owner_generation,
            old_owner_token,
            old_terminal_generation,
        ) = match &record.owner {
            ActiveCoreOwnerState::Switching {
                transition_id,
                owner_identity,
                owner_generation,
                owner_token,
                stage:
                    ActiveOwnerSwitchStage::BindingNew {
                        engine_id,
                        token,
                        terminal_generation,
                    },
                ..
            } => (
                transition_id.clone(),
                engine_id.clone(),
                token.clone(),
                owner_identity.clone(),
                *owner_generation,
                owner_token.clone(),
                *terminal_generation,
            ),
            _ => {
                return Err(transition_rejected(
                    "no crashed candidate reservation is pending",
                ));
            }
        };
        let identity = terminal
            .identity
            .as_ref()
            .ok_or_else(|| transition_rejected("candidate terminal has no Core identity"))?;
        if identity.engine != engine_id
            || terminal.lifecycle != infiltrator_contract::snapshot::CoreLifecycle::Stopped
            || terminal.generation != 0
            || terminal.session_token.is_some()
        {
            return Err(transition_rejected(
                "candidate recovery requires a never-started Stopped readback without a session",
            ));
        }
        if local_owner_registration_state(&config_root, &token)?.is_some() {
            return Err(transition_rejected(
                "candidate owner is still registered in this process; use its binding lease",
            ));
        }
        if ActiveProfileReferenceFile::new(config_root.clone())
            .try_acquire_owner_lock()?
            .is_none()
        {
            return Err(transition_rejected(
                "candidate process still holds the active owner fence",
            ));
        }
        let mut proof = sync_read_active_core_owner_proof(&config_root)?;
        match &proof.state {
            ActiveCoreOwnerProof::Reserved {
                transition_id: proof_transition,
                engine_id: proof_engine,
                token: proof_token,
            } if proof_transition == &transition_id
                && proof_engine == &engine_id
                && proof_token == &token => {}
            ActiveCoreOwnerProof::Bound {
                identity: proof_identity,
                generation: proof_generation,
                token: proof_token,
            } if proof_identity.same_instance(identity)
                && *proof_generation == CoreGeneration::new(terminal.generation)
                && *proof_generation == CoreGeneration::new(0)
                && proof_token == &token => {}
            ActiveCoreOwnerProof::Terminal {
                identity: proof_identity,
                bound_generation: proof_bound_generation,
                terminal_generation: proof_terminal_generation,
                token: proof_token,
            } if proof_identity.same_instance(&old_owner_identity)
                && *proof_bound_generation == old_owner_generation
                && *proof_terminal_generation == old_terminal_generation
                && proof_token == &old_owner_token => {}
            _ => return Err(transition_rejected("candidate owner proof is stale")),
        }
        proof.state = ActiveCoreOwnerProof::Terminal {
            identity: old_owner_identity,
            bound_generation: old_owner_generation,
            terminal_generation: old_terminal_generation,
            token: old_owner_token,
        };
        sync_publish_active_core_owner_proof(&config_root, &proof)?;
        record.owner = ActiveCoreOwnerState::Unowned;
        sync_publish_active_profile_record(&config_root, &record)?;
        Ok(record.selection)
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

    /// Persist one derived active-profile document as a single atomic file
    /// replacement while the reference is selected and no Core owner is
    /// bound. This path deliberately cannot change sidecar bytes or perform a
    /// runtime apply, so caller cancellation leaves either the previous file
    /// or the complete replacement.
    pub(crate) async fn compare_and_save_unowned_active_derived_document(
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

        self.initialize_profile_namespace()
            .await
            .map_err(profile_namespace_initialization_error)?;
        let (config_root, _root_guard) = self.acquire_active_profile_reference_lock().await?;
        ensure_no_pending_reset_in_engine_namespaces(
            &config_root,
            ActiveProfileResetMutationAccess::OrdinaryMutation,
        )
        .await?;
        let mut record = self.read_active_profile_record_unlocked().await?;
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
        let _profile_guard = self.lock_profile_writes().await.map_err(lock_error)?;
        let observed = self
            .read_workspace_locked(&expected_reference.profile_id)
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
            .existing_profile_document_path(expected_reference.profile_id.as_str())
            .await
            .map_err(crate::profile_workspace_store::config_error)?;
        write_document_bytes_durable(&path, &update.document_bytes).await?;
        if let Err(error) = remove_optional(&profile_document_backup_path(&path)).await {
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
    pub(crate) async fn acquire_active_workspace_write_lease<'a>(
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
        self.initialize_profile_namespace()
            .await
            .map_err(profile_namespace_initialization_error)?;
        let (config_root, root_guard) = self.acquire_active_profile_reference_lock().await?;
        ensure_no_pending_reset_in_engine_namespaces(
            &config_root,
            ActiveProfileResetMutationAccess::OrdinaryMutation,
        )
        .await?;
        let mut record = sync_read_active_profile_record(&config_root)?;
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
        let profile_guard = lock_profile_writes_for_dir(self.config_dir())
            .await
            .map_err(lock_error)?;
        let observed = self
            .read_workspace_locked(&expected_reference.profile_id)
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
        Ok(Box::new(ConfigManagerObservedWorkspaceLease {
            manager: self,
            observed,
            guards,
        }))
    }

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
                self.validate_document_bytes(default_document.as_bytes())
                    .map_err(lock_error)?;
                if read_optional_bytes(&self.config_dir().join(ACTIVE_PROFILE_RESET_INTENT_FILE))
                    .await?
                    .is_none()
                {
                    self.initialize_profile_namespace()
                        .await
                        .map_err(profile_namespace_initialization_error)?;
                }
                let (config_root, root_guard) =
                    self.acquire_active_profile_reference_lock().await?;
                (expected, default_document, config_root, root_guard)
            }
            ActiveProfileReferenceResetRequest::ResumePendingIntent => {
                let (config_root, root_guard) =
                    self.acquire_active_profile_reference_lock().await?;
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
                        let default_path = self
                            .profile_document_path("default")
                            .await
                            .map_err(lock_error)?;
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
                                    infiltrator_contract::capability::Capability::Profiles,
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
        self.validate_document_bytes(default_content.as_bytes())
            .map_err(lock_error)?;
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

        let _profile_guard = self.lock_profile_writes().await.map_err(lock_error)?;
        let default_path = self
            .profile_document_path("default")
            .await
            .map_err(lock_error)?;
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
                    infiltrator_contract::capability::Capability::Profiles,
                    "legacy reset intent contains only a digest and its exact default document cannot be recovered",
                ));
            };
            let migrated = ActiveProfileResetIntentRecord::new(
                legacy.expected_reference.clone(),
                legacy.target_reference.clone(),
                recoverable_default,
            );
            let bytes = serde_json::to_vec_pretty(&migrated)
                .map_err(|error| PortError::Io(error.to_string()))?;
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
        self.validate_document_bytes(&reset_default_document)
            .map_err(lock_error)?;
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
            let workspace = self.read_workspace_locked(&default_id).await?;
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
                        infiltrator_contract::capability::Capability::Profiles,
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
            let bytes = serde_json::to_vec_pretty(&desired_intent)
                .map_err(|error| PortError::Io(error.to_string()))?;
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

        let workspace = match self.read_workspace_locked(&default_id).await {
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
        self.reset_profile_namespace_marker_unlocked()
            .await
            .map_err(lock_error)?;
        let mut entries = tokio::fs::read_dir(self.config_dir())
            .await
            .map_err(lock_error)?;
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
                tokio::fs::remove_dir_all(path).await.map_err(lock_error)?;
            } else {
                tokio::fs::remove_file(path).await.map_err(lock_error)?;
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
        let bytes =
            serde_json::to_vec_pretty(intent).map_err(|error| PortError::Io(error.to_string()))?;
        write_document_bytes_durable(complete_path, &bytes).await?;
        remove_optional_durable(intent_path)
    }
}

#[async_trait::async_trait]
impl<S: SecureStore> ActiveProfileWorkspaceWriteLease
    for ConfigManagerObservedWorkspaceLease<'_, S>
{
    fn observed_workspace(&self) -> &ProfileWorkspace {
        &self.observed
    }

    async fn publish(
        self: Box<Self>,
        update: ProfileWorkspaceUpdate,
    ) -> ActiveProfileWorkspacePublishOutcome<'async_trait> {
        let ConfigManagerObservedWorkspaceLease {
            manager,
            observed,
            guards,
        } = *self;
        let target = workspace_for_update(manager.document_codec_scope(), &observed, &update);
        let write_result = manager
            .publish_active_workspace_locked(&observed.identity(), &update)
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
        let recovery = Box::new(ConfigManagerWorkspaceRecoveryLease {
            manager,
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
    for ConfigManagerWorkspaceRecoveryLease<'_, S>
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
        let ConfigManagerWorkspaceRecoveryLease {
            manager,
            previous,
            purpose,
            guards,
            last_error,
        } = *self;
        let profile_id = previous.document.identity.profile_id.clone();
        let observed = match manager.read_workspace_locked(&profile_id).await {
            Ok(observed) => observed,
            Err(error) => {
                return ActiveProfileWorkspaceRecoveryResolution::RecoveryRequired(Box::new(
                    ConfigManagerWorkspaceRecoveryLease {
                        manager,
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
                        ConfigManagerPublishedWorkspaceLease {
                            manager,
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

                let recovery_result = manager
                    .recover_workspace_locked(&observed.identity(), &previous)
                    .await;
                let recovered = match manager.read_workspace_locked(&profile_id).await {
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
                            Box::new(ConfigManagerWorkspaceRecoveryLease {
                                manager,
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
                        ConfigManagerPublishedWorkspaceLease {
                            manager,
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
                        ConfigManagerRestoredWorkspaceLease {
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

                let recovery_result = manager
                    .recover_workspace_locked(&published.identity(), &previous)
                    .await;
                let recovered = match manager.read_workspace_locked(&profile_id).await {
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
                            Box::new(ConfigManagerWorkspaceRecoveryLease {
                                manager,
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
                        ConfigManagerRestoredWorkspaceLease {
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
                        ConfigManagerWorkspaceRecoveryLease {
                            manager,
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
    for ConfigManagerPublishedWorkspaceLease<'_, S>
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
        let ConfigManagerPublishedWorkspaceLease {
            manager,
            previous,
            published,
            guards,
        } = *self;
        let recovery = Box::new(ConfigManagerWorkspaceRecoveryLease {
            manager,
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
impl ActiveProfileWorkspaceRestoredLease for ConfigManagerRestoredWorkspaceLease {
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

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::core_identity::CoreVersion;
    use infiltrator_contract::core_process::CoreProcessStartContext;
    use infiltrator_contract::session::SessionToken;
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
            manager.acquire_selection_lease().await,
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
            .profile_document_path("default")
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
        assert_eq!(
            ActiveProfileReferenceStore::read(&restarted).await.unwrap(),
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
            .profile_document_path("default")
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
            tokio::fs::read(restarted.profile_document_path("default").await.unwrap())
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
                .profile_document_path("default")
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
            tokio::fs::read(restarted.profile_document_path("default").await.unwrap())
                .await
                .unwrap()
                .as_slice(),
            previous_default.as_slice()
        );
        assert_eq!(tokio::fs::read(intent_path).await.unwrap(), intent_bytes);
        assert_eq!(
            ActiveProfileReferenceStore::read(&restarted).await.unwrap(),
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
            .compare_and_set(
                migrated.revision(),
                ActiveProfileReferenceTarget::Unselected,
                ActiveProfileReferenceWriteAuthorization::Unowned,
            )
            .await
            .unwrap();
        let local_pointer = config.get_current_unlocked().await.unwrap();
        config.migrate_legacy_mihomo_selection().await.unwrap();

        assert_eq!(config.read().await.unwrap(), explicit_clear);
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
        assert_eq!(manager.read().await.unwrap(), selected);
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
        let lease = manager.acquire_selection_lease().await.unwrap();
        assert_eq!(lease.state(), &selected);

        let writer = Arc::clone(&manager);
        let expected = selected.revision();
        let pending = tokio::spawn(async move {
            writer
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

        let lease = manager.acquire_provider_selection_lease().await.unwrap();
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

        match manager.acquire_provider_selection_lease().await {
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
        let next_provider = manager.acquire_provider_selection_lease().await.unwrap();
        assert_eq!(next_provider.state(), &same_engine_update);
    }

    #[tokio::test]
    async fn persisted_owner_release_and_recovery_ignore_runtime_version_changes() {
        let directory = TempDir::new().unwrap();
        let manager = manager(&directory);
        let selected = manager.migrate_legacy_mihomo_selection().await.unwrap();
        let bound_identity = identity(crate::engine_id().as_str(), "version-refresh-owner");
        let owner = manager
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
        let provider_lease = manager.acquire_provider_selection_lease().await.unwrap();
        assert_eq!(provider_lease.state(), &selected);
    }

    #[tokio::test]
    async fn never_started_terminal_evidence_is_a_noop_without_a_persisted_owner() {
        let directory = TempDir::new().unwrap();
        let manager = manager(&directory);

        let outcome = manager
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
        let selection_lease = manager.acquire_selection_lease().await.unwrap();
        let release = tokio::spawn(async move { owner.release_after_terminal(&terminal).await });
        tokio::time::timeout(Duration::from_secs(3), release)
            .await
            .expect("terminal release waited for root selection lock")
            .expect("terminal release task panicked")
            .unwrap();

        let writer = Arc::clone(&manager);
        let pending = tokio::spawn(async move {
            writer
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
            .acquire_provider_selection_lease()
            .await
            .unwrap()
            .into_owner_binding(old_identity.clone(), CoreGeneration::new(1))
            .await
            .unwrap();
        let switch = manager
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
            manager.acquire_provider_selection_lease().await,
            Err(PortError::Rejected(_))
        ));

        let old_terminal = stopped_snapshot(old_identity.clone(), 1);
        owner.release_after_terminal(&old_terminal).await.unwrap();
        let recovery = manager.recover_owner_switch().await.unwrap();
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
        let changed = match manager.recover_owner_switch().await.unwrap() {
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
        let candidate = match manager.recover_owner_switch().await.unwrap() {
            ActiveProfileOwnerSwitchRecovery::ResumeBindingNew(lease) => lease,
            _ => panic!("expected resumable BindingNew stage"),
        };
        let new_owner = candidate
            .finalize(candidate_identity.clone(), CoreGeneration::new(0))
            .await
            .unwrap();
        let committed = manager.read().await.unwrap();
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
            .acquire_provider_selection_lease()
            .await
            .unwrap()
            .into_owner_binding(old_identity.clone(), CoreGeneration::new(1))
            .await
            .unwrap();
        let switch = manager
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
            manager.recover_owner_switch().await.unwrap(),
            ActiveProfileOwnerSwitchRecovery::OwnerTerminalEvidenceRequired {
                identity,
                bound_generation,
            } if identity == old_identity && bound_generation == CoreGeneration::new(1)
        ));

        let old_terminal = stopped_snapshot(observed_identity, 2);
        let old_stopped = manager
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
            manager.recover_owner_switch().await.unwrap(),
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
            manager.acquire_provider_selection_lease().await,
            Err(PortError::Rejected(_))
        ));
        assert!(
            manager
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

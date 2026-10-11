//! Product-wide active-profile reference storage and the explicit legacy
//! Mihomo selection migration.

use super::ConfigManager;
use crate::profile_write_boundary::shared_boundary;
use crate::sidecar_store::{read_optional, write_document};
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
    ActiveProfileReferenceTarget,
};
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::core_identity::{CoreGeneration, CoreIdentity};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_ports::active_profile_reference_store::{
    ActiveProfileOwnerLease, ActiveProfileReferenceLease, ActiveProfileReferenceStore,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::error::MihomoError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use tokio::sync::OwnedMutexGuard;

const ACTIVE_PROFILE_REFERENCE_SCHEMA_VERSION: u32 = 1;
const ACTIVE_PROFILE_REFERENCE_FILE: &str = "active-profile-reference.json";

#[derive(Clone, Debug, PartialEq, Eq)]
enum ActiveCoreOwnerState {
    Unowned,
    Bound {
        identity: CoreIdentity,
        generation: CoreGeneration,
        token: u64,
    },
}

fn owner_state_boundary(config_root: &Path) -> Arc<StdMutex<ActiveCoreOwnerState>> {
    static BOUNDARIES: OnceLock<StdMutex<HashMap<PathBuf, Arc<StdMutex<ActiveCoreOwnerState>>>>> =
        OnceLock::new();
    let canonical_root =
        std::fs::canonicalize(config_root).unwrap_or_else(|_| config_root.to_path_buf());
    let boundaries = BOUNDARIES.get_or_init(|| StdMutex::new(HashMap::new()));
    let mut boundaries = boundaries.lock().expect("owner boundary registry lock");
    Arc::clone(
        boundaries
            .entry(canonical_root)
            .or_insert_with(|| Arc::new(StdMutex::new(ActiveCoreOwnerState::Unowned))),
    )
}

struct ConfigManagerActiveCoreOwnerLease {
    identity: CoreIdentity,
    generation: CoreGeneration,
    token: u64,
    state: Arc<StdMutex<ActiveCoreOwnerState>>,
}

impl ActiveProfileOwnerLease for ConfigManagerActiveCoreOwnerLease {
    fn identity(&self) -> &CoreIdentity {
        &self.identity
    }

    fn generation(&self) -> CoreGeneration {
        self.generation
    }

    fn release_after_terminal(
        &self,
        terminal: &infiltrator_contract::snapshot::CoreLifecycleSnapshot,
    ) -> Result<(), PortError> {
        if terminal.identity.as_ref() != Some(&self.identity)
            || terminal.generation < self.generation.value()
            || terminal.lifecycle != infiltrator_contract::snapshot::CoreLifecycle::Stopped
            || terminal.session_token.is_some()
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active Core owner lease requires matching Stopped readback without a session",
                true,
            )));
        }

        let mut state = self
            .state
            .lock()
            .map_err(|_| PortError::Failed("active Core owner state lock is poisoned".into()))?;
        match &*state {
            ActiveCoreOwnerState::Bound {
                identity,
                generation,
                token,
            } if identity == &self.identity
                && *generation == self.generation
                && *token == self.token =>
            {
                *state = ActiveCoreOwnerState::Unowned;
                Ok(())
            }
            ActiveCoreOwnerState::Unowned | ActiveCoreOwnerState::Bound { .. } => {
                Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "active Core owner lease is stale or no longer bound",
                    true,
                )))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "reason", rename_all = "snake_case")]
enum LegacySelectionMigration {
    PendingDesktopMihomoV0,
    MigratedDesktopMihomoV0,
    RejectedDesktopMihomoV0,
    ExplicitSelection,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActiveProfileReferenceRecord {
    schema_version: u32,
    selection: ActiveProfileReferenceState,
    legacy_migration: LegacySelectionMigration,
}

impl ActiveProfileReferenceRecord {
    fn pending_legacy_migration() -> Self {
        Self {
            schema_version: ACTIVE_PROFILE_REFERENCE_SCHEMA_VERSION,
            selection: ActiveProfileReferenceState::Unselected {
                revision: ActiveProfileReferenceRevision::new(0),
            },
            legacy_migration: LegacySelectionMigration::PendingDesktopMihomoV0,
        }
    }

    fn validate(&self) -> Result<(), PortError> {
        if self.schema_version != ACTIVE_PROFILE_REFERENCE_SCHEMA_VERSION {
            return Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Profiles,
                format!(
                    "active-profile reference schema {} is unsupported",
                    self.schema_version
                ),
            ));
        }
        Ok(())
    }
}

struct ConfigManagerSelectionLease {
    state: ActiveProfileReferenceState,
    _guard: OwnedMutexGuard<()>,
    owner_state: Arc<StdMutex<ActiveCoreOwnerState>>,
}

impl ActiveProfileReferenceLease for ConfigManagerSelectionLease {
    fn state(&self) -> &ActiveProfileReferenceState {
        &self.state
    }

    fn into_owner_binding(
        self: Box<Self>,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<Box<dyn ActiveProfileOwnerLease>, PortError> {
        let Self {
            state,
            _guard,
            owner_state,
        } = *self;
        let selected_engine = match state {
            ActiveProfileReferenceState::Selected(reference) => reference.engine_id,
            ActiveProfileReferenceState::Unselected { .. } => {
                return Err(PortError::Failed(
                    "an unselected profile cannot bind a Core owner".into(),
                ));
            }
        };
        if selected_engine != identity.engine {
            return Err(PortError::Failed(format!(
                "active profile engine {} does not match owner engine {}",
                selected_engine.as_str(),
                identity.engine.as_str()
            )));
        }
        static NEXT_OWNER_TOKEN: AtomicU64 = AtomicU64::new(0);
        let token = NEXT_OWNER_TOKEN.fetch_add(1, Ordering::SeqCst) + 1;
        {
            let mut active_owner = owner_state.lock().map_err(|_| {
                PortError::Failed("active Core owner state lock is poisoned".into())
            })?;
            match &*active_owner {
                ActiveCoreOwnerState::Unowned => {
                    *active_owner = ActiveCoreOwnerState::Bound {
                        identity: identity.clone(),
                        generation,
                        token,
                    };
                }
                ActiveCoreOwnerState::Bound {
                    identity: existing, ..
                } => {
                    return Err(PortError::Rejected(Failure::new(
                        ErrorCode::NotReady,
                        format!(
                            "Core owner {} remains active; close it before binding {}",
                            existing.instance.as_str(),
                            identity.instance.as_str()
                        ),
                        true,
                    )));
                }
            }
        }
        drop(_guard);
        Ok(Box::new(ConfigManagerActiveCoreOwnerLease {
            identity,
            generation,
            token,
            state: owner_state,
        }))
    }
}

impl<S: SecureStore> ConfigManager<S> {
    fn active_profile_reference_path(&self) -> PathBuf {
        self.config_root.join(ACTIVE_PROFILE_REFERENCE_FILE)
    }

    fn active_profile_reference_boundary(&self) -> Result<Arc<tokio::sync::Mutex<()>>, PortError> {
        shared_boundary(&self.config_root).map_err(|error| PortError::Io(error.to_string()))
    }

    async fn read_active_profile_record_unlocked(
        &self,
    ) -> Result<ActiveProfileReferenceRecord, PortError> {
        let path = self.active_profile_reference_path();
        let Some(text) = read_optional(&path).await? else {
            return Ok(ActiveProfileReferenceRecord::pending_legacy_migration());
        };
        let record: ActiveProfileReferenceRecord = serde_json::from_str(&text)
            .map_err(|error| PortError::Io(format!("invalid active-profile reference: {error}")))?;
        record.validate()?;
        Ok(record)
    }

    async fn write_active_profile_record_unlocked(
        &self,
        record: &ActiveProfileReferenceRecord,
    ) -> Result<(), PortError> {
        let text = serde_json::to_string_pretty(record)
            .map_err(|error| PortError::Io(error.to_string()))?;
        write_document(&self.active_profile_reference_path(), &text).await
    }

    async fn publish_active_profile_record_unlocked(
        &self,
        record: &ActiveProfileReferenceRecord,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        self.write_active_profile_record_unlocked(record).await?;
        let observed = self.read_active_profile_record_unlocked().await?;
        if observed != *record {
            return Err(PortError::Failed(
                "active profile reference readback differed from the committed record".into(),
            ));
        }
        Ok(observed.selection)
    }

    /// Explicitly migrate the legacy Mihomo provider choice once. The current
    /// profile is accepted only when its persisted document
    /// identity proves both Mihomo engine and exact profile ID. A rejected
    /// migration is recorded as Unselected and will never silently retry as a
    /// Mihomo fallback.
    pub async fn migrate_legacy_mihomo_selection(
        &self,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        self.initialize_profile_namespace()
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let boundary = self.active_profile_reference_boundary()?;
        let _guard = Arc::clone(&boundary).lock_owned().await;
        let mut record = self.read_active_profile_record_unlocked().await?;
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
        match self.existing_profile_yaml_path(profile_id.as_str()).await {
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

    /// Update the global Core selector while holding the root selection lock.
    /// The legacy namespaced pointer is retained only as migration input.
    pub(crate) async fn set_current_with_active_reference(
        &self,
        profile: &str,
    ) -> Result<(), PortError> {
        self.initialize_profile_namespace()
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let profile_id = ProfileId::new(profile.to_owned()).map_err(|error| {
            PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                error.to_string(),
                false,
            ))
        })?;
        let boundary = self.active_profile_reference_boundary()?;
        let _guard = Arc::clone(&boundary).lock_owned().await;
        let workspace = self.read_workspace(&profile_id).await?;
        if workspace.document.identity.engine_id != crate::engine_id()
            || workspace.document.identity.profile_id != profile_id
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "selected profile document identity does not match its engine namespace",
                false,
            )));
        }
        let mut record = self.read_active_profile_record_unlocked().await?;
        let target = ActiveProfileReferenceTarget::selected(crate::engine_id(), profile_id);
        self.ensure_owner_allows_target(&target)?;
        let revision = record
            .selection
            .revision()
            .next()
            .ok_or_else(|| PortError::Failed("active-profile revision exhausted".into()))?;
        record.selection = match target {
            ActiveProfileReferenceTarget::Unselected => {
                ActiveProfileReferenceState::Unselected { revision }
            }
            ActiveProfileReferenceTarget::Selected {
                engine_id,
                profile_id,
            } => ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
                engine_id, profile_id, revision,
            )),
        };
        record.legacy_migration = LegacySelectionMigration::ExplicitSelection;
        self.publish_active_profile_record_unlocked(&record)
            .await
            .map(|_| ())
    }

    fn ensure_owner_allows_target(
        &self,
        target: &ActiveProfileReferenceTarget,
    ) -> Result<(), PortError> {
        let owner_state = owner_state_boundary(&self.config_root);
        let owner = owner_state
            .lock()
            .map_err(|_| PortError::Failed("active Core owner state lock is poisoned".into()))?;
        match (&*owner, target) {
            (ActiveCoreOwnerState::Unowned, _) => Ok(()),
            (
                ActiveCoreOwnerState::Bound {
                    identity,
                    generation,
                    ..
                },
                ActiveProfileReferenceTarget::Selected { engine_id, .. },
            ) if engine_id == &identity.engine => Ok(()),
            (
                ActiveCoreOwnerState::Bound {
                    identity,
                    generation,
                    ..
                },
                _,
            ) => Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                format!(
                    "active Core {} at generation {} must reach terminal before changing its engine selection",
                    identity.instance.as_str(),
                    generation.value()
                ),
                true,
            ))),
        }
    }
}

#[async_trait::async_trait]
impl<S: SecureStore> ActiveProfileReferenceStore for ConfigManager<S> {
    async fn read(&self) -> Result<ActiveProfileReferenceState, PortError> {
        let boundary = self.active_profile_reference_boundary()?;
        let _guard = boundary.lock().await;
        Ok(self.read_active_profile_record_unlocked().await?.selection)
    }

    async fn acquire_selection_lease(
        &self,
    ) -> Result<Box<dyn ActiveProfileReferenceLease>, PortError> {
        let boundary = self.active_profile_reference_boundary()?;
        let guard = Arc::clone(&boundary).lock_owned().await;
        let state = self.read_active_profile_record_unlocked().await?.selection;
        Ok(Box::new(ConfigManagerSelectionLease {
            state,
            _guard: guard,
            owner_state: owner_state_boundary(&self.config_root),
        }))
    }

    async fn acquire_provider_selection_lease(
        &self,
    ) -> Result<Box<dyn ActiveProfileReferenceLease>, PortError> {
        let boundary = self.active_profile_reference_boundary()?;
        let guard = Arc::clone(&boundary).lock_owned().await;
        let owner_state = owner_state_boundary(&self.config_root);
        let active_owner = {
            let owner = owner_state.lock().map_err(|_| {
                PortError::Failed("active Core owner state lock is poisoned".into())
            })?;
            match &*owner {
                ActiveCoreOwnerState::Bound {
                    identity,
                    generation,
                    ..
                } => Some((identity.clone(), *generation)),
                ActiveCoreOwnerState::Unowned => None,
            }
        };
        if let Some((identity, generation)) = active_owner {
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
        let state = self.read_active_profile_record_unlocked().await?.selection;
        Ok(Box::new(ConfigManagerSelectionLease {
            state,
            _guard: guard,
            owner_state,
        }))
    }

    async fn compare_and_set(
        &self,
        expected_revision: ActiveProfileReferenceRevision,
        target: ActiveProfileReferenceTarget,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        let boundary = self.active_profile_reference_boundary()?;
        let _guard = boundary.lock().await;
        let mut record = self.read_active_profile_record_unlocked().await?;
        if record.selection.revision() != expected_revision {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active profile reference revision is stale",
                true,
            )));
        }
        self.ensure_owner_allows_target(&target)?;
        let revision = expected_revision
            .next()
            .ok_or_else(|| PortError::Failed("active-profile revision exhausted".into()))?;
        record.selection = match target {
            ActiveProfileReferenceTarget::Unselected => {
                ActiveProfileReferenceState::Unselected { revision }
            }
            ActiveProfileReferenceTarget::Selected {
                engine_id,
                profile_id,
            } => ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
                engine_id, profile_id, revision,
            )),
        };
        record.legacy_migration = LegacySelectionMigration::ExplicitSelection;
        self.publish_active_profile_record_unlocked(&record).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_ports::profile_store::ProfileStore;
    use std::sync::Arc;
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

    fn manager(directory: &TempDir) -> ConfigManager<TestStore> {
        ConfigManager::with_home_and_store(directory.path().to_path_buf(), TestStore)
            .expect("isolated config manager")
    }

    #[tokio::test]
    async fn migration_persists_the_exact_legacy_mihomo_profile_once() {
        let directory = TempDir::new().unwrap();
        let config = manager(&directory);
        config.save("default", "mixed-port: 7890\n").await.unwrap();
        config
            .save("legacy-choice", "mixed-port: 7890\n")
            .await
            .unwrap();
        tokio::fs::write(
            &config.settings_file,
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

        let selected = manager.migrate_legacy_mihomo_selection().await.unwrap();

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
    async fn profile_store_selection_updates_only_global_reference() {
        let directory = TempDir::new().unwrap();
        let manager = manager(&directory);
        manager.save("default", "mixed-port: 7890\n").await.unwrap();
        manager.save("other", "mixed-port: 7891\n").await.unwrap();
        let legacy_pointer = manager.get_current_unlocked().await.unwrap();

        ProfileStore::set_current(&manager, "other").await.unwrap();

        let selected = manager.read().await.unwrap();
        assert!(matches!(
            &selected,
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == crate::engine_id()
                    && reference.profile_id.as_str() == "other"
                    && reference.revision.value() == 1
        ));
        assert_eq!(manager.get_current().await.unwrap(), "other");
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
                .compare_and_set(expected, ActiveProfileReferenceTarget::Unselected)
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
            .unwrap();

        let same_engine_update = manager
            .compare_and_set(
                selected.revision(),
                ActiveProfileReferenceTarget::selected(
                    crate::engine_id(),
                    ProfileId::new("default").unwrap(),
                ),
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
        owner.release_after_terminal(&terminal).unwrap();
        drop(owner);
        let next_provider = manager.acquire_provider_selection_lease().await.unwrap();
        assert_eq!(next_provider.state(), &same_engine_update);
    }
}

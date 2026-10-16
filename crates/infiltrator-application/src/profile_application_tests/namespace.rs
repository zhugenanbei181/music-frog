//! test-intent: behavior
//! Engine namespace boundaries for ProfileStore-owned profile state.

use std::mem;
use super::*;
use async_trait::async_trait;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
};
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::profile_document_identity::{
    ProfileDocument, ProfileDocumentIdentity, ProfileDocumentRevision,
    ProfileDocumentSourceIdentity, ProfileDocumentSourceKind, ProfileFormatId, ProfileId,
    ProfileSchemaVersion, ProfileSidecarRevision, ProfileSourceId, ProfileWorkspaceIdentity,
};
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_domain::profile_source::hash_bytes;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct NamespacedProfileKey {
    engine: EngineId,
    profile: ProfileId,
}

#[derive(Clone)]
enum SidecarState {
    Absent,
    Present(Vec<u8>),
}

#[derive(Clone)]
enum BackupState {
    Absent,
    Present(Vec<u8>),
}

#[derive(Clone)]
enum CredentialState {
    Absent,
    Present(String),
}

#[derive(Clone)]
struct NamespaceProfile {
    document: Vec<u8>,
    sidecar: SidecarState,
    metadata: ProfileMetadata,
    backup: BackupState,
}

#[derive(Default)]
struct NamespaceDatabase {
    profiles: HashMap<NamespacedProfileKey, NamespaceProfile>,
    active: HashMap<EngineId, ProfileId>,
    credentials: HashMap<NamespacedProfileKey, CredentialState>,
}

#[derive(Clone)]
struct NamespaceStore {
    engine: EngineId,
    database: Arc<Mutex<NamespaceDatabase>>,
}

impl NamespaceStore {
    fn new(engine: &str, database: Arc<Mutex<NamespaceDatabase>>) -> Self {
        Self {
            engine: EngineId::new(engine).expect("valid engine ID"),
            database,
        }
    }

    fn key(&self, profile: &ProfileId) -> NamespacedProfileKey {
        NamespacedProfileKey {
            engine: self.engine.clone(),
            profile: profile.clone(),
        }
    }

    fn seed(
        &self,
        profile: &str,
        document: &[u8],
        sidecar: Option<&[u8]>,
        subscription_url: &str,
        credential: &str,
        backup: &[u8],
        active: bool,
    ) {
        let profile_id = ProfileId::new(profile).expect("valid profile ID");
        let key = self.key(&profile_id);
        let mut database = self.database.lock().expect("namespace database");
        database.profiles.insert(
            key.clone(),
            NamespaceProfile {
                document: document.to_vec(),
                sidecar: match sidecar {
                    Some(bytes) => SidecarState::Present(bytes.to_vec()),
                    None => SidecarState::Absent,
                },
                metadata: ProfileMetadata {
                    subscription_url: Some(subscription_url.to_owned()),
                    ..ProfileMetadata::default()
                },
                backup: BackupState::Present(backup.to_vec()),
            },
        );
        database
            .credentials
            .insert(key, CredentialState::Present(credential.to_owned()));
        if active {
            database.active.insert(self.engine.clone(), profile_id);
        }
    }

    fn credential(&self, profile: &str) -> CredentialState {
        let profile = ProfileId::new(profile).expect("valid profile ID");
        self.database
            .lock()
            .expect("namespace database")
            .credentials
            .get(&self.key(&profile))
            .cloned()
            .unwrap_or(CredentialState::Absent)
    }

    fn backup(&self, profile: &str) -> BackupState {
        let profile = ProfileId::new(profile).expect("valid profile ID");
        self.database
            .lock()
            .expect("namespace database")
            .profiles
            .get(&self.key(&profile))
            .map(|entry| entry.backup.clone())
            .unwrap_or(BackupState::Absent)
    }
}

fn workspace(
    engine: &EngineId,
    profile: &ProfileId,
    stored: &NamespaceProfile,
) -> ProfileWorkspace {
    let document_revision = ProfileDocumentRevision::new(hash_bytes(&stored.document))
        .expect("valid document revision");
    let source_id = ProfileSourceId::new(format!("{}-{}", engine.as_str(), profile.as_str()))
        .expect("valid source ID");
    let identity = ProfileDocumentIdentity::new(
        profile.clone(),
        engine.clone(),
        ProfileDocumentSourceIdentity::new(ProfileDocumentSourceKind::Local, source_id),
        document_revision,
        ProfileFormatId::new("engine-neutral-test").expect("valid format ID"),
        ProfileSchemaVersion::new(1).expect("valid schema version"),
    );
    let (sidecar_revision, sidecar_bytes) = match &stored.sidecar {
        SidecarState::Absent => (ProfileSidecarRevision::Absent, None),
        SidecarState::Present(bytes) => (
            ProfileSidecarRevision::Present(
                ProfileDocumentRevision::new(hash_bytes(bytes)).expect("valid sidecar revision"),
            ),
            Some(bytes.clone()),
        ),
    };
    let write_protection = ProfileWriteProtection::from_subscription_url(
        stored
            .metadata
            .subscription_url
            .as_deref()
            .unwrap_or_default(),
    );
    ProfileWorkspace {
        document: ProfileDocument::new(identity, stored.document.clone()),
        sidecar_revision,
        write_protection,
        sidecar_bytes,
    }
}

#[async_trait]
impl ProfileStore for NamespaceStore {
    fn engine_id(&self) -> EngineId {
        self.engine.clone()
    }

    fn config_dir(&self) -> PathBuf {
        PathBuf::from(format!("/fake/configs/engines/{}", self.engine.as_str()))
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        let database = self.database.lock().expect("namespace database");
        let active = database.active.get(&self.engine);
        Ok(database
            .profiles
            .iter()
            .filter(|(key, _)| key.engine == self.engine)
            .map(|(key, profile)| ProfileInfo {
                name: key.profile.as_str().to_owned(),
                active: active == Some(&key.profile),
                path: self
                    .config_dir()
                    .join(format!("{}.yaml", key.profile.as_str()))
                    .to_string_lossy()
                    .into_owned(),
                subscription_url: profile.metadata.subscription_url.clone(),
                has_backup: matches!(profile.backup, BackupState::Present(_)),
                ..ProfileInfo::default()
            })
            .collect())
    }

    async fn get_current(&self) -> Result<String, PortError> {
        self.database
            .lock()
            .expect("namespace database")
            .active
            .get(&self.engine)
            .map(|profile| profile.as_str().to_owned())
            .ok_or_else(|| PortError::NotFound("active profile".into()))
    }

    async fn read_active_profile_reference(
        &self,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        let database = self.database.lock().expect("namespace database");
        Ok(match database.active.get(&self.engine) {
            Some(profile) => ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
                self.engine.clone(),
                profile.clone(),
                ActiveProfileReferenceRevision::new(1),
            )),
            None => ActiveProfileReferenceState::Unselected {
                revision: ActiveProfileReferenceRevision::default(),
            },
        })
    }

    async fn load(&self, profile: &str) -> Result<String, PortError> {
        let profile = ProfileId::new(profile).map_err(|error| PortError::Io(error.to_string()))?;
        let database = self.database.lock().expect("namespace database");
        let stored = database
            .profiles
            .get(&self.key(&profile))
            .ok_or_else(|| PortError::NotFound(profile.as_str().to_owned()))?;
        String::from_utf8(stored.document.clone()).map_err(|error| PortError::Io(error.to_string()))
    }

    async fn save_inactive_profile(&self, profile: &str, content: &str) -> Result<(), PortError> {
        let profile = ProfileId::new(profile).map_err(|error| PortError::Io(error.to_string()))?;
        let mut database = self.database.lock().expect("namespace database");
        let stored = database
            .profiles
            .get_mut(&self.key(&profile))
            .ok_or_else(|| PortError::NotFound(profile.as_str().to_owned()))?;
        stored.backup = BackupState::Present(stored.document.clone());
        stored.document = content.as_bytes().to_vec();
        Ok(())
    }

    async fn delete_profile(&self, profile: &str) -> Result<(), PortError> {
        let profile = ProfileId::new(profile).map_err(|error| PortError::Io(error.to_string()))?;
        let mut database = self.database.lock().expect("namespace database");
        let key = self.key(&profile);
        database
            .profiles
            .remove(&key)
            .ok_or_else(|| PortError::NotFound(profile.as_str().to_owned()))?;
        database.credentials.remove(&key);
        Ok(())
    }

    async fn get_profile_metadata(&self, profile: &str) -> Result<ProfileMetadata, PortError> {
        let profile = ProfileId::new(profile).map_err(|error| PortError::Io(error.to_string()))?;
        self.database
            .lock()
            .expect("namespace database")
            .profiles
            .get(&self.key(&profile))
            .map(|stored| stored.metadata.clone())
            .ok_or_else(|| PortError::NotFound(profile.as_str().to_owned()))
    }

    async fn update_profile_metadata(
        &self,
        profile: &str,
        metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        let profile = ProfileId::new(profile).map_err(|error| PortError::Io(error.to_string()))?;
        self.database
            .lock()
            .expect("namespace database")
            .profiles
            .get_mut(&self.key(&profile))
            .map(|stored| stored.metadata = metadata.clone())
            .ok_or_else(|| PortError::NotFound(profile.as_str().to_owned()))
    }

    async fn delete_subscription_credential(&self, profile: &str) -> Result<(), PortError> {
        let profile = ProfileId::new(profile).map_err(|error| PortError::Io(error.to_string()))?;
        self.database
            .lock()
            .expect("namespace database")
            .credentials
            .insert(self.key(&profile), CredentialState::Absent);
        Ok(())
    }

    async fn load_workspace(&self, profile: &ProfileId) -> Result<ProfileWorkspace, PortError> {
        let database = self.database.lock().expect("namespace database");
        let stored = database
            .profiles
            .get(&self.key(profile))
            .ok_or_else(|| PortError::NotFound(profile.as_str().to_owned()))?;
        Ok(workspace(&self.engine, profile, stored))
    }

    async fn compare_and_save_inactive_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        if expected.document.engine_id != self.engine {
            return Err(PortError::NotFound(
                expected.document.profile_id.as_str().to_owned(),
            ));
        }
        let profile = expected.document.profile_id.clone();
        let mut database = self.database.lock().expect("namespace database");
        let stored = database
            .profiles
            .get_mut(&self.key(&profile))
            .ok_or_else(|| PortError::NotFound(profile.as_str().to_owned()))?;
        if workspace(&self.engine, &profile, stored).identity() != *expected {
            return Err(PortError::NotFound("stale profile workspace".into()));
        }
        stored.document.clone_from(&update.document_bytes);
        stored.sidecar = match &update.sidecar_bytes {
            Some(bytes) => SidecarState::Present(bytes.clone()),
            None => SidecarState::Absent,
        };
        Ok(workspace(&self.engine, &profile, stored))
    }

    async fn load_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        let profile = ProfileId::new(self.get_current().await?)
            .map_err(|error| PortError::Io(error.to_string()))?;
        self.load_workspace(&profile).await
    }

    async fn clear_backup(&self, profile: &str) -> Result<(), PortError> {
        let profile = ProfileId::new(profile).map_err(|error| PortError::Io(error.to_string()))?;
        self.database
            .lock()
            .expect("namespace database")
            .profiles
            .get_mut(&self.key(&profile))
            .map(|stored| stored.backup = BackupState::Absent)
            .ok_or_else(|| PortError::NotFound(profile.as_str().to_owned()))
    }

    async fn restore_inactive_profile_backup(&self, profile: &str) -> Result<bool, PortError> {
        let profile = ProfileId::new(profile).map_err(|error| PortError::Io(error.to_string()))?;
        let mut database = self.database.lock().expect("namespace database");
        let stored = database
            .profiles
            .get_mut(&self.key(&profile))
            .ok_or_else(|| PortError::NotFound(profile.as_str().to_owned()))?;
        match mem::replace(&mut stored.backup, BackupState::Absent) {
            BackupState::Absent => Ok(false),
            BackupState::Present(bytes) => {
                stored.document = bytes;
                Ok(true)
            }
        }
    }
}

#[tokio::test]
async fn same_profile_id_keeps_documents_options_active_metadata_credentials_and_backups_per_engine()
 {
    let database = Arc::new(Mutex::new(NamespaceDatabase::default()));
    let mihomo = NamespaceStore::new("mihomo", database.clone());
    let sing_box = NamespaceStore::new("sing-box", database.clone());
    mihomo.seed(
        "main",
        b"# mihomo\r\nmode: rule\r\n",
        Some(b"# mihomo options\r\n"),
        "https://mihomo.example/sub",
        "mihomo-secret",
        b"# old mihomo\r\nmode: direct\r\n",
        true,
    );
    mihomo.seed(
        "alternate",
        b"# mihomo alternate\nmode: direct\n",
        None,
        "https://mihomo.example/alternate",
        "mihomo-alternate-secret",
        b"# old mihomo alternate\n",
        false,
    );
    sing_box.seed(
        "main",
        b"{\r\n  \"log\": {\"level\": \"info\"}\r\n}",
        Some(b"# sing-box options\r\n"),
        "https://sing-box.example/sub",
        "sing-box-secret",
        b"{\"log\": {\"level\": \"warn\"}}",
        false,
    );
    sing_box.seed(
        "alternate",
        b"{\"log\": {\"level\": \"debug\"}}",
        None,
        "https://sing-box.example/alternate",
        "sing-box-alternate-secret",
        b"{\"log\": {\"level\": \"error\"}}",
        true,
    );

    let mihomo_app = ProfileApplication::new(Arc::new(mihomo.clone()));
    let sing_box_app = ProfileApplication::new(Arc::new(sing_box.clone()));
    let main_id = ProfileId::new("main").unwrap();
    let mihomo_workspace = mihomo_app
        .load_workspace(&profile_id("main"))
        .await
        .unwrap();
    let sing_box_workspace = sing_box_app
        .load_workspace(&profile_id("main"))
        .await
        .unwrap();

    assert_eq!(mihomo_workspace.document.identity.engine_id, mihomo.engine);
    assert_eq!(
        sing_box_workspace.document.identity.engine_id,
        sing_box.engine
    );
    assert_eq!(mihomo_workspace.document.identity.profile_id, main_id);
    assert_eq!(sing_box_workspace.document.identity.profile_id, main_id);
    assert_eq!(
        mihomo_workspace.document.raw_bytes,
        b"# mihomo\r\nmode: rule\r\n"
    );
    assert_eq!(
        sing_box_workspace.document.raw_bytes,
        b"{\r\n  \"log\": {\"level\": \"info\"}\r\n}"
    );
    assert_eq!(
        mihomo_workspace.sidecar_bytes.as_deref(),
        Some(b"# mihomo options\r\n".as_slice())
    );
    assert_eq!(
        sing_box_workspace.sidecar_bytes.as_deref(),
        Some(b"# sing-box options\r\n".as_slice())
    );
    assert_eq!(mihomo_app.current_profile().await.unwrap(), "main");
    assert_eq!(sing_box_app.current_profile().await.unwrap(), "alternate");
    assert_eq!(
        mihomo_app
            .load_metadata("main")
            .await
            .unwrap()
            .subscription_url
            .as_deref(),
        Some("https://mihomo.example/sub")
    );
    assert_eq!(
        sing_box_app
            .load_metadata("main")
            .await
            .unwrap()
            .subscription_url
            .as_deref(),
        Some("https://sing-box.example/sub")
    );

    mihomo_app
        .delete_subscription_credential("main")
        .await
        .unwrap();
    assert!(matches!(mihomo.credential("main"), CredentialState::Absent));
    assert!(matches!(
        sing_box.credential("main"),
        CredentialState::Present(value) if value == "sing-box-secret"
    ));

    assert_eq!(
        mihomo_app
            .restore_inactive_profile_backup("main")
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidState
    );
    assert!(
        mihomo_app
            .restore_inactive_profile_backup("alternate")
            .await
            .unwrap()
    );
    assert_eq!(
        mihomo.load("alternate").await.unwrap(),
        "# old mihomo alternate\n"
    );
    assert!(matches!(mihomo.backup("alternate"), BackupState::Absent));
    assert!(matches!(
        mihomo.backup("main"),
        BackupState::Present(bytes) if bytes == b"# old mihomo\r\nmode: direct\r\n"
    ));
    assert_eq!(
        mihomo.load("main").await.unwrap(),
        "# mihomo\r\nmode: rule\r\n"
    );
    assert!(matches!(
        sing_box.backup("main"),
        BackupState::Present(bytes) if bytes == b"{\"log\": {\"level\": \"warn\"}}"
    ));
    assert_eq!(
        sing_box.load("main").await.unwrap(),
        "{\r\n  \"log\": {\"level\": \"info\"}\r\n}"
    );
    assert!(
        sing_box_app
            .restore_inactive_profile_backup("main")
            .await
            .unwrap()
    );
    assert_eq!(
        sing_box.load("main").await.unwrap(),
        "{\"log\": {\"level\": \"warn\"}}"
    );
}

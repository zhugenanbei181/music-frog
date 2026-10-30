//! Isolated profile-store port fixture.
//!
//! The store keeps its documents in memory and publishes them through the
//! typed [`ProfileStore`] port. It holds one small profile, one large profile
//! (so an editor capture can render a bounded window over a real document)
//! and the aggregation-template library. Every refusal is typed: an unknown
//! profile is `NotFound`, writing the active profile is refused, and a stale
//! workspace identity is refused with a `NotReady` failure.

use crate::profile_workspace_test_support::workspace_with_raw_sidecar;
use async_trait::async_trait;
use infiltrator_contract::aggregator::AggregationTemplate;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceCasOutcome, ActiveProfileReferenceRevision,
    ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::profile_document_identity::{ProfileId, ProfileWorkspaceIdentity};
use lurker_core_ports::error::PortError;
use std::collections::BTreeMap;
use std::mem::replace;
use std::path::PathBuf;
use std::sync::Mutex;

/// The engine the capture store owns.
pub const CAPTURE_ENGINE: &str = "mihomo";
/// The small profile the capture store publishes.
pub const CAPTURE_PROFILE: &str = "capture-profile";
/// The large profile the capture store publishes for editor captures.
pub const CAPTURE_LARGE_PROFILE: &str = "capture-large-profile";
/// The number of routing rules the large document carries.
pub const LARGE_PROFILE_RULE_COUNT: usize = 4_000;
/// The number of routing rules the 50,000-rule document carries for stress scaling.
pub const LARGE_PROFILE_50K_RULE_COUNT: usize = 50_000;

/// The small document the capture store publishes.
pub const CAPTURE_PROFILE_DOCUMENT: &str =
    "mixed-port: 7890\nproxies: []\nproxy-groups: []\nrules:\n  - MATCH,DIRECT\n";

/// Builds a large but real Mihomo document with `rule_count` routing rules.
///
/// The document is generated instead of stored as a literal so the fixture
/// stays reviewable while still giving an editor capture a document whose
/// line count is far beyond one viewport.
pub fn large_profile_document(rule_count: usize) -> String {
    let mut document = String::from("mixed-port: 7890\nproxies: []\nproxy-groups: []\nrules:\n");
    for index in 0..rule_count {
        document.push_str(&format!(
            "  - DOMAIN-SUFFIX,node-{index}.example.com,DIRECT\n"
        ));
    }
    document
}

/// Builds a 50,000-rule Mihomo document for editor scaling benchmarks and tests.
pub fn large_profile_document_50k() -> String {
    large_profile_document(LARGE_PROFILE_50K_RULE_COUNT)
}

/// An isolated profile store that publishes real documents and metadata.
pub struct ProfileStoreCapture {
    engine_id: EngineId,
    files: Mutex<BTreeMap<String, String>>,
    metadata: Mutex<BTreeMap<String, ProfileMetadata>>,
    backups: Mutex<BTreeMap<String, String>>,
    credentials: Mutex<BTreeMap<String, String>>,
    templates: Mutex<Vec<AggregationTemplate>>,
    active_reference: Mutex<ActiveProfileReferenceState>,
}

impl Default for ProfileStoreCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileStoreCapture {
    pub fn new() -> Self {
        let engine_id = EngineId::new(CAPTURE_ENGINE).expect("valid capture engine ID");
        let mut files = BTreeMap::new();
        files.insert(
            CAPTURE_PROFILE.to_owned(),
            CAPTURE_PROFILE_DOCUMENT.to_owned(),
        );
        files.insert(
            CAPTURE_LARGE_PROFILE.to_owned(),
            large_profile_document(LARGE_PROFILE_RULE_COUNT),
        );
        let active_reference = ActiveProfileReference::new(
            engine_id.clone(),
            ProfileId::new(CAPTURE_PROFILE).expect("valid capture profile ID"),
            ActiveProfileReferenceRevision::new(1),
        );
        let mut metadata = BTreeMap::new();
        metadata.insert(CAPTURE_PROFILE.to_owned(), ProfileMetadata::default());
        metadata.insert(CAPTURE_LARGE_PROFILE.to_owned(), ProfileMetadata::default());
        Self {
            engine_id,
            files: Mutex::new(files),
            metadata: Mutex::new(metadata),
            backups: Mutex::new(BTreeMap::new()),
            credentials: Mutex::new(BTreeMap::new()),
            templates: Mutex::new(Vec::new()),
            active_reference: Mutex::new(ActiveProfileReferenceState::Selected(active_reference)),
        }
    }

    /// The profile names the store currently holds.
    pub fn profile_names(&self) -> Vec<String> {
        self.files
            .lock()
            .expect("profile capture files")
            .keys()
            .cloned()
            .collect()
    }

    /// The aggregation-template library the store publishes.
    pub fn template_library(&self) -> Vec<AggregationTemplate> {
        self.templates
            .lock()
            .expect("profile capture templates")
            .clone()
    }

    /// Stores a subscription credential for one profile.
    pub fn set_credential(&self, profile: &str, credential: &str) {
        self.credentials
            .lock()
            .expect("profile capture credentials")
            .insert(profile.to_owned(), credential.to_owned());
    }

    /// Stores a backup copy for one profile.
    pub fn set_backup(&self, profile: &str, content: &str) {
        self.backups
            .lock()
            .expect("profile capture backups")
            .insert(profile.to_owned(), content.to_owned());
    }

    fn active_profile(&self) -> Option<String> {
        match &*self
            .active_reference
            .lock()
            .expect("profile capture reference")
        {
            ActiveProfileReferenceState::Selected(reference) => {
                Some(reference.profile_id.as_str().to_owned())
            }
            ActiveProfileReferenceState::Unselected { .. } => None,
        }
    }

    fn workspace_for(&self, profile: &str) -> Result<ProfileWorkspace, PortError> {
        let content = self
            .files
            .lock()
            .expect("profile capture files")
            .get(profile)
            .cloned()
            .ok_or_else(|| PortError::NotFound(profile.to_owned()))?;
        workspace_with_raw_sidecar(profile, content.as_bytes(), None, None)
    }
}

#[async_trait]
impl ProfileStore for ProfileStoreCapture {
    fn engine_id(&self) -> EngineId {
        self.engine_id.clone()
    }

    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/isolated-profile-store")
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        let active = self.active_profile();
        let metadata = self
            .metadata
            .lock()
            .expect("profile capture metadata")
            .clone();
        let backups = self
            .backups
            .lock()
            .expect("profile capture backups")
            .clone();
        Ok(self
            .files
            .lock()
            .expect("profile capture files")
            .keys()
            .map(|name| {
                let stored = metadata.get(name).cloned().unwrap_or_default();
                ProfileInfo {
                    name: name.clone(),
                    active: active.as_deref() == Some(name.as_str()),
                    path: format!("/isolated-profile-store/{name}.yaml"),
                    has_backup: backups.contains_key(name),
                    subscription_url: stored.subscription_url,
                    auto_update_enabled: stored.auto_update_enabled,
                    update_interval_hours: stored.update_interval_hours,
                    last_updated: stored.last_updated,
                    next_update: stored.next_update,
                    traffic_upload: stored.traffic_upload,
                    traffic_download: stored.traffic_download,
                    traffic_total: stored.traffic_total,
                    expire_at: stored.expire_at,
                    user_agent: stored.user_agent,
                    etag: stored.etag,
                    last_modified: stored.last_modified,
                    cron_expression: stored.cron_expression,
                    insecure_skip_verify: stored.insecure_skip_verify,
                    auto_reload_core: stored.auto_reload_core,
                    controller_url: None,
                    controller_changed: None,
                }
            })
            .collect())
    }

    async fn get_current(&self) -> Result<String, PortError> {
        self.active_profile().ok_or_else(|| {
            PortError::unsupported(
                Capability::Profiles,
                "capture profile store has no selected active profile",
            )
        })
    }

    async fn read_active_profile_reference(
        &self,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        Ok(self
            .active_reference
            .lock()
            .expect("profile capture reference")
            .clone())
    }

    async fn compare_and_set_active_profile_reference(
        &self,
        expected: ActiveProfileReferenceState,
        target: ActiveProfileReferenceTarget,
        _: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<ActiveProfileReferenceCasOutcome, PortError> {
        let mut active_reference = self
            .active_reference
            .lock()
            .expect("profile capture reference");
        if *active_reference != expected {
            return Ok(ActiveProfileReferenceCasOutcome::Stale {
                expected,
                observed: active_reference.clone(),
            });
        }
        let revision = expected
            .revision()
            .next()
            .ok_or_else(|| PortError::Io("capture active-profile revision overflow".into()))?;
        let current = match target {
            ActiveProfileReferenceTarget::Unselected => {
                ActiveProfileReferenceState::Unselected { revision }
            }
            ActiveProfileReferenceTarget::Selected {
                engine_id,
                profile_id,
            } => {
                if engine_id != self.engine_id() {
                    return Err(PortError::unsupported(
                        Capability::Profiles,
                        "capture profile store cannot select a foreign engine",
                    ));
                }
                if !self
                    .files
                    .lock()
                    .expect("profile capture files")
                    .contains_key(profile_id.as_str())
                {
                    return Err(PortError::NotFound(profile_id.as_str().to_owned()));
                }
                ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
                    engine_id, profile_id, revision,
                ))
            }
        };
        let previous = replace(&mut *active_reference, current.clone());
        Ok(ActiveProfileReferenceCasOutcome::Committed { previous, current })
    }

    async fn load(&self, profile: &str) -> Result<String, PortError> {
        self.files
            .lock()
            .expect("profile capture files")
            .get(profile)
            .cloned()
            .ok_or_else(|| PortError::NotFound(profile.to_owned()))
    }

    async fn save_inactive_profile(&self, profile: &str, content: &str) -> Result<(), PortError> {
        if self.active_profile().as_deref() == Some(profile) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                format!("profile {profile} is active and cannot be replaced in place"),
                false,
            )));
        }
        self.files
            .lock()
            .expect("profile capture files")
            .insert(profile.to_owned(), content.to_owned());
        Ok(())
    }

    async fn delete_profile(&self, profile: &str) -> Result<(), PortError> {
        if self.active_profile().as_deref() == Some(profile) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                format!("profile {profile} is active and cannot be deleted"),
                false,
            )));
        }
        let mut files = self.files.lock().expect("profile capture files");
        if files.remove(profile).is_none() {
            return Err(PortError::NotFound(profile.to_owned()));
        }
        drop(files);
        self.metadata
            .lock()
            .expect("profile capture metadata")
            .remove(profile);
        self.credentials
            .lock()
            .expect("profile capture credentials")
            .remove(profile);
        Ok(())
    }

    async fn get_profile_metadata(&self, profile: &str) -> Result<ProfileMetadata, PortError> {
        self.metadata
            .lock()
            .expect("profile capture metadata")
            .get(profile)
            .cloned()
            .ok_or_else(|| PortError::NotFound(profile.to_owned()))
    }

    async fn update_profile_metadata(
        &self,
        profile: &str,
        metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        let mut stored = self.metadata.lock().expect("profile capture metadata");
        if !stored.contains_key(profile) {
            return Err(PortError::NotFound(profile.to_owned()));
        }
        stored.insert(profile.to_owned(), metadata.clone());
        Ok(())
    }

    async fn delete_subscription_credential(&self, profile: &str) -> Result<(), PortError> {
        self.credentials
            .lock()
            .expect("profile capture credentials")
            .remove(profile);
        Ok(())
    }

    async fn clear_backup(&self, profile: &str) -> Result<(), PortError> {
        self.backups
            .lock()
            .expect("profile capture backups")
            .remove(profile);
        Ok(())
    }

    async fn restore_inactive_profile_backup(&self, profile: &str) -> Result<bool, PortError> {
        if self.active_profile().as_deref() == Some(profile) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                format!("profile {profile} is active and cannot be restored from a backup"),
                false,
            )));
        }
        let backup = self
            .backups
            .lock()
            .expect("profile capture backups")
            .get(profile)
            .cloned();
        let Some(backup) = backup else {
            return Ok(false);
        };
        self.files
            .lock()
            .expect("profile capture files")
            .insert(profile.to_owned(), backup);
        Ok(true)
    }

    async fn load_workspace(&self, profile: &ProfileId) -> Result<ProfileWorkspace, PortError> {
        self.workspace_for(profile.as_str())
    }

    async fn load_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        let profile = self.active_profile().ok_or_else(|| {
            PortError::unsupported(
                Capability::Profiles,
                "capture profile store has no selected active profile",
            )
        })?;
        self.workspace_for(&profile)
    }

    async fn compare_and_save_inactive_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        let profile = expected.document.profile_id.as_str();
        if self.active_profile().as_deref() == Some(profile) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                format!("profile {profile} is active and needs the active workspace lease"),
                false,
            )));
        }
        let observed = self.workspace_for(profile)?;
        if observed.identity() != *expected {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "profile document changed since the observed workspace identity",
                true,
            )));
        }
        let content = String::from_utf8(update.document_bytes.clone())
            .map_err(|error| PortError::Io(error.to_string()))?;
        self.files
            .lock()
            .expect("profile capture files")
            .insert(profile.to_owned(), content);
        workspace_with_raw_sidecar(
            profile,
            &update.document_bytes,
            update.sidecar_bytes.clone(),
            None,
        )
    }

    async fn load_aggregation_templates(&self) -> Result<Vec<AggregationTemplate>, PortError> {
        Ok(self.template_library())
    }

    async fn save_aggregation_templates(
        &self,
        templates: &[AggregationTemplate],
    ) -> Result<(), PortError> {
        *self.templates.lock().expect("profile capture templates") = templates.to_vec();
        Ok(())
    }
}

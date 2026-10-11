//! Focused unit tests for the shared profile application and its
//! conditional subscription lifecycle.

use super::*;
use crate::profile_document_codec_test_support::TestProfileDocumentCodec;
use crate::profile_workspace_test_support::{workspace_with_origin, workspace_with_raw_sidecar};
use async_trait::async_trait;
use chrono::Timelike;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::profile_document_identity::{ProfileId, ProfileWorkspaceIdentity};
use infiltrator_contract::profile_protection::ProfileWriteAuthorization;
use infiltrator_domain::profile_options::ProfileOptions;
use infiltrator_domain::profiles::ProfileMetadata;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_workspace::ProfileWorkspace;
use infiltrator_ports::subscription_source::SubscriptionDocument;
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Default)]
struct FakeStore {
    current: Mutex<String>,
    profiles: Mutex<BTreeMap<String, (String, ProfileMetadata)>>,
    sidecars: Mutex<BTreeMap<String, Vec<u8>>>,
    options_read_error: Mutex<Option<PortError>>,
    workspace_write_error: Mutex<Option<PortError>>,
    list_read_error: Mutex<Option<PortError>>,
    metadata_read_error: Mutex<Option<PortError>>,
    deleted_options: Mutex<Vec<String>>,
    cleared_backups: Mutex<Vec<String>>,
    restorable_backups: Mutex<Vec<String>>,
    restored_backups: Mutex<Vec<String>>,
    workspace_reads: AtomicUsize,
    workspace_writes: AtomicUsize,
    foreign_workspace_receipt: AtomicBool,
    stale_document_revision_receipt: AtomicBool,
    stale_sidecar_revision_receipt: AtomicBool,
}

fn profiles_with_mihomo_codec(store: Arc<dyn ProfileStore>) -> ProfileApplication {
    ProfileApplication::new(store).with_document_codec(Arc::new(TestProfileDocumentCodec::mihomo()))
}

impl FakeStore {
    fn with_profile(name: &str, content: &str, active: bool) -> Self {
        let mut profiles = BTreeMap::new();
        profiles.insert(
            name.to_string(),
            (content.to_string(), ProfileMetadata::default()),
        );
        Self {
            current: Mutex::new(if active {
                name.to_string()
            } else {
                String::new()
            }),
            profiles: Mutex::new(profiles),
            ..Self::default()
        }
    }

    fn set_options(&self, profile: &str, options: &ProfileOptions) {
        let bytes = serde_yaml_ng::to_string(options)
            .expect("serialize Mihomo options fixture")
            .into_bytes();
        self.sidecars
            .lock()
            .expect("sidecars lock")
            .insert(profile.to_owned(), bytes);
    }
}

async fn load_workspace(store: &FakeStore, profile: &str) -> ProfileWorkspace {
    let profile = ProfileId::new(profile).expect("valid profile ID");
    ProfileStore::load_workspace(store, &profile)
        .await
        .expect("profile workspace")
}

#[async_trait]
impl ProfileStore for FakeStore {
    fn engine_id(&self) -> EngineId {
        EngineId::new("mihomo").expect("test engine ID")
    }

    async fn load_workspace(&self, profile: &ProfileId) -> Result<ProfileWorkspace, PortError> {
        self.workspace_reads.fetch_add(1, Ordering::SeqCst);
        let profile = profile.as_str();
        if let Some(error) = self.options_read_error.lock().unwrap().clone() {
            return Err(error);
        }
        let profiles = self.profiles.lock().unwrap();
        let sidecars = self.sidecars.lock().unwrap();
        let (content, metadata) = profiles
            .get(profile)
            .ok_or_else(|| PortError::NotFound(profile.into()))?;
        let mut result = workspace_with_raw_sidecar(
            profile,
            content.as_bytes(),
            sidecars.get(profile).cloned(),
            metadata.subscription_url.as_deref(),
        )?;
        result.write_protection = ProfileWriteProtection::from_subscription_url(
            metadata.subscription_url.as_deref().unwrap_or_default(),
        );
        Ok(result)
    }

    async fn load_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        let profile = self.current.lock().unwrap().clone();
        let profile = ProfileId::new(profile).map_err(|error| PortError::Io(error.to_string()))?;
        self.load_workspace(&profile).await
    }

    async fn compare_and_save_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        let profile = expected.document.profile_id.as_str();
        let mut profiles = self.profiles.lock().unwrap();
        let mut sidecars = self.sidecars.lock().unwrap();
        let (content, metadata) = profiles
            .get(profile)
            .ok_or_else(|| PortError::NotFound(profile.into()))?;
        if workspace_with_raw_sidecar(
            profile,
            content.as_bytes(),
            sidecars.get(profile).cloned(),
            metadata.subscription_url.as_deref(),
        )?
        .identity()
            != *expected
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "changed source",
                true,
            )));
        }
        let protection = ProfileWriteProtection::from_subscription_url(
            metadata.subscription_url.as_deref().unwrap_or_default(),
        );
        if matches!(
            update.purpose,
            ProfileWorkspacePurpose::DirectEdit {
                authorization: ProfileWriteAuthorization::RequireUnlock
            }
        ) && protection.is_protected()
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                "protected direct edit",
                false,
            )));
        }
        if let Some(error) = self.workspace_write_error.lock().unwrap().clone() {
            return Err(error);
        }
        let content = String::from_utf8(update.document_bytes.clone())
            .map_err(|error| PortError::Io(error.to_string()))?;
        let mut result = workspace_with_raw_sidecar(
            profile,
            &update.document_bytes,
            update.sidecar_bytes.clone(),
            metadata.subscription_url.as_deref(),
        )?;
        result.write_protection = protection;
        profiles.get_mut(profile).unwrap().0.clone_from(&content);
        match update.sidecar_bytes.as_ref() {
            Some(sidecar) => {
                sidecars.insert(profile.to_owned(), sidecar.clone());
            }
            None => {
                sidecars.remove(profile);
            }
        }
        self.workspace_writes.fetch_add(1, Ordering::SeqCst);
        if self.foreign_workspace_receipt.load(Ordering::SeqCst) {
            result.document.identity.engine_id = EngineId::new("sing-box").expect("test engine ID");
        }
        if self.stale_document_revision_receipt.load(Ordering::SeqCst) {
            result.document.identity.revision = expected.document.revision.clone();
        }
        if self.stale_sidecar_revision_receipt.load(Ordering::SeqCst) {
            result.sidecar_revision = expected.sidecar_revision.clone();
        }
        Ok(result)
    }

    async fn compare_and_save_active_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        if self.current.lock().unwrap().as_str() != expected.document.profile_id.as_str() {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active profile changed",
                true,
            )));
        }
        self.compare_and_save_workspace(expected, update).await
    }
    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/fake/configs")
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        if let Some(error) = self.list_read_error.lock().unwrap().clone() {
            return Err(error);
        }
        let current = self.current.lock().expect("current lock").clone();
        Ok(self
            .profiles
            .lock()
            .expect("profiles lock")
            .iter()
            .map(|(name, (_content, metadata))| ProfileInfo {
                name: name.clone(),
                active: current == *name,
                path: format!("/fake/configs/{name}.yaml"),
                subscription_url: metadata.subscription_url.clone(),
                auto_update_enabled: metadata.auto_update_enabled,
                update_interval_hours: metadata.update_interval_hours,
                last_updated: metadata.last_updated,
                next_update: metadata.next_update,
                traffic_upload: metadata.traffic_upload,
                traffic_download: metadata.traffic_download,
                traffic_total: metadata.traffic_total,
                expire_at: metadata.expire_at,
                controller_url: None,
                controller_changed: None,
                user_agent: metadata.user_agent.clone(),
                etag: metadata.etag.clone(),
                last_modified: metadata.last_modified.clone(),
                cron_expression: metadata.cron_expression.clone(),
                insecure_skip_verify: metadata.insecure_skip_verify,
                auto_reload_core: metadata.auto_reload_core,
                has_backup: self
                    .restorable_backups
                    .lock()
                    .expect("backup lock")
                    .contains(name),
            })
            .collect())
    }

    async fn get_current(&self) -> Result<String, PortError> {
        Ok(self.current.lock().expect("current lock").clone())
    }

    async fn set_current(&self, profile: &str) -> Result<(), PortError> {
        if !self
            .profiles
            .lock()
            .expect("profiles lock")
            .contains_key(profile)
        {
            return Err(PortError::NotFound(profile.to_string()));
        }
        *self.current.lock().expect("current lock") = profile.to_string();
        Ok(())
    }

    async fn load(&self, profile: &str) -> Result<String, PortError> {
        self.profiles
            .lock()
            .expect("profiles lock")
            .get(profile)
            .map(|(content, _)| content.clone())
            .ok_or_else(|| PortError::NotFound(profile.to_string()))
    }

    async fn save(&self, profile: &str, content: &str) -> Result<(), PortError> {
        self.profiles
            .lock()
            .expect("profiles lock")
            .entry(profile.to_string())
            .or_insert_with(|| (String::new(), ProfileMetadata::default()))
            .0 = content.to_string();
        Ok(())
    }

    async fn delete_profile(&self, profile: &str) -> Result<(), PortError> {
        self.profiles
            .lock()
            .expect("profiles lock")
            .remove(profile)
            .map(|_| ())
            .ok_or_else(|| PortError::NotFound(profile.to_string()))?;
        self.deleted_options
            .lock()
            .expect("options lock")
            .push(profile.to_owned());
        self.sidecars.lock().expect("sidecars lock").remove(profile);
        Ok(())
    }

    async fn get_profile_metadata(&self, profile: &str) -> Result<ProfileMetadata, PortError> {
        if let Some(error) = self.metadata_read_error.lock().unwrap().clone() {
            return Err(error);
        }
        self.profiles
            .lock()
            .expect("profiles lock")
            .get(profile)
            .map(|(_, metadata)| metadata.clone())
            .ok_or_else(|| PortError::NotFound(profile.to_string()))
    }

    async fn update_profile_metadata(
        &self,
        profile: &str,
        metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        self.profiles
            .lock()
            .expect("profiles lock")
            .get_mut(profile)
            .map(|(_, current)| *current = metadata.clone())
            .ok_or_else(|| PortError::NotFound(profile.to_string()))
    }

    async fn delete_subscription_credential(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn clear_backup(&self, profile: &str) -> Result<(), PortError> {
        self.cleared_backups
            .lock()
            .expect("backup lock")
            .push(profile.to_string());
        Ok(())
    }

    async fn restore_backup(&self, profile: &str) -> Result<bool, PortError> {
        let mut restorable = self.restorable_backups.lock().expect("backup lock");
        if let Some(index) = restorable.iter().position(|name| name == profile) {
            restorable.remove(index);
            self.restored_backups
                .lock()
                .expect("backup lock")
                .push(profile.to_string());
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

struct FakeSource {
    result: Mutex<Option<ConditionalDocumentResult>>,
    calls: Mutex<Vec<ConditionalFetchHeaders>>,
}

impl FakeSource {
    fn modified(content: &str, etag: Option<&str>) -> Self {
        Self {
            result: Mutex::new(Some(ConditionalDocumentResult::Modified {
                document: SubscriptionDocument {
                    content: content.to_string(),
                    userinfo: Some(SubscriptionUserInfo {
                        upload: Some(90),
                        download: Some(910),
                        total: Some(1000),
                        expire: Some(2_000_000_000),
                    }),
                },
                etag: etag.map(str::to_string),
                last_modified: Some("Wed, 21 Oct 2026 07:28:00 GMT".to_string()),
            })),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn not_modified(etag: Option<&str>) -> Self {
        Self {
            result: Mutex::new(Some(ConditionalDocumentResult::NotModified {
                userinfo: Some(SubscriptionUserInfo {
                    upload: Some(90),
                    download: Some(910),
                    total: Some(1000),
                    expire: Some(2_000_000_000),
                }),
                etag: etag.map(str::to_string),
                last_modified: None,
            })),
            calls: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl SubscriptionSource for FakeSource {
    async fn fetch(
        &self,
        _profile: &str,
        _url: &CheckedSubscriptionUrl,
    ) -> Result<SubscriptionDocument, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "use fetch_conditional",
        ))
    }

    async fn fetch_conditional(
        &self,
        _profile: &str,
        _url: &CheckedSubscriptionUrl,
        headers: &ConditionalFetchHeaders,
    ) -> Result<ConditionalDocumentResult, PortError> {
        self.calls.lock().expect("calls lock").push(headers.clone());
        Ok(self
            .result
            .lock()
            .expect("result lock")
            .clone()
            .expect("result set"))
    }
}

async fn subscription_store(
    etag: Option<&str>,
    last_modified: Option<&str>,
    user_agent: Option<&str>,
    insecure_skip_verify: bool,
) -> Arc<FakeStore> {
    let store = Arc::new(FakeStore::with_profile(
        "main",
        "proxies:\n  - name: n1\n    type: ss\n",
        true,
    ));
    {
        let mut profiles = store.profiles.lock().expect("profiles lock");
        let metadata = &mut profiles.get_mut("main").expect("profile").1;
        metadata.subscription_url = Some("https://example.com/sub.yaml".to_string());
        metadata.auto_update_enabled = true;
        metadata.update_interval_hours = Some(12);
        metadata.etag = etag.map(str::to_string);
        metadata.last_modified = last_modified.map(str::to_string);
        metadata.user_agent = user_agent.map(str::to_string);
        metadata.insecure_skip_verify = insecure_skip_verify;
    }
    store
}

// ---- DUAL-09-12: remote-subscription write protection -----------------------

/// Mark a fake profile as downloaded from a subscription URL.
fn mark_subscription(store: &FakeStore, profile: &str, url: &str) {
    let mut profiles = store.profiles.lock().expect("profiles lock");
    profiles
        .get_mut(profile)
        .expect("profile exists")
        .1
        .subscription_url = Some(url.to_string());
}

// ---- DUAL-09-14: one option-sidecar use-case for both editor panes --------

#[path = "profile_application_tests/apply.rs"]
mod apply;
#[path = "profile_application_tests/auto.rs"]
mod auto;
#[path = "profile_application_tests/batch.rs"]
mod batch;
#[path = "profile_application_tests/conditional.rs"]
mod conditional;
#[path = "profile_application_tests/deletion.rs"]
mod deletion;
#[path = "profile_application_tests/edited.rs"]
mod edited;
#[path = "profile_application_tests/fetch.rs"]
mod fetch;
#[path = "profile_application_tests/filter.rs"]
mod filter;
#[path = "profile_application_tests/import.rs"]
mod import;
#[path = "profile_application_tests/inactive.rs"]
mod inactive;
#[path = "profile_application_tests/invalid.rs"]
mod invalid;
#[path = "profile_application_tests/list.rs"]
mod list;
#[path = "profile_application_tests/local.rs"]
mod local;
#[path = "profile_application_tests/mixin.rs"]
mod mixin;
#[path = "profile_application_tests/options.rs"]
mod options;
#[path = "profile_application_tests/restore.rs"]
mod restore;
#[path = "profile_application_tests/schedule.rs"]
mod schedule;
#[path = "profile_application_tests/selection.rs"]
mod selection;
#[path = "profile_application_tests/successful.rs"]
mod successful;
#[path = "profile_application_tests/update.rs"]
mod update;
#[path = "profile_application_tests/write.rs"]
mod write;

#[path = "profile_application_tests/editor_observations.rs"]
mod editor_observations;

#[path = "profile_application_tests/source_bound_edits.rs"]
mod source_bound_edits;

#[path = "profile_application_tests/engine_scope.rs"]
mod engine_scope;
#[path = "profile_application_tests/namespace.rs"]
mod namespace;
#[path = "profile_application_tests/quota_reader.rs"]
mod quota_reader;
#[path = "profile_application_tests/read_status.rs"]
mod read_status;

#[path = "profile_application_tests/configuration_codec.rs"]
mod configuration_codec;

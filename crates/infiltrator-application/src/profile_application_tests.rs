//! Focused unit tests for the shared profile application and its
//! conditional subscription lifecycle.

use super::*;
use crate::profile_workspace_test_support::workspace;
use async_trait::async_trait;
use chrono::Timelike;
use infiltrator_contract::capability::Capability;
use infiltrator_domain::profile_options::ProfileOptions;
use infiltrator_domain::profiles::ProfileMetadata;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_workspace::ProfileWorkspace;
use infiltrator_ports::subscription_source::SubscriptionDocument;
use std::collections::BTreeMap;
use std::sync::Mutex;

#[derive(Default)]
struct FakeStore {
    current: Mutex<String>,
    profiles: Mutex<BTreeMap<String, (String, ProfileMetadata)>>,
    options: Mutex<BTreeMap<String, ProfileOptions>>,
    options_read_error: Mutex<Option<PortError>>,
    list_read_error: Mutex<Option<PortError>>,
    metadata_read_error: Mutex<Option<PortError>>,
    deleted_options: Mutex<Vec<String>>,
    cleared_backups: Mutex<Vec<String>>,
    restorable_backups: Mutex<Vec<String>>,
    restored_backups: Mutex<Vec<String>>,
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
}

#[async_trait]
impl ProfileStore for FakeStore {
    async fn load_workspace(&self, profile: &str) -> Result<ProfileWorkspace, PortError> {
        if let Some(error) = self.options_read_error.lock().unwrap().clone() {
            return Err(error);
        }
        let profiles = self.profiles.lock().unwrap();
        let options = self.options.lock().unwrap();
        let (content, metadata) = profiles
            .get(profile)
            .ok_or_else(|| PortError::NotFound(profile.into()))?;
        let mut result = workspace(profile, content, options.get(profile))?;
        result.write_protection = ProfileWriteProtection::from_subscription_url(
            metadata.subscription_url.as_deref().unwrap_or_default(),
        );
        Ok(result)
    }

    async fn compare_and_save_workspace(
        &self,
        expected: &ProfileSourceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        let mut profiles = self.profiles.lock().unwrap();
        let mut options = self.options.lock().unwrap();
        let (content, metadata) = profiles
            .get(&expected.profile)
            .ok_or_else(|| PortError::NotFound(expected.profile.clone()))?;
        if workspace(&expected.profile, content, options.get(&expected.profile))?.source
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
                allow_protected: false
            }
        ) && protection.is_protected()
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                "protected direct edit",
                false,
            )));
        }
        let stored_options = (!update.options.is_empty()).then_some(&update.options);
        let mut result = workspace(&expected.profile, &update.content, stored_options)?;
        result.write_protection = protection;
        profiles
            .get_mut(&expected.profile)
            .unwrap()
            .0
            .clone_from(&update.content);
        match stored_options {
            Some(stored) => {
                options.insert(expected.profile.clone(), stored.clone());
            }
            None => {
                options.remove(&expected.profile);
            }
        }
        Ok(result)
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
            .ok_or_else(|| PortError::NotFound(profile.to_string()))
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

    async fn delete_options(&self, profile: &str) -> Result<(), PortError> {
        self.deleted_options
            .lock()
            .expect("options lock")
            .push(profile.to_string());
        self.options.lock().expect("options lock").remove(profile);
        Ok(())
    }

    async fn load_options(&self, profile: &str) -> Result<ProfileOptions, PortError> {
        if let Some(error) = self
            .options_read_error
            .lock()
            .expect("options error lock")
            .clone()
        {
            return Err(error);
        }
        Ok(self
            .options
            .lock()
            .expect("options lock")
            .get(profile)
            .cloned()
            .unwrap_or_default())
    }

    async fn save_options(&self, profile: &str, options: &ProfileOptions) -> Result<(), PortError> {
        self.options
            .lock()
            .expect("options lock")
            .insert(profile.to_string(), options.clone());
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

#[path = "profile_application_tests/quota_reader.rs"]
mod quota_reader;
#[path = "profile_application_tests/read_status.rs"]
mod read_status;

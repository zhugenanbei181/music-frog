//! Focused unit tests for the shared profile application and its
//! conditional subscription lifecycle.

use super::*;
use async_trait::async_trait;
use infiltrator_domain::profiles::ProfileMetadata;
use infiltrator_ports::error::PortError;
use std::collections::BTreeMap;
use std::sync::Mutex;

#[derive(Default)]
struct FakeStore {
    current: Mutex<String>,
    profiles: Mutex<BTreeMap<String, (String, ProfileMetadata)>>,
    deleted_options: Mutex<Vec<String>>,
    cleared_backups: Mutex<Vec<String>>,
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
    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/fake/configs")
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
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
        Ok(())
    }

    async fn clear_backup(&self, profile: &str) -> Result<(), PortError> {
        self.cleared_backups
            .lock()
            .expect("backup lock")
            .push(profile.to_string());
        Ok(())
    }

    async fn restore_backup(&self, _profile: &str) -> Result<bool, PortError> {
        Ok(false)
    }
}

#[tokio::test]
async fn list_and_detail_are_projected_from_the_store() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let application = ProfileApplication::new(store);

    let profiles = application.list_profiles().await.expect("list");
    assert_eq!(profiles.len(), 1);
    assert!(profiles[0].active);

    let detail = application
        .load_profile_detail("main")
        .await
        .expect("detail");
    assert_eq!(detail.content, "mode: rule\n");
    assert!(detail.active);
}

#[tokio::test]
async fn invalid_names_are_rejected_before_store_access() {
    let application = ProfileApplication::new(Arc::new(FakeStore::default()));
    let failure = application
        .load_profile_info("../outside")
        .await
        .expect_err("path-like name must fail");
    assert_eq!(failure.code, ErrorCode::InvalidInput);
}

#[tokio::test]
async fn selection_updates_the_current_profile() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", false));
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);

    let selected = application.select_profile("main").await.expect("select");
    assert!(selected.active);
    assert_eq!(
        application.current_profile().await.expect("current"),
        "main"
    );
}

#[tokio::test]
async fn deletion_cleans_the_profile_sidecar() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", false));
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);

    application.delete_profile("main").await.expect("delete");
    assert_eq!(
        store
            .deleted_options
            .lock()
            .expect("options lock")
            .as_slice(),
        &["main".to_string()]
    );
}

#[tokio::test]
async fn inactive_save_clears_the_transient_backup() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", false));
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let runtime: Option<Arc<dyn ManagedRuntime>> = None;

    application
        .save_profile_content(
            runtime,
            "main".to_string(),
            "mode: direct\n".to_string(),
            ApplyStrategy::PreferReload,
        )
        .await
        .expect("save");
    assert_eq!(store.load("main").await.expect("load"), "mode: direct\n");
    assert_eq!(
        store
            .cleared_backups
            .lock()
            .expect("backup lock")
            .as_slice(),
        &["main".to_string()]
    );
}

struct FakeSource {
    result: Mutex<Option<ConditionalDocumentResult>>,
    calls: Mutex<Vec<ConditionalFetchHeaders>>,
}

impl FakeSource {
    fn modified(content: &str, etag: Option<&str>) -> Self {
        Self {
            result: Mutex::new(Some(ConditionalDocumentResult::Modified {
                document: infiltrator_ports::subscription_source::SubscriptionDocument {
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
    ) -> Result<infiltrator_ports::subscription_source::SubscriptionDocument, PortError> {
        Err(PortError::unsupported(
            infiltrator_contract::capability::Capability::Profiles,
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

#[tokio::test]
async fn conditional_update_sends_stored_validators_and_persists_new_etag() {
    let store = subscription_store(
        Some("\"old\""),
        Some("Wed, 21 Oct 2026 07:28:00 GMT"),
        Some("UA-1"),
        true,
    )
    .await;
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let source = FakeSource::modified(
        "proxies:\n  - name: a\n    type: ss\n  - name: b\n    type: ss\n",
        Some("\"new\""),
    );

    let report = application
        .update_subscription_conditional(&source, "main")
        .await
        .expect("conditional update");
    assert_eq!(
        report.outcome,
        SubscriptionUpdateOutcome::Updated {
            new_bytes: 59,
            node_count: 2
        }
    );
    assert!(report.backed_up);
    assert!(report.usage_warning); // 1000/1000 used
    let headers = source.calls.lock().expect("calls lock")[0].clone();
    assert_eq!(headers.etag.as_deref(), Some("\"old\""));
    assert_eq!(headers.custom_user_agent.as_deref(), Some("UA-1"));
    assert!(headers.insecure_skip_verify);
    let metadata = application.load_metadata("main").await.expect("metadata");
    assert_eq!(metadata.etag.as_deref(), Some("\"new\""));
    assert_eq!(metadata.traffic_total, Some(1000));
}

#[tokio::test]
async fn conditional_update_not_modified_keeps_content_and_keeps_last_updated() {
    let store = subscription_store(None, None, None, false).await;
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let source = FakeSource::not_modified(Some("\"same\""));
    let original = store.load("main").await.expect("load");

    let report = application
        .update_subscription_conditional(&source, "main")
        .await
        .expect("conditional update");
    assert!(matches!(
        report.outcome,
        SubscriptionUpdateOutcome::NotModified { .. }
    ));
    assert!(!report.backed_up);
    assert_eq!(store.load("main").await.expect("load"), original);
    let metadata = application.load_metadata("main").await.expect("metadata");
    assert_eq!(metadata.etag.as_deref(), Some("\"same\""));
    assert!(metadata.last_updated.is_none());
}

#[tokio::test]
async fn fetch_settings_are_persisted_and_blank_ua_cleared() {
    let store = subscription_store(None, None, None, false).await;
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);

    application
        .update_subscription_fetch_settings("main", Some("UA-X".to_string()), true)
        .await
        .expect("save settings");
    let metadata = application.load_metadata("main").await.expect("metadata");
    assert_eq!(metadata.user_agent.as_deref(), Some("UA-X"));
    assert!(metadata.insecure_skip_verify);

    application
        .update_subscription_fetch_settings("main", Some("   ".to_string()), false)
        .await
        .expect("clear settings");
    let metadata = application.load_metadata("main").await.expect("metadata");
    assert_eq!(metadata.user_agent, None);
    assert!(!metadata.insecure_skip_verify);
}

#[tokio::test]
async fn update_subscription_delegates_to_conditional_path() {
    let store = subscription_store(None, None, None, false).await;
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let source = FakeSource::modified("proxies:\n  - name: a\n    type: ss\n", None);

    let updated = application
        .update_subscription(&source, "main")
        .await
        .expect("update");
    assert_eq!(updated.name, "main");
    assert_eq!(source.calls.lock().expect("calls lock").len(), 1);
}

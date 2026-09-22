//! Focused unit tests for the shared profile application and its
//! conditional subscription lifecycle.

use super::*;
use async_trait::async_trait;
use chrono::Timelike;
use infiltrator_domain::profiles::ProfileMetadata;
use infiltrator_ports::error::PortError;
use std::collections::BTreeMap;
use std::sync::Mutex;

#[derive(Default)]
struct FakeStore {
    current: Mutex<String>,
    profiles: Mutex<BTreeMap<String, (String, ProfileMetadata)>>,
    options: Mutex<BTreeMap<String, infiltrator_domain::profile_options::ProfileOptions>>,
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

    async fn load_options(
        &self,
        profile: &str,
    ) -> Result<infiltrator_domain::profile_options::ProfileOptions, PortError> {
        Ok(self
            .options
            .lock()
            .expect("options lock")
            .get(profile)
            .cloned()
            .unwrap_or_default())
    }

    async fn save_options(
        &self,
        profile: &str,
        options: &infiltrator_domain::profile_options::ProfileOptions,
    ) -> Result<(), PortError> {
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

#[tokio::test]
async fn restore_backup_reports_availability_and_is_one_shot() {
    let store = Arc::new(FakeStore::with_profile("main", "proxies: []\n", true));
    store
        .restorable_backups
        .lock()
        .expect("backup lock")
        .push("main".to_string());
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);

    let listed = application.list_profiles().await.expect("list");
    assert!(listed[0].has_backup, "backup presence is projected");

    assert!(
        application.restore_backup("main").await.expect("restore"),
        "an existing backup restores"
    );
    assert!(
        !application.restore_backup("main").await.expect("restore"),
        "a consumed backup is not restored twice"
    );
    let listed = application.list_profiles().await.expect("list");
    assert!(!listed[0].has_backup, "restore clears the projected flag");
}

#[tokio::test]
async fn batch_update_skips_url_less_profiles_and_aggregates_counts() {
    let store = subscription_store(None, None, None, false).await;
    store.profiles.lock().expect("profiles lock").insert(
        "local".to_string(),
        ("mode: rule\n".to_string(), ProfileMetadata::default()),
    );
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let source = FakeSource::modified("proxies:\n  - name: n2\n    type: ss\n", None);

    let report = application
        .update_all_subscriptions(&source, 4)
        .await
        .expect("batch");
    assert_eq!(report.total, 2);
    assert_eq!(report.skipped, 1, "profile without a URL is skipped");
    assert_eq!(report.updated, 1);
    assert_eq!(report.failed, 0);
    assert_eq!(report.not_modified, 0);
    assert_eq!(report.outcomes.len(), 1);
    assert_eq!(source.calls.lock().expect("calls lock").len(), 1);
}

/// DUAL-07-08: the shared filter runner reshapes the stored document and
/// persists the spec sidecar so the next subscription update recomposes from
/// the same node strategy.
#[tokio::test]
async fn apply_subscription_filter_reshapes_document_and_persists_spec() {
    let store = Arc::new(FakeStore::with_profile(
        "main",
        "proxies:\n  - name: 香港-01\n    type: ss\n  - name: 广告-02\n    type: vmess\n",
        true,
    ));
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let spec = infiltrator_domain::profile_options::FilterSpec {
        exclude_keywords: vec!["广告".to_string()],
        ..Default::default()
    };

    let runtime: Option<Arc<dyn ManagedRuntime>> = None;
    let report = application
        .apply_subscription_filter(runtime, "main", spec)
        .await
        .expect("filter");
    assert_eq!(report.total_input, 2);
    assert_eq!(report.passed, 1);
    let saved = store.load("main").await.expect("load");
    assert!(!saved.contains("广告-02"), "excluded node must be dropped");
    assert!(saved.contains("香港-01"));

    let options = application.load_options("main").await.expect("options");
    assert_eq!(
        options.filter.expect("filter stored").exclude_keywords,
        vec!["广告".to_string()]
    );
}

/// DUAL-07-03: a successful update on a cron-only profile advances
/// `next_update` to the Cron expression's next occurrence, not to an interval.
#[tokio::test]
async fn successful_cron_update_advances_to_the_next_occurrence() {
    let store = subscription_store(None, None, None, false).await;
    {
        let mut profiles = store.profiles.lock().expect("profiles lock");
        let metadata = &mut profiles.get_mut("main").expect("profile").1;
        metadata.update_interval_hours = None;
        metadata.cron_expression = Some("0 */6 * * *".to_string());
    }
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let source = FakeSource::modified("proxies:\n  - name: a\n    type: ss\n", None);

    application
        .update_subscription_conditional(&source, "main")
        .await
        .expect("update");

    let metadata = store.get_profile_metadata("main").await.expect("metadata");
    let next = metadata
        .next_update
        .expect("cron schedule must set next_update");
    let now = Utc::now();
    assert!(next > now, "next run must be in the future");
    let schedule =
        infiltrator_domain::subscription_scheduler_policy::SubscriptionSchedule::from_metadata(
            None,
            Some("0 */6 * * *"),
        )
        .expect("schedule");
    assert_eq!(
        schedule.next_run(now),
        Some(next),
        "next_update must be the cron occurrence after now"
    );
    assert_eq!(next.minute(), 0);
    assert_eq!(next.hour() % 6, 0);
}

/// DUAL-07-01: a local/clipboard document is normalized and validated before
/// it is committed, reporting the detected format and node count.
#[tokio::test]
async fn import_document_validates_and_reports_format() {
    let store = Arc::new(FakeStore::default());
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);

    let report = application
        .import_document(
            "imported",
            "proxies:\n  - name: a\n    type: ss\n  - name: b\n    type: ss\n",
            infiltrator_contract::subscription_import::SubscriptionImportChannel::LocalFile,
        )
        .await
        .expect("import");
    assert_eq!(report.node_count, 2);
    assert_eq!(
        report.channel,
        infiltrator_contract::subscription_import::SubscriptionImportChannel::LocalFile
    );
    assert!(store.load("imported").await.is_ok());

    let failure = application
        .import_document(
            "broken",
            "proxies: [unbalanced",
            infiltrator_contract::subscription_import::SubscriptionImportChannel::Clipboard,
        )
        .await
        .expect_err("malformed yaml must fail");
    assert_eq!(failure.code, ErrorCode::Configuration);
}

/// DUAL-07-14: the shared schedule draft validates and persists URL,
/// auto-update, interval, and cron in one application call, so neither surface
/// assembles profile metadata itself.
#[tokio::test]
async fn schedule_draft_validates_and_persists_the_subscription_shape() {
    let store = subscription_store(None, None, None, false).await;
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);

    // Happy path: an interval-based schedule with a cron cadence on top.
    application
        .update_subscription_schedule(
            "main",
            &SubscriptionScheduleDraft {
                url: "https://sub.example.com/token".to_string(),
                auto_update_enabled: true,
                update_interval_hours: "12".to_string(),
                cron_expression: Some("0 */6 * * *".to_string()),
            },
        )
        .await
        .expect("valid draft persists");
    let metadata = application.load_metadata("main").await.expect("metadata");
    assert_eq!(
        metadata.subscription_url.as_deref(),
        Some("https://sub.example.com/token")
    );
    assert!(metadata.auto_update_enabled);
    assert_eq!(metadata.update_interval_hours, Some(12));
    assert_eq!(metadata.cron_expression.as_deref(), Some("0 */6 * * *"));
    assert!(
        metadata.next_update.is_none(),
        "schedule recomputes on update"
    );

    // An interval-less draft with cron is a cron-only schedule.
    application
        .update_subscription_schedule(
            "main",
            &SubscriptionScheduleDraft {
                url: "https://sub.example.com/token".to_string(),
                auto_update_enabled: true,
                update_interval_hours: String::new(),
                cron_expression: Some("@daily".to_string()),
            },
        )
        .await
        .expect("cron-only draft persists");
    let metadata = application.load_metadata("main").await.expect("metadata");
    assert_eq!(metadata.update_interval_hours, None);
    assert_eq!(
        metadata.cron_expression.as_deref(),
        Some("0 0 * * *"),
        "a macro schedule is stored in its normalized five-field form"
    );

    // A draft without a cron falls back to the 24h default.
    application
        .update_subscription_schedule(
            "main",
            &SubscriptionScheduleDraft {
                url: "https://sub.example.com/token".to_string(),
                auto_update_enabled: true,
                update_interval_hours: String::new(),
                cron_expression: None,
            },
        )
        .await
        .expect("default interval draft persists");
    let metadata = application.load_metadata("main").await.expect("metadata");
    assert_eq!(metadata.update_interval_hours, Some(24));

    // Typed rejections: malformed cron / zero interval / auto-update without URL.
    for draft in [
        SubscriptionScheduleDraft {
            url: "https://x".to_string(),
            auto_update_enabled: true,
            update_interval_hours: "24".to_string(),
            cron_expression: Some("not a cron".to_string()),
        },
        SubscriptionScheduleDraft {
            url: "https://x".to_string(),
            auto_update_enabled: true,
            update_interval_hours: "0".to_string(),
            cron_expression: None,
        },
        SubscriptionScheduleDraft {
            url: "https://x".to_string(),
            auto_update_enabled: true,
            update_interval_hours: "not-a-number".to_string(),
            cron_expression: None,
        },
        SubscriptionScheduleDraft {
            url: "  ".to_string(),
            auto_update_enabled: true,
            update_interval_hours: "24".to_string(),
            cron_expression: None,
        },
    ] {
        let failure = application
            .update_subscription_schedule("main", &draft)
            .await
            .expect_err("invalid draft is rejected");
        assert_eq!(failure.code, ErrorCode::InvalidInput);
    }

    // Clearing the URL clears the whole schedule.
    application
        .update_subscription_schedule("main", &SubscriptionScheduleDraft::default())
        .await
        .expect("clearing persists");
    let metadata = application.load_metadata("main").await.expect("metadata");
    assert!(metadata.subscription_url.is_none());
    assert!(!metadata.auto_update_enabled);
    assert!(metadata.update_interval_hours.is_none());
    assert!(metadata.cron_expression.is_none());
    assert!(metadata.last_updated.is_none());
}

/// DUAL-07-09: the auto-reload preference round-trips through the shared
/// application so the refresh path can consume it.
#[tokio::test]
async fn auto_reload_preference_is_persisted_and_readable() {
    let store = subscription_store(None, None, None, false).await;
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);

    assert!(
        !application
            .load_metadata("main")
            .await
            .expect("metadata")
            .auto_reload_core,
        "default metadata starts opted out"
    );
    application
        .update_subscription_auto_reload("main", true)
        .await
        .expect("enable persists");
    assert!(
        application
            .load_metadata("main")
            .await
            .expect("metadata")
            .auto_reload_core
    );
    application
        .update_subscription_auto_reload("main", false)
        .await
        .expect("disable persists");
    assert!(
        !application
            .load_metadata("main")
            .await
            .expect("metadata")
            .auto_reload_core
    );
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

#[tokio::test]
async fn write_protection_is_derived_from_the_subscription_source() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    // assert_eq fails on ProfileWriteProtection without Debug; use is_protected.
    assert!(
        !application
            .write_protection("main")
            .await
            .expect("protection")
            .is_protected()
    );

    mark_subscription(&store, "main", "https://example.com/sub");
    let protected = application
        .write_protection("main")
        .await
        .expect("protection");
    assert!(protected.is_protected());
    assert_eq!(protected.label_zh(), "远程订阅 · 只读保护");
}

#[tokio::test]
async fn edited_writes_refuse_protected_subscriptions_until_unlocked() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    mark_subscription(&store, "main", "https://example.com/sub");
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);

    let failure = application
        .save_edited_profile_content(
            None::<Arc<dyn ManagedRuntime>>,
            "main".to_string(),
            "mode: global\n".to_string(),
            ApplyStrategy::PreferReload,
            false,
        )
        .await
        .expect_err("direct edit of a protected subscription must fail");
    assert_eq!(failure.code, ErrorCode::Configuration);
    assert_eq!(
        store.load("main").await.expect("content"),
        "mode: rule\n",
        "the refused write must not touch the profile"
    );

    application
        .save_edited_profile_content(
            None::<Arc<dyn ManagedRuntime>>,
            "main".to_string(),
            "mode: global\n".to_string(),
            ApplyStrategy::PreferReload,
            true,
        )
        .await
        .expect("the explicit unlock commits");
    assert_eq!(store.load("main").await.expect("content"), "mode: global\n");
}

#[tokio::test]
async fn local_profiles_stay_directly_editable() {
    let store = Arc::new(FakeStore::with_profile("lab", "mode: rule\n", true));
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    application
        .save_edited_profile_content(
            None::<Arc<dyn ManagedRuntime>>,
            "lab".to_string(),
            "mode: global\n".to_string(),
            ApplyStrategy::PreferReload,
            false,
        )
        .await
        .expect("local profile edits need no unlock");
    assert_eq!(store.load("lab").await.expect("content"), "mode: global\n");
}

// ---- DUAL-09-14: one option-sidecar use-case for both editor panes --------

/// The shared sidecar loader renders the stored mixin back into the editor's
/// YAML buffer, converts the stored filter into the shared draft and publishes
/// the snapshot both surfaces read.
#[tokio::test]
async fn options_load_publishes_the_sidecar_draft_for_both_surfaces() {
    use crate::profile_options_application::ProfileOptionsApplication;
    use infiltrator_contract::profile_options::{clear_profile_options, last_profile_options};

    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    {
        let mut options = store.options.lock().expect("options lock");
        options.insert(
            "main".to_string(),
            infiltrator_domain::profile_options::ProfileOptions {
                mixin: infiltrator_domain::mixin::MixinConfig {
                    mode: Some("global".to_string()),
                    ..Default::default()
                },
                filter: Some(infiltrator_domain::profile_options::FilterSpec {
                    include_keywords: vec!["香港".to_string()],
                    ..Default::default()
                }),
            },
        );
    }
    let profiles = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let application = ProfileOptionsApplication::new(profiles);

    let snapshot = application.load(None).await.expect("sidecar");
    assert_eq!(snapshot.profile, "main");
    assert!(
        snapshot.mixin_yaml.contains("mode: global"),
        "the stored mixin is rendered back into the editor buffer: {}",
        snapshot.mixin_yaml
    );
    assert_eq!(snapshot.filter.include, "香港");
    assert_eq!(
        last_profile_options(),
        Some(snapshot),
        "the load publishes the same snapshot for the surface projection"
    );
    clear_profile_options();
}

/// The Mixin commit strips the outgoing mixin's injected rule lines, so
/// re-saving an edited overlay never duplicates rules and never loses the
/// hand-written comments around them.
#[tokio::test]
async fn mixin_save_is_idempotent_and_keeps_handwritten_comments() {
    use crate::profile_options_application::ProfileOptionsApplication;

    let store = Arc::new(FakeStore::with_profile(
        "main",
        "# 手写注释\nmode: rule\nrules:\n  - MATCH,DIRECT\n",
        true,
    ));
    let profiles = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let application = ProfileOptionsApplication::new(profiles);
    let overlay = "rules:\n  append:\n    - DOMAIN-SUFFIX,example.com,DIRECT\n";

    for _ in 0..2 {
        application
            .save_mixin(None::<Arc<dyn ManagedRuntime>>, "main", overlay)
            .await
            .expect("mixin save");
    }

    let saved = store.load("main").await.expect("content");
    assert!(
        saved.contains("# 手写注释"),
        "the byte-faithful merge keeps the hand-written comment: {saved}"
    );
    assert_eq!(
        saved.matches("DOMAIN-SUFFIX,example.com,DIRECT").count(),
        1,
        "the second save strips the first injection before re-applying: {saved}"
    );
    let options = store
        .options
        .lock()
        .expect("options lock")
        .get("main")
        .cloned()
        .expect("sidecar stored");
    assert_eq!(
        options.mixin.rules.expect("rule mixin").append,
        vec!["DOMAIN-SUFFIX,example.com,DIRECT".to_string()]
    );
}

/// The filter commit compiles the surface draft through the shared parser: a
/// malformed rename line is a typed failure, a valid draft reshapes the stored
/// document and persists the spec.
#[tokio::test]
async fn filter_draft_save_uses_the_shared_parser_and_persists_the_spec() {
    use crate::profile_options_application::ProfileOptionsApplication;
    use infiltrator_contract::subscription_import::SubscriptionFilterDraft;

    let store = Arc::new(FakeStore::with_profile(
        "main",
        "proxies:\n  - name: 香港-01\n    type: ss\n  - name: 广告-02\n    type: vmess\n",
        true,
    ));
    let profiles = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let application = ProfileOptionsApplication::new(profiles);

    let broken = SubscriptionFilterDraft {
        renames: "missing arrow".to_string(),
        ..SubscriptionFilterDraft::default()
    };
    let failure = application
        .save_filter(None::<Arc<dyn ManagedRuntime>>, "main", &broken)
        .await
        .expect_err("a malformed rename line must fail before any write");
    assert_eq!(failure.code, ErrorCode::InvalidInput);

    let draft = SubscriptionFilterDraft {
        exclude: "广告".to_string(),
        dedup_index: 1,
        ..SubscriptionFilterDraft::default()
    };
    let report = application
        .save_filter(None::<Arc<dyn ManagedRuntime>>, "main", &draft)
        .await
        .expect("filter run");
    assert_eq!(report.total_input, 2);
    assert_eq!(report.passed, 1);
    let saved = store.load("main").await.expect("content");
    assert!(!saved.contains("广告-02"));
    let stored = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>)
        .load_options("main")
        .await
        .expect("options")
        .filter
        .expect("filter stored");
    assert_eq!(stored.exclude_keywords, vec!["广告".to_string()]);
    assert_eq!(
        stored.deduplication,
        infiltrator_domain::profile_options::FilterDedup::KeepFirst
    );
}

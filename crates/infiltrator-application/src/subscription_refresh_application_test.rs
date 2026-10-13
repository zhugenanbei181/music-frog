//! Headless tests for the shared subscription refresh orchestration
//! (DUAL-07-05 retry/backoff, DUAL-07-06 single-flight).

use super::*;
use crate::profile_application::ProfileRuntimeAccess;
use crate::profile_document_codec_test_support::TestProfileDocumentCodec;
use crate::profile_options_application::ProfileOptionsApplication;
use crate::profile_workspace_test_support::workspace_with_raw_sidecar;
use async_trait::async_trait;
use chrono::Timelike;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceCasOutcome, ActiveProfileReferenceState,
    ActiveProfileReferenceTarget,
};
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::profile_document_identity::{ProfileId, ProfileWorkspaceIdentity};
use infiltrator_contract::subscription_import::{
    SubscriptionFilterDraft, SubscriptionImportChannel,
};
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_domain::subscription::CheckedSubscriptionUrl;
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use infiltrator_ports::runtime_gateway::ActiveProfileReferenceApplier;
use infiltrator_ports::subscription_source::{
    ConditionalDocumentResult, ConditionalFetchHeaders, SubscriptionDocument,
};
use std::collections::BTreeMap;
use std::future::ready;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Waker};
use std::thread::yield_now;
use std::time::Duration;

/// Runtime that executes futures inline and records the backoff delays the
/// application asked for. `.sleep()` never actually blocks.
#[derive(Default)]
struct RecordingRuntime {
    sleeps: Mutex<Vec<Duration>>,
}

impl ApplicationRuntime for RecordingRuntime {
    fn block_on(&self, future: ApplicationFuture) {
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        let mut future = future;
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(()) => return,
                Poll::Pending => yield_now(),
            }
        }
    }

    fn sleep(&self, duration: Duration) -> ApplicationSleep<'_> {
        self.sleeps.lock().expect("sleeps lock").push(duration);
        Box::pin(ready(()))
    }
}

struct FakeStore {
    dir: PathBuf,
    active_reference: Mutex<ActiveProfileReferenceState>,
    profiles: Mutex<BTreeMap<String, (String, ProfileMetadata)>>,
    sidecars: Mutex<BTreeMap<String, Vec<u8>>>,
}

impl FakeStore {
    fn new(dir: &str, profile: &str, url: Option<&str>) -> Self {
        let metadata = ProfileMetadata {
            subscription_url: url.map(str::to_string),
            ..ProfileMetadata::default()
        };
        let mut profiles = BTreeMap::new();
        profiles.insert(profile.to_string(), ("proxies: []\n".to_string(), metadata));
        Self {
            dir: PathBuf::from(dir),
            active_reference: Mutex::new(ActiveProfileReferenceState::default()),
            profiles: Mutex::new(profiles),
            sidecars: Mutex::new(BTreeMap::new()),
        }
    }
}

#[async_trait]
impl ProfileStore for FakeStore {
    fn engine_id(&self) -> EngineId {
        EngineId::new("mihomo").expect("test engine ID")
    }

    async fn load_workspace(&self, profile: &ProfileId) -> Result<ProfileWorkspace, PortError> {
        let profile = profile.as_str();
        let profiles = self.profiles.lock().unwrap();
        let sidecars = self.sidecars.lock().unwrap();
        let (content, metadata) = profiles
            .get(profile)
            .ok_or_else(|| PortError::NotFound(profile.into()))?;
        workspace_with_raw_sidecar(
            profile,
            content.as_bytes(),
            sidecars.get(profile).cloned(),
            metadata.subscription_url.as_deref(),
        )
    }

    async fn compare_and_save_inactive_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        let mut profiles = self.profiles.lock().unwrap();
        let mut sidecars = self.sidecars.lock().unwrap();
        let profile = expected.document.profile_id.as_str();
        let (content, _) = profiles
            .get(profile)
            .ok_or_else(|| PortError::NotFound(profile.into()))?;
        let metadata = &profiles.get(profile).expect("profile exists").1;
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
        let result = workspace_with_raw_sidecar(
            profile,
            &update.document_bytes,
            update.sidecar_bytes.clone(),
            metadata.subscription_url.as_deref(),
        )?;
        profiles.get_mut(profile).unwrap().0.clone_from(
            &String::from_utf8(update.document_bytes.clone())
                .map_err(|error| PortError::Io(error.to_string()))?,
        );
        match update.sidecar_bytes.as_ref() {
            Some(sidecar) => {
                sidecars.insert(profile.to_owned(), sidecar.clone());
            }
            None => {
                sidecars.remove(profile);
            }
        }
        Ok(result)
    }
    fn config_dir(&self) -> PathBuf {
        self.dir.clone()
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        let current = match &*self.active_reference.lock().expect("active reference lock") {
            ActiveProfileReferenceState::Selected(reference) => {
                Some(reference.profile_id.as_str().to_owned())
            }
            ActiveProfileReferenceState::Unselected { .. } => None,
        };
        Ok(self
            .profiles
            .lock()
            .expect("profiles lock")
            .iter()
            .map(|(name, (_content, metadata))| ProfileInfo {
                name: name.clone(),
                active: current.as_deref() == Some(name.as_str()),
                path: format!("/fake/{name}.yaml"),
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
                has_backup: false,
            })
            .collect())
    }

    async fn get_current(&self) -> Result<String, PortError> {
        match &*self.active_reference.lock().expect("active reference lock") {
            ActiveProfileReferenceState::Selected(reference) => {
                Ok(reference.profile_id.as_str().to_owned())
            }
            ActiveProfileReferenceState::Unselected { .. } => Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Profiles,
                "test store has no selected profile",
            )),
        }
    }

    async fn read_active_profile_reference(
        &self,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        Ok(self
            .active_reference
            .lock()
            .expect("active reference lock")
            .clone())
    }

    async fn compare_and_set_active_profile_reference(
        &self,
        expected: ActiveProfileReferenceState,
        target: ActiveProfileReferenceTarget,
        _: infiltrator_contract::active_profile_reference::ActiveProfileReferenceWriteAuthorization,
    ) -> Result<ActiveProfileReferenceCasOutcome, PortError> {
        let mut active_reference = self.active_reference.lock().expect("active reference lock");
        if *active_reference != expected {
            return Ok(ActiveProfileReferenceCasOutcome::Stale {
                expected,
                observed: active_reference.clone(),
            });
        }
        let revision = expected
            .revision()
            .next()
            .ok_or_else(|| PortError::Io("test active-profile revision overflow".into()))?;
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
                        infiltrator_contract::capability::Capability::Profiles,
                        "test store cannot select a foreign engine",
                    ));
                }
                if !self
                    .profiles
                    .lock()
                    .expect("profiles lock")
                    .contains_key(profile_id.as_str())
                {
                    return Err(PortError::NotFound(profile_id.as_str().into()));
                }
                ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
                    engine_id, profile_id, revision,
                ))
            }
        };
        let previous = std::mem::replace(&mut *active_reference, current.clone());
        Ok(ActiveProfileReferenceCasOutcome::Committed { previous, current })
    }

    async fn load(&self, profile: &str) -> Result<String, PortError> {
        self.profiles
            .lock()
            .expect("profiles lock")
            .get(profile)
            .map(|(content, _)| content.clone())
            .ok_or_else(|| PortError::NotFound(profile.to_string()))
    }

    async fn save_inactive_profile(&self, profile: &str, content: &str) -> Result<(), PortError> {
        if matches!(
            &*self.active_reference.lock().expect("active reference lock"),
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == self.engine_id()
                    && reference.profile_id.as_str() == profile
        ) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake inactive save cannot write the selected profile",
                true,
            )));
        }
        self.profiles
            .lock()
            .expect("profiles lock")
            .entry(profile.to_string())
            .or_insert_with(|| (String::new(), ProfileMetadata::default()))
            .0 = content.to_string();
        Ok(())
    }

    async fn delete_profile(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
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

    async fn clear_backup(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn restore_inactive_profile_backup(&self, _profile: &str) -> Result<bool, PortError> {
        Ok(false)
    }
}

/// Source that fails the first `failures_before_success` conditional fetches
/// and then returns a fresh document, counting every attempt.
struct FlakySource {
    failures_before_success: usize,
    calls: AtomicUsize,
}

impl FlakySource {
    fn new(failures_before_success: usize) -> Self {
        Self {
            failures_before_success,
            calls: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl SubscriptionSource for FlakySource {
    async fn fetch(
        &self,
        _profile: &str,
        _url: &CheckedSubscriptionUrl,
    ) -> Result<SubscriptionDocument, PortError> {
        Err(PortError::Network("use fetch_conditional".to_string()))
    }

    async fn fetch_conditional(
        &self,
        _profile: &str,
        _url: &CheckedSubscriptionUrl,
        _headers: &ConditionalFetchHeaders,
    ) -> Result<ConditionalDocumentResult, PortError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if call <= self.failures_before_success {
            return Err(PortError::Network(format!("attempt {call} failed")));
        }
        Ok(ConditionalDocumentResult::Modified {
            document: SubscriptionDocument {
                content: "proxies:\n  - name: n1\n    type: ss\n".to_string(),
                userinfo: None,
            },
            etag: Some("\"new\"".to_string()),
            last_modified: None,
        })
    }
}

fn application(
    dir: &str,
    policy: RetryBackoffPolicy,
) -> (SubscriptionRefreshApplication, Arc<RecordingRuntime>) {
    let store = Arc::new(FakeStore::new(dir, "main", Some("https://example.com/sub")));
    let profile = ProfileApplication::new(store);
    let runtime = Arc::new(RecordingRuntime::default());
    (
        SubscriptionRefreshApplication::new(profile, runtime.clone(), policy),
        runtime,
    )
}

#[tokio::test]
async fn retries_transient_failures_then_succeeds() {
    let (app, _runtime) = application("/fake/retry-ok", RetryBackoffPolicy::test_immediate());
    let source = FlakySource::new(2);

    let report = app
        .refresh_profile(&source, "main")
        .await
        .expect("eventually succeeds");
    assert!(matches!(
        report.outcome,
        SubscriptionUpdateOutcome::Updated { .. }
    ));
    assert_eq!(source.calls(), 3, "two failures then one success");
}

#[tokio::test]
async fn gives_up_when_the_policy_is_exhausted() {
    let (app, _runtime) = application("/fake/retry-give-up", RetryBackoffPolicy::test_immediate());
    let source = FlakySource::new(usize::MAX);

    let failure = app
        .refresh_profile(&source, "main")
        .await
        .expect_err("all attempts fail");
    assert_eq!(failure.code, ErrorCode::Network);
    // test_immediate carries three delays: attempts 1..=3 back off, attempt 4
    // observes `None` and returns the failure.
    assert_eq!(source.calls(), 4, "initial attempt plus three retries");
}

#[tokio::test]
async fn backoff_delays_ride_the_injected_runtime_sleep_seam() {
    let policy = RetryBackoffPolicy::new(vec![Duration::from_millis(5), Duration::from_millis(10)]);
    let (app, runtime) = application("/fake/retry-delays", policy);
    let source = FlakySource::new(2);

    app.refresh_profile(&source, "main")
        .await
        .expect("succeeds after two retries");
    assert_eq!(
        runtime.sleeps.lock().expect("sleeps lock").as_slice(),
        &[Duration::from_millis(5), Duration::from_millis(10)],
        "the application slept through the runtime port, not a hardcoded executor"
    );
}

#[tokio::test]
async fn refresh_is_single_flight_per_store_and_profile() {
    let (app, _runtime) = application("/fake/single-flight", RetryBackoffPolicy::test_immediate());

    let guard = app.begin_refresh("main").expect("first claim");
    assert_eq!(guard.profile(), "main");

    let busy = app
        .begin_refresh("main")
        .expect_err("second concurrent claim is rejected");
    assert_eq!(busy.code, ErrorCode::InvalidState);

    // The refresh path itself observes the held slot with a typed failure.
    let source = FlakySource::new(0);
    let blocked = app
        .refresh_profile(&source, "main")
        .await
        .expect_err("in-flight refresh blocks a second one");
    assert_eq!(blocked.code, ErrorCode::InvalidState);
    assert_eq!(source.calls(), 0, "no duplicate download was attempted");

    drop(guard);
    app.refresh_profile(&source, "main")
        .await
        .expect("slot released");
    assert_eq!(source.calls(), 1);
}

#[tokio::test]
async fn refresh_all_aggregates_and_skips_url_less_profiles() {
    let store = Arc::new(FakeStore::new(
        "/fake/batch",
        "main",
        Some("https://example.com/sub"),
    ));
    store.profiles.lock().expect("profiles lock").insert(
        "local".to_string(),
        (String::new(), ProfileMetadata::default()),
    );
    let profile = ProfileApplication::new(store);
    let app = SubscriptionRefreshApplication::with_default_policy(
        profile,
        Arc::new(RecordingRuntime::default()),
    );
    let source = FlakySource::new(0);

    let report = app.refresh_all(&source, 4).await.expect("batch");
    assert_eq!(report.total, 2);
    assert_eq!(report.skipped, 1);
    assert_eq!(report.updated, 1);
    assert_eq!(report.failed, 0);
    assert_eq!(source.calls(), 1);
}

/// DUAL-07-10: a recording host notification port.
#[derive(Default)]
struct RecordingNotifier {
    seen: Mutex<Vec<SubscriptionNotification>>,
}

impl SubscriptionNotificationPort for RecordingNotifier {
    fn notify(&self, notification: SubscriptionNotification) {
        self.seen.lock().expect("notifier lock").push(notification);
    }
}

#[tokio::test]
async fn refresh_profile_notifies_success_through_the_host_port() {
    let store = Arc::new(FakeStore::new(
        "/fake/notify-ok",
        "main",
        Some("https://example.com/sub"),
    ));
    let notifier = Arc::new(RecordingNotifier::default());
    let app = SubscriptionRefreshApplication::with_default_policy(
        ProfileApplication::new(store),
        Arc::new(RecordingRuntime::default()),
    )
    .with_notifier(notifier.clone());
    let source = FlakySource::new(0);

    app.refresh_profile(&source, "main").await.expect("refresh");
    let seen = notifier.seen.lock().expect("notifier lock").clone();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].kind, SubscriptionNotificationKind::Updated);
    assert_eq!(seen[0].profiles, vec!["main".to_string()]);
}

#[tokio::test]
async fn exhausted_refresh_notifies_failure_through_the_host_port() {
    let store = Arc::new(FakeStore::new(
        "/fake/notify-fail",
        "main",
        Some("https://example.com/sub"),
    ));
    let notifier = Arc::new(RecordingNotifier::default());
    let app = SubscriptionRefreshApplication::with_default_policy(
        ProfileApplication::new(store),
        Arc::new(RecordingRuntime::default()),
    )
    .with_notifier(notifier.clone());
    let source = FlakySource::new(usize::MAX);

    let _ = app.refresh_profile(&source, "main").await;
    let seen = notifier.seen.lock().expect("notifier lock").clone();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].kind, SubscriptionNotificationKind::Failed);
    assert!(seen[0].error.is_some(), "failure carries the error summary");
}

#[tokio::test]
async fn refresh_all_emits_one_aggregated_notification() {
    let store = Arc::new(FakeStore::new(
        "/fake/notify-batch",
        "main",
        Some("https://example.com/sub"),
    ));
    store.profiles.lock().expect("profiles lock").insert(
        "local".to_string(),
        (String::new(), ProfileMetadata::default()),
    );
    let notifier = Arc::new(RecordingNotifier::default());
    let app = SubscriptionRefreshApplication::with_default_policy(
        ProfileApplication::new(store),
        Arc::new(RecordingRuntime::default()),
    )
    .with_notifier(notifier.clone());
    let source = FlakySource::new(0);

    app.refresh_all(&source, 4).await.expect("batch");
    let seen = notifier.seen.lock().expect("notifier lock").clone();
    assert_eq!(seen.len(), 1, "the batch aggregates into one notification");
    assert_eq!(seen[0].kind, SubscriptionNotificationKind::Updated);
    assert_eq!(seen[0].profiles, vec!["main".to_string()]);
}

/// DUAL-07-15: honest headless regression matrix over the shared subscription
/// update pipeline, covering every item this batch closed plus the retry,
/// single-flight and conditional-request stages they build on.
#[tokio::test]
async fn subscription_update_pipeline_regression_matrix() {
    let store = Arc::new(FakeStore::new(
        "/fake/matrix",
        "main",
        Some("https://example.com/sub"),
    ));
    {
        let mut profiles = store.profiles.lock().expect("profiles lock");
        let metadata = &mut profiles.get_mut("main").expect("profile").1;
        metadata.auto_update_enabled = true;
        metadata.update_interval_hours = None;
        metadata.cron_expression = Some("0 */6 * * *".to_string());
    }
    let profile = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>)
        .with_document_codec(Arc::new(TestProfileDocumentCodec::mihomo()));

    // 07-05/07-10: retry a transient failure then notify success.
    let notifier = Arc::new(RecordingNotifier::default());
    let refresh = SubscriptionRefreshApplication::with_default_policy(
        profile.clone(),
        Arc::new(RecordingRuntime::default()),
    )
    .with_notifier(notifier.clone());
    let flaky = FlakySource::new(1);
    refresh
        .refresh_profile(&flaky, "main")
        .await
        .expect("retry then success");
    assert_eq!(flaky.calls(), 2, "07-05 retried the transient failure");

    // 07-03: the cron-only profile advanced `next_update` to a cron occurrence.
    let metadata = store
        .get_profile_metadata("main")
        .await
        .expect("metadata after refresh");
    let next = metadata.next_update.expect("cron next_update");
    assert_eq!(next.minute(), 0, "07-03 cron minute");
    assert_eq!(next.hour() % 6, 0, "07-03 cron hour step");
    assert_eq!(
        notifier.seen.lock().expect("notifier lock").len(),
        1,
        "07-10 success notification"
    );

    // 07-06: a held single-flight slot rejects a concurrent refresh.
    let guard = refresh.begin_refresh("main").expect("claim slot");
    let busy = refresh
        .refresh_profile(&FlakySource::new(0), "main")
        .await
        .expect_err("second refresh rejected");
    assert_eq!(busy.code, ErrorCode::InvalidState, "07-06 single flight");
    drop(guard);

    // 07-08: the node-keyword pipeline reshapes content and persists the spec.
    profile
        .save_profile(
            "main",
            "proxies:\n  - name: 广告-02\n    type: ss\n  - name: 香港-01\n    type: ss\n",
        )
        .await
        .expect("seed document");
    let draft = SubscriptionFilterDraft {
        exclude: "广告".to_string(),
        ..Default::default()
    };
    let runtime = ProfileRuntimeAccess::<dyn ActiveProfileReferenceApplier>::Offline;
    let report = ProfileOptionsApplication::new(profile.clone())
        .save_filter(
            runtime,
            &profile
                .load_workspace(&ProfileId::new("main").unwrap())
                .await
                .unwrap()
                .identity(),
            &draft,
        )
        .await
        .expect("filter pipeline")
        .report;
    assert_eq!(report.passed, 1, "07-08 one node survives");
    let saved = profile
        .load_profile_detail("main")
        .await
        .expect("detail")
        .content;
    assert!(!saved.contains("广告-02"), "07-08 exclusion applied");

    // 07-01: a local document imports through the shared application.
    let import_report = profile
        .import_document(
            "matrix-import",
            "proxies:\n  - name: a\n    type: ss\n  - name: b\n    type: ss\n",
            SubscriptionImportChannel::LocalFile,
        )
        .await
        .expect("import document");
    assert_eq!(import_report.node_count, 2, "07-01 import reports nodes");
}

#[tokio::test]
async fn command_application_update_profile_uses_the_shared_retry_seam() {
    use crate::command_application::CommandApplication;
    use crate::core_capability_test_support::{execute_core_command, with_test_capabilities};
    use infiltrator_contract::command::CommandIntent;

    let store = Arc::new(FakeStore::new(
        "/fake/command-retry",
        "main",
        Some("https://example.com/sub"),
    ));
    let profile = ProfileApplication::new(store);
    let runtime = Arc::new(RecordingRuntime::default());
    let source = Arc::new(FlakySource::new(2));
    let application = with_test_capabilities(
        CommandApplication::new()
            .with_profile(profile)
            .with_subscription_source(source.clone() as Arc<dyn SubscriptionSource>)
            .with_application_runtime(runtime.clone()),
    );

    execute_core_command(
        &application,
        CommandIntent::UpdateProfile {
            profile_id: "main".to_owned(),
        },
    )
    .await
    .expect("the shared command path retries to success");

    assert_eq!(
        source.calls(),
        3,
        "the Bevy/command path retries transient failures"
    );
    assert_eq!(
        runtime.sleeps.lock().expect("sleeps lock").as_slice(),
        &[Duration::from_secs(30), Duration::from_secs(60)],
        "the command path backs off through the injected runtime, not a busy loop"
    );
}

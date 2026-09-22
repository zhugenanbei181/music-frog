//! Headless tests for the shared subscription refresh orchestration
//! (DUAL-07-05 retry/backoff, DUAL-07-06 single-flight).

use super::*;
use async_trait::async_trait;
use chrono::Timelike;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep,
};
use infiltrator_ports::core_reload::CoreReloadPort;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::runtime_gateway::ManagedRuntime;
use infiltrator_ports::subscription_source::{
    ConditionalDocumentResult, ConditionalFetchHeaders, SubscriptionDocument,
};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Waker};
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
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    fn sleep(&self, duration: Duration) -> ApplicationSleep<'_> {
        self.sleeps.lock().expect("sleeps lock").push(duration);
        Box::pin(std::future::ready(()))
    }
}

struct FakeStore {
    dir: PathBuf,
    current: Mutex<String>,
    profiles: Mutex<BTreeMap<String, (String, ProfileMetadata)>>,
    options: Mutex<BTreeMap<String, infiltrator_domain::profile_options::ProfileOptions>>,
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
            current: Mutex::new(profile.to_string()),
            profiles: Mutex::new(profiles),
            options: Mutex::new(BTreeMap::new()),
        }
    }
}

#[async_trait]
impl ProfileStore for FakeStore {
    fn config_dir(&self) -> PathBuf {
        self.dir.clone()
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
        Ok(self.current.lock().expect("current lock").clone())
    }

    async fn set_current(&self, profile: &str) -> Result<(), PortError> {
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

    async fn delete_options(&self, _profile: &str) -> Result<(), PortError> {
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

    async fn clear_backup(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn restore_backup(&self, _profile: &str) -> Result<bool, PortError> {
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
        _url: &infiltrator_domain::subscription::CheckedSubscriptionUrl,
    ) -> Result<SubscriptionDocument, PortError> {
        Err(PortError::Network("use fetch_conditional".to_string()))
    }

    async fn fetch_conditional(
        &self,
        _profile: &str,
        _url: &infiltrator_domain::subscription::CheckedSubscriptionUrl,
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
    let profile = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);

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
    let spec = infiltrator_domain::profile_options::FilterSpec {
        exclude_keywords: vec!["广告".to_string()],
        ..Default::default()
    };
    let runtime: Option<Arc<dyn ManagedRuntime>> = None;
    let report = profile
        .apply_subscription_filter(runtime, "main", spec)
        .await
        .expect("filter pipeline");
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
            infiltrator_contract::subscription_import::SubscriptionImportChannel::LocalFile,
        )
        .await
        .expect("import document");
    assert_eq!(import_report.node_count, 2, "07-01 import reports nodes");
}

#[tokio::test]
async fn command_application_update_profile_uses_the_shared_retry_seam() {
    use crate::command_application::CommandApplication;
    use infiltrator_contract::command::CommandIntent;

    let store = Arc::new(FakeStore::new(
        "/fake/command-retry",
        "main",
        Some("https://example.com/sub"),
    ));
    let profile = ProfileApplication::new(store);
    let runtime = Arc::new(RecordingRuntime::default());
    let source = Arc::new(FlakySource::new(2));
    let application = CommandApplication::new()
        .with_profile(profile)
        .with_subscription_source(source.clone() as Arc<dyn SubscriptionSource>)
        .with_application_runtime(runtime.clone());

    application
        .execute(CommandIntent::UpdateProfile {
            profile_id: "main".to_owned(),
        })
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

/// DUAL-07-09: records every reload the shared refresh asks the host to make.
#[derive(Clone, Default)]
struct RecordingReload {
    calls: Arc<AtomicUsize>,
    fail: bool,
}

impl RecordingReload {
    fn failing() -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            fail: true,
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl CoreReloadPort for RecordingReload {
    async fn reload_active_profile(&self) -> Result<(), PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Err(PortError::Failed("core reload rejected".to_string()));
        }
        Ok(())
    }
}

/// DUAL-07-09: the post-update reload follows the persisted preference, the
/// active-profile fact, and the host seam — every other case is a typed,
/// visible outcome rather than a silent no-op.
#[tokio::test]
async fn core_reload_follows_the_preference_and_the_host_seam() {
    let store = Arc::new(FakeStore::new(
        "/fake/core-reload",
        "main",
        Some("https://example.com/sub"),
    ));
    let profile = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let runtime = Arc::new(RecordingRuntime::default());
    let reload = RecordingReload::default();
    let app = SubscriptionRefreshApplication::new(
        profile.clone(),
        runtime,
        RetryBackoffPolicy::test_immediate(),
    )
    .with_core_reload(reload.clone());
    let source = FlakySource::new(0);

    // Opted out: content is committed, the core is untouched.
    let report = app.refresh_profile(&source, "main").await.expect("refresh");
    assert_eq!(report.core_reload, CoreReloadOutcome::Disabled);
    assert!(!report.reloaded_core());
    assert_eq!(reload.calls(), 0);

    // Enabled + active + seam: the host really reloads.
    profile
        .update_subscription_auto_reload("main", true)
        .await
        .expect("preference persists");
    let report = app.refresh_profile(&source, "main").await.expect("refresh");
    assert_eq!(report.core_reload, CoreReloadOutcome::Reloaded);
    assert!(report.reloaded_core());
    assert_eq!(reload.calls(), 1);

    // A non-active profile never reloads.
    store.set_current("other").await.expect("switch pointer");
    let report = app.refresh_profile(&source, "main").await.expect("refresh");
    assert_eq!(report.core_reload, CoreReloadOutcome::NotActive);
    assert_eq!(reload.calls(), 1);
    store.set_current("main").await.expect("restore pointer");

    // A host without the seam reports the typed unsupported outcome.
    let without_seam = SubscriptionRefreshApplication::new(
        profile.clone(),
        Arc::new(RecordingRuntime::default()),
        RetryBackoffPolicy::test_immediate(),
    );
    assert!(!without_seam.has_core_reload());
    let report = without_seam
        .refresh_profile(&source, "main")
        .await
        .expect("refresh");
    assert_eq!(report.core_reload, CoreReloadOutcome::Unsupported);
    assert!(!report.reloaded_core());

    // A failing seam keeps the update and records the adapter error.
    let failing = RecordingReload::failing();
    let failing_app = SubscriptionRefreshApplication::new(
        profile.clone(),
        Arc::new(RecordingRuntime::default()),
        RetryBackoffPolicy::test_immediate(),
    )
    .with_core_reload(failing.clone());
    let report = failing_app
        .refresh_profile(&source, "main")
        .await
        .expect("the update itself succeeds");
    assert!(matches!(
        report.core_reload,
        CoreReloadOutcome::Failed { .. }
    ));
    assert_eq!(failing.calls(), 1);
    assert!(
        matches!(report.outcome, SubscriptionUpdateOutcome::Updated { .. }),
        "the committed content survives a failed reload"
    );
}

/// DUAL-07-09: a batch reloads the core at most once — only the active
/// profile's own report carries the reload decision.
#[tokio::test]
async fn batch_reloads_the_core_once_for_the_active_profile() {
    let store = Arc::new(FakeStore::new(
        "/fake/batch-reload",
        "main",
        Some("https://example.com/sub"),
    ));
    store.profiles.lock().expect("profiles lock").insert(
        "backup".to_string(),
        (
            "proxies: []\n".to_string(),
            ProfileMetadata {
                subscription_url: Some("https://example.com/backup".to_string()),
                auto_reload_core: true,
                ..ProfileMetadata::default()
            },
        ),
    );
    let profile = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    profile
        .update_subscription_auto_reload("main", true)
        .await
        .expect("preference persists");
    let reload = RecordingReload::default();
    let app = SubscriptionRefreshApplication::new(
        profile,
        Arc::new(RecordingRuntime::default()),
        RetryBackoffPolicy::test_immediate(),
    )
    .with_core_reload(reload.clone());
    let source = FlakySource::new(0);

    let report = app.refresh_all(&source, 4).await.expect("batch");
    assert_eq!(report.updated, 2);
    assert_eq!(reload.calls(), 1, "one batch-wide reload");
    let active = report
        .outcomes
        .iter()
        .find(|outcome| outcome.profile_name == "main")
        .expect("active outcome");
    assert_eq!(active.core_reload, CoreReloadOutcome::Reloaded);
    let inactive = report
        .outcomes
        .iter()
        .find(|outcome| outcome.profile_name == "backup")
        .expect("inactive outcome");
    assert_eq!(
        inactive.core_reload,
        CoreReloadOutcome::NotAttempted,
        "an inactive profile's report carries no reload claim"
    );
}

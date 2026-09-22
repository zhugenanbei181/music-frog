//! Headless tests for the shared subscription refresh orchestration
//! (DUAL-07-05 retry/backoff, DUAL-07-06 single-flight).

use super::*;
use async_trait::async_trait;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
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

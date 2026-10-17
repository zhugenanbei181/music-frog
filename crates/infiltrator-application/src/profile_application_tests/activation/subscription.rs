//! Conditional subscription refresh behavior under active workspace leases.

use super::*;
use crate::subscription_refresh_application::SubscriptionRefreshApplication;
use async_trait::async_trait;
use infiltrator_contract::subscription_import::StoreForNextStartReason;
use infiltrator_domain::subscription::CheckedSubscriptionUrl;
use infiltrator_domain::subscription_scheduler_policy::RetryBackoffPolicy;
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep,
};
use infiltrator_ports::subscription_source::{
    ConditionalDocumentResult, ConditionalFetchHeaders, SubscriptionDocument, SubscriptionSource,
};
use lurker_core_contract::lifecycle::CoreLifecycle;
use std::future::ready;
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::Duration;

#[derive(Default)]
struct ImmediateApplicationRuntime;

impl ApplicationRuntime for ImmediateApplicationRuntime {
    fn block_on(&self, future: ApplicationFuture) {
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        let mut future = future;
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(()) => return,
                Poll::Pending => thread::yield_now(),
            }
        }
    }

    fn sleep(&self, _duration: Duration) -> ApplicationSleep<'_> {
        Box::pin(ready(()))
    }
}

enum RefreshMutation {
    None,
    Select(String),
    ReplaceDocument(String),
}

struct SubscriptionProbe {
    store: Arc<FakeStore>,
    mutation: RefreshMutation,
    calls: AtomicUsize,
}

#[async_trait]
impl SubscriptionSource for SubscriptionProbe {
    async fn fetch(
        &self,
        _profile: &str,
        _url: &CheckedSubscriptionUrl,
    ) -> Result<SubscriptionDocument, PortError> {
        Err(PortError::Network("conditional fetch expected".to_owned()))
    }

    async fn fetch_conditional(
        &self,
        _profile: &str,
        _url: &CheckedSubscriptionUrl,
        _headers: &ConditionalFetchHeaders,
    ) -> Result<ConditionalDocumentResult, PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match &self.mutation {
            RefreshMutation::None => {}
            RefreshMutation::Select(profile) => self.store.set_external_current(profile)?,
            RefreshMutation::ReplaceDocument(content) => {
                self.store.replace_document_for_test("main", content);
            }
        }
        Ok(ConditionalDocumentResult::Modified {
            document: SubscriptionDocument {
                content: "mode: direct\nproxies: []\n".to_owned(),
                userinfo: None,
            },
            etag: Some("updated".to_owned()),
            last_modified: None,
        })
    }
}

fn subscription_refresh_app(
    store: Arc<FakeStore>,
    runtime: Arc<ActivationRuntime>,
) -> SubscriptionRefreshApplication {
    let profile = ProfileApplication::new(store as Arc<dyn ProfileStore>);
    let runtime: Arc<dyn ActiveProfileReferenceApplier> = runtime;
    SubscriptionRefreshApplication::new(
        profile,
        Arc::new(ImmediateApplicationRuntime),
        RetryBackoffPolicy::test_immediate(),
    )
    .with_profile_reference_applier(runtime)
}

fn subscription_store() -> Arc<FakeStore> {
    let store = store_with_profiles("main", &["rival"]);
    {
        let mut profiles = store.profiles.lock().expect("profiles");
        let metadata = &mut profiles.get_mut("main").expect("main profile").1;
        metadata.subscription_url = Some("https://example.test/subscription".to_owned());
        metadata.auto_reload_core = true;
    }
    store
}

fn subscription_source(store: Arc<FakeStore>, mutation: RefreshMutation) -> SubscriptionProbe {
    SubscriptionProbe {
        store,
        mutation,
        calls: AtomicUsize::new(0),
    }
}

#[tokio::test]
async fn running_subscription_refresh_applies_inside_the_exact_workspace_lease() {
    let store = subscription_store();
    let runtime = Arc::new(ActivationRuntime::new(
        store.clone(),
        [ApplyBehavior::Succeed],
    ));
    let app = subscription_refresh_app(store.clone(), runtime.clone());
    let source = subscription_source(store.clone(), RefreshMutation::None);

    let report = app
        .refresh_profile(&source, "main")
        .await
        .expect("selected subscription applies transactionally");

    assert_eq!(
        report.content_effect,
        SubscriptionContentEffectOutcome::Applied
    );
    assert_eq!(
        store.load("main").await.unwrap(),
        "mode: direct\nproxies: []\n"
    );
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 1);
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 1);
    assert_eq!(source.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn stopped_and_failed_subscription_refreshes_store_with_distinct_effects() {
    for (lifecycle, expected_effect) in [
        (
            CoreLifecycle::Stopped,
            SubscriptionContentEffectOutcome::StoredForNextStart {
                reason: StoreForNextStartReason::CoreStopped,
            },
        ),
        (
            CoreLifecycle::Failed,
            SubscriptionContentEffectOutcome::StoredForNextStart {
                reason: StoreForNextStartReason::CoreFailed,
            },
        ),
    ] {
        let store = subscription_store();
        let runtime = Arc::new(ActivationRuntime::new(store.clone(), []));
        runtime.set_lifecycle(lifecycle);
        let app = subscription_refresh_app(store.clone(), runtime.clone());
        let source = subscription_source(store.clone(), RefreshMutation::None);

        let report = app
            .refresh_profile(&source, "main")
            .await
            .expect("inactive Core stores the exact selected workspace");

        assert_eq!(report.content_effect, expected_effect);
        assert_eq!(
            store.load("main").await.unwrap(),
            "mode: direct\nproxies: []\n"
        );
        assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 1);
        assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn transitioning_subscription_refresh_has_no_fetch_store_or_apply_effect() {
    for lifecycle in [CoreLifecycle::Starting, CoreLifecycle::Stopping] {
        let store = subscription_store();
        let runtime = Arc::new(ActivationRuntime::new(store.clone(), []));
        runtime.set_lifecycle(lifecycle);
        let app = subscription_refresh_app(store.clone(), runtime.clone());
        let source = subscription_source(store.clone(), RefreshMutation::None);

        let failure = app
            .refresh_profile(&source, "main")
            .await
            .expect_err("transitioning Core rejects before fetching");

        assert_eq!(failure.code, ErrorCode::InvalidState);
        assert_eq!(source.calls.load(Ordering::SeqCst), 0);
        assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 0);
        assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn stale_subscription_reference_or_workspace_has_zero_write_and_apply() {
    let store = subscription_store();
    let runtime = Arc::new(ActivationRuntime::new(store.clone(), []));
    let app = subscription_refresh_app(store.clone(), runtime.clone());
    let source = subscription_source(store.clone(), RefreshMutation::Select("rival".to_owned()));
    let failure = app
        .refresh_profile(&source, "main")
        .await
        .expect_err("a later profile selection invalidates the downloaded workspace");
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 0);
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);

    let store = subscription_store();
    let runtime = Arc::new(ActivationRuntime::new(store.clone(), []));
    let app = subscription_refresh_app(store.clone(), runtime.clone());
    let source = subscription_source(
        store.clone(),
        RefreshMutation::ReplaceDocument("mode: rule\nproxies: [later edit]\n".to_owned()),
    );
    let failure = app
        .refresh_profile(&source, "main")
        .await
        .expect_err("a later same-profile edit invalidates the captured workspace identity");
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 0);
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);
}

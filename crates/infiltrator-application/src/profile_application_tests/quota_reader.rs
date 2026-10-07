//! Actual reader composition proves quota independence from page-list failures.
//! test-intent: behavior
use super::*;
use crate::core_application::CoreApplication;
use crate::language_choice_fixtures::LanguageCaptureProcess;
use crate::profile_document_application::ProfileDocumentApplication;
use crate::surface_reader::ApplicationSurfaceReader;
use infiltrator_contract::subscription_quota::SubscriptionQuotaStatus;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep,
};
use infiltrator_ports::surface::SurfaceReader;
use std::time::Duration;
use tokio::runtime::Builder;
use tokio::time::sleep;

struct TestRuntime;
impl ApplicationRuntime for TestRuntime {
    fn block_on(&self, future: ApplicationFuture) {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(future);
    }
    fn sleep(&self, duration: Duration) -> ApplicationSleep<'_> {
        Box::pin(sleep(duration))
    }
}

#[tokio::test]
async fn subscription_quota_reader_retains_same_provider_zero_and_independent_editor_when_the_list_fails()
 {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    {
        let mut records = store.profiles.lock().unwrap();
        let (_, metadata) = records.get_mut("main").unwrap();
        metadata.subscription_url = Some("https://provider.test/account".into());
        metadata.traffic_upload = Some(0);
        metadata.traffic_download = Some(0);
        metadata.traffic_total = Some(100);
    }
    let profiles = ProfileApplication::new(store.clone());
    let document = ProfileDocumentApplication::new(profiles.clone())
        .load(None)
        .await
        .unwrap();
    let core = CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        Arc::new(TestRuntime),
    );
    let reader =
        ApplicationSurfaceReader::new(Arc::new(core), SurfaceKind::IcedDesktop, HostKind::Desktop)
            .with_profiles(profiles);
    let observed = reader.read().await.unwrap();
    assert_eq!(observed.subscription_quota.used_bytes, Some(0));
    assert_eq!(
        observed.subscription_quota.status,
        SubscriptionQuotaStatus::Ready
    );
    *store.list_read_error.lock().unwrap() =
        Some(PortError::PermissionDenied("list denied {detail}".into()));
    let failed = reader.read().await.unwrap();
    assert!(failed.pages.profiles.data.is_none());
    assert_eq!(failed.subscription_quota.used_bytes, Some(0));
    assert_eq!(failed.subscription_quota.total_bytes, Some(100));
    assert!(failed.subscription_quota.retained);
    assert_eq!(
        failed.subscription_quota.failure.unwrap().code,
        ErrorCode::Permission
    );
    assert_eq!(failed.profile_editor.document, Some(document));
    assert!(failed.profile_editor.read.source_current());
    store
        .profiles
        .lock()
        .unwrap()
        .get_mut("main")
        .unwrap()
        .1
        .subscription_url = Some("https://other-provider.test/account".into());
    let replaced = reader.read().await.unwrap();
    assert!(replaced.subscription_quota.used_bytes.is_none());
    assert!(replaced.subscription_quota.source.is_none());
    assert!(!replaced.subscription_quota.retained);
    *store.metadata_read_error.lock().unwrap() =
        Some(PortError::PermissionDenied("credential denied".into()));
    let unknown = reader.read().await.unwrap();
    assert!(unknown.subscription_quota.used_bytes.is_none());
    assert_eq!(
        unknown.subscription_quota.failure.unwrap().code,
        ErrorCode::Permission
    );
    *store.list_read_error.lock().unwrap() = None;
    *store.metadata_read_error.lock().unwrap() = None;
    store
        .profiles
        .lock()
        .unwrap()
        .get_mut("main")
        .unwrap()
        .1
        .traffic_upload = Some(45);
    let recovered = reader.read().await.unwrap();
    assert_eq!(recovered.subscription_quota.used_bytes, Some(45));
    assert!(recovered.subscription_quota.failure.is_none());
    assert!(!recovered.subscription_quota.retained);
    assert!(recovered.pages.profiles.data.is_some());
}

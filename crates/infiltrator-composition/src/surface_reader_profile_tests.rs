//! Profile quota remains source-bound when the independent profile page fails.
use super::*;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::subscription_quota::SubscriptionQuotaStatus;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::surface::SurfaceReader;
use infiltrator_test_support::language_choice_fixtures::LanguageCaptureProcess;
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::identity::EngineId;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct QuotaProfileStore {
    metadata: Mutex<ProfileMetadata>,
    list_error: Mutex<Option<PortError>>,
    metadata_error: Mutex<Option<PortError>>,
}

#[async_trait::async_trait]
impl ProfileStore for QuotaProfileStore {
    fn engine_id(&self) -> EngineId {
        EngineId::new("mihomo").expect("fixture engine ID")
    }

    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/isolated/profiles")
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        if let Some(error) = self.list_error.lock().expect("list error").clone() {
            return Err(error);
        }
        let metadata = self.metadata.lock().expect("profile metadata").clone();
        Ok(vec![ProfileInfo {
            name: "main".into(),
            active: true,
            path: "/isolated/profiles/main.yaml".into(),
            subscription_url: metadata.subscription_url,
            traffic_upload: metadata.traffic_upload,
            traffic_download: metadata.traffic_download,
            traffic_total: metadata.traffic_total,
            ..Default::default()
        }])
    }

    async fn get_current(&self) -> Result<String, PortError> {
        Ok("main".into())
    }

    async fn load(&self, _profile: &str) -> Result<String, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "quota fixture does not load profile documents",
        ))
    }

    async fn save_inactive_profile(&self, _profile: &str, _content: &str) -> Result<(), PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "quota fixture does not write profile documents",
        ))
    }

    async fn delete_profile(&self, _profile: &str) -> Result<(), PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "quota fixture does not delete profiles",
        ))
    }

    async fn get_profile_metadata(&self, _profile: &str) -> Result<ProfileMetadata, PortError> {
        if let Some(error) = self.metadata_error.lock().expect("metadata error").clone() {
            return Err(error);
        }
        Ok(self.metadata.lock().expect("profile metadata").clone())
    }

    async fn update_profile_metadata(
        &self,
        _profile: &str,
        metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        *self.metadata.lock().expect("profile metadata") = metadata.clone();
        Ok(())
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

#[tokio::test]
async fn quota_retains_only_the_same_provider_across_profile_page_failures() {
    let store = Arc::new(QuotaProfileStore::default());
    *store.metadata.lock().expect("profile metadata") = ProfileMetadata {
        subscription_url: Some("https://provider.test/account".into()),
        traffic_upload: Some(0),
        traffic_download: Some(0),
        traffic_total: Some(100),
        ..Default::default()
    };
    let profiles = ProfileApplication::new(store.clone());
    let runtime = crate::tokio_application_runtime().expect("composition runtime");
    let core = Arc::new(CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        runtime,
    ));
    let reader = ProductSurfaceReader::new(core, SurfaceKind::IcedDesktop, HostKind::Desktop)
        .with_profiles(profiles);

    let observed = reader.read().await.expect("initial surface read");
    assert_eq!(observed.subscription_quota.used_bytes, Some(0));
    assert_eq!(
        observed.subscription_quota.status,
        SubscriptionQuotaStatus::Ready
    );

    *store.list_error.lock().expect("list error") =
        Some(PortError::PermissionDenied("list denied".into()));
    let failed = reader
        .read()
        .await
        .expect("profile list failure is observed");
    assert!(failed.pages.profiles.data.is_none());
    assert_eq!(failed.subscription_quota.used_bytes, Some(0));
    assert_eq!(failed.subscription_quota.total_bytes, Some(100));
    assert!(failed.subscription_quota.retained);
    assert_eq!(
        failed.subscription_quota.failure.unwrap().code,
        ErrorCode::Permission
    );

    store
        .metadata
        .lock()
        .expect("profile metadata")
        .subscription_url = Some("https://other-provider.test/account".into());
    let replaced = reader.read().await.expect("provider replacement read");
    assert!(replaced.subscription_quota.used_bytes.is_none());
    assert!(replaced.subscription_quota.source.is_none());
    assert!(!replaced.subscription_quota.retained);

    *store.metadata_error.lock().expect("metadata error") =
        Some(PortError::PermissionDenied("metadata denied".into()));
    let unknown = reader
        .read()
        .await
        .expect("metadata read failure is observed");
    assert!(unknown.subscription_quota.used_bytes.is_none());
    assert_eq!(
        unknown.subscription_quota.failure.unwrap().code,
        ErrorCode::Permission
    );

    *store.list_error.lock().expect("list error") = None;
    *store.metadata_error.lock().expect("metadata error") = None;
    store
        .metadata
        .lock()
        .expect("profile metadata")
        .traffic_upload = Some(45);
    let recovered = reader.read().await.expect("quota recovery read");
    assert_eq!(recovered.subscription_quota.used_bytes, Some(45));
    assert!(recovered.subscription_quota.failure.is_none());
    assert!(!recovered.subscription_quota.retained);
    assert!(recovered.pages.profiles.data.is_some());
}

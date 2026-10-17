//! Foreign requests fail before adapter reads; malformed adapter receipts are detected after return.
use super::*;
use crate::core_capability_test_support::core_scope;
use crate::profile_application::ProfileRuntimeAccess;
use crate::profile_document_application::ProfileDocumentApplication;
use crate::profile_options_application::ProfileOptionsApplication;
use crate::runtime_query_application::RuntimeQueryApplication;
use async_trait::async_trait;
use infiltrator_contract::active_profile_reference::ActiveProfileReference;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceWriteAuthorization;
use infiltrator_contract::command::CoreLogLevel;
use infiltrator_contract::profile_document_identity::ProfileSidecarRevision;
use infiltrator_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use infiltrator_contract::profile_protection::ProfileWriteAuthorization;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_ports::active_profile_reference_applier::ActiveProfileReferenceApplier;
use infiltrator_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use infiltrator_ports::profile_workspace::{ProfileWorkspacePurpose, ProfileWorkspaceUpdate};
use infiltrator_ports::runtime_stream::UnsupportedRuntimeStreamPort;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
struct NoCallRuntime {
    calls: AtomicUsize,
}

#[tokio::test]
async fn missing_typed_log_level_port_is_unsupported_without_config_fallback() {
    let gateway = Arc::new(NoCallRuntime::default());
    let failure =
        RuntimeQueryApplication::new(gateway.clone(), Arc::new(UnsupportedRuntimeStreamPort))
            .set_core_log_level(CoreLogLevel::Debug)
            .await
            .unwrap_err();
    assert_eq!(failure.code, ErrorCode::Unsupported);
    assert_eq!(gateway.calls.load(Ordering::SeqCst), 0);
}

impl NoCallRuntime {
    fn unexpected(&self) -> ! {
        self.calls.fetch_add(1, Ordering::SeqCst);
        panic!("foreign engine identity must fail before runtime calls")
    }
}

impl CoreScopeSource for NoCallRuntime {
    fn scope_observation(&self) -> CoreScopeObservation {
        CoreScopeObservation::Observed(core_scope())
    }
}

#[async_trait]
impl ActiveProfileReferenceApplier for NoCallRuntime {
    fn engine_id(&self) -> Option<EngineId> {
        self.unexpected()
    }
    fn core_lifecycle(&self) -> CoreLifecycle {
        self.unexpected()
    }
    fn write_authorization(&self) -> Result<ActiveProfileReferenceWriteAuthorization, PortError> {
        self.unexpected()
    }
    async fn apply_active_profile_workspace(
        &self,
        _: &ActiveProfileReference,
        _: &ProfileWorkspaceIdentity,
        _: &ProfileWorkspaceUpdate,
        _: ActiveProfileReferenceWriteAuthorization,
        _: ApplyStrategy,
    ) -> Result<ProfileWorkspace, PortError> {
        self.unexpected()
    }
}

#[tokio::test]
async fn foreign_engine_cas_is_rejected_before_store_or_runtime_calls() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let profiles = ProfileApplication::new(store.clone())
        .with_document_codec(Arc::new(TestProfileDocumentCodec::mihomo()));
    let mut foreign = profiles
        .load_workspace(&profile_id("main"))
        .await
        .unwrap()
        .identity();
    let store_reads_before = store.workspace_reads.load(Ordering::SeqCst);
    foreign.document.engine_id = EngineId::new("sing-box").unwrap();
    let runtime = Arc::new(NoCallRuntime::default());
    let update = ProfileWorkspaceUpdate {
        purpose: ProfileWorkspacePurpose::Derived,
        document_bytes: b"mode: global\n".to_vec(),
        sidecar_bytes: None,
    };

    let failure = profiles
        .commit_workspace(
            ProfileRuntimeAccess::CoreOwner(runtime.clone()),
            &foreign,
            &update,
        )
        .await
        .unwrap_err();

    assert_eq!(failure.code, ErrorCode::Unsupported);
    assert_eq!(
        store.workspace_reads.load(Ordering::SeqCst),
        store_reads_before
    );
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 0);
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);
    assert_eq!(store.load("main").await.unwrap(), "mode: rule\n");
}

#[tokio::test]
async fn foreign_source_bound_writes_fail_before_workspace_reads_or_validation() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let codec = Arc::new(TestProfileDocumentCodec::mihomo());
    let profiles = ProfileApplication::new(store.clone()).with_document_codec(codec.clone());
    let mut foreign = profiles
        .load_workspace(&profile_id("main"))
        .await
        .unwrap()
        .identity();
    let reads_before = store.workspace_reads.load(Ordering::SeqCst);
    foreign.document.engine_id = EngineId::new("sing-box").unwrap();
    let document = ProfileDocumentApplication::new(profiles.clone());
    let runtime = Arc::new(NoCallRuntime::default());
    let options = ProfileOptionsApplication::new(profiles);

    let document_failure = document
        .save::<NoCallRuntime>(
            ProfileRuntimeAccess::CoreOwner(runtime.clone()),
            &foreign,
            "proxies: [malformed",
            ProfileWriteAuthorization::RequireUnlock,
        )
        .await
        .unwrap_err();
    assert_eq!(document_failure.code, ErrorCode::Unsupported);
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);

    let mixin_failure = options
        .save_mixin::<NoCallRuntime>(ProfileRuntimeAccess::Offline, &foreign, "not: [valid")
        .await
        .unwrap_err();
    assert_eq!(mixin_failure.code, ErrorCode::Unsupported);

    let filter_failure = options
        .save_filter::<NoCallRuntime>(
            ProfileRuntimeAccess::Offline,
            &foreign,
            &SubscriptionFilterDraft {
                advanced_policy: Some("not: [valid".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
    assert_eq!(filter_failure.code, ErrorCode::Unsupported);
    assert_eq!(store.workspace_reads.load(Ordering::SeqCst), reads_before);
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 0);
    assert_eq!(codec.options_call_count(), 0);
    assert_eq!(store.load("main").await.unwrap(), "mode: rule\n");
}

#[tokio::test]
async fn options_codec_rejects_foreign_document_format_and_schema_before_fake_codec_or_store_access()
 {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let codec = Arc::new(TestProfileDocumentCodec::mihomo());
    let profiles = ProfileApplication::new(store.clone()).with_document_codec(codec.clone());
    let options = ProfileOptionsApplication::new(profiles.clone());
    let identity = workspace_with_origin("main", b"mode: rule\n", None, None)
        .unwrap()
        .document
        .identity;
    let reads_before = store.workspace_reads.load(Ordering::SeqCst);

    for foreign in [
        {
            let mut identity = identity.clone();
            identity.format_id = ProfileFormatId::new("sing-box-json").unwrap();
            identity
        },
        {
            let mut identity = identity.clone();
            identity.schema_version = ProfileSchemaVersion::new(2).unwrap();
            identity
        },
    ] {
        let expected = ProfileWorkspaceIdentity::new(foreign, ProfileSidecarRevision::Absent);
        let mixin_failure = options
            .save_mixin(
                ProfileRuntimeAccess::<dyn ActiveProfileReferenceApplier>::Offline,
                &expected,
                "not: [valid",
            )
            .await
            .unwrap_err();
        assert_eq!(mixin_failure.code, ErrorCode::Unsupported);
        let filter_failure = options
            .save_filter(
                ProfileRuntimeAccess::<dyn ActiveProfileReferenceApplier>::Offline,
                &expected,
                &SubscriptionFilterDraft {
                    advanced_policy: Some("not: [valid".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert_eq!(filter_failure.code, ErrorCode::Unsupported);
    }

    assert_eq!(codec.options_call_count(), 0);
    assert_eq!(store.workspace_reads.load(Ordering::SeqCst), reads_before);
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn foreign_engine_workspace_receipt_is_rejected_after_adapter_return() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let profiles = ProfileApplication::new(store.clone());
    let expected = profiles
        .load_workspace(&profile_id("main"))
        .await
        .unwrap()
        .identity();
    store
        .foreign_workspace_receipt
        .store(true, Ordering::SeqCst);
    let update = ProfileWorkspaceUpdate {
        purpose: ProfileWorkspacePurpose::Derived,
        document_bytes: b"mode: global\n".to_vec(),
        sidecar_bytes: None,
    };

    let failure = profiles
        .commit_workspace::<NoCallRuntime>(ProfileRuntimeAccess::Offline, &expected, &update)
        .await
        .unwrap_err();

    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert_eq!(
        store.workspace_writes.load(Ordering::SeqCst),
        2,
        "the invalid adapter receipt is followed by a conditional rollback"
    );
    assert_eq!(store.load("main").await.unwrap(), "mode: rule\n");
}

#[tokio::test]
async fn workspace_receipt_must_advance_changed_document_and_sidecar_revisions() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let profiles = ProfileApplication::new(store.clone());
    let expected = profiles
        .load_workspace(&profile_id("main"))
        .await
        .unwrap()
        .identity();
    store
        .stale_document_revision_receipt
        .store(true, Ordering::SeqCst);
    let failure = profiles
        .commit_workspace::<NoCallRuntime>(
            ProfileRuntimeAccess::Offline,
            &expected,
            &ProfileWorkspaceUpdate {
                purpose: ProfileWorkspacePurpose::Derived,
                document_bytes: b"mode: global\n".to_vec(),
                sidecar_bytes: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::InvalidState);

    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    store
        .sidecars
        .lock()
        .unwrap()
        .insert("main".into(), b"previous sidecar".to_vec());
    let profiles = ProfileApplication::new(store.clone());
    let expected = profiles
        .load_workspace(&profile_id("main"))
        .await
        .unwrap()
        .identity();
    store
        .stale_sidecar_revision_receipt
        .store(true, Ordering::SeqCst);
    let failure = profiles
        .commit_workspace::<NoCallRuntime>(
            ProfileRuntimeAccess::Offline,
            &expected,
            &ProfileWorkspaceUpdate {
                purpose: ProfileWorkspacePurpose::Derived,
                document_bytes: b"mode: rule\n".to_vec(),
                sidecar_bytes: Some(b"opaque sidecar bytes".to_vec()),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::InvalidState);
}

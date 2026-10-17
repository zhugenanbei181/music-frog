use super::*;
use crate::profile_document_codec::{PROFILE_FORMAT, PROFILE_SCHEMA_VERSION};
use infiltrator_contract::aggregator::AggregationTemplate;
use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::profile_store::{
    ActiveProfileWorkspacePublishOutcome, ActiveProfileWorkspacePublishedLease,
    ActiveProfileWorkspaceRestoreOutcome, ActiveProfileWorkspaceRestoredLease,
    ActiveProfileWorkspaceWriteLease,
};
use infiltrator_ports::profile_workspace::ProfileWorkspacePurpose;
use infiltrator_ports::profile_workspace::ProfileWorkspaceUpdate;
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
    ActiveProfileReferenceWriteAuthorization,
};
use lurker_core_contract::identity::{CoreIdentity, CoreInstanceId, CoreVersion, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycleSnapshot;
use lurker_core_contract::profile_document_identity::{
    ProfileDocument, ProfileDocumentIdentity, ProfileDocumentRevision,
    ProfileDocumentSourceIdentity, ProfileDocumentSourceKind, ProfileId, ProfileSidecarRevision,
    ProfileSourceId,
};
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use lurker_core_contract::profile_protection::ProfileWriteProtection;
use lurker_core_contract::session::SessionToken;
use singbox_process::command::{NativeConfigCheck, SingBoxChildProcess, SingBoxCommandPort};
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{self, RwLock};
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::{Mutex, OwnedMutexGuard};

struct FakeCommand {
    checks: AtomicUsize,
    outcomes: sync::Mutex<VecDeque<NativeConfigCheck>>,
}

impl FakeCommand {
    fn accepting() -> Self {
        Self {
            checks: AtomicUsize::new(0),
            outcomes: sync::Mutex::new(VecDeque::new()),
        }
    }

    fn with_outcomes(outcomes: impl IntoIterator<Item = NativeConfigCheck>) -> Self {
        Self {
            checks: AtomicUsize::new(0),
            outcomes: sync::Mutex::new(outcomes.into_iter().collect()),
        }
    }
}

#[async_trait::async_trait]
impl SingBoxCommandPort for FakeCommand {
    async fn check_config(
        &self,
        _binary_path: &Path,
        _config_path: &Path,
    ) -> Result<NativeConfigCheck, PortError> {
        self.checks.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .outcomes
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(NativeConfigCheck::Accepted))
    }

    async fn spawn_core(
        &self,
        _binary_path: &Path,
        _config_path: &Path,
    ) -> Result<Box<dyn SingBoxChildProcess>, PortError> {
        Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "test command",
        ))
    }
}

struct FakeSession {
    identity: CoreIdentity,
    lifecycle: RwLock<CoreLifecycle>,
}

impl FakeSession {
    fn new(lifecycle: CoreLifecycle) -> Self {
        Self::for_instance(lifecycle, "singbox-test")
    }

    fn for_instance(lifecycle: CoreLifecycle, instance: &str) -> Self {
        Self {
            identity: CoreIdentity::new(
                EngineId::sing_box(),
                CoreInstanceId::new(instance).unwrap(),
                Some(CoreVersion::new("v1.14.3").unwrap()),
            ),
            lifecycle: RwLock::new(lifecycle),
        }
    }
}

#[async_trait::async_trait]
impl CoreLifecyclePort for FakeSession {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(self.identity.clone())
    }

    fn lifecycle(&self) -> CoreLifecycle {
        self.lifecycle.read().unwrap().clone()
    }

    fn generation(&self) -> u64 {
        1
    }

    fn session_token(&self) -> Option<SessionToken> {
        None
    }

    async fn start(&self) -> Result<u64, PortError> {
        Ok(1)
    }

    async fn stop(&self) -> Result<(), PortError> {
        Ok(())
    }

    async fn restart(&self) -> Result<u64, PortError> {
        Ok(2)
    }

    async fn wait_for_ready(&self, _generation: u64, _timeout: Duration) -> Result<(), PortError> {
        Ok(())
    }

    fn lifecycle_snapshot(&self) -> CoreLifecycleSnapshot {
        CoreLifecycleSnapshot {
            identity: Some(self.identity.clone()),
            lifecycle: self.lifecycle(),
            generation: 1,
            session_token: None,
            revision: 1,
            failure: None,
        }
    }
}

struct FakeProfileStore {
    engine_id: EngineId,
    reference: Mutex<ActiveProfileReferenceState>,
    active: Mutex<ProfileWorkspace>,
    workspace_write_gate: Arc<Mutex<()>>,
    writes: AtomicUsize,
    next_revision: AtomicUsize,
}

struct FakeWorkspaceWriteLease<'a> {
    store: &'a FakeProfileStore,
    expected_reference: ActiveProfileReference,
    observed: ProfileWorkspace,
    _guard: OwnedMutexGuard<()>,
}

struct FakeWorkspacePublishedLease<'a> {
    store: &'a FakeProfileStore,
    expected_reference: ActiveProfileReference,
    previous: ProfileWorkspace,
    published: ProfileWorkspace,
    _guard: OwnedMutexGuard<()>,
}

struct FakeWorkspaceRestoredLease {
    workspace: ProfileWorkspace,
    _guard: OwnedMutexGuard<()>,
}

impl FakeProfileStore {
    fn new(active: ProfileWorkspace) -> Self {
        let reference = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
            active.document.identity.engine_id.clone(),
            active.document.identity.profile_id.clone(),
            ActiveProfileReferenceRevision::default(),
        ));
        Self {
            engine_id: active.document.identity.engine_id.clone(),
            reference: Mutex::new(reference),
            active: Mutex::new(active),
            workspace_write_gate: Arc::new(Mutex::new(())),
            writes: AtomicUsize::new(0),
            next_revision: AtomicUsize::new(2),
        }
    }

    async fn replace(&self, workspace: ProfileWorkspace) {
        let _guard = Arc::clone(&self.workspace_write_gate).lock_owned().await;
        *self.active.lock().await = workspace;
    }
}

#[async_trait::async_trait]
impl ActiveProfileWorkspaceWriteLease for FakeWorkspaceWriteLease<'_> {
    fn observed_workspace(&self) -> &ProfileWorkspace {
        &self.observed
    }

    async fn publish(
        self: Box<Self>,
        update: ProfileWorkspaceUpdate,
    ) -> ActiveProfileWorkspacePublishOutcome<'async_trait> {
        let reference = self.store.reference.lock().await;
        if *reference != ActiveProfileReferenceState::Selected(self.expected_reference.clone()) {
            return ActiveProfileWorkspacePublishOutcome::Unpublished(rejected(
                ErrorCode::NotReady,
                "fake active reference changed before publication",
            ));
        }
        let mut active = self.store.active.lock().await;
        if active.identity() != self.observed.identity() {
            return ActiveProfileWorkspacePublishOutcome::Unpublished(rejected(
                ErrorCode::NotReady,
                "fake active workspace is stale",
            ));
        }
        let revision = self.store.next_revision.fetch_add(1, Ordering::SeqCst);
        let document_identity = ProfileDocumentIdentity::new(
            active.document.identity.profile_id.clone(),
            active.document.identity.engine_id.clone(),
            active.document.identity.source.clone(),
            ProfileDocumentRevision::new(format!("revision-{revision}")).unwrap(),
            active.document.identity.format_id.clone(),
            active.document.identity.schema_version,
        );
        let sidecar_revision = match &update.sidecar_bytes {
            Some(_) => ProfileSidecarRevision::Present(
                ProfileDocumentRevision::new(format!("sidecar-{revision}")).unwrap(),
            ),
            None => ProfileSidecarRevision::Absent,
        };
        let write_protection = active.write_protection;
        *active = ProfileWorkspace {
            document: ProfileDocument::new(document_identity, update.document_bytes),
            sidecar_revision,
            write_protection,
            sidecar_bytes: update.sidecar_bytes,
        };
        let published = active.clone();
        self.store.writes.fetch_add(1, Ordering::SeqCst);
        drop(active);
        drop(reference);
        ActiveProfileWorkspacePublishOutcome::Published(Box::new(FakeWorkspacePublishedLease {
            store: self.store,
            expected_reference: self.expected_reference.clone(),
            previous: self.observed.clone(),
            published,
            _guard: self._guard,
        }))
    }
}

#[async_trait::async_trait]
impl ActiveProfileWorkspacePublishedLease for FakeWorkspacePublishedLease<'_> {
    fn previous_workspace(&self) -> &ProfileWorkspace {
        &self.previous
    }

    fn published_workspace(&self) -> &ProfileWorkspace {
        &self.published
    }

    fn commit(self: Box<Self>) -> ProfileWorkspace {
        self.published.clone()
    }

    async fn restore_if_current(
        self: Box<Self>,
    ) -> Result<ActiveProfileWorkspaceRestoreOutcome<'async_trait>, PortError> {
        let reference = self.store.reference.lock().await;
        let mut active = self.store.active.lock().await;
        if *reference != ActiveProfileReferenceState::Selected(self.expected_reference.clone())
            || active.identity() != self.published.identity()
        {
            return Ok(ActiveProfileWorkspaceRestoreOutcome::Superseded {
                observed: active.identity(),
            });
        }
        *active = self.previous.clone();
        let restored = active.clone();
        self.store.writes.fetch_add(1, Ordering::SeqCst);
        drop(active);
        drop(reference);
        Ok(ActiveProfileWorkspaceRestoreOutcome::Restored(Box::new(
            FakeWorkspaceRestoredLease {
                workspace: restored,
                _guard: self._guard,
            },
        )))
    }
}

#[async_trait::async_trait]
impl ActiveProfileWorkspaceRestoredLease for FakeWorkspaceRestoredLease {
    fn restored_workspace(&self) -> &ProfileWorkspace {
        &self.workspace
    }

    fn finish(self: Box<Self>) -> ProfileWorkspace {
        self.workspace.clone()
    }
}

#[async_trait::async_trait]
impl ProfileStore for FakeProfileStore {
    fn engine_id(&self) -> EngineId {
        self.engine_id.clone()
    }

    fn config_dir(&self) -> PathBuf {
        PathBuf::from("singbox-test")
    }

    fn apply_transaction(&self, _profile: &str) -> Option<ApplyTransactionSnapshot> {
        None
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        Err(unsupported_profile())
    }

    async fn get_current(&self) -> Result<String, PortError> {
        Err(unsupported_profile())
    }

    async fn load(&self, _profile: &str) -> Result<String, PortError> {
        Err(unsupported_profile())
    }

    async fn save_inactive_profile(&self, _profile: &str, _content: &str) -> Result<(), PortError> {
        Err(unsupported_profile())
    }

    async fn delete_profile(&self, _profile: &str) -> Result<(), PortError> {
        Err(unsupported_profile())
    }

    async fn get_profile_metadata(&self, _profile: &str) -> Result<ProfileMetadata, PortError> {
        Err(unsupported_profile())
    }

    async fn update_profile_metadata(
        &self,
        _profile: &str,
        _metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        Err(unsupported_profile())
    }

    async fn delete_subscription_credential(&self, _profile: &str) -> Result<(), PortError> {
        Err(unsupported_profile())
    }

    async fn load_workspace(&self, _profile: &ProfileId) -> Result<ProfileWorkspace, PortError> {
        Err(unsupported_profile())
    }

    async fn load_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        Ok(self.active.lock().await.clone())
    }

    async fn read_active_profile_reference(
        &self,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        Ok(self.reference.lock().await.clone())
    }

    async fn acquire_active_workspace_write_lease(
        &self,
        expected_reference: ActiveProfileReference,
        expected: ProfileWorkspaceIdentity,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<Box<dyn ActiveProfileWorkspaceWriteLease + '_>, PortError> {
        let guard = Arc::clone(&self.workspace_write_gate).lock_owned().await;
        let reference = self.reference.lock().await;
        if authorization != ActiveProfileReferenceWriteAuthorization::Unowned
            || *reference != ActiveProfileReferenceState::Selected(expected_reference.clone())
            || expected_reference.engine_id != expected.document.engine_id
            || expected_reference.profile_id != expected.document.profile_id
        {
            return Err(rejected(
                ErrorCode::NotReady,
                "fake active reference is stale",
            ));
        }
        let observed = self.active.lock().await.clone();
        if observed.identity() != expected {
            return Err(rejected(
                ErrorCode::NotReady,
                "fake active workspace is stale",
            ));
        }
        drop(reference);
        Ok(Box::new(FakeWorkspaceWriteLease {
            store: self,
            expected_reference,
            observed,
            _guard: guard,
        }))
    }

    async fn load_aggregation_templates(&self) -> Result<Vec<AggregationTemplate>, PortError> {
        Err(unsupported_profile())
    }

    async fn save_aggregation_templates(
        &self,
        _templates: &[AggregationTemplate],
    ) -> Result<(), PortError> {
        Err(unsupported_profile())
    }

    async fn clear_backup(&self, _profile: &str) -> Result<(), PortError> {
        Err(unsupported_profile())
    }

    async fn restore_inactive_profile_backup(&self, _profile: &str) -> Result<bool, PortError> {
        Err(unsupported_profile())
    }
}

fn unsupported_profile() -> PortError {
    PortError::unsupported(Capability::Profiles, "unused fake profile operation")
}

fn workspace(raw_bytes: &[u8]) -> ProfileWorkspace {
    let document = ProfileDocumentIdentity::new(
        ProfileId::new("main").unwrap(),
        EngineId::sing_box(),
        ProfileDocumentSourceIdentity::new(
            ProfileDocumentSourceKind::Local,
            ProfileSourceId::new("singbox-local").unwrap(),
        ),
        ProfileDocumentRevision::new("revision-1").unwrap(),
        ProfileFormatId::new(PROFILE_FORMAT).unwrap(),
        ProfileSchemaVersion::new(PROFILE_SCHEMA_VERSION).unwrap(),
    );
    ProfileWorkspace {
        document: ProfileDocument::new(document, raw_bytes.to_vec()),
        sidecar_revision: ProfileSidecarRevision::Absent,
        write_protection: ProfileWriteProtection::Editable,
        sidecar_bytes: None,
    }
}

fn request(workspace: &ProfileWorkspace, update: &[u8]) -> ProfileConfigurationTransactionRequest {
    let expected = workspace.identity();
    let expected_reference = ActiveProfileReference::new(
        expected.document.engine_id.clone(),
        expected.document.profile_id.clone(),
        ActiveProfileReferenceRevision::default(),
    );
    ProfileConfigurationTransactionRequest {
        expected_reference,
        expected,
        update: ProfileWorkspaceUpdate {
            purpose: ProfileWorkspacePurpose::Derived,
            document_bytes: update.to_vec(),
            sidecar_bytes: None,
        },
        authorization: ActiveProfileReferenceWriteAuthorization::Unowned,
        strategy: ApplyStrategy::PreferReload,
    }
}

#[test]
fn transaction_rejects_a_session_from_another_sing_box_instance() {
    let directory = TempDir::new().unwrap();
    let store = Arc::new(FakeProfileStore::new(workspace(
        br#"{"log":{"level":"warn"}}"#,
    )));
    let command = Arc::new(FakeCommand::accepting());
    let command_port: Arc<dyn SingBoxCommandPort> = command.clone();
    let config = Arc::new(SingBoxProfileConfigurationAdapter::new(
        PathBuf::from("fake-sing-box"),
        directory.path().join("active.json"),
        command_port,
    ));
    let session = Arc::new(FakeSession::for_instance(
        CoreLifecycle::Stopped,
        "another-singbox-instance",
    ));
    let process_identity = CoreIdentity::new(
        EngineId::sing_box(),
        CoreInstanceId::new("singbox-test").unwrap(),
        None,
    );

    let result = SingBoxProfileConfigurationTransactionAdapter::new(
        store.clone(),
        session,
        config,
        Arc::new(SingBoxCoreOperationGate::new()),
        process_identity,
    );

    assert!(matches!(result, Err(PortError::Failed(_))));
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    assert_eq!(command.checks.load(Ordering::SeqCst), 0);
}

fn transaction(
    store: Arc<FakeProfileStore>,
    session: Arc<FakeSession>,
    config_path: PathBuf,
    command: Arc<FakeCommand>,
) -> SingBoxProfileConfigurationTransactionAdapter {
    let config: Arc<SingBoxProfileConfigurationAdapter> =
        Arc::new(SingBoxProfileConfigurationAdapter::new(
            PathBuf::from("fake-sing-box"),
            config_path,
            command,
        ));
    let process_identity = session
        .core_identity()
        .expect("test session has a Core identity");
    SingBoxProfileConfigurationTransactionAdapter::new(
        store,
        session,
        config,
        Arc::new(SingBoxCoreOperationGate::new()),
        process_identity,
    )
    .unwrap()
}

#[tokio::test]
async fn stopped_transaction_cas_saves_and_reads_back_exact_json_bytes() {
    let directory = TempDir::new().unwrap();
    let config_path = directory.path().join("active.json");
    let previous_bytes = br#"{"log":{"level":"warn"}}"#;
    fs::write(&config_path, previous_bytes).unwrap();
    let initial = workspace(previous_bytes);
    let store = Arc::new(FakeProfileStore::new(initial.clone()));
    let command = Arc::new(FakeCommand::accepting());
    let adapter = transaction(
        Arc::clone(&store),
        Arc::new(FakeSession::new(CoreLifecycle::Stopped)),
        config_path.clone(),
        Arc::clone(&command),
    );
    let submitted = br#"{"log":{"level":"info"}}"#;

    let outcome = adapter
        .transact(request(&initial, submitted))
        .await
        .unwrap();

    match outcome {
        ProfileConfigurationTransactionOutcome::Committed { workspace, effect } => {
            assert_eq!(workspace.document.raw_bytes, submitted);
            assert_ne!(workspace.identity(), initial.identity());
            assert_eq!(effect, ProfileConfigurationCommitEffect::StoredWhileStopped);
        }
        other => panic!("expected a committed sing-box workspace, received {other:?}"),
    }
    assert_eq!(fs::read(config_path).unwrap(), submitted);
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    assert_eq!(command.checks.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn foreign_stale_and_invalid_requests_do_not_write_the_workspace_or_active_file() {
    let directory = TempDir::new().unwrap();
    let config_path = directory.path().join("active.json");
    let original_bytes = br#"{"inbounds":[]}"#;
    fs::write(&config_path, original_bytes).unwrap();
    let initial = workspace(original_bytes);
    let store = Arc::new(FakeProfileStore::new(initial.clone()));
    let command = Arc::new(FakeCommand::accepting());
    let adapter = transaction(
        Arc::clone(&store),
        Arc::new(FakeSession::new(CoreLifecycle::Stopped)),
        config_path.clone(),
        Arc::clone(&command),
    );

    let mut foreign = request(&initial, br#"{"inbounds":[]}"#);
    foreign.expected.document.engine_id = EngineId::new("mihomo").unwrap();
    assert!(matches!(
        adapter.transact(foreign).await,
        Err(PortError::Unsupported { .. })
    ));

    let stale = request(&initial, br#"{"inbounds":[{"type":"tun"}]}"#);
    let mut newer = workspace(br#"{"inbounds":[{"type":"mixed"}]}"#);
    newer.document.identity.revision = ProfileDocumentRevision::new("revision-2").unwrap();
    store.replace(newer).await;
    assert!(matches!(
        adapter.transact(stale).await,
        Err(PortError::Rejected(_))
    ));

    let current = store.load_active_workspace().await.unwrap();
    let invalid = request(&current, b"[]");
    assert!(matches!(
        adapter.transact(invalid).await,
        Err(PortError::Rejected(_))
    ));
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    assert_eq!(command.checks.load(Ordering::SeqCst), 0);
    assert_eq!(fs::read(config_path).unwrap(), original_bytes);
}

#[tokio::test]
async fn running_core_is_unsupported_without_native_or_profile_dispatch() {
    let directory = TempDir::new().unwrap();
    let config_path = directory.path().join("active.json");
    let original_bytes = br#"{"inbounds":[]}"#;
    fs::write(&config_path, original_bytes).unwrap();
    let initial = workspace(original_bytes);
    let store = Arc::new(FakeProfileStore::new(initial.clone()));
    let command = Arc::new(FakeCommand::accepting());
    let adapter = transaction(
        Arc::clone(&store),
        Arc::new(FakeSession::new(CoreLifecycle::Running)),
        config_path.clone(),
        Arc::clone(&command),
    );

    assert!(matches!(
        adapter
            .transact(request(&initial, br#"{"inbounds":[{"type":"tun"}]}"#))
            .await,
        Err(PortError::Unsupported { .. })
    ));
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    assert_eq!(command.checks.load(Ordering::SeqCst), 0);
    assert_eq!(fs::read(config_path).unwrap(), original_bytes);
}

#[tokio::test]
async fn native_apply_rejection_rolls_back_workspace_and_exact_file_bytes() {
    let directory = TempDir::new().unwrap();
    let config_path = directory.path().join("active.json");
    let original_bytes = br#"{"inbounds":[]}"#;
    fs::write(&config_path, original_bytes).unwrap();
    let initial = workspace(original_bytes);
    let store = Arc::new(FakeProfileStore::new(initial.clone()));
    let command = Arc::new(FakeCommand::with_outcomes([
        NativeConfigCheck::Accepted,
        NativeConfigCheck::Rejected,
    ]));
    let adapter = transaction(
        Arc::clone(&store),
        Arc::new(FakeSession::new(CoreLifecycle::Failed)),
        config_path.clone(),
        Arc::clone(&command),
    );

    let outcome = adapter
        .transact(request(&initial, br#"{"inbounds":[{"type":"tun"}]}"#))
        .await
        .unwrap();

    match outcome {
        ProfileConfigurationTransactionOutcome::RolledBack { restored, .. } => {
            assert_eq!(restored, initial);
        }
        other => panic!("expected exact rollback, received {other:?}"),
    }
    assert_eq!(store.load_active_workspace().await.unwrap(), initial);
    assert_eq!(fs::read(config_path).unwrap(), original_bytes);
    assert_eq!(store.writes.load(Ordering::SeqCst), 2);
    assert_eq!(command.checks.load(Ordering::SeqCst), 2);
}

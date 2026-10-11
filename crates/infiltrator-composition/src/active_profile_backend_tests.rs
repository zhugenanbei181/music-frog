use super::{ActiveProfileBackendSelector, PreviousCoreDisposition, PreviousCoreOwner};
use async_trait::async_trait;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
    ActiveProfileReferenceTarget,
};
use infiltrator_contract::adapter_operation_declaration::AdapterOperationOwner;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, CoreCapabilityEntry, CoreCapabilityMatrix, CoreCapabilityScope,
    CoreLifecycleOperation, CoreOperation, EngineBinaryTarget, EngineCapabilityDecision,
    HostCapabilityDecision,
};
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreVersion, EngineId,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document::DocumentValidation;
use infiltrator_contract::profile_document_identity::{
    ProfileDocument, ProfileDocumentIdentity, ProfileDocumentRevision,
    ProfileDocumentSourceIdentity, ProfileDocumentSourceKind, ProfileFormatId, ProfileId,
    ProfileSchemaVersion, ProfileSidecarRevision, ProfileSourceId,
};
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};
use infiltrator_contract::surface::HostKind;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::core_capability_provider::{
    CoreCapabilityProvider, CoreCapabilityProviderError,
};
use infiltrator_ports::core_lifecycle::{CoreLifecyclePort, CoreOwnerTerminationPort};
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::engine_backend_factory::{
    EngineBackendPorts, EngineBackendProvider, EngineBackendProviderAvailability,
    EngineOperationPorts,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationTransactionBinding, ProfileConfigurationTransactionScope,
};
use infiltrator_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::ProfileWorkspace;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct MemoryReferenceStore {
    state: Arc<tokio::sync::Mutex<ActiveProfileReferenceState>>,
    owner: Arc<Mutex<Option<(CoreIdentity, CoreGeneration)>>>,
}

struct MemoryReferenceLease {
    state: tokio::sync::OwnedMutexGuard<ActiveProfileReferenceState>,
    owner: Arc<Mutex<Option<(CoreIdentity, CoreGeneration)>>>,
}

impl infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceLease
    for MemoryReferenceLease
{
    fn state(&self) -> &ActiveProfileReferenceState {
        &self.state
    }

    fn into_owner_binding(
        self: Box<Self>,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<
        Box<dyn infiltrator_ports::active_profile_reference_store::ActiveProfileOwnerLease>,
        PortError,
    > {
        let owner_state = Arc::clone(&self.owner);
        match &*self.state {
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == identity.engine => {}
            _ => {
                return Err(PortError::Failed(
                    "active reference does not authorize fake owner binding".into(),
                ));
            }
        }
        let mut owner = self.owner.lock().expect("owner state");
        if owner.is_some() {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake Core owner remains active",
                true,
            )));
        }
        *owner = Some((identity.clone(), generation));
        drop(owner);
        drop(self);
        Ok(Box::new(MemoryOwnerLease {
            identity,
            generation,
            owner: owner_state,
        }))
    }
}

struct MemoryOwnerLease {
    identity: CoreIdentity,
    generation: CoreGeneration,
    owner: Arc<Mutex<Option<(CoreIdentity, CoreGeneration)>>>,
}

impl infiltrator_ports::active_profile_reference_store::ActiveProfileOwnerLease
    for MemoryOwnerLease
{
    fn identity(&self) -> &CoreIdentity {
        &self.identity
    }

    fn generation(&self) -> CoreGeneration {
        self.generation
    }

    fn release_after_terminal(&self, terminal: &CoreLifecycleSnapshot) -> Result<(), PortError> {
        if terminal.identity.as_ref() != Some(&self.identity)
            || terminal.generation < self.generation.value()
            || terminal.lifecycle != CoreLifecycle::Stopped
            || terminal.session_token.is_some()
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "owner lease requires matching Stopped readback without a session",
                true,
            )));
        }
        let mut owner = self.owner.lock().expect("owner state");
        if owner.as_ref() != Some(&(self.identity.clone(), self.generation)) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "owner lease is stale",
                true,
            )));
        }
        *owner = None;
        Ok(())
    }
}

impl MemoryReferenceStore {
    fn new(state: ActiveProfileReferenceState) -> Self {
        Self {
            state: Arc::new(tokio::sync::Mutex::new(state)),
            owner: Arc::new(Mutex::new(None)),
        }
    }
}

#[async_trait]
impl ActiveProfileReferenceStore for MemoryReferenceStore {
    async fn read(&self) -> Result<ActiveProfileReferenceState, PortError> {
        Ok(self.state.lock().await.clone())
    }

    async fn acquire_selection_lease(
        &self,
    ) -> Result<
        Box<dyn infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceLease>,
        PortError,
    > {
        Ok(Box::new(MemoryReferenceLease {
            state: Arc::clone(&self.state).lock_owned().await,
            owner: Arc::clone(&self.owner),
        }))
    }

    async fn acquire_provider_selection_lease(
        &self,
    ) -> Result<
        Box<dyn infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceLease>,
        PortError,
    > {
        let state = Arc::clone(&self.state).lock_owned().await;
        if self.owner.lock().expect("owner state").is_some() {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake active Core owner must close before another provider is composed",
                true,
            )));
        }
        Ok(Box::new(MemoryReferenceLease {
            state,
            owner: Arc::clone(&self.owner),
        }))
    }

    async fn compare_and_set(
        &self,
        expected_revision: ActiveProfileReferenceRevision,
        target: ActiveProfileReferenceTarget,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        let mut state = self.state.lock().await;
        if state.revision() != expected_revision {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active profile reference revision is stale",
                true,
            )));
        }
        if let Some((identity, generation)) = self.owner.lock().expect("owner state").as_ref() {
            match &target {
                ActiveProfileReferenceTarget::Selected { engine_id, .. }
                    if engine_id == &identity.engine => {}
                _ => {
                    return Err(PortError::Rejected(Failure::new(
                        ErrorCode::NotReady,
                        format!(
                            "fake Core owner {} at generation {} must terminate before engine selection changes",
                            identity.instance.as_str(),
                            generation.value()
                        ),
                        true,
                    )));
                }
            }
        }
        let revision = expected_revision
            .next()
            .ok_or_else(|| PortError::Failed("active profile revision exhausted".into()))?;
        *state = match target {
            ActiveProfileReferenceTarget::Unselected => {
                ActiveProfileReferenceState::Unselected { revision }
            }
            ActiveProfileReferenceTarget::Selected {
                engine_id,
                profile_id,
            } => ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
                engine_id, profile_id, revision,
            )),
        };
        Ok(state.clone())
    }
}

struct FakeProfileStore {
    engine_id: EngineId,
    document: ProfileDocument,
    current_reads: AtomicUsize,
}

impl FakeProfileStore {
    fn new(engine_id: &str, document_engine: &str, document_profile: &str) -> Self {
        Self {
            engine_id: engine_id_value(engine_id),
            document: profile_document(document_engine, document_profile),
            current_reads: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl ProfileStore for FakeProfileStore {
    fn engine_id(&self) -> EngineId {
        self.engine_id.clone()
    }

    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/memory/profiles")
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        Ok(Vec::new())
    }

    async fn get_current(&self) -> Result<String, PortError> {
        self.current_reads.fetch_add(1, Ordering::SeqCst);
        Ok("namespace-current-must-not-select-provider".into())
    }

    async fn set_current(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn load(&self, _profile: &str) -> Result<String, PortError> {
        Ok(String::from_utf8_lossy(&self.document.raw_bytes).into_owned())
    }

    async fn save(&self, _profile: &str, _content: &str) -> Result<(), PortError> {
        Err(unsupported_profiles())
    }

    async fn delete_profile(&self, _profile: &str) -> Result<(), PortError> {
        Err(unsupported_profiles())
    }

    async fn get_profile_metadata(&self, _profile: &str) -> Result<ProfileMetadata, PortError> {
        Ok(ProfileMetadata::default())
    }

    async fn update_profile_metadata(
        &self,
        _profile: &str,
        _metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        Err(unsupported_profiles())
    }

    async fn delete_subscription_credential(&self, _profile: &str) -> Result<(), PortError> {
        Err(unsupported_profiles())
    }

    async fn load_workspace(&self, _profile: &ProfileId) -> Result<ProfileWorkspace, PortError> {
        Ok(ProfileWorkspace {
            document: self.document.clone(),
            sidecar_revision: ProfileSidecarRevision::Absent,
            write_protection: ProfileWriteProtection::Editable,
            sidecar_bytes: None,
        })
    }

    async fn clear_backup(&self, _profile: &str) -> Result<(), PortError> {
        Err(unsupported_profiles())
    }

    async fn restore_backup(&self, _profile: &str) -> Result<bool, PortError> {
        Err(unsupported_profiles())
    }
}

fn unsupported_profiles() -> PortError {
    PortError::unsupported(Capability::Profiles, "unused fake profile operation")
}

#[derive(Clone, Copy)]
enum CapabilityFixtureState {
    Available,
    TargetUnavailable,
    StartUnsupported,
}

struct FakeCapabilities {
    state: CapabilityFixtureState,
}

impl CoreCapabilityProvider for FakeCapabilities {
    fn matrix_for(
        &self,
        engine_id: &EngineId,
        core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        if matches!(self.state, CapabilityFixtureState::TargetUnavailable) {
            return Err(CoreCapabilityProviderError::EngineUnavailable {
                engine_id: engine_id.clone(),
                reason: "this target has no sing-box capability evidence".into(),
            });
        }
        let target = EngineBinaryTarget::current();
        let host = target.host_kind().unwrap_or(HostKind::Desktop);
        let engine = if matches!(self.state, CapabilityFixtureState::StartUnsupported) {
            EngineCapabilityDecision::Unsupported {
                reason: "Core start is not supported by the selected adapter".into(),
            }
        } else {
            EngineCapabilityDecision::OwnerManaged {
                owner: AdapterOperationOwner::CoreLifecycle,
            }
        };
        CoreCapabilityMatrix::new(
            CoreCapabilityScope::new_for_target(
                engine_id.clone(),
                core_version.cloned(),
                host,
                target,
                infiltrator_contract::core_capabilities::AdapterRevision::new(1),
            ),
            vec![CoreCapabilityEntry::new(
                CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
                engine,
                HostCapabilityDecision::Supported,
                AdapterCapabilityDecision::Supported,
            )],
        )
        .map_err(|error| CoreCapabilityProviderError::MatrixUnavailable {
            reason: error.to_string(),
        })
    }
}

#[derive(Default)]
struct ProviderCounts {
    created: AtomicUsize,
    starts: AtomicUsize,
}

struct FakeProvider {
    engine_id: EngineId,
    instance_id: CoreInstanceId,
    counts: Arc<ProviderCounts>,
    events: Arc<Mutex<Vec<String>>>,
}

impl FakeProvider {
    fn new(
        engine: &str,
        instance: &str,
        events: Arc<Mutex<Vec<String>>>,
    ) -> (Self, Arc<ProviderCounts>) {
        let counts = Arc::new(ProviderCounts::default());
        (
            Self {
                engine_id: engine_id_value(engine),
                instance_id: CoreInstanceId::new(instance).unwrap(),
                counts: Arc::clone(&counts),
                events,
            },
            counts,
        )
    }
}

impl EngineBackendProvider for FakeProvider {
    fn engine_id(&self) -> EngineId {
        self.engine_id.clone()
    }

    fn create_backend(&self) -> Result<EngineBackendPorts, PortError> {
        self.counts.created.fetch_add(1, Ordering::SeqCst);
        self.events
            .lock()
            .expect("event lock")
            .push(format!("provider:{}", self.engine_id.as_str()));
        let identity = CoreIdentity::new(
            self.engine_id.clone(),
            self.instance_id.clone(),
            Some(CoreVersion::new("test-version").unwrap()),
        );
        let process: Arc<dyn CoreProcess> = Arc::new(FakeProcess {
            identity,
            starts: Arc::clone(&self.counts),
        });
        let codec_scope = ProfileDocumentCodecScope {
            engine_id: self.engine_id.clone(),
            format_id: format_id(self.engine_id.as_str()),
            schema_version: ProfileSchemaVersion::new(1).unwrap(),
        };
        let codec: Arc<dyn ProfileDocumentCodecPort> = Arc::new(FakeCodec {
            scope: codec_scope.clone(),
        });
        let transaction = Arc::new(ProfileConfigurationTransactionBinding::new(
            ProfileConfigurationTransactionScope::new(codec_scope),
        ));
        EngineBackendPorts::new(
            self.engine_id.clone(),
            process,
            Arc::new(FakeReadiness),
            codec,
            transaction,
            EngineOperationPorts::unsupported("selection test has no runtime operations"),
        )
    }
}

struct FakeProcess {
    identity: CoreIdentity,
    starts: Arc<ProviderCounts>,
}

#[async_trait]
impl CoreProcess for FakeProcess {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(self.identity.clone())
    }

    async fn start(&self) -> Result<(), PortError> {
        self.starts.starts.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn stop(&self) -> Result<(), PortError> {
        Ok(())
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(CoreLifecycle::Stopped)
    }

    fn controller_endpoint(&self) -> Option<String> {
        None
    }
}

struct FakeReadiness;

#[async_trait]
impl CoreReadiness for FakeReadiness {
    async fn probe(&self) -> Result<String, PortError> {
        Ok("ready".into())
    }
}

struct FakeCodec {
    scope: ProfileDocumentCodecScope,
}

impl ProfileDocumentCodecPort for FakeCodec {
    fn scope(&self) -> ProfileDocumentCodecScope {
        self.scope.clone()
    }

    fn inspect(&self, _document: &[u8]) -> Result<DocumentValidation, PortError> {
        Ok(DocumentValidation::Valid)
    }
}

struct FakeOwner {
    before: CoreLifecycleSnapshot,
    terminal: Result<CoreLifecycleSnapshot, PortError>,
    terminations: AtomicUsize,
    events: Arc<Mutex<Vec<String>>>,
}

impl FakeOwner {
    fn new(
        before: CoreLifecycleSnapshot,
        terminal: Result<CoreLifecycleSnapshot, PortError>,
        events: Arc<Mutex<Vec<String>>>,
    ) -> Self {
        Self {
            before,
            terminal,
            terminations: AtomicUsize::new(0),
            events,
        }
    }
}

#[async_trait]
impl CoreLifecyclePort for FakeOwner {
    fn core_identity(&self) -> Option<CoreIdentity> {
        self.before.identity.clone()
    }

    fn lifecycle(&self) -> CoreLifecycle {
        self.before.lifecycle.clone()
    }

    fn generation(&self) -> u64 {
        self.before.generation
    }

    fn session_token(&self) -> Option<SessionToken> {
        self.before.session_token
    }

    fn lifecycle_snapshot(&self) -> CoreLifecycleSnapshot {
        self.before.clone()
    }

    async fn start(&self) -> Result<u64, PortError> {
        Err(unsupported_lifecycle())
    }

    async fn stop(&self) -> Result<(), PortError> {
        Err(unsupported_lifecycle())
    }

    async fn restart(&self) -> Result<u64, PortError> {
        Err(unsupported_lifecycle())
    }

    async fn wait_for_ready(&self, _generation: u64, _timeout: Duration) -> Result<(), PortError> {
        Err(unsupported_lifecycle())
    }
}

#[async_trait]
impl CoreOwnerTerminationPort for FakeOwner {
    async fn terminate_owner(&self) -> Result<CoreLifecycleSnapshot, PortError> {
        self.terminations.fetch_add(1, Ordering::SeqCst);
        self.events
            .lock()
            .expect("event lock")
            .push("old-owner-close".into());
        self.terminal.clone()
    }
}

fn unsupported_lifecycle() -> PortError {
    PortError::unsupported(Capability::CoreLifecycle, "unused fake lifecycle operation")
}

fn engine_id_value(value: &str) -> EngineId {
    EngineId::new(value).unwrap()
}

fn profile_document(engine: &str, profile: &str) -> ProfileDocument {
    ProfileDocument::new(
        ProfileDocumentIdentity::new(
            ProfileId::new(profile).unwrap(),
            engine_id_value(engine),
            ProfileDocumentSourceIdentity::new(
                ProfileDocumentSourceKind::Local,
                ProfileSourceId::new("local-test").unwrap(),
            ),
            ProfileDocumentRevision::new("doc-r1").unwrap(),
            format_id(engine),
            ProfileSchemaVersion::new(1).unwrap(),
        ),
        b"test-profile".to_vec(),
    )
}

fn format_id(engine: &str) -> ProfileFormatId {
    ProfileFormatId::new(match engine {
        "mihomo" => "mihomo-yaml",
        "sing-box" => "sing-box-json",
        other => other,
    })
    .unwrap()
}

fn reference(engine: &str, profile: &str, revision: u64) -> ActiveProfileReferenceState {
    ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
        engine_id_value(engine),
        ProfileId::new(profile).unwrap(),
        ActiveProfileReferenceRevision::new(revision),
    ))
}

fn selector(
    reference_store: Arc<MemoryReferenceStore>,
    stores: Vec<Arc<dyn ProfileStore>>,
    providers: Vec<Arc<dyn EngineBackendProvider>>,
    capabilities: Arc<dyn CoreCapabilityProvider>,
) -> ActiveProfileBackendSelector {
    let factory = Arc::new(
        crate::engine_backend_factory::EngineBackendFactoryRegistry::new(providers).unwrap(),
    );
    let reference_store: Arc<dyn ActiveProfileReferenceStore> = reference_store;
    let factory: Arc<dyn infiltrator_ports::engine_backend_factory::EngineBackendFactory> = factory;
    ActiveProfileBackendSelector::new(reference_store, stores, factory, capabilities)
}

fn supported_capabilities() -> Arc<dyn CoreCapabilityProvider> {
    Arc::new(FakeCapabilities {
        state: CapabilityFixtureState::Available,
    })
}

fn terminal_owner_snapshot(before: &CoreLifecycleSnapshot) -> CoreLifecycleSnapshot {
    CoreLifecycleSnapshot {
        identity: before.identity.clone(),
        lifecycle: CoreLifecycle::Stopped,
        generation: before.generation,
        session_token: None,
        revision: before.revision + 1,
        failure: None,
    }
}

fn running_owner_snapshot(engine: &str, instance: &str, generation: u64) -> CoreLifecycleSnapshot {
    CoreLifecycleSnapshot {
        identity: Some(CoreIdentity::new(
            engine_id_value(engine),
            CoreInstanceId::new(instance).unwrap(),
            Some(CoreVersion::new("old-version").unwrap()),
        )),
        lifecycle: CoreLifecycle::Running,
        generation,
        session_token: Some(SessionToken::new(99)),
        revision: 4,
        failure: None,
    }
}

#[tokio::test]
async fn active_reference_compare_and_set_rejects_a_stale_revision_without_write() {
    let store = MemoryReferenceStore::new(reference("mihomo", "main", 4));
    let before = store.read().await.unwrap();

    let error = store
        .compare_and_set(
            ActiveProfileReferenceRevision::new(3),
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
        )
        .await
        .unwrap_err();

    assert!(matches!(error, PortError::Rejected(_)));
    assert_eq!(store.read().await.unwrap(), before);
}

#[tokio::test]
async fn selected_candidate_holds_revision_lease_against_concurrent_switches() {
    let refs = Arc::new(MemoryReferenceStore::new(reference(
        "sing-box", "primary", 2,
    )));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let events = Arc::new(Mutex::new(Vec::new()));
    let (sing_box, counts) = FakeProvider::new("sing-box", "singbox-instance", events);
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(sing_box)],
        supported_capabilities(),
    );

    let selected = selector
        .select_backend(&PreviousCoreOwner::Unowned)
        .await
        .unwrap();
    let write_store = Arc::clone(&refs);
    let write = tokio::spawn(async move {
        write_store
            .compare_and_set(
                ActiveProfileReferenceRevision::new(2),
                ActiveProfileReferenceTarget::selected(
                    engine_id_value("mihomo"),
                    ProfileId::new("other").unwrap(),
                ),
            )
            .await
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(10), async {
            let _ = &write;
            tokio::task::yield_now().await;
            assert!(!write.is_finished());
        })
        .await
        .is_ok()
    );
    assert_eq!(counts.created.load(Ordering::SeqCst), 1);
    drop(selected);
    let updated = write.await.unwrap().unwrap();
    assert_eq!(updated.revision(), ActiveProfileReferenceRevision::new(3));
}

#[tokio::test]
async fn profile_document_engine_and_id_must_match_the_global_reference() {
    let refs = Arc::new(MemoryReferenceStore::new(reference(
        "sing-box", "primary", 1,
    )));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "mihomo", "primary"));
    let events = Arc::new(Mutex::new(Vec::new()));
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", events);
    let selector = selector(
        refs,
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );

    let result = selector.select_backend(&PreviousCoreOwner::Unowned).await;

    assert!(matches!(result, Err(PortError::Rejected(_))));
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn reference_selects_exactly_one_registered_provider_and_never_reads_namespace_current() {
    let refs = Arc::new(MemoryReferenceStore::new(reference(
        "sing-box", "primary", 7,
    )));
    let store = Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let store_as_port: Arc<dyn ProfileStore> = store.clone();
    let events = Arc::new(Mutex::new(Vec::new()));
    let (mihomo, mihomo_counts) =
        FakeProvider::new("mihomo", "mihomo-instance", Arc::clone(&events));
    let (sing_box, sing_box_counts) =
        FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let selector = selector(
        refs,
        vec![store_as_port],
        vec![Arc::new(mihomo), Arc::new(sing_box)],
        supported_capabilities(),
    );

    let selected = selector
        .select_backend(&PreviousCoreOwner::Unowned)
        .await
        .unwrap();

    assert_eq!(selected.reference.engine_id, engine_id_value("sing-box"));
    assert_eq!(
        selected.reference.profile_id,
        ProfileId::new("primary").unwrap()
    );
    assert_eq!(
        selected.reference.revision,
        ActiveProfileReferenceRevision::new(7)
    );
    assert_eq!(
        selected.document.identity.engine_id,
        engine_id_value("sing-box")
    );
    assert_eq!(selected.core_identity.engine, engine_id_value("sing-box"));
    assert_eq!(selected.core_identity.instance.as_str(), "singbox-instance");
    assert_eq!(selected.initial_generation, CoreGeneration::new(0));
    assert_eq!(
        selected.previous_owner,
        PreviousCoreDisposition::NoPriorOwner
    );
    assert_eq!(mihomo_counts.created.load(Ordering::SeqCst), 0);
    assert_eq!(sing_box_counts.created.load(Ordering::SeqCst), 1);
    assert_eq!(store.current_reads.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn previous_owner_reaches_exact_stopped_terminal_before_provider_creation() {
    let refs = Arc::new(MemoryReferenceStore::new(reference(
        "sing-box", "primary", 1,
    )));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let events = Arc::new(Mutex::new(Vec::new()));
    let (provider, _) = FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let selector = selector(
        refs,
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 12);
    let owner = Arc::new(FakeOwner::new(
        before.clone(),
        Ok(terminal_owner_snapshot(&before)),
        Arc::clone(&events),
    ));
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();
    let prior = PreviousCoreOwner::Owned(owner_as_port);

    let selected = selector.select_backend(&prior).await.unwrap();

    assert_eq!(
        selected.previous_owner,
        PreviousCoreDisposition::Stopped {
            identity: before.identity.unwrap(),
            generation: CoreGeneration::new(12),
        }
    );
    assert_eq!(
        *events.lock().unwrap(),
        vec!["old-owner-close", "provider:sing-box"]
    );
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn failed_or_nonterminal_old_owner_is_retained_and_never_binds_new_provider() {
    let refs = Arc::new(MemoryReferenceStore::new(reference(
        "sing-box", "primary", 1,
    )));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let events = Arc::new(Mutex::new(Vec::new()));
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let selector = selector(
        refs,
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 12);
    let mut nonterminal = terminal_owner_snapshot(&before);
    nonterminal.lifecycle = CoreLifecycle::Failed;
    nonterminal.session_token = Some(SessionToken::new(99));
    let owner = Arc::new(FakeOwner::new(before, Ok(nonterminal), Arc::clone(&events)));
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();
    let prior = PreviousCoreOwner::Owned(owner_as_port);
    let strong_count_before = Arc::strong_count(&owner);

    let result = selector.select_backend(&prior).await;

    assert!(matches!(result, Err(PortError::Rejected(_))));
    assert_eq!(Arc::strong_count(&owner), strong_count_before);
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn missing_provider_or_target_capability_has_no_stop_dispatch_start_or_write() {
    let refs = Arc::new(MemoryReferenceStore::new(reference(
        "sing-box", "primary", 1,
    )));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let events = Arc::new(Mutex::new(Vec::new()));
    let (mihomo, mihomo_counts) =
        FakeProvider::new("mihomo", "mihomo-instance", Arc::clone(&events));
    let missing_provider_selector = selector(
        Arc::clone(&refs),
        vec![Arc::clone(&store)],
        vec![Arc::new(mihomo)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
    let owner = Arc::new(FakeOwner::new(
        before.clone(),
        Ok(terminal_owner_snapshot(&before)),
        Arc::clone(&events),
    ));
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();
    let prior = PreviousCoreOwner::Owned(owner_as_port);

    let missing_provider = missing_provider_selector.select_backend(&prior).await;
    assert!(matches!(
        missing_provider,
        Err(PortError::Unsupported { .. })
    ));
    assert_eq!(mihomo_counts.created.load(Ordering::SeqCst), 0);
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 0);

    let (sing_box, sing_box_counts) =
        FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let missing_target_selector = selector(
        refs,
        vec![store],
        vec![Arc::new(sing_box)],
        Arc::new(FakeCapabilities {
            state: CapabilityFixtureState::TargetUnavailable,
        }),
    );
    let missing_target = missing_target_selector.select_backend(&prior).await;
    assert!(matches!(missing_target, Err(PortError::Unsupported { .. })));
    assert_eq!(sing_box_counts.created.load(Ordering::SeqCst), 0);
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 0);
    assert_eq!(
        *events.lock().unwrap(),
        Vec::<String>::new(),
        "preflight failures must not terminate, dispatch, start, or write"
    );
}

#[tokio::test]
async fn unsupported_start_capability_is_rejected_before_owner_termination() {
    let refs = Arc::new(MemoryReferenceStore::new(reference(
        "sing-box", "primary", 1,
    )));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let events = Arc::new(Mutex::new(Vec::new()));
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let selector = selector(
        refs,
        vec![store],
        vec![Arc::new(provider)],
        Arc::new(FakeCapabilities {
            state: CapabilityFixtureState::StartUnsupported,
        }),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
    let owner = Arc::new(FakeOwner::new(
        before.clone(),
        Ok(terminal_owner_snapshot(&before)),
        Arc::clone(&events),
    ));
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();
    let prior = PreviousCoreOwner::Owned(owner_as_port);

    let result = selector.select_backend(&prior).await;

    assert!(matches!(result, Err(PortError::Unsupported { .. })));
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 0);
    assert!(events.lock().unwrap().is_empty());
}

#[test]
fn registry_registration_probe_does_not_dispatch_a_provider() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (provider, counts) = FakeProvider::new("mihomo", "mihomo-instance", events);
    let provider: Arc<dyn EngineBackendProvider> = Arc::new(provider);
    let registry =
        crate::engine_backend_factory::EngineBackendFactoryRegistry::new([provider]).unwrap();
    let factory: &dyn infiltrator_ports::engine_backend_factory::EngineBackendFactory = &registry;

    assert_eq!(
        factory.provider_availability(&engine_id_value("mihomo")),
        EngineBackendProviderAvailability::Registered
    );
    assert!(matches!(
        factory.provider_availability(&engine_id_value("sing-box")),
        EngineBackendProviderAvailability::Unsupported { .. }
    ));
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
}

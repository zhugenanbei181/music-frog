use super::*;
use crate::active_profile_backend::SelectedActiveProfileBackend;
use crate::engine_backend_factory::EngineBackendFactoryRegistry;
use infiltrator_contract::core_capabilities::AdapterRevision;
use infiltrator_ports::engine_backend_factory::EngineBackendFactory;
use lurker_core_contract::core_process::CoreProcessStartContext;
use tokio::time::timeout;

pub(crate) struct FakeProfileStore {
    engine_id: EngineId,
    document: ProfileDocument,
    pub(crate) current_reads: AtomicUsize,
    pub(crate) workspace_reads: AtomicUsize,
    deleted: Option<Arc<AtomicBool>>,
}

impl FakeProfileStore {
    pub(crate) fn new(engine_id: &str, document_engine: &str, document_profile: &str) -> Self {
        Self {
            engine_id: engine_id_value(engine_id),
            document: profile_document(document_engine, document_profile),
            current_reads: AtomicUsize::new(0),
            workspace_reads: AtomicUsize::new(0),
            deleted: None,
        }
    }

    pub(crate) fn with_deletion_flag(mut self, deleted: Arc<AtomicBool>) -> Self {
        self.deleted = Some(deleted);
        self
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

    async fn load(&self, _profile: &str) -> Result<String, PortError> {
        Ok(String::from_utf8_lossy(&self.document.raw_bytes).into_owned())
    }

    async fn save_inactive_profile(&self, _profile: &str, _content: &str) -> Result<(), PortError> {
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
        self.workspace_reads.fetch_add(1, Ordering::SeqCst);
        if matches!(&self.deleted, Some(deleted) if deleted.load(Ordering::SeqCst)) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake target profile was deleted before the switch reservation",
                true,
            )));
        }
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

    async fn restore_inactive_profile_backup(&self, _profile: &str) -> Result<bool, PortError> {
        Err(unsupported_profiles())
    }
}

pub(crate) fn unsupported_profiles() -> PortError {
    PortError::unsupported(Capability::Profiles, "unused fake profile operation")
}

#[derive(Clone, Copy)]
pub(crate) enum CapabilityFixtureState {
    Available,
    TargetUnavailable,
    StartUnsupported,
}

pub(crate) struct FakeCapabilities {
    pub(crate) state: CapabilityFixtureState,
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
                AdapterRevision::new(1),
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
pub(crate) struct ProviderCounts {
    pub(crate) created: AtomicUsize,
    pub(crate) starts: AtomicUsize,
}

pub(crate) struct FakeProvider {
    engine_id: EngineId,
    instance_id: CoreInstanceId,
    counts: Arc<ProviderCounts>,
    events: Arc<Mutex<Vec<String>>>,
    identity_engine_override: Option<EngineId>,
    fail_creation: bool,
}

impl FakeProvider {
    pub(crate) fn new(
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
                identity_engine_override: None,
                fail_creation: false,
            },
            counts,
        )
    }

    pub(crate) fn with_identity_engine(mut self, engine: &str) -> Self {
        self.identity_engine_override = Some(engine_id_value(engine));
        self
    }

    pub(crate) fn failing_creation(mut self) -> Self {
        self.fail_creation = true;
        self
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
        if self.fail_creation {
            return Err(PortError::Failed("fake provider dispatch failed".into()));
        }
        let identity = CoreIdentity::new(
            self.identity_engine_override
                .clone()
                .unwrap_or_else(|| self.engine_id.clone()),
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

pub(crate) struct FakeProcess {
    pub(crate) identity: CoreIdentity,
    pub(crate) starts: Arc<ProviderCounts>,
}

#[async_trait]
impl CoreProcess for FakeProcess {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(self.identity.clone())
    }

    async fn start(&self, _context: CoreProcessStartContext) -> Result<(), PortError> {
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

pub(crate) struct FakeReadiness;

#[async_trait]
impl CoreReadiness for FakeReadiness {
    async fn probe(&self) -> Result<String, PortError> {
        Ok("ready".into())
    }
}

pub(crate) struct FakeCodec {
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

pub(crate) struct FakeOwner {
    before: CoreLifecycleSnapshot,
    terminal: Result<CoreLifecycleSnapshot, PortError>,
    pub(crate) terminations: AtomicUsize,
    events: Arc<Mutex<Vec<String>>>,
    owner_lease: Option<Box<dyn ActiveProfileOwnerLease>>,
    references: Option<Arc<MemoryReferenceStore>>,
}

impl FakeOwner {
    pub(crate) fn with_binding(
        before: CoreLifecycleSnapshot,
        terminal: Result<CoreLifecycleSnapshot, PortError>,
        events: Arc<Mutex<Vec<String>>>,
        owner_lease: Box<dyn ActiveProfileOwnerLease>,
        references: Arc<MemoryReferenceStore>,
    ) -> Self {
        Self {
            before,
            terminal,
            terminations: AtomicUsize::new(0),
            events,
            owner_lease: Some(owner_lease),
            references: Some(references),
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
    fn active_profile_reference_write_authorization(
        &self,
    ) -> Result<ActiveProfileReferenceWriteAuthorization, PortError> {
        self.owner_lease
            .as_ref()
            .map(
                |lease| ActiveProfileReferenceWriteAuthorization::BoundOwner {
                    identity: lease.identity().clone(),
                    generation: lease.generation(),
                },
            )
            .ok_or_else(unsupported_lifecycle)
    }

    async fn terminate_owner(&self) -> Result<CoreLifecycleSnapshot, PortError> {
        self.terminations.fetch_add(1, Ordering::SeqCst);
        self.events
            .lock()
            .expect("event lock")
            .push("old-owner-close".into());
        let terminal = self.terminal.clone()?;
        if let Some(references) = &self.references {
            let active = timeout(Duration::from_secs(2), references.read())
                .await
                .map_err(|_| PortError::Failed("fake close blocked on the root lock".into()))??;
            self.events
                .lock()
                .expect("event lock")
                .push("close-root-read".into());
            let revision = active.revision();
            let current_engine = match &active {
                ActiveProfileReferenceState::Selected(reference) => reference.engine_id.clone(),
                ActiveProfileReferenceState::Unselected { .. } => {
                    return Err(PortError::Failed(
                        "fake close observed an unselected active reference".into(),
                    ));
                }
            };
            let ordinary_cas = references
                .compare_and_set(
                    revision,
                    ActiveProfileReferenceTarget::selected(
                        current_engine,
                        ProfileId::new("ordinary-cas-during-switch").unwrap(),
                    ),
                    ActiveProfileReferenceWriteAuthorization::Unowned,
                )
                .await;
            if !matches!(ordinary_cas, Err(PortError::Rejected(_))) {
                return Err(PortError::Failed(
                    "ordinary active-reference CAS was admitted during switch".into(),
                ));
            }
            self.events
                .lock()
                .expect("event lock")
                .push("ordinary-cas-blocked".into());
            if !matches!(
                references.acquire_provider_selection_lease().await,
                Err(PortError::Rejected(_))
            ) {
                return Err(PortError::Failed(
                    "provider selection was admitted during switch".into(),
                ));
            }
            self.events
                .lock()
                .expect("event lock")
                .push("ordinary-provider-blocked".into());
        }
        if let Some(owner_lease) = &self.owner_lease {
            owner_lease.release_after_terminal(&terminal).await?;
        }
        Ok(terminal)
    }
}

pub(crate) fn unsupported_lifecycle() -> PortError {
    PortError::unsupported(Capability::CoreLifecycle, "unused fake lifecycle operation")
}

pub(crate) struct SelectorTestRuntime;

impl ApplicationRuntime for SelectorTestRuntime {
    fn block_on(&self, future: ApplicationFuture) {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("selector test runtime")
            .block_on(future);
    }

    fn sleep(&self, duration: Duration) -> ApplicationSleep<'_> {
        Box::pin(sleep(duration))
    }
}

pub(crate) fn engine_id_value(value: &str) -> EngineId {
    EngineId::new(value).unwrap()
}

pub(crate) fn profile_document(engine: &str, profile: &str) -> ProfileDocument {
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

pub(crate) fn format_id(engine: &str) -> ProfileFormatId {
    ProfileFormatId::new(match engine {
        "mihomo" => "mihomo-yaml",
        "sing-box" => "sing-box-json",
        other => other,
    })
    .unwrap()
}

pub(crate) fn reference(engine: &str, profile: &str, revision: u64) -> ActiveProfileReferenceState {
    ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
        engine_id_value(engine),
        ProfileId::new(profile).unwrap(),
        ActiveProfileReferenceRevision::new(revision),
    ))
}

pub(crate) fn selector(
    reference_store: Arc<MemoryReferenceStore>,
    stores: Vec<Arc<dyn ProfileStore>>,
    providers: Vec<Arc<dyn EngineBackendProvider>>,
    capabilities: Arc<dyn CoreCapabilityProvider>,
) -> ActiveProfileBackendSelector {
    let factory = Arc::new(EngineBackendFactoryRegistry::new(providers).unwrap());
    let reference_store: Arc<dyn ActiveProfileReferenceStore> = reference_store;
    let factory: Arc<dyn EngineBackendFactory> = factory;
    ActiveProfileBackendSelector::new(reference_store, stores, factory, capabilities)
}

pub(crate) async fn load_prepared_profile_document(store: &dyn ProfileStore) -> ProfileDocument {
    store
        .load_workspace(&ProfileId::new("primary").unwrap())
        .await
        .unwrap()
        .document
}

pub(crate) fn switch_failure(
    result: Result<SelectedActiveProfileBackend, ActiveProfileBackendSwitchError>,
    expected_phase: ActiveProfileBackendSwitchFailurePhase,
) -> PortError {
    match result {
        Err(error) => {
            assert_eq!(error.phase(), expected_phase);
            error.into_failure()
        }
        Ok(_) => panic!("owner switch unexpectedly produced a candidate"),
    }
}

pub(crate) fn supported_capabilities() -> Arc<dyn CoreCapabilityProvider> {
    Arc::new(FakeCapabilities {
        state: CapabilityFixtureState::Available,
    })
}

pub(crate) fn terminal_owner_snapshot(before: &CoreLifecycleSnapshot) -> CoreLifecycleSnapshot {
    CoreLifecycleSnapshot {
        identity: before.identity.clone(),
        lifecycle: CoreLifecycle::Stopped,
        generation: before.generation,
        session_token: None,
        revision: before.revision + 1,
        failure: None,
    }
}

pub(crate) fn running_owner_snapshot(
    engine: &str,
    instance: &str,
    generation: u64,
) -> CoreLifecycleSnapshot {
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

pub(crate) async fn bound_fake_owner(
    references: Arc<MemoryReferenceStore>,
    before: CoreLifecycleSnapshot,
    terminal: Result<CoreLifecycleSnapshot, PortError>,
    events: Arc<Mutex<Vec<String>>>,
) -> Arc<FakeOwner> {
    let identity = before.identity.clone().expect("test owner identity");
    let lease = references
        .acquire_selection_lease()
        .await
        .unwrap()
        .into_owner_binding(identity, CoreGeneration::new(0))
        .await
        .unwrap();
    Arc::new(FakeOwner::with_binding(
        before, terminal, events, lease, references,
    ))
}

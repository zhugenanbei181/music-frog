use super::*;

pub(crate) trait ConformanceLifecycleControl: CoreLifecyclePort {
    fn update_observed_lifecycle(&self, lifecycle: CoreLifecycle);
}

/// Lifecycle view used by the production transaction adapters during the
/// suite. The runner updates it only after observing the matching process
/// transition, so live Unsupported checks see the actual process phase.
pub(crate) struct ConformanceLifecycleSession {
    identity: CoreIdentity,
    lifecycle: std::sync::RwLock<CoreLifecycle>,
}

impl ConformanceLifecycleSession {
    pub(crate) fn for_process(
        process: &dyn CoreProcess,
        expected_version: CoreVersion,
    ) -> Result<Self, String> {
        let mut identity = process
            .core_identity()
            .ok_or_else(|| "production provider process has no Core identity".to_owned())?;
        if let Some(process_version) = &identity.version
            && process_version != &expected_version
        {
            return Err(format!(
                "provider process version {} differs from transaction owner version {}",
                process_version.raw(),
                expected_version.raw()
            ));
        }
        identity.version = Some(expected_version);
        Ok(Self {
            identity,
            lifecycle: std::sync::RwLock::new(CoreLifecycle::Stopped),
        })
    }
}

#[async_trait]
impl CoreLifecyclePort for ConformanceLifecycleSession {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(self.identity.clone())
    }

    fn lifecycle(&self) -> CoreLifecycle {
        self.lifecycle
            .read()
            .expect("conformance lifecycle lock")
            .clone()
    }

    fn generation(&self) -> u64 {
        1
    }

    fn session_token(&self) -> Option<SessionToken> {
        None
    }

    async fn start(&self) -> Result<u64, PortError> {
        Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "the conformance process adapter owns lifecycle operations",
        ))
    }

    async fn stop(&self) -> Result<(), PortError> {
        Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "the conformance process adapter owns lifecycle operations",
        ))
    }

    async fn restart(&self) -> Result<u64, PortError> {
        Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "the conformance process adapter owns lifecycle operations",
        ))
    }

    async fn wait_for_ready(&self, _generation: u64, _timeout: Duration) -> Result<(), PortError> {
        Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "the conformance process adapter owns readiness",
        ))
    }
}

impl ConformanceLifecycleControl for ConformanceLifecycleSession {
    fn update_observed_lifecycle(&self, lifecycle: CoreLifecycle) {
        *self.lifecycle.write().expect("conformance lifecycle lock") = lifecycle;
    }
}

/// Isolated storage fixture for exercising real transaction adapters whose
/// production `ProfileStore` implementation is owned by a different host.
pub(crate) struct ConformanceProfileStore {
    engine: EngineId,
    active: tokio::sync::Mutex<ProfileWorkspace>,
    next_revision: AtomicU64,
    writes: AtomicU64,
}

impl ConformanceProfileStore {
    pub(crate) fn new(
        scope: &infiltrator_ports::profile_document_codec::ProfileDocumentCodecScope,
        initial_bytes: &[u8],
    ) -> Self {
        let identity = ProfileDocumentIdentity::new(
            ProfileId::new("conformance").expect("static profile ID is valid"),
            scope.engine_id.clone(),
            ProfileDocumentSourceIdentity::new(
                ProfileDocumentSourceKind::Local,
                ProfileSourceId::new("real-conformance").expect("static source ID is valid"),
            ),
            ProfileDocumentRevision::new("real-conformance-1")
                .expect("static document revision is valid"),
            scope.format_id.clone(),
            scope.schema_version,
        );
        Self {
            engine: scope.engine_id.clone(),
            active: tokio::sync::Mutex::new(ProfileWorkspace {
                document: ProfileDocument::new(identity, initial_bytes.to_vec()),
                sidecar_revision: ProfileSidecarRevision::Absent,
                write_protection: ProfileWriteProtection::Editable,
                sidecar_bytes: None,
            }),
            next_revision: AtomicU64::new(2),
            writes: AtomicU64::new(0),
        }
    }

    pub(crate) fn writes(&self) -> u64 {
        self.writes.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ProfileStore for ConformanceProfileStore {
    fn engine_id(&self) -> EngineId {
        self.engine.clone()
    }

    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/conformance/profile-store")
    }

    async fn list_profiles(
        &self,
    ) -> Result<Vec<infiltrator_domain::profiles::ProfileInfo>, PortError> {
        Err(unused_profile_store_operation())
    }

    async fn get_current(&self) -> Result<String, PortError> {
        Err(unused_profile_store_operation())
    }

    async fn set_current(&self, _profile: &str) -> Result<(), PortError> {
        Err(unused_profile_store_operation())
    }

    async fn load(&self, _profile: &str) -> Result<String, PortError> {
        Err(unused_profile_store_operation())
    }

    async fn save(&self, _profile: &str, _content: &str) -> Result<(), PortError> {
        Err(unused_profile_store_operation())
    }

    async fn delete_profile(&self, _profile: &str) -> Result<(), PortError> {
        Err(unused_profile_store_operation())
    }

    async fn get_profile_metadata(
        &self,
        _profile: &str,
    ) -> Result<infiltrator_domain::profiles::ProfileMetadata, PortError> {
        Err(unused_profile_store_operation())
    }

    async fn update_profile_metadata(
        &self,
        _profile: &str,
        _metadata: &infiltrator_domain::profiles::ProfileMetadata,
    ) -> Result<(), PortError> {
        Err(unused_profile_store_operation())
    }

    async fn delete_subscription_credential(&self, _profile: &str) -> Result<(), PortError> {
        Err(unused_profile_store_operation())
    }

    async fn load_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        Ok(self.active.lock().await.clone())
    }

    async fn compare_and_save_active_workspace(
        &self,
        expected: &infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        let mut active = self.active.lock().await;
        if active.identity() != *expected {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "conformance profile workspace revision is stale",
                false,
            )));
        }
        let revision = self.next_revision.fetch_add(1, Ordering::SeqCst);
        let mut document_identity = active.document.identity.clone();
        document_identity.revision =
            ProfileDocumentRevision::new(format!("real-conformance-{revision}"))
                .expect("generated document revision is valid");
        let sidecar_revision = match &update.sidecar_bytes {
            Some(_) => ProfileSidecarRevision::Present(
                ProfileDocumentRevision::new(format!("real-conformance-sidecar-{revision}"))
                    .expect("generated sidecar revision is valid"),
            ),
            None => ProfileSidecarRevision::Absent,
        };
        *active = ProfileWorkspace {
            document: ProfileDocument::new(document_identity, update.document_bytes.clone()),
            sidecar_revision,
            write_protection: active.write_protection,
            sidecar_bytes: update.sidecar_bytes.clone(),
        };
        self.writes.fetch_add(1, Ordering::SeqCst);
        Ok(active.clone())
    }

    async fn restore_workspace(
        &self,
        expected: &infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity,
        previous: &ProfileWorkspace,
    ) -> Result<ProfileWorkspace, PortError> {
        let mut active = self.active.lock().await;
        if active.identity() != *expected {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "conformance profile workspace changed before rollback",
                false,
            )));
        }
        *active = previous.clone();
        self.writes.fetch_add(1, Ordering::SeqCst);
        Ok(active.clone())
    }

    async fn clear_backup(&self, _profile: &str) -> Result<(), PortError> {
        Err(unused_profile_store_operation())
    }

    async fn restore_backup(&self, _profile: &str) -> Result<bool, PortError> {
        Err(unused_profile_store_operation())
    }
}

fn unused_profile_store_operation() -> PortError {
    PortError::unsupported(
        infiltrator_contract::capability::Capability::Profiles,
        "conformance store does not implement this profile operation",
    )
}

pub(super) async fn verify_native_profile_behavior(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
) -> Result<(), String> {
    let transaction_scope = ports.configuration_transaction.scope();
    if transaction_scope.document != ports.codec.scope() {
        return Err(format!(
            "{} typed transaction scope differs from its codec scope",
            case.engine.as_str()
        ));
    }

    match ports.codec.inspect(&case.native_invalid_profile) {
        Ok(DocumentValidation::Valid) => {}
        Ok(other) => {
            return Err(format!(
                "native-invalid {} fixture must pass neutral syntax inspection: {other:?}",
                case.engine.as_str()
            ));
        }
        Err(error) => {
            return Err(format!(
                "native-invalid {} fixture failed neutral syntax inspection: {error}",
                case.engine.as_str()
            ));
        }
    }

    let initial_workspace = ports
        .profile_store
        .load_active_workspace()
        .await
        .map_err(|error| {
            format!(
                "{} initial profile workspace read failed: {error}",
                case.engine.as_str()
            )
        })?;
    if initial_workspace.document.raw_bytes != case.valid_profile {
        return Err(format!(
            "{} active workspace is not seeded with the native-valid fixture",
            case.engine.as_str()
        ));
    }

    let outcome = ports
        .configuration_transaction
        .transact(ProfileConfigurationTransactionRequest {
            expected: initial_workspace.identity(),
            update: ProfileWorkspaceUpdate {
                purpose: ProfileWorkspacePurpose::Derived,
                document_bytes: case.valid_profile.clone(),
                sidecar_bytes: initial_workspace.sidecar_bytes.clone(),
            },
            strategy: infiltrator_domain::apply::ApplyStrategy::PreferReload,
        })
        .await
        .map_err(|error| {
            format!(
                "{} production typed configuration transaction failed: {error}",
                case.engine.as_str()
            )
        })?;
    let committed = match outcome {
        ProfileConfigurationTransactionOutcome::Committed {
            workspace,
            effect: ProfileConfigurationCommitEffect::StoredWhileStopped,
        } if workspace.document.raw_bytes == case.valid_profile
            && workspace.sidecar_bytes == initial_workspace.sidecar_bytes =>
        {
            workspace
        }
        ProfileConfigurationTransactionOutcome::Committed { effect, .. } => {
            return Err(format!(
                "{} stopped transaction returned the wrong effect or readback: {effect:?}",
                case.engine.as_str(),
            ));
        }
        ProfileConfigurationTransactionOutcome::RolledBack { cause, .. } => {
            return Err(format!(
                "{} stopped transaction rolled back a valid config: {cause:?}",
                case.engine.as_str()
            ));
        }
        ProfileConfigurationTransactionOutcome::RollbackFailed { cause, rollback } => {
            return Err(format!(
                "{} stopped transaction failed and rollback was incomplete: cause={cause:?}, rollback={rollback:?}",
                case.engine.as_str(),
            ));
        }
    };
    let stored = ports
        .profile_store
        .load_active_workspace()
        .await
        .map_err(|error| {
            format!(
                "{} profile workspace readback after commit failed: {error}",
                case.engine.as_str()
            )
        })?;
    if stored != committed {
        return Err(format!(
            "{} ProfileStore readback differs from typed transaction receipt",
            case.engine.as_str()
        ));
    }

    let stale_before = stored.clone();
    let mut stale_identity = stored.identity();
    stale_identity.document.revision = ProfileDocumentRevision::new("stale-real-conformance")
        .expect("static stale profile revision is valid");
    let stale_error = ports
        .configuration_transaction
        .transact(ProfileConfigurationTransactionRequest {
            expected: stale_identity,
            update: ProfileWorkspaceUpdate {
                purpose: ProfileWorkspacePurpose::Derived,
                document_bytes: case.valid_profile.clone(),
                sidecar_bytes: stored.sidecar_bytes.clone(),
            },
            strategy: infiltrator_domain::apply::ApplyStrategy::PreferReload,
        })
        .await
        .expect_err("stale profile revision must be rejected");
    if stale_error.error_code() != ErrorCode::NotReady
        || ports
            .profile_store
            .load_active_workspace()
            .await
            .map_err(|error| error.to_string())?
            != stale_before
    {
        return Err(format!(
            "{} stale configuration transaction had the wrong error or changed the workspace",
            case.engine.as_str()
        ));
    }

    let mut foreign_identity = stale_before.identity();
    foreign_identity.document.engine_id =
        EngineId::new("conformance-foreign-engine").expect("static foreign engine ID is valid");
    let foreign_error = ports
        .configuration_transaction
        .transact(ProfileConfigurationTransactionRequest {
            expected: foreign_identity,
            update: ProfileWorkspaceUpdate {
                purpose: ProfileWorkspacePurpose::Derived,
                document_bytes: case.valid_profile.clone(),
                sidecar_bytes: stale_before.sidecar_bytes.clone(),
            },
            strategy: infiltrator_domain::apply::ApplyStrategy::PreferReload,
        })
        .await
        .expect_err("foreign profile engine must be rejected");
    if foreign_error.error_code() != ErrorCode::Unsupported
        || ports
            .profile_store
            .load_active_workspace()
            .await
            .map_err(|error| error.to_string())?
            != stale_before
    {
        return Err(format!(
            "{} foreign configuration transaction had the wrong error or changed the workspace",
            case.engine.as_str()
        ));
    }

    let invalid_error = ports
        .configuration_transaction
        .transact(ProfileConfigurationTransactionRequest {
            expected: stale_before.identity(),
            update: ProfileWorkspaceUpdate {
                purpose: ProfileWorkspacePurpose::Derived,
                document_bytes: case.native_invalid_profile.clone(),
                sidecar_bytes: stale_before.sidecar_bytes.clone(),
            },
            strategy: infiltrator_domain::apply::ApplyStrategy::PreferReload,
        })
        .await
        .expect_err("native-invalid profile must be rejected by the transaction");
    if invalid_error.error_code() != ErrorCode::Configuration
        || ports
            .profile_store
            .load_active_workspace()
            .await
            .map_err(|error| error.to_string())?
            != stale_before
    {
        return Err(format!(
            "{} native-invalid configuration transaction had the wrong error or changed the workspace",
            case.engine.as_str()
        ));
    }
    Ok(())
}

//! Engine-scoped sing-box profile CAS and native config-file transaction.

use crate::config::{SingBoxConfigFileState, SingBoxProfileConfigurationAdapter};
use crate::process::SingBoxCoreOperationGate;
use crate::profile_document_codec::SingBoxProfileDocumentCodec;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::CoreIdentity;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document::DocumentValidation;
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationCommitEffect, ProfileConfigurationTransactionOutcome,
    ProfileConfigurationTransactionPort, ProfileConfigurationTransactionRequest,
    ProfileConfigurationTransactionScope,
};
use infiltrator_ports::profile_document_codec::ProfileDocumentCodecPort;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LifecycleFenceState {
    Retained,
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommittedWorkspaceReadback {
    Exact,
    SameIdentityWithDifferentContent,
    DifferentIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailedCasReadback {
    Unchanged,
    PartialPublication,
    ConcurrentChange,
}

#[derive(Debug)]
enum NativeConfigReadback {
    Exact,
    Different,
    Unavailable(PortError),
}

#[derive(Debug)]
enum FinalWorkspaceReadback {
    Exact(ProfileWorkspace),
    SameIdentityWithDifferentSnapshot,
    ConcurrentChange,
    Unavailable(PortError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateBytesState {
    Previous,
    Submitted,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkspaceScopeState {
    Same,
    Changed,
}

#[derive(Debug)]
enum WorkspaceRestoreState {
    Exact,
    Different,
    Failed(PortError),
}

#[derive(Debug)]
enum WorkspaceRollbackReadback {
    Exact(ProfileWorkspace),
    Different,
    Failed(PortError),
}

#[derive(Debug)]
enum FileAndWorkspaceRestore {
    Exact,
    Different {
        file: Result<(), PortError>,
        workspace: Result<ProfileWorkspace, PortError>,
    },
}

#[derive(Debug)]
enum FileAndWorkspaceReadback {
    Exact(ProfileWorkspace),
    Different {
        workspace: Result<ProfileWorkspace, PortError>,
        file: Result<SingBoxConfigFileState, PortError>,
    },
}

/// Owns the profile workspace CAS and sing-box native config-file publication.
/// It commits only while the Core is Stopped or Failed. sing-box Clash API has
/// no verified live configuration apply operation, so active Core states stay
/// explicitly Unsupported with zero persistence or controller dispatch.
pub struct SingBoxProfileConfigurationTransactionAdapter {
    profile_store: Arc<dyn ProfileStore>,
    session: Arc<dyn CoreLifecyclePort>,
    configuration: Arc<SingBoxProfileConfigurationAdapter>,
    operation_gate: Arc<SingBoxCoreOperationGate>,
    process_identity: CoreIdentity,
    scope: ProfileConfigurationTransactionScope,
    transaction_lock: Mutex<()>,
}

impl SingBoxProfileConfigurationTransactionAdapter {
    pub fn new(
        profile_store: Arc<dyn ProfileStore>,
        session: Arc<dyn CoreLifecyclePort>,
        configuration: Arc<SingBoxProfileConfigurationAdapter>,
        operation_gate: Arc<SingBoxCoreOperationGate>,
        process_identity: CoreIdentity,
    ) -> Result<Self, PortError> {
        let codec = SingBoxProfileDocumentCodec;
        let scope = ProfileConfigurationTransactionScope::new(codec.scope());
        let expected_engine = crate::engine_id();
        if profile_store.engine_id() != expected_engine {
            return Err(PortError::Failed(
                "sing-box configuration transaction requires a sing-box-namespaced profile store"
                    .to_owned(),
            ));
        }
        let session_identity = session.core_identity().ok_or_else(|| {
            rejected(
                ErrorCode::NotReady,
                "sing-box configuration transaction requires an identified Core owner",
            )
        })?;
        if process_identity.engine != expected_engine
            || session_identity.engine != process_identity.engine
            || session_identity.instance != process_identity.instance
        {
            return Err(PortError::Failed(
                "sing-box configuration transaction session does not own its process instance"
                    .to_owned(),
            ));
        }
        Ok(Self {
            profile_store,
            session,
            configuration,
            operation_gate,
            process_identity,
            scope,
            transaction_lock: Mutex::new(()),
        })
    }

    async fn transact_inner(
        &self,
        request: ProfileConfigurationTransactionRequest,
    ) -> Result<ProfileConfigurationTransactionOutcome, PortError> {
        if !self.scope.supports(&request.expected) {
            return Err(PortError::unsupported(
                Capability::Profiles,
                "sing-box configuration transaction request has a foreign engine, format, or schema",
            ));
        }
        self.verify_active_process_instance()?;
        let previous = self
            .profile_store
            .load_active_workspace()
            .await
            .map_err(PortError::from)?;
        if previous.identity() != request.expected {
            return Err(rejected(
                ErrorCode::NotReady,
                "sing-box profile workspace revision changed before the transaction began",
            ));
        }

        let initial_lifecycle = self.session.lifecycle();
        let effect = match initial_lifecycle {
            CoreLifecycle::Stopped => ProfileConfigurationCommitEffect::StoredWhileStopped,
            CoreLifecycle::Failed => ProfileConfigurationCommitEffect::StoredWhileFailed,
            CoreLifecycle::Ready | CoreLifecycle::Running => {
                return Err(PortError::unsupported(
                    Capability::Profiles,
                    "sing-box live configuration apply is unavailable; stop the Core before saving",
                ));
            }
            CoreLifecycle::Starting | CoreLifecycle::Stopping => {
                return Err(rejected(
                    ErrorCode::NotReady,
                    "sing-box lifecycle transition prevents configuration save",
                ));
            }
        };

        match SingBoxProfileDocumentCodec.inspect(&request.update.document_bytes)? {
            DocumentValidation::Valid => {}
            DocumentValidation::Invalid { diagnostics } => {
                return Err(invalid_candidate(diagnostics));
            }
        }
        match self
            .configuration
            .validate_native(&request.update.document_bytes)
            .await?
        {
            DocumentValidation::Valid => {}
            DocumentValidation::Invalid { diagnostics } => {
                return Err(invalid_candidate(diagnostics));
            }
        }

        match lifecycle_fence(&self.session, &initial_lifecycle) {
            LifecycleFenceState::Retained => {}
            LifecycleFenceState::Changed => {
                return Err(rejected(
                    ErrorCode::NotReady,
                    "sing-box lifecycle changed during native candidate validation",
                ));
            }
        }
        self.verify_active_process_instance()?;
        if self
            .profile_store
            .load_active_workspace()
            .await
            .map_err(PortError::from)?
            != previous
        {
            return Err(rejected(
                ErrorCode::NotReady,
                "sing-box profile workspace changed during native candidate validation",
            ));
        }

        let previous_config = self.configuration.file_state().await?;
        let committed = match self
            .profile_store
            .compare_and_save_active_workspace(&request.expected, &request.update)
            .await
        {
            Ok(committed) => committed,
            Err(cause) => {
                return self
                    .recover_ambiguous_store_write(&previous, &request.update, cause)
                    .await;
            }
        };

        let stored_readback = match self.profile_store.load_active_workspace().await {
            Ok(workspace) => workspace,
            Err(readback) => {
                return Ok(self
                    .rollback_workspace_only(
                        committed.identity(),
                        &previous,
                        "sing-box profile workspace readback failed after CAS",
                        readback.to_string(),
                    )
                    .await);
            }
        };
        match classify_committed_readback(
            &request.expected,
            &previous,
            &request.update,
            &committed,
            &stored_readback,
        ) {
            CommittedWorkspaceReadback::Exact => {}
            CommittedWorkspaceReadback::SameIdentityWithDifferentContent => {
                return Ok(self
                    .rollback_workspace_only(
                        committed.identity(),
                        &previous,
                        "sing-box profile workspace did not read back the submitted bytes",
                        "the committed identity or exact document/sidecar bytes differed",
                    )
                    .await);
            }
            CommittedWorkspaceReadback::DifferentIdentity => {
                return Ok(terminal_outcome(
                    "sing-box profile workspace changed before exact CAS readback",
                    "preserved the newer workspace instead of overwriting it during recovery",
                ));
            }
        }

        match lifecycle_fence(&self.session, &initial_lifecycle) {
            LifecycleFenceState::Retained => {}
            LifecycleFenceState::Changed => {
                return Ok(self
                    .rollback_workspace_only(
                        committed.identity(),
                        &previous,
                        "sing-box Core lifecycle changed before native config publication",
                        "configuration file was not changed",
                    )
                    .await);
            }
        }

        if let Err(apply_error) = self
            .configuration
            .apply_and_readback(&request.update.document_bytes)
            .await
        {
            return Ok(self
                .rollback_after_file_failure(
                    committed.identity(),
                    &previous,
                    &previous_config,
                    "sing-box native config file publication failed",
                    apply_error.to_string(),
                )
                .await);
        }
        match classify_native_readback(
            self.configuration.readback().await,
            &request.update.document_bytes,
        ) {
            NativeConfigReadback::Exact => {}
            NativeConfigReadback::Different => {
                return Ok(self
                    .rollback_after_file_failure(
                        committed.identity(),
                        &previous,
                        &previous_config,
                        "sing-box native config file readback differed",
                        "the active JSON file did not match the submitted bytes",
                    )
                    .await);
            }
            NativeConfigReadback::Unavailable(readback) => {
                return Ok(self
                    .rollback_after_file_failure(
                        committed.identity(),
                        &previous,
                        &previous_config,
                        "sing-box native config file readback failed",
                        readback.to_string(),
                    )
                    .await);
            }
        }

        match lifecycle_fence(&self.session, &initial_lifecycle) {
            LifecycleFenceState::Retained => {}
            LifecycleFenceState::Changed => {
                return Ok(self
                    .rollback_after_file_failure(
                        committed.identity(),
                        &previous,
                        &previous_config,
                        "sing-box Core lifecycle changed during native config publication",
                        "configuration transaction did not retain its inactive lifecycle",
                    )
                    .await);
            }
        }
        match classify_final_workspace_readback(
            self.profile_store.load_active_workspace().await,
            &stored_readback,
            &committed,
        ) {
            FinalWorkspaceReadback::Exact(final_readback) => {
                Ok(ProfileConfigurationTransactionOutcome::Committed {
                    workspace: final_readback,
                    effect,
                })
            }
            FinalWorkspaceReadback::SameIdentityWithDifferentSnapshot => Ok(self
                .rollback_after_file_failure(
                    committed.identity(),
                    &previous,
                    &previous_config,
                    "sing-box workspace changed after native config publication",
                    "exact workspace readback differed from its committed snapshot",
                )
                .await),
            FinalWorkspaceReadback::ConcurrentChange => {
                let file_restore = self
                    .configuration
                    .restore_file_state(&previous_config)
                    .await;
                Ok(terminal_outcome(
                    "sing-box workspace changed after native config publication",
                    format!(
                        "preserved the newer workspace; config-file recovery: {}",
                        result_description(file_restore)
                    ),
                ))
            }
            FinalWorkspaceReadback::Unavailable(readback) => Ok(self
                .rollback_after_file_failure(
                    committed.identity(),
                    &previous,
                    &previous_config,
                    "sing-box workspace readback failed after native config publication",
                    readback.to_string(),
                )
                .await),
        }
    }

    fn verify_active_process_instance(&self) -> Result<(), PortError> {
        let identity = self.session.core_identity().ok_or_else(|| {
            rejected(ErrorCode::NotReady, "sing-box Core identity is unavailable")
        })?;
        if identity.engine != self.process_identity.engine
            || identity.instance != self.process_identity.instance
        {
            return Err(PortError::unsupported(
                Capability::Profiles,
                "sing-box configuration transaction is bound to another active Core instance",
            ));
        }
        Ok(())
    }

    async fn recover_ambiguous_store_write(
        &self,
        previous: &ProfileWorkspace,
        update: &ProfileWorkspaceUpdate,
        cause: PortError,
    ) -> Result<ProfileConfigurationTransactionOutcome, PortError> {
        let current = match self.profile_store.load_active_workspace().await {
            Ok(current) => match classify_failed_cas_readback(previous, update, &current) {
                FailedCasReadback::Unchanged | FailedCasReadback::ConcurrentChange => {
                    return Err(cause);
                }
                FailedCasReadback::PartialPublication => current,
            },
            Err(readback) => {
                return Ok(terminal_outcome(
                    "sing-box profile CAS reported failure and its result could not be read",
                    format!("CAS: {cause}; readback: {readback}"),
                ));
            }
        };
        Ok(self
            .rollback_workspace_only(
                current.identity(),
                previous,
                "sing-box profile CAS failed after publishing workspace bytes",
                cause.to_string(),
            )
            .await)
    }

    async fn rollback_workspace_only(
        &self,
        expected_current: ProfileWorkspaceIdentity,
        previous: &ProfileWorkspace,
        cause: impl Into<String>,
        context: impl Into<String>,
    ) -> ProfileConfigurationTransactionOutcome {
        let cause = cause.into();
        let context = context.into();
        match classify_workspace_restore(
            self.profile_store
                .restore_workspace(&expected_current, previous)
                .await,
            previous,
        ) {
            WorkspaceRestoreState::Exact => {
                match classify_workspace_rollback_readback(
                    self.profile_store.load_active_workspace().await,
                    previous,
                ) {
                    WorkspaceRollbackReadback::Exact(readback) => {
                        ProfileConfigurationTransactionOutcome::RolledBack {
                            restored: readback,
                            cause: Failure::new(ErrorCode::Storage, cause, true),
                        }
                    }
                    WorkspaceRollbackReadback::Different => terminal_outcome(
                        cause,
                        format!("{context}; previous workspace did not read back exactly"),
                    ),
                    WorkspaceRollbackReadback::Failed(error) => terminal_outcome(
                        cause,
                        format!("{context}; workspace rollback readback failed: {error}"),
                    ),
                }
            }
            WorkspaceRestoreState::Different => terminal_outcome(
                cause,
                format!("{context}; restore returned a different workspace"),
            ),
            WorkspaceRestoreState::Failed(error) => terminal_outcome(
                cause,
                format!("{context}; workspace restore failed: {error}"),
            ),
        }
    }

    async fn rollback_after_file_failure(
        &self,
        expected_current: ProfileWorkspaceIdentity,
        previous: &ProfileWorkspace,
        previous_config: &SingBoxConfigFileState,
        cause: impl Into<String>,
        context: impl Into<String>,
    ) -> ProfileConfigurationTransactionOutcome {
        let cause = cause.into();
        let context = context.into();
        let file_restored = self.configuration.restore_file_state(previous_config).await;
        let workspace_restored = self
            .profile_store
            .restore_workspace(&expected_current, previous)
            .await;
        match classify_file_and_workspace_restore(file_restored, workspace_restored, previous) {
            FileAndWorkspaceRestore::Exact => {
                match classify_file_and_workspace_readback(
                    self.profile_store.load_active_workspace().await,
                    self.configuration.file_state().await,
                    previous,
                    previous_config,
                ) {
                    FileAndWorkspaceReadback::Exact(readback) => {
                        ProfileConfigurationTransactionOutcome::RolledBack {
                            restored: readback,
                            cause: Failure::new(ErrorCode::Configuration, cause, true),
                        }
                    }
                    FileAndWorkspaceReadback::Different { workspace, file } => terminal_outcome(
                        cause,
                        format!(
                            "{context}; workspace rollback: {}; config rollback: {}",
                            result_description(workspace),
                            result_description(file)
                        ),
                    ),
                }
            }
            FileAndWorkspaceRestore::Different { file, workspace } => terminal_outcome(
                cause,
                format!(
                    "{context}; config restore: {}; workspace restore: {}",
                    result_description(file),
                    result_description(workspace)
                ),
            ),
        }
    }
}

#[async_trait::async_trait]
impl ProfileConfigurationTransactionPort for SingBoxProfileConfigurationTransactionAdapter {
    fn scope(&self) -> ProfileConfigurationTransactionScope {
        self.scope.clone()
    }

    async fn transact(
        &self,
        request: ProfileConfigurationTransactionRequest,
    ) -> Result<ProfileConfigurationTransactionOutcome, PortError> {
        let _transaction = self.transaction_lock.lock().await;
        let _process_gate = self.operation_gate.acquire().await;
        self.transact_inner(request).await
    }
}

fn lifecycle_fence(
    session: &Arc<dyn CoreLifecyclePort>,
    expected: &CoreLifecycle,
) -> LifecycleFenceState {
    match session.lifecycle() {
        observed if &observed == expected => LifecycleFenceState::Retained,
        _ => LifecycleFenceState::Changed,
    }
}

fn classify_committed_readback(
    expected: &ProfileWorkspaceIdentity,
    previous: &ProfileWorkspace,
    update: &ProfileWorkspaceUpdate,
    committed: &ProfileWorkspace,
    observed: &ProfileWorkspace,
) -> CommittedWorkspaceReadback {
    let identity = observed.identity();
    if committed.identity() != identity {
        return CommittedWorkspaceReadback::DifferentIdentity;
    }
    if identity.document.engine_id != expected.document.engine_id
        || identity.document.profile_id != expected.document.profile_id
        || identity.document.source != expected.document.source
        || identity.document.format_id != expected.document.format_id
        || identity.document.schema_version != expected.document.schema_version
        || observed.document.raw_bytes != update.document_bytes
        || observed.sidecar_bytes != update.sidecar_bytes
        || observed.write_protection != previous.write_protection
        || (update.document_bytes != previous.document.raw_bytes
            && identity.document.revision == previous.document.identity.revision)
        || (update.sidecar_bytes != previous.sidecar_bytes
            && identity.sidecar_revision == previous.sidecar_revision)
    {
        return CommittedWorkspaceReadback::SameIdentityWithDifferentContent;
    }
    CommittedWorkspaceReadback::Exact
}

fn classify_failed_cas_readback(
    previous: &ProfileWorkspace,
    update: &ProfileWorkspaceUpdate,
    observed: &ProfileWorkspace,
) -> FailedCasReadback {
    if observed == previous {
        return FailedCasReadback::Unchanged;
    }
    let previous_identity = previous.identity();
    let observed_identity = observed.identity();
    if classify_workspace_scope(&previous_identity, &observed_identity, previous, observed)
        == WorkspaceScopeState::Changed
    {
        return FailedCasReadback::ConcurrentChange;
    }
    let document = classify_candidate_value(
        &previous.document.raw_bytes,
        &update.document_bytes,
        &observed.document.raw_bytes,
    );
    let sidecar = classify_candidate_value(
        &previous.sidecar_bytes,
        &update.sidecar_bytes,
        &observed.sidecar_bytes,
    );
    match (document, sidecar) {
        (CandidateBytesState::Previous, CandidateBytesState::Previous)
        | (CandidateBytesState::Other, _)
        | (_, CandidateBytesState::Other) => FailedCasReadback::ConcurrentChange,
        (CandidateBytesState::Submitted, _) | (_, CandidateBytesState::Submitted) => {
            FailedCasReadback::PartialPublication
        }
    }
}

fn classify_workspace_scope(
    previous_identity: &ProfileWorkspaceIdentity,
    observed_identity: &ProfileWorkspaceIdentity,
    previous: &ProfileWorkspace,
    observed: &ProfileWorkspace,
) -> WorkspaceScopeState {
    if observed_identity.document.engine_id == previous_identity.document.engine_id
        && observed_identity.document.profile_id == previous_identity.document.profile_id
        && observed_identity.document.source == previous_identity.document.source
        && observed_identity.document.format_id == previous_identity.document.format_id
        && observed_identity.document.schema_version == previous_identity.document.schema_version
        && observed.write_protection == previous.write_protection
    {
        WorkspaceScopeState::Same
    } else {
        WorkspaceScopeState::Changed
    }
}

fn classify_candidate_value<T: PartialEq>(
    previous: &T,
    submitted: &T,
    observed: &T,
) -> CandidateBytesState {
    if observed == previous {
        CandidateBytesState::Previous
    } else if observed == submitted {
        CandidateBytesState::Submitted
    } else {
        CandidateBytesState::Other
    }
}

fn classify_native_readback(
    result: Result<Vec<u8>, PortError>,
    submitted: &[u8],
) -> NativeConfigReadback {
    match result {
        Ok(bytes) if bytes == submitted => NativeConfigReadback::Exact,
        Ok(_) => NativeConfigReadback::Different,
        Err(error) => NativeConfigReadback::Unavailable(error),
    }
}

fn classify_final_workspace_readback(
    result: Result<ProfileWorkspace, PortError>,
    committed: &ProfileWorkspace,
    returned_by_cas: &ProfileWorkspace,
) -> FinalWorkspaceReadback {
    match result {
        Ok(workspace) if workspace == *committed => FinalWorkspaceReadback::Exact(workspace),
        Ok(workspace) if workspace.identity() == returned_by_cas.identity() => {
            FinalWorkspaceReadback::SameIdentityWithDifferentSnapshot
        }
        Ok(_) => FinalWorkspaceReadback::ConcurrentChange,
        Err(error) => FinalWorkspaceReadback::Unavailable(error),
    }
}

fn classify_workspace_restore(
    result: Result<ProfileWorkspace, PortError>,
    previous: &ProfileWorkspace,
) -> WorkspaceRestoreState {
    match result {
        Ok(restored) if restored == *previous => WorkspaceRestoreState::Exact,
        Ok(_) => WorkspaceRestoreState::Different,
        Err(error) => WorkspaceRestoreState::Failed(error),
    }
}

fn classify_workspace_rollback_readback(
    result: Result<ProfileWorkspace, PortError>,
    previous: &ProfileWorkspace,
) -> WorkspaceRollbackReadback {
    match result {
        Ok(readback) if readback == *previous => WorkspaceRollbackReadback::Exact(readback),
        Ok(_) => WorkspaceRollbackReadback::Different,
        Err(error) => WorkspaceRollbackReadback::Failed(error),
    }
}

fn classify_file_and_workspace_restore(
    file: Result<(), PortError>,
    workspace: Result<ProfileWorkspace, PortError>,
    previous: &ProfileWorkspace,
) -> FileAndWorkspaceRestore {
    match (&file, &workspace) {
        (Ok(()), Ok(restored)) if restored == previous => FileAndWorkspaceRestore::Exact,
        _ => FileAndWorkspaceRestore::Different { file, workspace },
    }
}

fn classify_file_and_workspace_readback(
    workspace: Result<ProfileWorkspace, PortError>,
    file: Result<SingBoxConfigFileState, PortError>,
    previous: &ProfileWorkspace,
    previous_config: &SingBoxConfigFileState,
) -> FileAndWorkspaceReadback {
    match (&workspace, &file) {
        (Ok(readback), Ok(file_state)) if readback == previous && file_state == previous_config => {
            FileAndWorkspaceReadback::Exact(readback.clone())
        }
        _ => FileAndWorkspaceReadback::Different { workspace, file },
    }
}

fn invalid_candidate(
    diagnostics: Vec<infiltrator_contract::profile_document::DocumentDiagnosticSnapshot>,
) -> PortError {
    let detail = diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>()
        .join("; ");
    rejected(
        ErrorCode::Configuration,
        if detail.is_empty() {
            "sing-box rejected the native JSON configuration".to_owned()
        } else {
            detail
        },
    )
}

fn rejected(code: ErrorCode, message: impl Into<String>) -> PortError {
    PortError::Rejected(Failure::new(code, message, false))
}

fn terminal_outcome(
    cause: impl Into<String>,
    rollback: impl Into<String>,
) -> ProfileConfigurationTransactionOutcome {
    ProfileConfigurationTransactionOutcome::RollbackFailed {
        cause: Failure::new(
            ErrorCode::InvalidState,
            format!(
                "sing-box configuration transaction is uncertain: {}",
                cause.into()
            ),
            true,
        ),
        rollback: Failure::new(ErrorCode::Storage, rollback, true),
    }
}

fn result_description<T, E: std::fmt::Display>(result: Result<T, E>) -> String {
    match result {
        Ok(_) => "restored".to_owned(),
        Err(error) => error.to_string(),
    }
}

#[cfg(test)]
#[path = "profile_configuration_transaction_tests.rs"]
mod tests;

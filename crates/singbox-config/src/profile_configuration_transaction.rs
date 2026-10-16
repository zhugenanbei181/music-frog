//! Engine-scoped sing-box profile CAS and native config-file transaction.

use crate::config::{SingBoxConfigFileState, SingBoxProfileConfigurationAdapter};
use crate::profile_document_codec::SingBoxProfileDocumentCodec;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceState;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::{CoreIdentity, EngineId};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document::{DocumentDiagnosticSnapshot, DocumentValidation};
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
use infiltrator_ports::profile_store::{
    ActiveProfileWorkspacePublishOutcome, ActiveProfileWorkspacePublishedLease,
    ActiveProfileWorkspaceRecoveryLease, ActiveProfileWorkspaceRecoveryResolution,
    ActiveProfileWorkspaceRestoreOutcome, ActiveProfileWorkspaceRestoredLease, ProfileStore,
};
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use singbox_process::process::SingBoxCoreOperationGate;
use std::fmt::Display;
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

#[derive(Debug)]
enum NativeConfigReadback {
    Exact,
    Different,
    Unavailable(PortError),
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
    transaction_lock: Arc<Mutex<()>>,
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
        let expected_engine = EngineId::sing_box();
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
            transaction_lock: Arc::new(Mutex::new(())),
        })
    }

    fn owned_clone(&self) -> Self {
        Self {
            profile_store: Arc::clone(&self.profile_store),
            session: Arc::clone(&self.session),
            configuration: Arc::clone(&self.configuration),
            operation_gate: Arc::clone(&self.operation_gate),
            process_identity: self.process_identity.clone(),
            scope: self.scope.clone(),
            transaction_lock: Arc::clone(&self.transaction_lock),
        }
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
        if request.expected_reference.engine_id != self.scope.document.engine_id
            || request.expected_reference.engine_id != request.expected.document.engine_id
            || request.expected_reference.profile_id != request.expected.document.profile_id
        {
            return Err(PortError::unsupported(
                Capability::Profiles,
                "sing-box reference does not identify the expected workspace",
            ));
        }
        if self.profile_store.read_active_profile_reference().await?
            != ActiveProfileReferenceState::Selected(
                request.expected_reference.clone(),
            )
        {
            return Err(rejected(
                ErrorCode::NotReady,
                "sing-box active profile reference changed before configuration save",
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

        let lease = self
            .profile_store
            .acquire_active_workspace_write_lease(
                request.expected_reference.clone(),
                request.expected.clone(),
                request.authorization.clone(),
            )
            .await?;
        if lease.observed_workspace() != &previous {
            return Err(rejected(
                ErrorCode::NotReady,
                "sing-box profile workspace changed before the transaction began",
            ));
        }
        let previous_config = self.configuration.file_state().await?;
        let published = match lease.publish(request.update.clone()).await {
            ActiveProfileWorkspacePublishOutcome::Published(published) => published,
            ActiveProfileWorkspacePublishOutcome::Unpublished(error) => return Err(error),
            ActiveProfileWorkspacePublishOutcome::Superseded { .. } => {
                return Err(rejected(
                    ErrorCode::NotReady,
                    "sing-box workspace was superseded before publication",
                ));
            }
            ActiveProfileWorkspacePublishOutcome::RecoveryRequired(recovery) => {
                resolve_workspace_publication(recovery).await?
            }
        };
        let committed = published.published_workspace().clone();
        let stored_readback = committed.clone();
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
                        published,
                        &previous,
                        "sing-box profile workspace did not read back the submitted bytes",
                        "the committed identity or exact document/sidecar bytes differed",
                    )
                    .await);
            }
            CommittedWorkspaceReadback::DifferentIdentity => {
                return Ok(self
                    .rollback_workspace_only(
                        published,
                        &previous,
                        "sing-box profile workspace identity changed before readback",
                        "the published workspace did not retain the submitted identity",
                    )
                    .await);
            }
        }

        match lifecycle_fence(&self.session, &initial_lifecycle) {
            LifecycleFenceState::Retained => {}
            LifecycleFenceState::Changed => {
                return Ok(self
                    .rollback_workspace_only(
                        published,
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
                    published,
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
                        published,
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
                        published,
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
                        published,
                        &previous,
                        &previous_config,
                        "sing-box Core lifecycle changed during native config publication",
                        "configuration transaction did not retain its inactive lifecycle",
                    )
                    .await);
            }
        }
        let final_readback = published.published_workspace().clone();
        if final_readback != stored_readback {
            return Ok(self
                .rollback_after_file_failure(
                    published,
                    &previous,
                    &previous_config,
                    "sing-box workspace changed after native config publication",
                    "exact workspace readback differed from its committed snapshot",
                )
                .await);
        }
        Ok(ProfileConfigurationTransactionOutcome::Committed {
            workspace: published.commit(),
            effect,
        })
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

    async fn rollback_workspace_only(
        &self,
        published: Box<dyn ActiveProfileWorkspacePublishedLease + '_>,
        previous: &ProfileWorkspace,
        cause: impl Into<String>,
        context: impl Into<String>,
    ) -> ProfileConfigurationTransactionOutcome {
        let cause = cause.into();
        let context = context.into();
        match restore_published_workspace(published).await {
            Ok(restored) if restored.restored_workspace() == previous => {
                ProfileConfigurationTransactionOutcome::RolledBack {
                    restored: restored.finish(),
                    cause: Failure::new(ErrorCode::Storage, cause, true),
                }
            }
            Ok(restored) => {
                let observed = restored.restored_workspace().identity();
                restored.finish();
                terminal_outcome(
                    cause,
                    format!("{context}; restore read back a different workspace: {observed:?}"),
                )
            }
            Err(error) => terminal_outcome(
                cause,
                format!("{context}; conditional workspace restore failed: {error}"),
            ),
        }
    }

    async fn rollback_after_file_failure(
        &self,
        published: Box<dyn ActiveProfileWorkspacePublishedLease + '_>,
        previous: &ProfileWorkspace,
        previous_config: &SingBoxConfigFileState,
        cause: impl Into<String>,
        context: impl Into<String>,
    ) -> ProfileConfigurationTransactionOutcome {
        let cause = cause.into();
        let context = context.into();
        let file_restored = self.configuration.restore_file_state(previous_config).await;
        match restore_published_workspace(published).await {
            Ok(restored) => {
                let workspace_matches = restored.restored_workspace() == previous;
                let workspace_readback = restored.restored_workspace().clone();
                let file_readback = self.configuration.file_state().await;
                let file_matches = matches!(file_restored, Ok(()))
                    && matches!(&file_readback, Ok(state) if state == previous_config);
                if workspace_matches && file_matches {
                    ProfileConfigurationTransactionOutcome::RolledBack {
                        restored: restored.finish(),
                        cause: Failure::new(ErrorCode::Configuration, cause, true),
                    }
                } else {
                    let file_restore_detail = result_description(file_restored);
                    let file_readback_detail = result_description(file_readback);
                    let _ = restored.finish();
                    terminal_outcome(
                        cause,
                        format!(
                            "{context}; workspace exact: {workspace_matches}; workspace readback: {:?}; config restore: {file_restore_detail}; config readback: {file_readback_detail}",
                            workspace_readback.identity()
                        ),
                    )
                }
            }
            Err(workspace_error) => {
                let file_detail = result_description(file_restored);
                terminal_outcome(
                    cause,
                    format!(
                        "{context}; config restore: {file_detail}; workspace restore: {workspace_error}"
                    ),
                )
            }
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
        let adapter = self.owned_clone();
        tokio::spawn(async move {
            let _transaction = adapter.transaction_lock.lock().await;
            let _process_gate = adapter.operation_gate.acquire().await;
            adapter.transact_inner(request).await
        })
        .await
        .map_err(|error| {
            PortError::Failed(format!(
                "sing-box profile transaction continuation failed: {error}"
            ))
        })?
    }
}

async fn resolve_workspace_publication<'a>(
    mut recovery: Box<dyn ActiveProfileWorkspaceRecoveryLease + 'a>,
) -> Result<Box<dyn ActiveProfileWorkspacePublishedLease + 'a>, PortError> {
    loop {
        match recovery.resolve().await {
            ActiveProfileWorkspaceRecoveryResolution::Published(published) => {
                return Ok(published);
            }
            ActiveProfileWorkspaceRecoveryResolution::Restored(restored) => {
                let _ = restored.finish();
                return Err(rejected(
                    ErrorCode::NotReady,
                    "sing-box workspace publication recovered to its previous bytes",
                ));
            }
            ActiveProfileWorkspaceRecoveryResolution::Unpublished(error) => return Err(error),
            ActiveProfileWorkspaceRecoveryResolution::Superseded { .. } => {
                return Err(rejected(
                    ErrorCode::NotReady,
                    "sing-box workspace was superseded during publication recovery",
                ));
            }
            ActiveProfileWorkspaceRecoveryResolution::RecoveryRequired(next) => {
                recovery = next;
            }
        }
    }
}

async fn restore_published_workspace<'a>(
    mut published: Box<dyn ActiveProfileWorkspacePublishedLease + 'a>,
) -> Result<Box<dyn ActiveProfileWorkspaceRestoredLease + 'a>, PortError> {
    loop {
        match published.restore_if_current().await? {
            ActiveProfileWorkspaceRestoreOutcome::Restored(restored) => return Ok(restored),
            ActiveProfileWorkspaceRestoreOutcome::Superseded { .. } => {
                return Err(rejected(
                    ErrorCode::NotReady,
                    "sing-box workspace changed before conditional restore",
                ));
            }
            ActiveProfileWorkspaceRestoreOutcome::RecoveryRequired(mut recovery) => loop {
                match recovery.resolve().await {
                    ActiveProfileWorkspaceRecoveryResolution::Restored(restored) => {
                        return Ok(restored);
                    }
                    ActiveProfileWorkspaceRecoveryResolution::Published(next) => {
                        published = next;
                        break;
                    }
                    ActiveProfileWorkspaceRecoveryResolution::Unpublished(error) => {
                        return Err(error);
                    }
                    ActiveProfileWorkspaceRecoveryResolution::Superseded { .. } => {
                        return Err(rejected(
                            ErrorCode::NotReady,
                            "sing-box workspace changed during conditional restore recovery",
                        ));
                    }
                    ActiveProfileWorkspaceRecoveryResolution::RecoveryRequired(next) => {
                        recovery = next;
                    }
                }
            },
        }
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

fn invalid_candidate(
    diagnostics: Vec<DocumentDiagnosticSnapshot>,
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

fn result_description<T, E: Display>(result: Result<T, E>) -> String {
    match result {
        Ok(_) => "restored".to_owned(),
        Err(error) => error.to_string(),
    }
}

#[cfg(test)]
#[path = "profile_configuration_transaction_tests.rs"]
mod tests;

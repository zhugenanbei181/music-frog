//! Engine-scoped Mihomo validate/apply/readback/rollback adapter.

use crate::apply::{ApplyError, ApplyParams, ConfigReloader};
use crate::apply_workspace::apply_codec_validated_workspace;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document::DocumentValidation;
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationCandidateValidatorPort, ProfileConfigurationCommitEffect,
    ProfileConfigurationTransactionOutcome, ProfileConfigurationTransactionPort,
    ProfileConfigurationTransactionRequest, ProfileConfigurationTransactionScope,
};
use infiltrator_ports::profile_document_codec::ProfileDocumentCodecPort;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use infiltrator_ports::secure_store::SecureStore;
use mihomo_config::manager::ConfigManager;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::Mutex;

static PREFLIGHT_CANDIDATE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Mihomo's native transaction adapter. Pure codec inspection is injected
/// independently and is used before any workspace persistence/runtime effect.
pub struct MihomoProfileConfigurationTransactionAdapter<S: SecureStore> {
    codec: Arc<dyn ProfileDocumentCodecPort>,
    stopped_validator: Arc<dyn ProfileConfigurationCandidateValidatorPort>,
    scope: ProfileConfigurationTransactionScope,
    session: Arc<dyn CoreLifecyclePort>,
    config: Arc<ConfigManager<S>>,
    reloader: Arc<dyn ConfigReloader>,
    apply_guard: Arc<Mutex<()>>,
}

impl<S: SecureStore + 'static> MihomoProfileConfigurationTransactionAdapter<S> {
    pub fn new(
        codec: Arc<dyn ProfileDocumentCodecPort>,
        stopped_validator: Arc<dyn ProfileConfigurationCandidateValidatorPort>,
        session: Arc<dyn CoreLifecyclePort>,
        config: Arc<ConfigManager<S>>,
        reloader: Arc<dyn ConfigReloader>,
        apply_guard: Arc<Mutex<()>>,
    ) -> Result<Self, PortError> {
        let codec_scope = codec.scope();
        if codec_scope.engine_id != config.engine_id() {
            return Err(PortError::Failed(
                "Mihomo transaction codec and profile store have different engines".to_owned(),
            ));
        }
        let scope = ProfileConfigurationTransactionScope::new(codec_scope);
        if stopped_validator.scope() != scope {
            return Err(PortError::Failed(
                "stopped-state validator and configuration transaction have different scopes"
                    .to_owned(),
            ));
        }
        Ok(Self {
            codec,
            stopped_validator,
            scope,
            session,
            config,
            reloader,
            apply_guard,
        })
    }

    async fn transact_inner(
        &self,
        request: ProfileConfigurationTransactionRequest,
    ) -> Result<ProfileConfigurationTransactionOutcome, PortError> {
        if !self.scope.supports(&request.expected) {
            return Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Profiles,
                "configuration transaction request has a foreign engine, format, or schema",
            ));
        }
        let runtime_engine = self
            .session
            .core_identity()
            .map(|identity| identity.engine)
            .ok_or_else(|| rejected(ErrorCode::NotReady, "active Core identity is unavailable"))?;
        if runtime_engine != self.scope.document.engine_id {
            return Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Profiles,
                "configuration transaction does not belong to the active Core engine",
            ));
        }
        match self
            .codec
            .inspect(&request.update.document_bytes)
            .map_err(|error| rejected(ErrorCode::Configuration, error.to_string()))?
        {
            DocumentValidation::Valid => {}
            DocumentValidation::Invalid { diagnostics } => {
                let detail = diagnostics
                    .into_iter()
                    .map(|diagnostic| diagnostic.message)
                    .collect::<Vec<_>>()
                    .join("; ");
                return Err(rejected(
                    ErrorCode::Configuration,
                    if detail.is_empty() {
                        "candidate configuration is invalid".to_owned()
                    } else {
                        detail
                    },
                ));
            }
        }

        let _guard = self.apply_guard.lock().await;
        let previous = self
            .config
            .load_active_workspace()
            .await
            .map_err(PortError::from)?;
        if previous.identity() != request.expected {
            return Err(rejected(
                ErrorCode::NotReady,
                "profile workspace revision changed before the transaction began",
            ));
        }
        let observed_lifecycle = self.session.lifecycle();
        match &observed_lifecycle {
            CoreLifecycle::Starting | CoreLifecycle::Stopping => {
                return Err(rejected(
                    ErrorCode::NotReady,
                    "Core lifecycle transition prevents configuration apply",
                ));
            }
            CoreLifecycle::Ready | CoreLifecycle::Running => {
                native_preflight(
                    self.reloader.as_ref(),
                    self.config.config_dir(),
                    &request.update.document_bytes,
                )
                .await?;
            }
            CoreLifecycle::Stopped | CoreLifecycle::Failed => {
                self.stopped_validator
                    .validate_candidate(&request.expected, &request.update.document_bytes)
                    .await?;
            }
        }

        match &observed_lifecycle {
            CoreLifecycle::Ready | CoreLifecycle::Running => {
                let params = ApplyParams {
                    strategy: request.strategy,
                    ..ApplyParams::default()
                };
                match apply_codec_validated_workspace(
                    self.session.as_ref(),
                    self.config.as_ref(),
                    self.reloader.as_ref(),
                    &request.expected,
                    &request.update,
                    params,
                )
                .await
                {
                    Ok(committed) => {
                        if !committed_matches(
                            &request.expected,
                            &previous,
                            &request.update,
                            &committed,
                        ) {
                            return Ok(terminal_outcome(
                                "transaction committed but exact readback differed",
                                "committed workspace identity or bytes did not match the request",
                            ));
                        }
                        Ok(ProfileConfigurationTransactionOutcome::Committed {
                            workspace: committed,
                            effect: ProfileConfigurationCommitEffect::AppliedToRunningCore,
                        })
                    }
                    Err(ApplyError::RolledBack { cause }) => {
                        match self.config.load_active_workspace().await {
                            Ok(restored) if restored == previous => {
                                Ok(ProfileConfigurationTransactionOutcome::RolledBack {
                                    restored,
                                    cause: Failure::new(ErrorCode::Configuration, cause, true),
                                })
                            }
                            Ok(_) => Ok(terminal_outcome(
                                "apply failed and rollback readback differed",
                                "the exact previous workspace was not restored",
                            )),
                            Err(rollback) => Ok(terminal_outcome(
                                "apply failed and rollback readback failed",
                                rollback.to_string(),
                            )),
                        }
                    }
                    Err(ApplyError::RollbackFailed { cause, rollback }) => {
                        Ok(ProfileConfigurationTransactionOutcome::RollbackFailed {
                            cause: Failure::new(ErrorCode::Configuration, cause, true),
                            rollback: Failure::new(ErrorCode::Storage, rollback, true),
                        })
                    }
                    Err(error) => Err(apply_error(error)),
                }
            }
            CoreLifecycle::Stopped | CoreLifecycle::Failed => {
                // Never route an inactive commit through the live apply helper:
                // that helper owns start/restart behavior for legacy apply flows.
                // Recheck before the CAS so a changed lifecycle fails closed.
                if self.session.lifecycle() != observed_lifecycle.clone() {
                    return Err(rejected(
                        ErrorCode::NotReady,
                        "Core lifecycle changed during inactive configuration validation",
                    ));
                }
                let effect = match &observed_lifecycle {
                    CoreLifecycle::Stopped => ProfileConfigurationCommitEffect::StoredWhileStopped,
                    CoreLifecycle::Failed => ProfileConfigurationCommitEffect::StoredWhileFailed,
                    CoreLifecycle::Starting
                    | CoreLifecycle::Stopping
                    | CoreLifecycle::Ready
                    | CoreLifecycle::Running => {
                        unreachable!("inactive transaction branch has an active lifecycle")
                    }
                };
                self.store_while_inactive(&request, &previous, effect).await
            }
            CoreLifecycle::Starting | CoreLifecycle::Stopping => {
                unreachable!("lifecycle transition was rejected before transaction dispatch")
            }
        }
    }

    async fn store_while_inactive(
        &self,
        request: &ProfileConfigurationTransactionRequest,
        previous: &ProfileWorkspace,
        effect: ProfileConfigurationCommitEffect,
    ) -> Result<ProfileConfigurationTransactionOutcome, PortError> {
        let committed = match self
            .config
            .compare_and_save_active_workspace(&request.expected, &request.update)
            .await
        {
            Ok(committed) => committed,
            Err(cause) => {
                let publication_recovery_failed = cause.error_code() == ErrorCode::InvalidState;
                return match self.config.load_active_workspace().await {
                    Ok(current) if current == *previous => Err(cause),
                    Ok(current)
                        if publication_recovery_failed
                            && is_partial_transaction_workspace(
                                &request.expected,
                                previous,
                                &request.update,
                                &current,
                            ) =>
                    {
                        Ok(self
                            .rollback_inactive_write(
                                current.identity(),
                                previous,
                                "inactive workspace persistence failed after publishing bytes",
                                cause.to_string(),
                            )
                            .await)
                    }
                    Ok(_) if publication_recovery_failed => Ok(terminal_outcome(
                        "inactive workspace persistence reported recovery failure",
                        cause.to_string(),
                    )),
                    Ok(_) => Err(cause),
                    Err(readback) if publication_recovery_failed => Ok(terminal_outcome(
                        "inactive workspace persistence reported recovery failure and readback failed",
                        format!("{cause}; readback: {readback}"),
                    )),
                    Err(_) => Err(cause),
                };
            }
        };

        let observed = match self.config.load_active_workspace().await {
            Ok(observed) => observed,
            Err(readback) => {
                return Ok(self
                    .rollback_inactive_write(
                        committed.identity(),
                        previous,
                        "inactive workspace was stored but exact readback failed",
                        readback.to_string(),
                    )
                    .await);
            }
        };
        if committed_matches(&request.expected, previous, &request.update, &observed) {
            return Ok(ProfileConfigurationTransactionOutcome::Committed {
                workspace: observed,
                effect,
            });
        }
        if observed.identity() == committed.identity() {
            return Ok(self
                .rollback_inactive_write(
                    committed.identity(),
                    previous,
                    "inactive workspace readback differed from the committed bytes",
                    "the committed identity did not read back as the submitted workspace",
                )
                .await);
        }
        Ok(terminal_outcome(
            "inactive workspace changed before exact readback",
            "preserved a newer workspace instead of overwriting it during recovery",
        ))
    }

    async fn rollback_inactive_write(
        &self,
        expected_current: ProfileWorkspaceIdentity,
        previous: &ProfileWorkspace,
        cause: impl Into<String>,
        rollback_context: impl Into<String>,
    ) -> ProfileConfigurationTransactionOutcome {
        let cause = cause.into();
        let rollback_context = rollback_context.into();
        if let Err(rollback) = self
            .config
            .restore_workspace(&expected_current, previous)
            .await
        {
            return terminal_outcome(
                cause,
                format!("{rollback_context}; restore failed: {rollback}"),
            );
        }
        match self.config.load_active_workspace().await {
            Ok(restored) if restored == *previous => {
                ProfileConfigurationTransactionOutcome::RolledBack {
                    restored,
                    cause: Failure::new(ErrorCode::Storage, cause, true),
                }
            }
            Ok(_) => terminal_outcome(
                cause,
                format!("{rollback_context}; exact previous workspace did not read back"),
            ),
            Err(rollback) => terminal_outcome(
                cause,
                format!("{rollback_context}; rollback readback failed: {rollback}"),
            ),
        }
    }
}

/// Runs Mihomo's native `-t` config check using the exact executable selected
/// for the product session. The candidate lives beside the active profile so
/// relative resource paths resolve exactly as they will after publication.
pub struct MihomoBinaryConfigurationCandidateValidator {
    scope: ProfileConfigurationTransactionScope,
    binary_path: PathBuf,
    config_dir: PathBuf,
}

impl MihomoBinaryConfigurationCandidateValidator {
    pub fn new(
        scope: ProfileConfigurationTransactionScope,
        binary_path: PathBuf,
        config_dir: PathBuf,
    ) -> Result<Self, PortError> {
        let native_scope = ProfileConfigurationTransactionScope::new(
            mihomo_config::profile_document_codec::MihomoProfileDocumentCodec.scope(),
        );
        if scope != native_scope {
            return Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Profiles,
                "Mihomo native validator cannot validate a foreign engine, format, or schema",
            ));
        }
        Ok(Self {
            scope,
            binary_path,
            config_dir,
        })
    }
}

#[async_trait::async_trait]
impl ProfileConfigurationCandidateValidatorPort for MihomoBinaryConfigurationCandidateValidator {
    fn scope(&self) -> ProfileConfigurationTransactionScope {
        self.scope.clone()
    }

    async fn validate_candidate(
        &self,
        expected: &ProfileWorkspaceIdentity,
        document_bytes: &[u8],
    ) -> Result<(), PortError> {
        if !self.scope.supports(expected) {
            return Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Profiles,
                "Mihomo native validation request has a foreign engine, format, or schema",
            ));
        }
        let (candidate, file) = reserve_preflight_candidate(&self.config_dir).await?;
        let path = candidate.0.clone();
        let validation_path = path.clone();
        let binary_path = self.binary_path.clone();
        let config_dir = self.config_dir.clone();
        let validation_bytes = document_bytes.to_vec();
        let validation = async move {
            let mut file = file;
            file.write_all(&validation_bytes)
                .await
                .map_err(|error| PortError::Io(error.to_string()))?;
            file.flush()
                .await
                .map_err(|error| PortError::Io(error.to_string()))?;
            drop(file);
            let output = Command::new(&binary_path)
                .arg("-t")
                .arg("-f")
                .arg(&validation_path)
                .arg("-d")
                .arg(&config_dir)
                .output()
                .await
                .map_err(|error| {
                    PortError::Failed(format!(
                        "run the selected Mihomo binary for native config validation: {error}"
                    ))
                })?;
            if output.status.success() {
                return Ok(());
            }
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            let detail = if detail.is_empty() {
                String::from_utf8_lossy(&output.stdout).trim().to_owned()
            } else {
                detail
            };
            Err(rejected(
                ErrorCode::Configuration,
                if detail.is_empty() {
                    format!(
                        "selected Mihomo binary rejected candidate config with {}",
                        output.status
                    )
                } else {
                    format!("selected Mihomo binary rejected candidate config: {detail}")
                },
            ))
        }
        .await;
        let cleanup = tokio::fs::remove_file(&path).await;
        match (validation, cleanup) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) => Err(error),
            (Ok(()), Err(error)) => Err(PortError::Io(format!(
                "failed to remove Mihomo native-validation candidate: {error}"
            ))),
            (Err(validation), Err(cleanup)) => Err(PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                format!(
                    "{}; failed to remove Mihomo native-validation candidate: {cleanup}",
                    validation
                ),
                false,
            ))),
        }
    }
}

async fn native_preflight(
    reloader: &dyn ConfigReloader,
    config_dir: &Path,
    raw_bytes: &[u8],
) -> Result<(), PortError> {
    let (candidate, file) = reserve_preflight_candidate(config_dir).await?;
    let path = candidate.0.clone();
    let validation_path = path.clone();
    let validation_bytes = raw_bytes.to_vec();
    let validation = async move {
        let mut file = file;
        file.write_all(&validation_bytes)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        file.flush()
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        drop(file);
        reloader
            .preflight_check(&validation_path)
            .await
            .map_err(|error| {
                PortError::Rejected(Failure::new(ErrorCode::Configuration, error, false))
            })
    }
    .await;
    let cleanup = tokio::fs::remove_file(&path).await;
    match (validation, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(PortError::Io(format!(
            "failed to remove Mihomo preflight candidate: {error}"
        ))),
        (Err(validation), Err(cleanup)) => Err(PortError::Rejected(Failure::new(
            ErrorCode::Configuration,
            format!(
                "{}; failed to remove Mihomo preflight candidate: {cleanup}",
                validation
            ),
            false,
        ))),
    }
}

struct PreflightCandidate(PathBuf);

impl Drop for PreflightCandidate {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

async fn reserve_preflight_candidate(
    config_dir: &Path,
) -> Result<(PreflightCandidate, tokio::fs::File), PortError> {
    for _ in 0..16 {
        let sequence = PREFLIGHT_CANDIDATE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = config_dir.join(format!(
            ".mihomo-profile-preflight-{}-{sequence}.yaml",
            std::process::id()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
        {
            Ok(file) => return Ok((PreflightCandidate(path), file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(PortError::Io(error.to_string())),
        }
    }
    Err(PortError::Failed(
        "could not reserve a unique Mihomo preflight candidate".to_owned(),
    ))
}

#[async_trait::async_trait]
impl<S: SecureStore + 'static> ProfileConfigurationTransactionPort
    for MihomoProfileConfigurationTransactionAdapter<S>
{
    fn scope(&self) -> ProfileConfigurationTransactionScope {
        self.scope.clone()
    }

    async fn transact(
        &self,
        request: ProfileConfigurationTransactionRequest,
    ) -> Result<ProfileConfigurationTransactionOutcome, PortError> {
        self.transact_inner(request).await
    }
}

fn committed_matches(
    expected: &ProfileWorkspaceIdentity,
    previous: &ProfileWorkspace,
    update: &ProfileWorkspaceUpdate,
    committed: &ProfileWorkspace,
) -> bool {
    let identity = committed.identity();
    identity.document.engine_id == expected.document.engine_id
        && identity.document.profile_id == expected.document.profile_id
        && identity.document.source == expected.document.source
        && identity.document.format_id == expected.document.format_id
        && identity.document.schema_version == expected.document.schema_version
        && committed.document.raw_bytes == update.document_bytes
        && committed.sidecar_bytes == update.sidecar_bytes
        && committed.write_protection == previous.write_protection
        && (update.document_bytes == previous.document.raw_bytes
            || identity.document.revision != previous.document.identity.revision)
        && (update.sidecar_bytes == previous.sidecar_bytes
            || identity.sidecar_revision != previous.sidecar_revision)
}

fn is_partial_transaction_workspace(
    expected: &ProfileWorkspaceIdentity,
    previous: &ProfileWorkspace,
    update: &ProfileWorkspaceUpdate,
    observed: &ProfileWorkspace,
) -> bool {
    let identity = observed.identity();
    identity.document.engine_id == expected.document.engine_id
        && identity.document.profile_id == expected.document.profile_id
        && identity.document.source == expected.document.source
        && identity.document.format_id == expected.document.format_id
        && identity.document.schema_version == expected.document.schema_version
        && observed.write_protection == previous.write_protection
        && (observed.document.raw_bytes == previous.document.raw_bytes
            || observed.document.raw_bytes == update.document_bytes)
        && (observed.sidecar_bytes == previous.sidecar_bytes
            || observed.sidecar_bytes == update.sidecar_bytes)
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
                "configuration transaction terminal state is uncertain: {}",
                cause.into()
            ),
            true,
        ),
        rollback: Failure::new(ErrorCode::Storage, rollback, true),
    }
}

fn apply_error(error: ApplyError) -> PortError {
    let message = error.to_string();
    match error {
        ApplyError::SourceChanged | ApplyError::Busy { .. } => {
            rejected(ErrorCode::NotReady, message)
        }
        ApplyError::Validation(_) => rejected(ErrorCode::Configuration, message),
        ApplyError::Write(_) => rejected(ErrorCode::Storage, message),
        ApplyError::Permission(_) => rejected(ErrorCode::Permission, message),
        ApplyError::Lifecycle(_) => rejected(ErrorCode::InvalidState, message),
        ApplyError::RolledBack { .. } | ApplyError::RollbackFailed { .. } => rejected(
            ErrorCode::InvalidState,
            format!("transaction adapter missed a terminal apply state: {message}"),
        ),
    }
}

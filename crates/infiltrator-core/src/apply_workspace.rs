//! Source-bound profile/options application and conditional exact-byte recovery.
use crate::apply::{
    ApplyError, ApplyOutcome, ApplyParams, ApplyResult, ConfigReloader, apply_prepared_strategy,
    prepare_apply_strategy, restart_and_check, validate_config,
};
use crate::history::{DEFAULT_KEEP, prune_snapshots, save_snapshot};
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_store::{
    ActiveProfileWorkspacePublishOutcome, ActiveProfileWorkspacePublishedLease,
    ActiveProfileWorkspaceRecoveryLease, ActiveProfileWorkspaceRecoveryResolution,
    ActiveProfileWorkspaceRestoreOutcome, ActiveProfileWorkspaceRestoredLease,
};
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use infiltrator_ports::secure_store::SecureStore;
use mihomo_config::manager::ConfigManager;
use std::str;

/// Exact readback and native lifecycle receipt from a committed workspace
/// transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveProfileWorkspaceApplyReceipt {
    pub workspace: ProfileWorkspace,
    pub apply: ApplyOutcome,
}

/// Apply a complete active workspace against its exact reference revision and
/// opening-time document/sidecar identity. This is the Android/Core adapter
/// entry for document edits that do not use the desktop codec transaction.
pub async fn apply_active_profile_workspace<S: SecureStore, C: CoreLifecyclePort + ?Sized>(
    session: &C,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    expected_reference: &ActiveProfileReference,
    expected: &ProfileWorkspaceIdentity,
    update: &ProfileWorkspaceUpdate,
    authorization: ActiveProfileReferenceWriteAuthorization,
    params: ApplyParams,
) -> ApplyResult<ProfileWorkspace> {
    apply_active_profile_workspace_with_receipt(
        session,
        config,
        reloader,
        expected_reference,
        expected,
        update,
        authorization,
        params,
    )
    .await
    .map(|receipt| receipt.workspace)
}

/// Apply a complete active workspace and return the native lifecycle receipt
/// as well as the exact persisted workspace readback.
pub async fn apply_active_profile_workspace_with_receipt<
    S: SecureStore,
    C: CoreLifecyclePort + ?Sized,
>(
    session: &C,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    expected_reference: &ActiveProfileReference,
    expected: &ProfileWorkspaceIdentity,
    update: &ProfileWorkspaceUpdate,
    authorization: ActiveProfileReferenceWriteAuthorization,
    params: ApplyParams,
) -> ApplyResult<ActiveProfileWorkspaceApplyReceipt> {
    apply_confirmed_workspace_inner(
        session,
        config,
        reloader,
        expected_reference,
        expected,
        update,
        authorization,
        params,
        WorkspaceCandidateValidation::DomainYaml,
    )
    .await
}

#[cfg(test)]
pub(crate) async fn apply_confirmed_workspace<S: SecureStore, C: CoreLifecyclePort + ?Sized>(
    session: &C,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    expected_reference: &ActiveProfileReference,
    expected: &ProfileWorkspaceIdentity,
    update: &ProfileWorkspaceUpdate,
    authorization: ActiveProfileReferenceWriteAuthorization,
    params: ApplyParams,
) -> ApplyResult<ProfileWorkspace> {
    apply_confirmed_workspace_inner(
        session,
        config,
        reloader,
        expected_reference,
        expected,
        update,
        authorization,
        params,
        WorkspaceCandidateValidation::DomainYaml,
    )
    .await
    .map(|receipt| receipt.workspace)
}

pub(crate) async fn apply_codec_validated_workspace<
    S: SecureStore,
    C: CoreLifecyclePort + ?Sized,
>(
    session: &C,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    expected_reference: &ActiveProfileReference,
    expected: &ProfileWorkspaceIdentity,
    update: &ProfileWorkspaceUpdate,
    authorization: ActiveProfileReferenceWriteAuthorization,
    params: ApplyParams,
) -> ApplyResult<ProfileWorkspace> {
    apply_confirmed_workspace_inner(
        session,
        config,
        reloader,
        expected_reference,
        expected,
        update,
        authorization,
        params,
        WorkspaceCandidateValidation::CodecInspectionAccepted,
    )
    .await
    .map(|receipt| receipt.workspace)
}

#[derive(Clone, Copy)]
enum WorkspaceCandidateValidation {
    DomainYaml,
    CodecInspectionAccepted,
}

async fn apply_confirmed_workspace_inner<S: SecureStore, C: CoreLifecyclePort + ?Sized>(
    session: &C,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    expected_reference: &ActiveProfileReference,
    expected: &ProfileWorkspaceIdentity,
    update: &ProfileWorkspaceUpdate,
    authorization: ActiveProfileReferenceWriteAuthorization,
    params: ApplyParams,
    validation: WorkspaceCandidateValidation,
) -> ApplyResult<ActiveProfileWorkspaceApplyReceipt> {
    let profile = expected.document.profile_id.as_str();
    let store_engine = config.engine_id();
    let runtime_engine = session
        .core_identity()
        .map(|identity| identity.engine)
        .ok_or(ApplyError::SourceChanged)?;
    if expected.document.engine_id != store_engine
        || expected.document.engine_id != runtime_engine
        || expected_reference.engine_id != store_engine
        || expected_reference.profile_id != expected.document.profile_id
    {
        return Err(ApplyError::SourceChanged);
    }
    match validation {
        WorkspaceCandidateValidation::DomainYaml => {
            let proposed = str::from_utf8(&update.document_bytes)
                .map_err(|error| ApplyError::Validation(error.to_string()))?;
            validate_config(proposed).map_err(ApplyError::Validation)?;
        }
        WorkspaceCandidateValidation::CodecInspectionAccepted => {}
    }
    let status = session.lifecycle();
    match status {
        CoreLifecycle::Ready | CoreLifecycle::Running => {}
        CoreLifecycle::Starting | CoreLifecycle::Stopping => {
            return Err(ApplyError::Busy { status });
        }
        CoreLifecycle::Stopped | CoreLifecycle::Failed => {
            return Err(ApplyError::NotRunning { status });
        }
    }
    let prepared = prepare_apply_strategy(params.strategy, reloader).await;

    // The store lease checks the full selected reference, owner identity and
    // workspace revision in root -> owner -> profile lock order. It remains
    // alive through native apply and any conditional recovery below.
    let observed_lease = ProfileStore::acquire_active_workspace_write_lease(
        config,
        expected_reference.clone(),
        expected.clone(),
        authorization,
    )
    .await
    .map_err(persistence_error)?;
    let previous = observed_lease.observed_workspace().clone();
    if previous.identity() != *expected {
        return Err(ApplyError::SourceChanged);
    }
    let path = config
        .existing_profile_document_path(profile)
        .await
        .map_err(|error| ApplyError::Write(error.to_string()))?;
    let published = match observed_lease.publish(update.clone()).await {
        ActiveProfileWorkspacePublishOutcome::Published(published) => published,
        ActiveProfileWorkspacePublishOutcome::Unpublished(error) => {
            return Err(persistence_error(error));
        }
        ActiveProfileWorkspacePublishOutcome::Superseded { .. } => {
            return Err(ApplyError::SourceChanged);
        }
        ActiveProfileWorkspacePublishOutcome::RecoveryRequired(recovery) => {
            resolve_workspace_publication(recovery).await?
        }
    };
    let committed = published.published_workspace().clone();
    let committed_identity = committed.identity();
    if committed.document.raw_bytes != update.document_bytes
        || committed.sidecar_bytes != update.sidecar_bytes
        || committed_identity.document.engine_id != expected.document.engine_id
        || committed_identity.document.profile_id != expected.document.profile_id
        || committed_identity.document.source != expected.document.source
        || committed_identity.document.format_id != expected.document.format_id
        || committed_identity.document.schema_version != expected.document.schema_version
        || (committed.document.raw_bytes != previous.document.raw_bytes
            && committed_identity.document.revision == previous.document.identity.revision)
        || (committed.sidecar_bytes != previous.sidecar_bytes
            && committed_identity.sidecar_revision == previous.sidecar_revision)
    {
        let cause = "profile store readback differed from the submitted workspace".to_owned();
        match restore_published_workspace(published).await {
            Ok(restored) if restored.restored_workspace() == &previous => {
                restored.finish();
                config.record_apply_transaction(ApplyTransactionSnapshot::rolled_back(
                    profile, &cause,
                ));
                return Err(ApplyError::RolledBack { cause });
            }
            Ok(restored) => {
                restored.finish();
                return Err(rollback_failed(
                    config,
                    profile,
                    cause,
                    "conditional restore did not read back the exact previous workspace".into(),
                ));
            }
            Err(rollback) => {
                return Err(rollback_failed(
                    config,
                    profile,
                    cause,
                    rollback.to_string(),
                ));
            }
        }
    }

    let outcome = apply_prepared_strategy(session, &prepared, &path, &params).await;
    match outcome {
        Ok(outcome) => {
            if params.snapshot_history {
                let directory = config.config_dir();
                match committed.document.utf8_text() {
                    Ok(committed_text) => {
                        match save_snapshot(&config.engine_id(), directory, profile, committed_text)
                            .await
                        {
                            Ok(_) => {
                                if let Err(error) = prune_snapshots(
                                    &config.engine_id(),
                                    directory,
                                    profile,
                                    DEFAULT_KEEP,
                                )
                                .await
                                {
                                    log::warn!("workspace snapshot prune failed: {error}");
                                }
                            }
                            Err(error) => log::warn!("workspace snapshot save failed: {error}"),
                        }
                    }
                    Err(error) => {
                        log::warn!("workspace snapshot skipped for invalid UTF-8: {error}")
                    }
                }
            }
            let committed = published.commit();
            config.record_apply_transaction(ApplyTransactionSnapshot::committed(
                profile,
                outcome.method.as_str(),
            ));
            Ok(ActiveProfileWorkspaceApplyReceipt {
                workspace: committed,
                apply: outcome,
            })
        }
        Err(cause) => {
            let cause = cause.to_string();
            match restore_published_workspace(published).await {
                Ok(restored) if restored.restored_workspace() == &previous => {
                    if let Err(rollback) = restart_and_check(session, &params).await {
                        return Err(rollback_failed(
                            config,
                            profile,
                            cause,
                            rollback.to_string(),
                        ));
                    }
                    let restored = restored.finish();
                    if restored != previous {
                        return Err(rollback_failed(
                            config,
                            profile,
                            cause,
                            "conditional restore did not read back the exact previous workspace"
                                .into(),
                        ));
                    }
                    config.record_apply_transaction(ApplyTransactionSnapshot::rolled_back(
                        profile, &cause,
                    ));
                    Err(ApplyError::RolledBack { cause })
                }
                Ok(restored) => {
                    restored.finish();
                    Err(rollback_failed(
                        config,
                        profile,
                        cause,
                        "conditional restore did not read back the exact previous workspace".into(),
                    ))
                }
                Err(rollback) => Err(rollback_failed(
                    config,
                    profile,
                    cause,
                    rollback.to_string(),
                )),
            }
        }
    }
}

async fn resolve_workspace_publication<'a>(
    mut recovery: Box<dyn ActiveProfileWorkspaceRecoveryLease + 'a>,
) -> ApplyResult<Box<dyn ActiveProfileWorkspacePublishedLease + 'a>> {
    loop {
        match recovery.resolve().await {
            ActiveProfileWorkspaceRecoveryResolution::Published(published) => {
                return Ok(published);
            }
            ActiveProfileWorkspaceRecoveryResolution::Restored(restored) => {
                let _ = restored.finish();
                return Err(ApplyError::SourceChanged);
            }
            ActiveProfileWorkspaceRecoveryResolution::Unpublished(error) => {
                return Err(persistence_error(error));
            }
            ActiveProfileWorkspaceRecoveryResolution::Superseded { .. } => {
                return Err(ApplyError::SourceChanged);
            }
            ActiveProfileWorkspaceRecoveryResolution::RecoveryRequired(next) => {
                recovery = next;
            }
        }
    }
}

async fn restore_published_workspace<'a>(
    mut published: Box<dyn ActiveProfileWorkspacePublishedLease + 'a>,
) -> ApplyResult<Box<dyn ActiveProfileWorkspaceRestoredLease + 'a>> {
    loop {
        let outcome = published
            .restore_if_current()
            .await
            .map_err(persistence_error)?;
        loop {
            match outcome {
                ActiveProfileWorkspaceRestoreOutcome::Restored(restored) => {
                    return Ok(restored);
                }
                ActiveProfileWorkspaceRestoreOutcome::Superseded { .. } => {
                    return Err(ApplyError::SourceChanged);
                }
                ActiveProfileWorkspaceRestoreOutcome::RecoveryRequired(mut recovery) => loop {
                    match recovery.resolve().await {
                        ActiveProfileWorkspaceRecoveryResolution::Restored(restored) => {
                            return Ok(restored);
                        }
                        ActiveProfileWorkspaceRecoveryResolution::Published(next_published) => {
                            published = next_published;
                            break;
                        }
                        ActiveProfileWorkspaceRecoveryResolution::Unpublished(error) => {
                            return Err(persistence_error(error));
                        }
                        ActiveProfileWorkspaceRecoveryResolution::Superseded { .. } => {
                            return Err(ApplyError::SourceChanged);
                        }
                        ActiveProfileWorkspaceRecoveryResolution::RecoveryRequired(next) => {
                            recovery = next;
                        }
                    }
                },
            }
            break;
        }
    }
}

fn rollback_failed<S: SecureStore>(
    config: &ConfigManager<S>,
    profile: &str,
    cause: String,
    rollback: String,
) -> ApplyError {
    config.record_apply_transaction(ApplyTransactionSnapshot::rollback_failed(
        profile, &cause, &rollback,
    ));
    ApplyError::RollbackFailed { cause, rollback }
}

fn persistence_error(error: PortError) -> ApplyError {
    match error.error_code() {
        ErrorCode::NotReady => ApplyError::SourceChanged,
        ErrorCode::Permission => ApplyError::Permission(error.to_string()),
        ErrorCode::Configuration | ErrorCode::InvalidInput => {
            ApplyError::Validation(error.to_string())
        }
        ErrorCode::InvalidState => ApplyError::RollbackFailed {
            cause: "profile/options publication failed".into(),
            rollback: error.to_string(),
        },
        _ => ApplyError::Write(error.to_string()),
    }
}

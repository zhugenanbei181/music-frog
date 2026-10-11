//! Source-bound profile/options application and conditional exact-byte recovery.
use crate::apply::{
    ApplyError, ApplyParams, ApplyResult, ConfigReloader, reload_and_check, restart_and_check,
    start_and_check, validate_config,
};
use crate::history::{DEFAULT_KEEP, prune_snapshots, save_snapshot};
use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use infiltrator_ports::secure_store::SecureStore;
use mihomo_config::manager::ConfigManager;
use std::str;

pub(crate) async fn apply_confirmed_workspace<S: SecureStore, C: CoreLifecyclePort + ?Sized>(
    session: &C,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    expected: &ProfileWorkspaceIdentity,
    update: &ProfileWorkspaceUpdate,
    params: ApplyParams,
) -> ApplyResult<ProfileWorkspace> {
    apply_confirmed_workspace_inner(
        session,
        config,
        reloader,
        expected,
        update,
        params,
        WorkspaceCandidateValidation::DomainYaml,
    )
    .await
}

pub(crate) async fn apply_codec_validated_workspace<
    S: SecureStore,
    C: CoreLifecyclePort + ?Sized,
>(
    session: &C,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    expected: &ProfileWorkspaceIdentity,
    update: &ProfileWorkspaceUpdate,
    params: ApplyParams,
) -> ApplyResult<ProfileWorkspace> {
    apply_confirmed_workspace_inner(
        session,
        config,
        reloader,
        expected,
        update,
        params,
        WorkspaceCandidateValidation::CodecInspectionAccepted,
    )
    .await
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
    expected: &ProfileWorkspaceIdentity,
    update: &ProfileWorkspaceUpdate,
    params: ApplyParams,
    validation: WorkspaceCandidateValidation,
) -> ApplyResult<ProfileWorkspace> {
    let profile = expected.document.profile_id.as_str();
    let store_engine = config.engine_id();
    let runtime_engine = session
        .core_identity()
        .map(|identity| identity.engine)
        .ok_or(ApplyError::SourceChanged)?;
    if expected.document.engine_id != store_engine || expected.document.engine_id != runtime_engine
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
    if matches!(status, CoreLifecycle::Starting | CoreLifecycle::Stopping) {
        return Err(ApplyError::Busy { status });
    }
    let was_running = matches!(status, CoreLifecycle::Ready | CoreLifecycle::Running);
    if matches!(
        validation,
        WorkspaceCandidateValidation::CodecInspectionAccepted
    ) && !was_running
    {
        // Codec-backed configuration transactions own the stopped/failed
        // persistence path separately. If lifecycle changed after their
        // preflight, fail before CAS instead of implicitly starting Core.
        return Err(ApplyError::Busy { status });
    }
    let previous = config
        .load_active_workspace()
        .await
        .map_err(persistence_error)?;
    if previous.identity() != *expected {
        return Err(ApplyError::SourceChanged);
    }
    let path = config
        .existing_profile_yaml_path(profile)
        .await
        .map_err(|error| ApplyError::Write(error.to_string()))?;
    let committed = config
        .compare_and_save_active_workspace(expected, update)
        .await
        .map_err(persistence_error)?;
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
        if let Err(rollback) = config
            .restore_workspace(&committed_identity, &previous)
            .await
        {
            return Err(rollback_failed(
                config,
                profile,
                cause,
                rollback.to_string(),
            ));
        }
        let restored = config
            .load_active_workspace()
            .await
            .map_err(|error| rollback_failed(config, profile, cause.clone(), error.to_string()))?;
        if restored != previous {
            return Err(rollback_failed(
                config,
                profile,
                cause,
                "profile store did not read back the exact previous workspace".into(),
            ));
        }
        config.record_apply_transaction(ApplyTransactionSnapshot::rolled_back(profile, &cause));
        return Err(ApplyError::RolledBack { cause });
    }
    let outcome = if !was_running {
        start_and_check(session, &params).await
    } else if params.strategy == ApplyStrategy::PreferReload {
        match reload_and_check(session, reloader, &path, &params).await {
            Ok(outcome) => Ok(outcome),
            Err(cause) => {
                log::warn!("workspace reload failed, restarting: {cause}");
                restart_and_check(session, &params).await
            }
        }
    } else {
        restart_and_check(session, &params).await
    };
    match outcome {
        Ok(outcome) => {
            let observed = config.load_active_workspace().await.map_err(|error| {
                rollback_failed(
                    config,
                    profile,
                    "Cannot verify applied workspace".into(),
                    error.to_string(),
                )
            })?;
            if observed != committed {
                return Err(rollback_failed(
                    config,
                    profile,
                    "Workspace changed during runtime application".into(),
                    "Later edits are preserved; inspect the current runtime and profile".into(),
                ));
            }
            if params.snapshot_history {
                let directory = config.config_dir();
                let committed_text = committed
                    .document
                    .utf8_text()
                    .map_err(|error| ApplyError::Write(error.to_string()))?;
                match save_snapshot(&config.engine_id(), directory, profile, committed_text).await {
                    Ok(_) => {
                        if let Err(error) =
                            prune_snapshots(&config.engine_id(), directory, profile, DEFAULT_KEEP)
                                .await
                        {
                            log::warn!("workspace snapshot prune failed: {error}");
                        }
                    }
                    Err(error) => log::warn!("workspace snapshot save failed: {error}"),
                }
            }
            config.record_apply_transaction(ApplyTransactionSnapshot::committed(
                profile,
                outcome.method.as_str(),
            ));
            Ok(committed)
        }
        Err(cause) => {
            let cause = cause.to_string();
            if let Err(rollback) = config
                .restore_workspace(&committed.identity(), &previous)
                .await
            {
                return Err(rollback_failed(
                    config,
                    profile,
                    cause,
                    rollback.to_string(),
                ));
            }
            let restored = config.load_active_workspace().await.map_err(|error| {
                rollback_failed(config, profile, cause.clone(), error.to_string())
            })?;
            if restored != previous {
                return Err(rollback_failed(
                    config,
                    profile,
                    cause,
                    "Profile store did not read back the exact previous workspace".into(),
                ));
            }
            if was_running {
                if let Err(rollback) = restart_and_check(session, &params).await {
                    return Err(rollback_failed(
                        config,
                        profile,
                        cause,
                        rollback.to_string(),
                    ));
                }
                let active = config.load_active_workspace().await.map_err(|error| {
                    rollback_failed(config, profile, cause.clone(), error.to_string())
                })?;
                if active != previous {
                    return Err(rollback_failed(
                        config,
                        profile,
                        cause,
                        "Profile changed during runtime recovery".into(),
                    ));
                }
            }
            config.record_apply_transaction(ApplyTransactionSnapshot::rolled_back(profile, &cause));
            Err(ApplyError::RolledBack { cause })
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

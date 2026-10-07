//! Source-bound profile/options application and conditional exact-byte recovery.
use crate::apply::{
    ApplyError, ApplyParams, ApplyResult, ConfigReloader, reload_and_check, restart_and_check,
    start_and_check, validate_config,
};
use crate::history::{DEFAULT_KEEP, prune_snapshots, save_snapshot};
use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::profile_source::ProfileSourceIdentity;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use infiltrator_ports::secure_store::SecureStore;
use mihomo_config::manager::ConfigManager;

pub async fn apply_confirmed_workspace<S: SecureStore>(
    session: &impl CoreLifecyclePort,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    expected: &ProfileSourceIdentity,
    update: &ProfileWorkspaceUpdate,
    params: ApplyParams,
) -> ApplyResult<ProfileWorkspace> {
    validate_config(&update.content).map_err(ApplyError::Validation)?;
    let status = session.lifecycle();
    if matches!(status, CoreLifecycle::Starting | CoreLifecycle::Stopping) {
        return Err(ApplyError::Busy { status });
    }
    let was_running = matches!(status, CoreLifecycle::Ready | CoreLifecycle::Running);
    let previous = config
        .load_active_workspace()
        .await
        .map_err(persistence_error)?;
    if previous.source != *expected {
        return Err(ApplyError::SourceChanged);
    }
    let path = config
        .existing_profile_yaml_path(&expected.profile)
        .await
        .map_err(|error| ApplyError::Write(error.to_string()))?;
    let committed = config
        .compare_and_save_active_workspace(expected, update)
        .await
        .map_err(persistence_error)?;
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
                    &expected.profile,
                    "Cannot verify applied workspace".into(),
                    error.to_string(),
                )
            })?;
            if observed.source != committed.source {
                config.record_apply_transaction(ApplyTransactionSnapshot::rollback_failed(
                    &expected.profile,
                    "Workspace changed during runtime application",
                    "Later edits are preserved; inspect the current runtime and profile",
                ));
                return Err(ApplyError::SourceChanged);
            }
            if params.snapshot_history {
                let directory = config.config_dir();
                match save_snapshot(directory, &expected.profile, &committed.content).await {
                    Ok(_) => {
                        if let Err(error) =
                            prune_snapshots(directory, &expected.profile, DEFAULT_KEEP).await
                        {
                            log::warn!("workspace snapshot prune failed: {error}");
                        }
                    }
                    Err(error) => log::warn!("workspace snapshot save failed: {error}"),
                }
            }
            config.record_apply_transaction(ApplyTransactionSnapshot::committed(
                &expected.profile,
                outcome.method.as_str(),
            ));
            Ok(committed)
        }
        Err(cause) => {
            let cause = cause.to_string();
            if let Err(rollback) = config.restore_workspace(&committed.source, &previous).await {
                return Err(rollback_failed(
                    config,
                    &expected.profile,
                    cause,
                    rollback.to_string(),
                ));
            }
            if was_running {
                let active = config.load_active_workspace().await.map_err(|error| {
                    rollback_failed(config, &expected.profile, cause.clone(), error.to_string())
                })?;
                if active.source != previous.source {
                    return Err(rollback_failed(
                        config,
                        &expected.profile,
                        cause,
                        "Active profile changed before runtime recovery".into(),
                    ));
                }
                if let Err(rollback) = restart_and_check(session, &params).await {
                    return Err(rollback_failed(
                        config,
                        &expected.profile,
                        cause,
                        rollback.to_string(),
                    ));
                }
                let active = config.load_active_workspace().await.map_err(|error| {
                    rollback_failed(config, &expected.profile, cause.clone(), error.to_string())
                })?;
                if active.source != previous.source {
                    return Err(rollback_failed(
                        config,
                        &expected.profile,
                        cause,
                        "Profile changed during runtime recovery".into(),
                    ));
                }
            }
            config.record_apply_transaction(ApplyTransactionSnapshot::rolled_back(
                &expected.profile,
                &cause,
            ));
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

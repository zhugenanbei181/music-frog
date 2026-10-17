//! Exact-reference Core apply and source-bound workspace edit helpers.
//!
//! [`apply_active_profile_reference`] applies the profile named by one exact
//! active-reference revision without changing stored content. Profile edits
//! use the workspace identity and lease in `apply_workspace`, which holds the
//! reference and content fences through native readback and conditional
//! recovery. [`apply_active_profile_edit`], [`apply_active_profile_set_scalar`],
//! [`apply_active_profile_append_rule`], [`apply_active_profile_remove_rule`],
//! [`apply_active_profile_rewrite_anchors`], and [`apply_active_profile_mixin_fidelity`]
//! apply edits through the same workspace transaction so comments, anchors,
//! aliases, and formatting remain byte-faithful where the domain editor allows.

use crate::apply_workspace::apply_active_profile_workspace_with_receipt;
use crate::history::{DEFAULT_KEEP, prune_snapshots, save_snapshot};
use async_trait::async_trait;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceState, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::mixin::{MixinConfig, merge_profile_with_config};
use infiltrator_domain::yaml_edit::mixin_fidelity::{
    apply_mixin_to_doc, can_apply_mixin_via_fidelity,
};
use infiltrator_domain::yaml_edit::{SourceDoc, YamlEditError};
use infiltrator_ports::active_profile_reference_store::{
    ActiveProfileReferenceLease, ActiveProfileReferenceStore,
};
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::endpoint::EndpointSource;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::error::{ErrorCode, Failure, FailureReason};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::owner_switch::CoreApplyFailureKind;
use lurker_core_contract::session::SessionToken;
use mihomo_api::client::MihomoClient;
use mihomo_api::error::MihomoError;
use mihomo_config::manager::ConfigManager;
use std::io::ErrorKind;
use std::path::Path;
use std::result;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use yaml_rust2::{Yaml, YamlLoader};

const DEFAULT_RESTART_TIMEOUT: Duration = Duration::from_secs(15);

/// Tunables for revision-bound profile apply transactions.
#[derive(Clone, Copy, Debug)]
pub struct ApplyParams {
    pub strategy: ApplyStrategy,
    /// Probe budget after a hot reload: the controller should answer quickly
    /// because the process itself was never restarted.
    pub health_timeout: Duration,
    /// Probe budget after a restart: covers a full mihomo boot.
    pub restart_timeout: Duration,
    /// Store the newly live content as a timestamped snapshot on success
    /// (config snapshot history, [缺口13]). Keep the default unless the
    /// caller manages history itself.
    pub snapshot_history: bool,
}

impl Default for ApplyParams {
    fn default() -> Self {
        Self {
            strategy: ApplyStrategy::PreferReload,
            health_timeout: Duration::from_secs(5),
            restart_timeout: DEFAULT_RESTART_TIMEOUT,
            snapshot_history: true,
        }
    }
}

/// How the new configuration actually became live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyMethod {
    /// Controller hot-reload; the process instance is unchanged, so the
    /// session generation is unchanged.
    HotReload,
    /// Process restart (or first start); a new generation was produced.
    Restart,
}

impl ApplyMethod {
    /// Wire name used by the shared apply-transaction read model.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HotReload => "hot_reload",
            Self::Restart => "restart",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplyOutcome {
    pub method: ApplyMethod,
    /// Session generation the caller must capture for subsequent work.
    pub generation: u64,
    /// Session identity paired with the generation. A hot reload keeps it;
    /// a restart returns the newly-created one.
    pub session_token: SessionToken,
}

/// Errors with transaction semantics. `RolledBack` means the new config is
/// *not* live but the previous known-good config was restored and the core
/// is healthy again; `RollbackFailed` means the caller must treat the whole
/// session as broken and surface both failures.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ApplyError {
    #[error("invalid profile content: {0}")]
    Validation(String),
    #[error("failed to write config atomically: {0}")]
    Write(String),
    #[error("profile transaction permission denied: {0}")]
    Permission(String),
    #[error("source profile changed; inspect and confirm the current document again")]
    SourceChanged,
    #[error(
        "active profile reference changed from revision {expected_revision} to {observed_revision}"
    )]
    ActiveProfileReferenceChanged {
        expected_revision: u64,
        observed_revision: u64,
    },
    #[error("Core is {status:?}; profile apply requires a running Core")]
    NotRunning { status: CoreLifecycle },
    #[error("core is {status:?}; wait for the transition to finish before applying")]
    Busy { status: CoreLifecycle },
    #[error("apply failed, previous config restored and core recovered: {cause}")]
    RolledBack { cause: String },
    #[error("apply failed ({cause}) and rollback failed as well; core is down: {rollback}")]
    RollbackFailed { cause: String, rollback: String },
    #[error("hot reload failed ({reload}) and restart recovery failed ({restart})")]
    ReloadAndRestartFailed { reload: String, restart: String },
    #[error("core lifecycle failed: {0}")]
    Lifecycle(String),
}

/// Preserve transaction semantics when a host adapter translates an apply
/// error into the runtime-neutral port error type.
pub fn failure_from_apply_error(error: ApplyError) -> Failure {
    let message = error.to_string();
    let (code, retryable, reason) = match error {
        ApplyError::Validation(_) => (
            ErrorCode::Configuration,
            false,
            FailureReason::CoreApplyFailure(CoreApplyFailureKind::Validation),
        ),
        ApplyError::Write(_) => (
            ErrorCode::Storage,
            false,
            FailureReason::CoreApplyFailure(CoreApplyFailureKind::Write),
        ),
        ApplyError::Permission(_) => (
            ErrorCode::Permission,
            false,
            FailureReason::CoreApplyFailure(CoreApplyFailureKind::Permission),
        ),
        ApplyError::SourceChanged => (
            ErrorCode::NotReady,
            true,
            FailureReason::CoreApplyFailure(CoreApplyFailureKind::SourceChanged),
        ),
        ApplyError::ActiveProfileReferenceChanged {
            expected_revision,
            observed_revision,
        } => {
            return Failure::new(ErrorCode::NotReady, message, true).with_reason(
                FailureReason::ActiveProfileReferenceStale {
                    expected_revision,
                    observed_revision,
                },
            );
        }
        ApplyError::NotRunning { .. } => (
            ErrorCode::NotReady,
            true,
            FailureReason::CoreApplyFailure(CoreApplyFailureKind::InactiveCore),
        ),
        ApplyError::Busy { .. } => (
            ErrorCode::InvalidState,
            true,
            FailureReason::CoreApplyFailure(CoreApplyFailureKind::Busy),
        ),
        ApplyError::RolledBack { .. } => (
            ErrorCode::InvalidState,
            false,
            FailureReason::CoreApplyFailure(CoreApplyFailureKind::RolledBack),
        ),
        ApplyError::RollbackFailed { .. } => (
            ErrorCode::InvalidState,
            false,
            FailureReason::CoreApplyFailure(CoreApplyFailureKind::RollbackFailed),
        ),
        ApplyError::Lifecycle(_) | ApplyError::ReloadAndRestartFailed { .. } => (
            ErrorCode::InvalidState,
            false,
            FailureReason::CoreApplyFailure(CoreApplyFailureKind::Lifecycle),
        ),
    };
    Failure::new(code, message, retryable).with_reason(reason)
}

pub type ApplyResult<T> = result::Result<T, ApplyError>;

/// Asks a *running* core to load the config file at `path` without a process
/// restart. Abstracted so ordinary tests exercise the transaction with a
/// mock instead of a live controller (QA-002).
#[async_trait]
pub trait ConfigReloader: Send + Sync {
    /// Resolve all reload dependencies before the transaction acquires its
    /// active-profile/workspace locks. The returned session is bound to the
    /// endpoint observed for this transaction and performs no profile lookup.
    async fn prepare(&self) -> Result<Box<dyn PreparedConfigReload + '_>, String>;
}

#[async_trait]
pub trait PreparedConfigReload: Send + Sync {
    /// Apply the profile through the backend's native live reload operation.
    async fn reload(&self, path: &Path) -> Result<(), String>;
}

#[async_trait]
impl<T> PreparedConfigReload for &T
where
    T: PreparedConfigReload + Sync + ?Sized,
{
    async fn reload(&self, path: &Path) -> Result<(), String> {
        (**self).reload(path).await
    }
}

pub(crate) enum PreparedApplyStrategy<'a> {
    PreferReload(Result<Box<dyn PreparedConfigReload + 'a>, String>),
    AlwaysRestart,
}

pub(crate) async fn prepare_apply_strategy<'a>(
    strategy: ApplyStrategy,
    reloader: &'a dyn ConfigReloader,
) -> PreparedApplyStrategy<'a> {
    match strategy {
        ApplyStrategy::PreferReload => {
            PreparedApplyStrategy::PreferReload(reloader.prepare().await)
        }
        ApplyStrategy::AlwaysRestart => PreparedApplyStrategy::AlwaysRestart,
    }
}

/// Production reloader: resolve the current endpoint before the active
/// workspace lease, then return a session bound to Mihomo's documented
/// `PUT /configs?force=true` operation.
pub struct EndpointConfigReloader {
    endpoints: Arc<dyn EndpointSource>,
}

impl EndpointConfigReloader {
    pub fn new(endpoints: Arc<dyn EndpointSource>) -> Self {
        Self { endpoints }
    }
}

#[async_trait]
impl ConfigReloader for EndpointConfigReloader {
    async fn prepare(&self) -> Result<Box<dyn PreparedConfigReload + '_>, String> {
        let endpoint = self
            .endpoints
            .resolve()
            .await
            .map_err(|err| err.to_string())?;
        let client = MihomoClient::new(&endpoint.url, endpoint.secret)
            .map_err(|err| format!("controller client: {err}"))?;
        Ok(Box::new(EndpointPreparedConfigReload { client }))
    }
}

struct EndpointPreparedConfigReload {
    client: MihomoClient,
}

#[async_trait]
impl PreparedConfigReload for EndpointPreparedConfigReload {
    async fn reload(&self, path: &Path) -> Result<(), String> {
        let path = path
            .to_str()
            .ok_or_else(|| "config path is not valid UTF-8".to_string())?;
        self.client
            .reload_config(Some(path))
            .await
            .map_err(|err| format!("reload rejected: {err}"))
    }
}

/// Lightweight structural validation performed before any disk write. Deep
/// validation is the core's job: if mihomo rejects the file on reload or
/// boot, the transaction rolls back — that is the safety net, not this check.
pub(crate) fn validate_config(content: &str) -> Result<(), String> {
    let docs =
        YamlLoader::load_from_str(content).map_err(|err| format!("YAML parse failed: {err}"))?;
    match docs.first() {
        Some(Yaml::Hash(_)) => Ok(()),
        Some(_) => Err("top-level YAML document must be a mapping".to_string()),
        None => Err("config is empty".to_string()),
    }
}

/// Hot-reload path: keep the process, reload, then prove the controller is
/// still healthy. Returns `Err(cause)` when either step fails; the caller
/// falls back to the restart path without rolling back yet.
pub(crate) async fn reload_and_check<C: CoreLifecyclePort + ?Sized>(
    session: &C,
    reloader: &dyn PreparedConfigReload,
    path: &Path,
    params: &ApplyParams,
) -> Result<ApplyOutcome, String> {
    // Mihomo's documented `/configs` reload uses `force=true`; the engine
    // validates the published candidate during that operation. The caller
    // keeps the workspace lease and restores the previous bytes if reload or
    // readiness fails.
    let session_token = session
        .begin_reload()
        .map_err(|error| format!("core cannot begin hot reload: {error}"))?;
    if let Err(error) = reloader.reload(path).await {
        let _ = session.fail_reload(session_token, error.clone());
        return Err(error);
    }
    let generation = session.generation();
    if let Err(error) = session
        .wait_for_ready_session(generation, session_token, params.health_timeout)
        .await
    {
        let cause = format!("core unhealthy after reload: {error}");
        let _ = session.fail_reload(session_token, cause.clone());
        return Err(cause);
    }
    session
        .complete_reload(session_token)
        .map_err(|error| format!("core reload completion was rejected: {error}"))?;
    Ok(ApplyOutcome {
        method: ApplyMethod::HotReload,
        generation,
        session_token,
    })
}

pub(crate) async fn apply_prepared_strategy<C: CoreLifecyclePort + ?Sized>(
    session: &C,
    prepared: &PreparedApplyStrategy<'_>,
    path: &Path,
    params: &ApplyParams,
) -> Result<ApplyOutcome, ApplyError> {
    match prepared {
        PreparedApplyStrategy::PreferReload(Ok(reloader)) => {
            match reload_and_check(session, reloader.as_ref(), path, params).await {
                Ok(outcome) => Ok(outcome),
                Err(cause) => {
                    log::warn!("hot reload failed, restarting: {cause}");
                    match restart_and_check(session, params).await {
                        Ok(outcome) => Ok(outcome),
                        Err(restart) => Err(ApplyError::ReloadAndRestartFailed {
                            reload: cause,
                            restart: restart.to_string(),
                        }),
                    }
                }
            }
        }
        PreparedApplyStrategy::PreferReload(Err(cause)) => {
            log::warn!("hot reload preparation failed, restarting: {cause}");
            restart_and_check(session, params).await
        }
        PreparedApplyStrategy::AlwaysRestart => restart_and_check(session, params).await,
    }
}

pub(crate) async fn restart_and_check<C: CoreLifecyclePort + ?Sized>(
    session: &C,
    params: &ApplyParams,
) -> Result<ApplyOutcome, ApplyError> {
    let generation = session
        .restart()
        .await
        .map_err(|error| ApplyError::Lifecycle(error.to_string()))?;
    let session_token = session.session_token().ok_or_else(|| {
        ApplyError::Lifecycle("core session token is unavailable after restart".to_string())
    })?;
    session
        .wait_for_ready_session(generation, session_token, params.restart_timeout)
        .await
        .map_err(|error| ApplyError::Lifecycle(error.to_string()))?;
    Ok(ApplyOutcome {
        method: ApplyMethod::Restart,
        generation,
        session_token,
    })
}

/// Apply the exact profile named by a committed product-wide active-profile
/// reference. The selection lease stays alive through write, apply, and any
/// rollback so a concurrent profile switch cannot redirect this transaction.
pub async fn apply_active_profile_reference<S: SecureStore>(
    session: &impl CoreLifecyclePort,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    expected: &ActiveProfileReference,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    if expected.engine_id != config.engine_id() {
        return Err(ApplyError::SourceChanged);
    }
    let prepared = prepare_apply_strategy(params.strategy, reloader).await;
    let lease = acquire_exact_selection_lease(config, expected).await?;
    apply_profile_transaction(session, config, expected, lease, &prepared, params).await
}

async fn acquire_exact_selection_lease<S: SecureStore>(
    config: &ConfigManager<S>,
    expected: &ActiveProfileReference,
) -> ApplyResult<Box<dyn ActiveProfileReferenceLease>> {
    if expected.engine_id != config.engine_id() {
        return Err(ApplyError::SourceChanged);
    }
    let profile_store = config.engine_profile_store();
    let lease = ActiveProfileReferenceStore::acquire_selection_lease(profile_store)
        .await
        .map_err(|error| ApplyError::Write(error.to_string()))?;
    if lease.state() != &ActiveProfileReferenceState::Selected(expected.clone()) {
        return Err(ApplyError::ActiveProfileReferenceChanged {
            expected_revision: expected.revision.value(),
            observed_revision: lease.state().revision().value(),
        });
    }
    Ok(lease)
}

async fn apply_profile_transaction<S: SecureStore>(
    session: &impl CoreLifecyclePort,
    config: &ConfigManager<S>,
    expected: &ActiveProfileReference,
    _selection_lease: Box<dyn ActiveProfileReferenceLease>,
    prepared: &PreparedApplyStrategy<'_>,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    config
        .initialize_profile_namespace()
        .await
        .map_err(config_apply_error)?;

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

    let write_guard = config
        .lock_profile_writes()
        .await
        .map_err(config_apply_error)?;
    let current = expected.profile_id.as_str().to_owned();
    let path = config
        .existing_profile_document_path(&current)
        .await
        .map_err(config_apply_error)?;
    let observed = fs::read_to_string(&path)
        .await
        .map_err(|error| config_apply_error(error.into()))?;
    validate_config(&observed).map_err(ApplyError::Validation)?;
    drop(write_guard);

    let outcome = apply_prepared_strategy(session, prepared, &path, &params).await;

    let cause = match outcome {
        Ok(outcome) => {
            if params.snapshot_history
                && let Some(config_dir) = path.parent()
            {
                match save_snapshot(&config.engine_id(), config_dir, &current, &observed).await {
                    Ok(meta) => {
                        log::info!("config snapshot stored: {}", meta.path.display());
                        if let Err(err) =
                            prune_snapshots(&config.engine_id(), config_dir, &current, DEFAULT_KEEP)
                                .await
                        {
                            log::warn!("snapshot prune failed: {err}");
                        }
                    }
                    Err(err) => log::warn!("config snapshot save failed: {err}"),
                }
            }
            // DUAL-09-11: both surfaces read the same typed outcome instead of
            // inferring a rollback from an error string.
            config.record_apply_transaction(ApplyTransactionSnapshot::committed(
                &current,
                outcome.method.as_str(),
            ));
            return Ok(outcome);
        }
        Err(failure) => failure.to_string(),
    };

    // `status` was checked before the candidate was written. A failed
    // candidate restart may leave the lifecycle in `Failed`, but rollback
    // still has to restart the restored bytes to recover the previously
    // active session. Do not infer whether recovery is needed from this
    // post-failure state.
    match restart_and_check(session, &params).await {
        Ok(_) => {
            config
                .record_apply_transaction(ApplyTransactionSnapshot::rolled_back(&current, &cause));
            Err(ApplyError::RolledBack { cause })
        }
        Err(err) => {
            let rollback = err.to_string();
            config.record_apply_transaction(ApplyTransactionSnapshot::rollback_failed(
                &current, &cause, &rollback,
            ));
            Err(ApplyError::RollbackFailed { cause, rollback })
        }
    }
}

fn config_apply_error(error: MihomoError) -> ApplyError {
    match error {
        MihomoError::Io(error) if error.kind() == ErrorKind::PermissionDenied => {
            ApplyError::Permission(error.to_string())
        }
        other => ApplyError::Write(other.to_string()),
    }
}

// ---- SourceDoc Fidelity Track Integration ----------------------------------

/// Apply a text-level [`SourceDoc`] edit to one exact active profile. The
/// coherent workspace identity is captured before running the pure edit and is
/// passed to the same fenced transaction that publishes, applies, and recovers
/// the complete workspace.
pub async fn apply_active_profile_edit<S: SecureStore, F>(
    session: &impl CoreLifecyclePort,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    expected: &ActiveProfileReference,
    authorization: ActiveProfileReferenceWriteAuthorization,
    edit_fn: F,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome>
where
    F: FnOnce(&mut SourceDoc) -> Result<(), YamlEditError>,
{
    let workspace = load_expected_workspace(config, expected).await?;
    let content = workspace
        .document
        .utf8_text()
        .map_err(|error| ApplyError::Validation(error.to_string()))?;
    let mut doc = SourceDoc::parse(content).map_err(|error| {
        ApplyError::Validation(format!("failed to parse profile for editing: {error}"))
    })?;
    edit_fn(&mut doc)
        .map_err(|error| ApplyError::Validation(format!("YAML edit failed: {error}")))?;
    apply_workspace_document(
        session,
        config,
        reloader,
        expected,
        &workspace,
        doc.render(),
        authorization,
        params,
    )
    .await
}

async fn load_expected_workspace<S: SecureStore>(
    config: &ConfigManager<S>,
    expected: &ActiveProfileReference,
) -> ApplyResult<ProfileWorkspace> {
    if expected.engine_id != config.engine_id() {
        return Err(ApplyError::SourceChanged);
    }
    let workspace =
        ProfileStore::load_workspace(config.engine_profile_store(), &expected.profile_id)
            .await
            .map_err(|error| ApplyError::Write(error.to_string()))?;
    if workspace.document.identity.engine_id != expected.engine_id
        || workspace.document.identity.profile_id != expected.profile_id
    {
        return Err(ApplyError::SourceChanged);
    }
    Ok(workspace)
}

async fn apply_workspace_document<S: SecureStore>(
    session: &impl CoreLifecyclePort,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    expected: &ActiveProfileReference,
    previous: &ProfileWorkspace,
    document: String,
    authorization: ActiveProfileReferenceWriteAuthorization,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    let expected_workspace = previous.identity();
    let update = ProfileWorkspaceUpdate {
        purpose: ProfileWorkspacePurpose::Derived,
        document_bytes: document.into_bytes(),
        sidecar_bytes: previous.sidecar_bytes.clone(),
    };
    apply_active_profile_workspace_with_receipt(
        session,
        config,
        reloader,
        expected,
        &expected_workspace,
        &update,
        authorization,
        params,
    )
    .await
    .map(|receipt| receipt.apply)
}

/// Update a top-level scalar key while preserving comments, anchors, and layout.
pub async fn apply_active_profile_set_scalar<S: SecureStore>(
    session: &impl CoreLifecyclePort,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    expected: &ActiveProfileReference,
    authorization: ActiveProfileReferenceWriteAuthorization,
    key: &str,
    value: &str,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    apply_active_profile_edit(
        session,
        config,
        reloader,
        expected,
        authorization,
        |doc| doc.set_top_scalar(key, value),
        params,
    )
    .await
}

/// Append a rule while preserving comments, anchors, and layout.
pub async fn apply_active_profile_append_rule<S: SecureStore>(
    session: &impl CoreLifecyclePort,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    expected: &ActiveProfileReference,
    authorization: ActiveProfileReferenceWriteAuthorization,
    rule: &str,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    apply_active_profile_edit(
        session,
        config,
        reloader,
        expected,
        authorization,
        |doc| doc.append_rule(rule),
        params,
    )
    .await
}

/// Remove a rule while preserving comments, anchors, and layout.
pub async fn apply_active_profile_remove_rule<S: SecureStore>(
    session: &impl CoreLifecyclePort,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    expected: &ActiveProfileReference,
    authorization: ActiveProfileReferenceWriteAuthorization,
    rule: &str,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    apply_active_profile_edit(
        session,
        config,
        reloader,
        expected,
        authorization,
        |doc| doc.remove_rule(rule),
        params,
    )
    .await
}

/// Rewrite anchor namespaces while preserving comments and layout.
pub async fn apply_active_profile_rewrite_anchors<S: SecureStore>(
    session: &impl CoreLifecyclePort,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    expected: &ActiveProfileReference,
    authorization: ActiveProfileReferenceWriteAuthorization,
    prefix: &str,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    apply_active_profile_edit(
        session,
        config,
        reloader,
        expected,
        authorization,
        |doc| {
            doc.rewrite_anchor_namespace(prefix)?;
            Ok(())
        },
        params,
    )
    .await
}

/// Apply a mixin through the fidelity path, with a structural merge fallback.
pub async fn apply_active_profile_mixin_fidelity<S: SecureStore>(
    session: &impl CoreLifecyclePort,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    expected: &ActiveProfileReference,
    authorization: ActiveProfileReferenceWriteAuthorization,
    mixin: &MixinConfig,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    let workspace = load_expected_workspace(config, expected).await?;
    let content = workspace
        .document
        .utf8_text()
        .map_err(|error| ApplyError::Validation(error.to_string()))?;
    let new_content = if can_apply_mixin_via_fidelity(mixin)
        && let Ok(mut doc) = SourceDoc::parse(content)
        && apply_mixin_to_doc(&mut doc, mixin).is_ok()
    {
        doc.render()
    } else {
        merge_profile_with_config(content, mixin)
            .map_err(|error| ApplyError::Validation(format!("mixin merge failed: {error}")))?
    };
    apply_workspace_document(
        session,
        config,
        reloader,
        expected,
        &workspace,
        new_content,
        authorization,
        params,
    )
    .await
}

// The apply transaction publishes a process-wide typed outcome, so its tests
// serialize through a lock; they run on a current-thread runtime, so holding
// the guard across an await cannot deadlock.
#[cfg(test)]
#[path = "apply_test.rs"]
mod apply_test;

#[cfg(test)]
#[path = "apply_reload_tests.rs"]
mod apply_reload_tests;

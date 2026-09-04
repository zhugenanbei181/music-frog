//! Config apply transaction (CORE-004) with SourceDoc YAML fidelity track.
//!
//! [`apply_current_profile`] replaces the content of the *current* profile
//! and makes the running core pick it up as one all-or-nothing step:
//! validate → temp write → atomic replace → hot-reload or restart via
//! [`CoreLifecyclePort`] → readiness health check → rollback to the previous
//! content on failure. A failed apply must never leave the core without a
//! usable configuration and must never strand the caller in a half-updated
//! state (the bug this transaction exists to fix: switching profiles used to
//! leave the old config running).
//!
//! [`apply_profile_edit`], [`apply_profile_set_scalar`], [`apply_profile_append_rule`],
//! [`apply_profile_remove_rule`], [`apply_profile_rewrite_anchors`], and
//! [`apply_profile_mixin_fidelity`] integrates the [`infiltrator_domain::yaml_edit::SourceDoc`]
//! fidelity track directly into this transaction so that hand-annotated comments,
//! anchors (`&anchor`), alias references (`*alias`), and custom formatting are 100%
//! preserved during configuration edits and profile switches.

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::client::MihomoClient;
use mihomo_config::manager::ConfigManager;
use tokio::io::AsyncWriteExt;
use yaml_rust2::{Yaml, YamlLoader};

use infiltrator_domain::yaml_edit::SourceDoc;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::endpoint::EndpointSource;

/// How the running core should pick up the new configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyStrategy {
    /// Try the controller hot-reload first (`PUT /configs?force=true`) and
    /// keep the process (and its connections) alive; fall back to a restart
    /// if the reload is rejected or the core turns unhealthy afterwards.
    PreferReload,
    /// Always restart the process. Required for changes hot-reload does not
    /// apply reliably (TUN device, DNS stack, listeners).
    AlwaysRestart,
}

const DEFAULT_RESTART_TIMEOUT: Duration = Duration::from_secs(15);

/// Tunables for [`apply_current_profile`].
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplyOutcome {
    pub method: ApplyMethod,
    /// Session generation the caller must capture for subsequent work.
    pub generation: u64,
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
    #[error("core is {status:?}; wait for the transition to finish before applying")]
    Busy { status: CoreLifecycle },
    #[error("apply failed, previous config restored and core recovered: {cause}")]
    RolledBack { cause: String },
    #[error("apply failed ({cause}) and rollback failed as well; core is down: {rollback}")]
    RollbackFailed { cause: String, rollback: String },
    #[error("core lifecycle failed: {0}")]
    Lifecycle(String),
}

pub type ApplyResult<T> = std::result::Result<T, ApplyError>;

/// Asks a *running* core to load the config file at `path` without a process
/// restart. Abstracted so ordinary tests exercise the transaction with a
/// mock instead of a live controller (QA-002).
#[async_trait]
pub trait ConfigReloader: Send + Sync {
    async fn reload(&self, path: &Path) -> Result<(), String>;
}

/// Production reloader: resolve the current endpoint per call (port rotation
/// safe), build a client, `PUT /configs?force=true` with the file path. It is
/// deliberately independent of the lifecycle owner so CoreApplication and
/// any lifecycle owner can share the same transaction.
pub struct EndpointConfigReloader {
    endpoints: std::sync::Arc<dyn EndpointSource>,
}

impl EndpointConfigReloader {
    pub fn new(endpoints: std::sync::Arc<dyn EndpointSource>) -> Self {
        Self { endpoints }
    }
}

#[async_trait]
impl ConfigReloader for EndpointConfigReloader {
    async fn reload(&self, path: &Path) -> Result<(), String> {
        let endpoint = self
            .endpoints
            .resolve()
            .await
            .map_err(|err| err.to_string())?;
        let client = MihomoClient::new(&endpoint.url, endpoint.secret)
            .map_err(|err| format!("controller client: {err}"))?;
        let path = path
            .to_str()
            .ok_or_else(|| "config path is not valid UTF-8".to_string())?;
        client
            .reload_config(Some(path))
            .await
            .map_err(|err| format!("reload rejected: {err}"))
    }
}

/// Lightweight structural validation performed before any disk write. Deep
/// validation is the core's job: if mihomo rejects the file on reload or
/// boot, the transaction rolls back — that is the safety net, not this check.
fn validate_config(content: &str) -> Result<(), String> {
    let docs =
        YamlLoader::load_from_str(content).map_err(|err| format!("YAML parse failed: {err}"))?;
    match docs.first() {
        Some(Yaml::Hash(_)) => Ok(()),
        Some(_) => Err("top-level YAML document must be a mapping".to_string()),
        None => Err("config is empty".to_string()),
    }
}

/// Write via a temp file in the same directory plus rename: interrupted
/// writes and crashes leave the previous config intact.
async fn atomic_write(path: &Path, content: &str) -> std::io::Result<()> {
    let dir = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "config path has no parent",
        )
    })?;
    let file_name = path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("config.yaml");
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let tmp = dir.join(format!(".{file_name}.tmp-{}-{nanos}", std::process::id()));

    let write = async {
        let mut file = tokio::fs::File::create(&tmp).await?;
        file.write_all(content.as_bytes()).await?;
        file.sync_all().await?;
        Ok::<_, std::io::Error>(())
    }
    .await;
    if let Err(err) = write {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(err);
    }
    if let Err(err) = tokio::fs::rename(&tmp, path).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(err);
    }
    Ok(())
}

/// Hot-reload path: keep the process, reload, then prove the controller is
/// still healthy. Returns `Err(cause)` when either step fails; the caller
/// falls back to the restart path without rolling back yet.
async fn reload_and_check(
    session: &impl CoreLifecyclePort,
    reloader: &dyn ConfigReloader,
    path: &Path,
    params: &ApplyParams,
) -> Result<ApplyOutcome, String> {
    reloader.reload(path).await?;
    let generation = session.generation();
    session
        .wait_for_ready(generation, params.health_timeout)
        .await
        .map_err(|err| format!("core unhealthy after reload: {err}"))?;
    Ok(ApplyOutcome {
        method: ApplyMethod::HotReload,
        generation,
    })
}

async fn restart_and_check(
    session: &impl CoreLifecyclePort,
    params: &ApplyParams,
) -> Result<ApplyOutcome, ApplyError> {
    let generation = session
        .restart()
        .await
        .map_err(|error| ApplyError::Lifecycle(error.to_string()))?;
    session
        .wait_for_ready(generation, params.restart_timeout)
        .await
        .map_err(|error| ApplyError::Lifecycle(error.to_string()))?;
    Ok(ApplyOutcome {
        method: ApplyMethod::Restart,
        generation,
    })
}

async fn start_and_check(
    session: &impl CoreLifecyclePort,
    params: &ApplyParams,
) -> Result<ApplyOutcome, ApplyError> {
    let generation = session
        .start()
        .await
        .map_err(|error| ApplyError::Lifecycle(error.to_string()))?;
    session
        .wait_for_ready(generation, params.restart_timeout)
        .await
        .map_err(|error| ApplyError::Lifecycle(error.to_string()))?;
    Ok(ApplyOutcome {
        method: ApplyMethod::Restart,
        generation,
    })
}

/// Apply `new_content` as the current profile through the full transaction.
///
/// Status handling:
/// - `Starting`/`Stopping`: rejected as [`ApplyError::Busy`]; retry after the
///   transition settles. This keeps concurrent applies from interleaving.
/// - `Ready`/`Running`: reload or restart per [`ApplyStrategy`].
/// - `Stopped`/`Failed`: content is written and the core is started with it.
///
/// On failure with a previous file on disk: the previous content is restored
/// atomically; if the core was running it is restarted on the restored
/// config before [`ApplyError::RolledBack`] is returned. If the core was not
/// running, restoring the file is enough — the session keeps its `Failed`
/// state and the next explicit start picks up the restored config.
pub async fn apply_current_profile<S: SecureStore>(
    session: &impl CoreLifecyclePort,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    new_content: &str,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    validate_config(new_content).map_err(ApplyError::Validation)?;

    let status = session.lifecycle();
    if matches!(status, CoreLifecycle::Starting | CoreLifecycle::Stopping) {
        return Err(ApplyError::Busy { status });
    }
    let was_running = matches!(status, CoreLifecycle::Ready | CoreLifecycle::Running);

    let current = config
        .get_current()
        .await
        .map_err(|err| ApplyError::Write(err.to_string()))?;
    let old_content = match config.load_backup(&current).await {
        Ok(Some(backup)) => Some(backup),
        Ok(None) | Err(_) => config.load(&current).await.ok(),
    };
    let path = config
        .get_current_path()
        .await
        .map_err(|err| ApplyError::Write(err.to_string()))?;

    atomic_write(&path, new_content)
        .await
        .map_err(|err| ApplyError::Write(format!("{}: {err}", path.display())))?;

    let outcome: Result<ApplyOutcome, String> = if !was_running {
        start_and_check(session, &params).await
    } else if params.strategy == ApplyStrategy::PreferReload {
        match reload_and_check(session, reloader, &path, &params).await {
            Ok(outcome) => Ok(outcome),
            Err(cause) => {
                log::warn!(
                    "hot reload of {} failed, restarting instead: {cause}",
                    path.display()
                );
                restart_and_check(session, &params).await
            }
        }
    } else {
        restart_and_check(session, &params).await
    }
    .map_err(|err| err.to_string());

    let cause = match outcome {
        Ok(outcome) => {
            if params.snapshot_history
                && let Some(config_dir) = path.parent()
            {
                let current = config.get_current().await.unwrap_or_default();
                match crate::history::save_snapshot(config_dir, &current, new_content).await {
                    Ok(meta) => {
                        log::info!("config snapshot stored: {}", meta.path.display());
                        if let Err(err) = crate::history::prune_snapshots(
                            config_dir,
                            &current,
                            crate::history::DEFAULT_KEEP,
                        )
                        .await
                        {
                            log::warn!("snapshot prune failed: {err}");
                        }
                    }
                    Err(err) => log::warn!("config snapshot save failed: {err}"),
                }
            }
            return Ok(outcome);
        }
        Err(failure) => failure,
    };

    // Rollback: restore the previous file; bring the core back if it had
    // been serving before this transaction.
    if let Some(old) = old_content {
        atomic_write(&path, &old)
            .await
            .map_err(|err| ApplyError::RollbackFailed {
                cause: cause.clone(),
                rollback: format!("restoring {}: {err}", path.display()),
            })?;
    } else {
        log::warn!(
            "no previous content for {}; rollback only marks the failure",
            path.display()
        );
    }

    if was_running {
        match restart_and_check(session, &params).await {
            Ok(_) => Err(ApplyError::RolledBack { cause }),
            Err(err) => Err(ApplyError::RollbackFailed {
                cause,
                rollback: err.to_string(),
            }),
        }
    } else {
        Err(ApplyError::RolledBack { cause })
    }
}

// ---- SourceDoc Fidelity Track Integration ----------------------------------

/// Apply a [`SourceDoc`] as the current profile through the full CORE-004 transaction.
pub async fn apply_current_profile_doc<S: SecureStore>(
    session: &impl CoreLifecyclePort,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    doc: &SourceDoc,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    let content = doc.render();
    apply_current_profile(session, config, reloader, &content, params).await
}

/// Apply a text-level [`SourceDoc`] edit closure to the current profile.
///
/// Loads the current profile text from `config`, parses it into a [`SourceDoc`],
/// invokes `edit_fn(&mut doc)`, renders the spliced document, and applies it
/// through the CORE-004 transaction. Untouched comments, anchors, formatting,
/// and line endings remain 100% preserved.
pub async fn apply_profile_edit<S: SecureStore, F>(
    session: &impl CoreLifecyclePort,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    edit_fn: F,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome>
where
    F: FnOnce(&mut SourceDoc) -> Result<(), infiltrator_domain::yaml_edit::YamlEditError>,
{
    let current = config
        .get_current()
        .await
        .map_err(|err| ApplyError::Write(err.to_string()))?;
    let content = match config.load(&current).await {
        Ok(c) => c,
        Err(err) => return Err(ApplyError::Write(err.to_string())),
    };
    let mut doc = SourceDoc::parse(&content).map_err(|err| {
        ApplyError::Validation(format!("failed to parse profile for editing: {err}"))
    })?;
    edit_fn(&mut doc).map_err(|err| ApplyError::Validation(format!("YAML edit failed: {err}")))?;
    let new_content = doc.render();
    apply_current_profile(session, config, reloader, &new_content, params).await
}

/// Update a top-level scalar configuration key (`mode`, `log-level`, `mixed-port`, etc.)
/// in the current profile while preserving 100% of existing comments, anchors, and formatting.
pub async fn apply_profile_set_scalar<S: SecureStore>(
    session: &impl CoreLifecyclePort,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    key: &str,
    value: &str,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    apply_profile_edit(
        session,
        config,
        reloader,
        |doc| doc.set_top_scalar(key, value),
        params,
    )
    .await
}

/// Append a rule to the `rules` section of the current profile while preserving
/// 100% of existing comments, anchors, and formatting.
pub async fn apply_profile_append_rule<S: SecureStore>(
    session: &impl CoreLifecyclePort,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    rule: &str,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    apply_profile_edit(
        session,
        config,
        reloader,
        |doc| doc.append_rule(rule),
        params,
    )
    .await
}

/// Remove a rule from the `rules` section of the current profile while preserving
/// 100% of existing comments, anchors, and formatting.
pub async fn apply_profile_remove_rule<S: SecureStore>(
    session: &impl CoreLifecyclePort,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    rule: &str,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    apply_profile_edit(
        session,
        config,
        reloader,
        |doc| doc.remove_rule(rule),
        params,
    )
    .await
}

/// Rewrite anchor namespaces in the current profile while preserving 100% of
/// existing comments and formatting.
pub async fn apply_profile_rewrite_anchors<S: SecureStore>(
    session: &impl CoreLifecyclePort,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    prefix: &str,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    apply_profile_edit(
        session,
        config,
        reloader,
        |doc| {
            doc.rewrite_anchor_namespace(prefix)?;
            Ok(())
        },
        params,
    )
    .await
}

/// Try applying a [`infiltrator_domain::mixin::MixinConfig`] to the current profile using
/// the [`SourceDoc`] fidelity path (preserving 100% of comments and anchors).
/// If the mixin contains complex structural edits that require AST merge,
/// falls back to full merge.
pub async fn apply_profile_mixin_fidelity<S: SecureStore>(
    session: &impl CoreLifecyclePort,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    mixin: &infiltrator_domain::mixin::MixinConfig,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    let current = config
        .get_current()
        .await
        .map_err(|err| ApplyError::Write(err.to_string()))?;
    let content = match config.load(&current).await {
        Ok(c) => c,
        Err(err) => return Err(ApplyError::Write(err.to_string())),
    };

    if infiltrator_domain::yaml_edit::mixin_fidelity::can_apply_mixin_via_fidelity(mixin)
        && let Ok(mut doc) = SourceDoc::parse(&content)
        && infiltrator_domain::yaml_edit::mixin_fidelity::apply_mixin_to_doc(&mut doc, mixin).is_ok()
    {
        let new_content = doc.render();
        return apply_current_profile(session, config, reloader, &new_content, params).await;
    }

    let new_content = infiltrator_domain::mixin::merge_profile_with_config(&content, mixin)
        .map_err(|err| ApplyError::Validation(format!("mixin merge failed: {err}")))?;
    apply_current_profile(session, config, reloader, &new_content, params).await
}

#[cfg(test)]
#[path = "apply_test.rs"]
mod apply_test;

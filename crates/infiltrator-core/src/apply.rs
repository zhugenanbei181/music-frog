//! Config apply transaction (CORE-004).
//!
//! [`apply_current_profile`] replaces the content of the *current* profile
//! and makes the running core pick it up as one all-or-nothing step:
//! validate → temp write → atomic replace → hot-reload or restart via
//! [`CoreSession`] → readiness health check → rollback to the previous
//! content on failure. A failed apply must never leave the core without a
//! usable configuration and must never strand the caller in a half-updated
//! state (the bug this transaction exists to fix: switching profiles used to
//! leave the old config running).
//!
//! The transaction operates on the current profile because that is the file
//! mihomo runs with; switching the active profile (`set_current`) is cheap
//! metadata and stays with the caller.

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use mihomo_api::client::MihomoClient;
use mihomo_config::manager::ConfigManager;
use mihomo_platform::traits::CredentialStore;
use tokio::io::AsyncWriteExt;
use yaml_rust2::{Yaml, YamlLoader};

use crate::session::{CoreSession, CoreStatus, READINESS_TIMEOUT, SessionError};

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
            restart_timeout: READINESS_TIMEOUT,
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
    Busy { status: CoreStatus },
    #[error("apply failed, previous config restored and core recovered: {cause}")]
    RolledBack { cause: String },
    #[error("apply failed ({cause}) and rollback failed as well; core is down: {rollback}")]
    RollbackFailed { cause: String, rollback: String },
    #[error(transparent)]
    Session(#[from] SessionError),
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
/// safe), build a client, `PUT /configs?force=true` with the file path.
pub struct SessionConfigReloader {
    session: std::sync::Arc<CoreSession>,
}

impl SessionConfigReloader {
    pub fn new(session: std::sync::Arc<CoreSession>) -> Self {
        Self { session }
    }
}

#[async_trait]
impl ConfigReloader for SessionConfigReloader {
    async fn reload(&self, path: &Path) -> Result<(), String> {
        let endpoint = self
            .session
            .endpoint()
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
    session: &CoreSession,
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
    session: &CoreSession,
    params: &ApplyParams,
) -> Result<ApplyOutcome, SessionError> {
    let generation = session.restart().await?;
    session
        .wait_for_ready(generation, params.restart_timeout)
        .await?;
    Ok(ApplyOutcome {
        method: ApplyMethod::Restart,
        generation,
    })
}

async fn start_and_check(
    session: &CoreSession,
    params: &ApplyParams,
) -> Result<ApplyOutcome, SessionError> {
    let generation = session.start().await?;
    session
        .wait_for_ready(generation, params.restart_timeout)
        .await?;
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
pub async fn apply_current_profile<S: CredentialStore>(
    session: &CoreSession,
    config: &ConfigManager<S>,
    reloader: &dyn ConfigReloader,
    new_content: &str,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    validate_config(new_content).map_err(ApplyError::Validation)?;

    let status = session.status();
    if matches!(status, CoreStatus::Starting | CoreStatus::Stopping) {
        return Err(ApplyError::Busy { status });
    }
    let was_running = matches!(status, CoreStatus::Ready | CoreStatus::Running);

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
            // Success: record the newly live content in the snapshot history
            // ([缺口13]). A history failure must not fail the apply itself —
            // the config IS live — so it only logs.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{ControllerEndpoint, EndpointSource, ReadinessProbe};

    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    const OLD: &str = "port: 7890\n";
    const NEW: &str = "port: 7891\n";

    struct MockController {
        running: AtomicBool,
        fail_starts_left: AtomicU64,
    }

    #[async_trait]
    impl mihomo_platform::traits::CoreController for MockController {
        async fn start(&self) -> mihomo_api::error::Result<()> {
            // load-then-fetch_sub: fetch_sub alone wraps at zero and would
            // turn "fail N times" into "fail forever".
            if self.fail_starts_left.load(Ordering::SeqCst) > 0 {
                self.fail_starts_left.fetch_sub(1, Ordering::SeqCst);
                return Err(mihomo_api::error::MihomoError::Service(
                    "start rejected".into(),
                ));
            }
            self.running.store(true, Ordering::SeqCst);
            Ok(())
        }

        async fn stop(&self) -> mihomo_api::error::Result<()> {
            self.running.store(false, Ordering::SeqCst);
            Ok(())
        }

        async fn is_running(&self) -> bool {
            self.running.load(Ordering::SeqCst)
        }

        fn controller_url(&self) -> Option<String> {
            None
        }
    }

    struct MockProbe {
        failures_left: AtomicU64,
    }

    #[async_trait]
    impl ReadinessProbe for MockProbe {
        async fn probe(&self) -> Result<(), SessionError> {
            if self.failures_left.load(Ordering::SeqCst) > 0 {
                self.failures_left.fetch_sub(1, Ordering::SeqCst);
                return Err(SessionError::Probe("not listening yet".into()));
            }
            Ok(())
        }
    }

    struct StaticEndpoints;

    #[async_trait]
    impl EndpointSource for StaticEndpoints {
        async fn resolve(&self) -> Result<ControllerEndpoint, SessionError> {
            Ok(ControllerEndpoint {
                url: "http://127.0.0.1:9090".into(),
                secret: None,
            })
        }
    }

    struct MockReloader {
        fail: bool,
        calls: AtomicU64,
    }

    #[async_trait]
    impl ConfigReloader for MockReloader {
        async fn reload(&self, _path: &Path) -> Result<(), String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                Err("reload rejected".into())
            } else {
                Ok(())
            }
        }
    }

    struct MockStore {
        entries: Mutex<HashMap<String, String>>,
    }

    #[async_trait]
    impl CredentialStore for MockStore {
        async fn get(&self, service: &str, key: &str) -> mihomo_api::error::Result<Option<String>> {
            Ok(self
                .entries
                .lock()
                .expect("store lock")
                .get(&format!("{service}/{key}"))
                .cloned())
        }

        async fn set(
            &self,
            service: &str,
            key: &str,
            value: &str,
        ) -> mihomo_api::error::Result<()> {
            self.entries
                .lock()
                .expect("store lock")
                .insert(format!("{service}/{key}"), value.to_string());
            Ok(())
        }

        async fn delete(&self, service: &str, key: &str) -> mihomo_api::error::Result<()> {
            self.entries
                .lock()
                .expect("store lock")
                .remove(&format!("{service}/{key}"));
            Ok(())
        }
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        session: CoreSession,
        config: ConfigManager<MockStore>,
        controller: Arc<MockController>,
        reloader: MockReloader,
    }

    async fn fixture(probe_failures: u64, reload_fails: bool) -> Fixture {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = ConfigManager::with_home_and_store(
            dir.path().to_path_buf(),
            MockStore {
                entries: Mutex::new(HashMap::new()),
            },
        )
        .expect("config manager");
        config
            .ensure_default_config()
            .await
            .expect("default config");
        config.save("main", OLD).await.expect("seed profile");
        config.set_current("main").await.expect("set current");

        let controller = Arc::new(MockController {
            running: AtomicBool::new(false),
            fail_starts_left: AtomicU64::new(0),
        });
        let session = CoreSession::new(
            controller.clone(),
            Arc::new(StaticEndpoints),
            Arc::new(MockProbe {
                failures_left: AtomicU64::new(probe_failures),
            }),
        );
        Fixture {
            _dir: dir,
            session,
            config,
            controller,
            reloader: MockReloader {
                fail: reload_fails,
                calls: AtomicU64::new(0),
            },
        }
    }

    fn params(strategy: ApplyStrategy) -> ApplyParams {
        ApplyParams {
            strategy,
            health_timeout: Duration::from_secs(5),
            restart_timeout: Duration::from_secs(5),
            snapshot_history: false,
        }
    }

    async fn file_content(config: &ConfigManager<MockStore>) -> String {
        let current = config.get_current().await.expect("current");
        config.load(&current).await.expect("profile content")
    }

    #[tokio::test]
    async fn hot_reload_success_keeps_generation_and_updates_file() {
        let f = fixture(0, false).await;
        let generation = f.session.start().await.expect("start");
        f.session
            .wait_for_ready(generation, Duration::from_secs(5))
            .await
            .expect("ready");

        let outcome = apply_current_profile(
            &f.session,
            &f.config,
            &f.reloader,
            NEW,
            params(ApplyStrategy::PreferReload),
        )
        .await
        .expect("apply");

        assert_eq!(outcome.method, ApplyMethod::HotReload);
        assert_eq!(outcome.generation, generation);
        assert_eq!(file_content(&f.config).await, NEW);
        assert_eq!(f.session.status(), CoreStatus::Ready);
        assert_eq!(f.reloader.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn reload_failure_falls_back_to_restart() {
        let f = fixture(0, true).await;
        let generation = f.session.start().await.expect("start");
        f.session
            .wait_for_ready(generation, Duration::from_secs(5))
            .await
            .expect("ready");
        let before = f.session.generation();

        let outcome = apply_current_profile(
            &f.session,
            &f.config,
            &f.reloader,
            NEW,
            params(ApplyStrategy::PreferReload),
        )
        .await
        .expect("apply via restart");

        assert_eq!(outcome.method, ApplyMethod::Restart);
        assert!(outcome.generation > before);
        assert_eq!(file_content(&f.config).await, NEW);
        assert_eq!(f.reloader.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn restart_failure_rolls_back_and_recovers() {
        let f = fixture(0, true).await;
        let generation = f.session.start().await.expect("start");
        f.session
            .wait_for_ready(generation, Duration::from_secs(5))
            .await
            .expect("ready");
        f.controller.fail_starts_left.store(1, Ordering::SeqCst);

        let err = apply_current_profile(
            &f.session,
            &f.config,
            &f.reloader,
            NEW,
            params(ApplyStrategy::AlwaysRestart),
        )
        .await
        .expect_err("restart was sabotaged");

        assert!(matches!(err, ApplyError::RolledBack { .. }));
        assert_eq!(file_content(&f.config).await, OLD);
        assert_eq!(f.session.status(), CoreStatus::Ready);
    }

    #[tokio::test]
    async fn stopped_core_starts_with_new_config_without_reload() {
        let f = fixture(0, false).await;

        let outcome = apply_current_profile(
            &f.session,
            &f.config,
            &f.reloader,
            NEW,
            params(ApplyStrategy::PreferReload),
        )
        .await
        .expect("apply");

        assert_eq!(outcome.method, ApplyMethod::Restart);
        assert_eq!(file_content(&f.config).await, NEW);
        assert_eq!(f.session.status(), CoreStatus::Ready);
        assert_eq!(f.reloader.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn successful_apply_stores_snapshot_history() {
        let f = fixture(0, false).await;
        let generation = f.session.start().await.expect("start");
        f.session
            .wait_for_ready(generation, Duration::from_secs(5))
            .await
            .expect("ready");
        let mut p = params(ApplyStrategy::PreferReload);
        p.snapshot_history = true;

        apply_current_profile(&f.session, &f.config, &f.reloader, NEW, p)
            .await
            .expect("apply");

        // profile files live under <home>/configs, so the snapshot dir is
        // <home>/configs/snapshots/<profile>.
        let config_dir = f._dir.path().join("configs");
        let snapshots = crate::history::list_snapshots(&config_dir, "main")
            .await
            .expect("list");
        assert_eq!(snapshots.len(), 1);
        assert_eq!(
            crate::history::read_snapshot(&snapshots[0].path)
                .await
                .unwrap(),
            NEW
        );
        assert_eq!(
            crate::history::read_snapshot(&snapshots[0].path)
                .await
                .unwrap(),
            NEW
        );
    }

    #[tokio::test]
    async fn invalid_content_aborts_before_any_write() {
        let f = fixture(0, false).await;

        // Top-level sequence: parses fine but is not a mapping.
        let err = apply_current_profile(
            &f.session,
            &f.config,
            &f.reloader,
            "- a\n- b\n",
            params(ApplyStrategy::PreferReload),
        )
        .await
        .expect_err("validation must fail");

        assert!(matches!(err, ApplyError::Validation(_)));
        assert_eq!(file_content(&f.config).await, OLD);
        assert_eq!(f.reloader.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn busy_transition_rejects_apply() {
        let f = fixture(0, false).await;
        f.session.start().await.expect("start");

        let err = apply_current_profile(
            &f.session,
            &f.config,
            &f.reloader,
            NEW,
            params(ApplyStrategy::PreferReload),
        )
        .await
        .expect_err("Starting must reject apply");

        assert_eq!(
            err,
            ApplyError::Busy {
                status: CoreStatus::Starting
            }
        );
        assert_eq!(file_content(&f.config).await, OLD);
    }
}

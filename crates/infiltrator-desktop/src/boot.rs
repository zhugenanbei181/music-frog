//! Kernel boot retry with external-controller port rotation (ledger:
//! "启动引导 3 次重试，失败轮换 external-controller 端口；就绪等待 15s；
//! rebuild 等端口释放 5s").
//!
//! [`MihomoRuntime::bootstrap`] is a single shot: once it spawns the core it
//! never returns a handle on failure, and [`CoreSession`] deliberately has no
//! `Drop`-stop — a readiness timeout would leave an unmanaged mihomo holding
//! the controller port and poisoning every later attempt. So the retry loop
//! cannot just call `bootstrap` repeatedly.
//!
//! Instead every *attempt* is a self-orchestrated probe that mirrors
//! `runtime.rs` line by line using public APIs only (ConfigManager ensure_*,
//! ServiceManager, CoreSession, wait_for_ready) and **owns its process**: on
//! failure the session is stopped explicitly under a timeout. Once an attempt
//! proves the core startable and ready, `materialize` runs the real
//! `MihomoRuntime::bootstrap`, which attaches to the now-running core (fast
//! path) and produces the full-fidelity runtime — geoip ensure, endpoint
//! resolve and client construction all stay in the one authoritative place.
//!
//! Deliberate policy decisions:
//! - Readiness/start/resolve-binary failures are retryable; profile
//!   (config-content) failures are not — rotating a port cannot fix YAML.
//! - `materialize` failure is fatal: the core is healthy, killing it to retry
//!   a geoip download would only destroy a good process.
//! - Proxy ports (mixed-port etc.) are never rotated: a silent change would
//!   repoint the user's system proxy. Only `external-controller` rotates,
//!   via the existing [`mihomo_config`] rotation (it picks its own port, so
//!   the injected picker is used solely to wait for port release).

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use infiltrator_core::session::{
    CoreSession, EndpointSource, MihomoVersionProbe, ProfileEndpointSource, READINESS_TIMEOUT,
};
use infiltrator_core::settings::app_config_manager;
use mihomo_config::port::is_port_available;
use mihomo_version::manager::VersionManager;

use crate::runtime::MihomoRuntime;
use crate::service::ServiceManager;
use crate::version::resolve_binary;

/// Ledger default: three boot attempts (initial try plus two retries).
pub const DEFAULT_MAX_ATTEMPTS: u32 = 3;

/// Pause between attempts so a just-killed core can finish dying.
pub const RETRY_DELAY: Duration = Duration::from_millis(100);

/// Ceiling for waiting until the previous controller port is released.
pub const PORT_RELEASE_TIMEOUT: Duration = Duration::from_secs(5);

/// Poll interval while waiting for port release.
const PORT_RELEASE_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Ceiling for the explicit stop between attempts; the attempt must not hang
/// forever on a wedged process.
const STOP_TIMEOUT: Duration = Duration::from_secs(5);

/// One failed (or skipped) boot attempt, for UI/telemetry consumption.
#[derive(Debug, Clone)]
pub struct BootAttempt {
    /// Controller URL in play during the attempt, once known.
    pub controller_url: Option<String>,
    /// Human-readable failure reason.
    pub error: String,
}

/// Successful boot: the runtime plus a trace of what it took to get there.
pub struct BootOutcome {
    pub runtime: MihomoRuntime,
    /// Attempts that failed before the successful one, in order.
    pub attempts: Vec<BootAttempt>,
    /// True when at least one controller-port rotation succeeded.
    pub rotated: bool,
}

/// Aggregated boot failure. `tried` lists every distinct controller port an
/// attempt actually ran with; `source` is the last attempt's error. Callers
/// can `downcast_ref::<BootError>()` for structured reporting/i18n.
#[derive(Debug)]
pub struct BootError {
    pub tried: Vec<u16>,
    pub source: anyhow::Error,
}

impl fmt::Display for BootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.tried.is_empty() {
            write!(f, "mihomo bootstrap failed: {}", self.source)
        } else {
            let ports = self
                .tried
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            write!(
                f,
                "mihomo bootstrap failed after {} attempt(s), controller ports tried: [{}]: {}",
                self.tried.len(),
                ports,
                self.source
            )
        }
    }
}

impl std::error::Error for BootError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&*self.source)
    }
}

/// Tunables for [`bootstrap_with_retry_opts`]. The picker contract:
/// `picker(port) == Some(port)` iff `port` is bindable right now. The
/// production picker is a plain availability probe; the default
/// `find_available_port` from `mihomo_config` satisfies it too.
pub struct BootRetryOptions {
    pub max_attempts: u32,
    pub retry_delay: Duration,
    pub port_release_timeout: Duration,
    pub port_picker: Arc<dyn Fn(u16) -> Option<u16> + Send + Sync>,
}

impl Default for BootRetryOptions {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            retry_delay: RETRY_DELAY,
            port_release_timeout: PORT_RELEASE_TIMEOUT,
            port_picker: Arc::new(production_port_picker),
        }
    }
}

/// Retry-guided boot with ledger defaults (3 attempts, 100ms pause, 5s port
/// release wait). See [`bootstrap_with_retry_opts`] for the customizable form.
pub async fn bootstrap_with_retry(
    vm: &VersionManager,
    use_bundled: bool,
    bundled_candidates: &[PathBuf],
    data_dir: &Path,
) -> anyhow::Result<BootOutcome> {
    bootstrap_with_retry_opts(
        vm,
        use_bundled,
        bundled_candidates,
        data_dir,
        BootRetryOptions::default(),
    )
    .await
}

/// [`bootstrap_with_retry`] with explicit retry behavior (tests, rebuild
/// flows that need different timings).
pub async fn bootstrap_with_retry_opts(
    vm: &VersionManager,
    use_bundled: bool,
    bundled_candidates: &[PathBuf],
    data_dir: &Path,
    options: BootRetryOptions,
) -> anyhow::Result<BootOutcome> {
    let engine = ProductionEngine {
        vm,
        use_bundled,
        bundled_candidates,
        data_dir,
    };
    let config = RetryLoopConfig {
        max_attempts: options.max_attempts,
        retry_delay: options.retry_delay,
        port_release_timeout: options.port_release_timeout,
    };
    let (runtime, attempts, rotated) =
        run_boot_retry(&engine, config, options.port_picker.as_ref()).await?;
    Ok(BootOutcome {
        runtime,
        attempts,
        rotated,
    })
}

/// One attempt's failure plus what the retry loop needs to act on it.
pub(crate) struct AttemptFailure {
    pub error: anyhow::Error,
    pub controller_url: Option<String>,
    pub controller_port: Option<u16>,
    /// False failures abort the whole boot immediately.
    pub retryable: bool,
}

impl AttemptFailure {
    fn new(error: anyhow::Error, controller_url: Option<&str>, retryable: bool) -> Self {
        let controller_url = controller_url.map(str::to_string);
        let controller_port = controller_url.as_deref().and_then(controller_port_from_url);
        Self {
            error,
            controller_url,
            controller_port,
            retryable,
        }
    }
}

/// The pluggable boot seam: production implementation wraps the real
/// lifecycle, tests substitute a scripted one.
pub(crate) trait BootEngine {
    type Runtime;

    /// Run one full boot attempt. `Ok` means a core process is alive and its
    /// controller proved ready; the engine has already cleaned up on `Err`.
    async fn attempt(&self) -> Result<(), AttemptFailure>;

    /// Rotate the external-controller port in the active profile; returns
    /// the new port.
    async fn rotate_controller_port(&self) -> anyhow::Result<u16>;

    /// Materialize the final runtime on top of a proven-ready core.
    async fn materialize(&self) -> anyhow::Result<Self::Runtime>;
}

pub(crate) struct RetryLoopConfig {
    pub max_attempts: u32,
    pub retry_delay: Duration,
    pub port_release_timeout: Duration,
}

/// Drive attempts with rotation between failures. Returns the runtime, the
/// failed-attempt trace and whether any rotation succeeded.
pub(crate) async fn run_boot_retry<E: BootEngine>(
    engine: &E,
    config: RetryLoopConfig,
    port_picker: &(dyn Fn(u16) -> Option<u16> + Send + Sync),
) -> anyhow::Result<(E::Runtime, Vec<BootAttempt>, bool)> {
    let max_attempts = config.max_attempts.max(1);
    let mut attempts = Vec::new();
    let mut tried_ports: Vec<u16> = Vec::new();
    let mut rotated = false;
    let mut last_error: Option<anyhow::Error> = None;

    for round in 1..=max_attempts {
        log::info!("mihomo boot attempt {round}/{max_attempts}");
        match engine.attempt().await {
            Ok(()) => {
                // Core proven startable and ready. Materialize failure is
                // fatal on purpose: a retry would kill a healthy core to
                // repeat a problem no port rotation can fix.
                let runtime = engine.materialize().await.map_err(|error| {
                    anyhow::Error::new(BootError {
                        tried: tried_ports.clone(),
                        source: error,
                    })
                })?;
                return Ok((runtime, attempts, rotated));
            }
            Err(failure) => {
                log::warn!(
                    "boot attempt {round}/{max_attempts} failed: {:#}",
                    failure.error
                );
                if let Some(port) = failure.controller_port
                    && !tried_ports.contains(&port)
                {
                    tried_ports.push(port);
                }
                attempts.push(BootAttempt {
                    controller_url: failure.controller_url.clone(),
                    error: failure.error.to_string(),
                });
                if !failure.retryable {
                    return Err(anyhow::Error::new(BootError {
                        tried: tried_ports,
                        source: failure.error,
                    }));
                }
                last_error = Some(failure.error);

                if round < max_attempts {
                    match engine.rotate_controller_port().await {
                        Ok(new_port) => {
                            rotated = true;
                            log::info!("rotated external-controller port to {new_port}");
                        }
                        Err(error) => {
                            log::warn!(
                                "controller port rotation failed, retrying on current port: {error:#}"
                            );
                        }
                    }
                    if let Some(port) = failure.controller_port {
                        wait_for_port_release(port, config.port_release_timeout, port_picker).await;
                    }
                    tokio::time::sleep(config.retry_delay).await;
                }
            }
        }
    }

    Err(anyhow::Error::new(BootError {
        tried: tried_ports,
        source: last_error.unwrap_or_else(|| anyhow!("bootstrap failed for an unknown reason")),
    }))
}

/// Poll until `picker(port)` reports the port bindable again, bounded by
/// `timeout` (ledger: "等端口释放 5s"). Best effort: a timeout is logged, not
/// fatal — rotation already picked a free port regardless.
async fn wait_for_port_release(
    port: u16,
    timeout: Duration,
    picker: &(dyn Fn(u16) -> Option<u16> + Send + Sync),
) {
    let deadline = tokio::time::Instant::now() + timeout;
    while picker(port) != Some(port) {
        if tokio::time::Instant::now() >= deadline {
            log::warn!("controller port {port} not released within {timeout:?}; continuing");
            return;
        }
        tokio::time::sleep(PORT_RELEASE_POLL_INTERVAL).await;
    }
}

/// Extract the port from a normalized controller URL
/// (`http://127.0.0.1:9090`, `127.0.0.1:9090`, `[::1]:9090`, ...).
fn controller_port_from_url(url: &str) -> Option<u16> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let host_port = rest.split(['/', '?']).next()?;
    host_port
        .rsplit_once(':')
        .and_then(|(_, port)| port.parse().ok())
}

/// Production picker: `Some(port)` iff the port is bindable on localhost now.
fn production_port_picker(port: u16) -> Option<u16> {
    is_port_available(port).then_some(port)
}

/// Production engine over the real lifecycle (pid-file process control makes
/// cross-attempt stop/attach valid).
struct ProductionEngine<'a> {
    vm: &'a VersionManager,
    use_bundled: bool,
    bundled_candidates: &'a [PathBuf],
    data_dir: &'a Path,
}

impl BootEngine for ProductionEngine<'_> {
    type Runtime = MihomoRuntime;

    async fn attempt(&self) -> Result<(), AttemptFailure> {
        // Config preparation. Fatal on failure: YAML-level breakage is not
        // something a port rotation can fix (mirrors runtime.rs:60-66).
        let cm = Arc::new(app_config_manager().await.map_err(|error| {
            AttemptFailure::new(anyhow!("load config manager: {error:#}"), None, false)
        })?);
        cm.ensure_default_config().await.map_err(|error| {
            AttemptFailure::new(anyhow!("prepare default profile: {error}"), None, false)
        })?;
        cm.ensure_proxy_ports().await.map_err(|error| {
            AttemptFailure::new(anyhow!("prepare proxy ports: {error}"), None, false)
        })?;
        let controller_url = cm.ensure_external_controller().await.map_err(|error| {
            AttemptFailure::new(anyhow!("prepare external controller: {error}"), None, false)
        })?;
        let config_path = cm.get_current_path().await.map_err(|error| {
            AttemptFailure::new(
                anyhow!("resolve current profile path: {error}"),
                None,
                false,
            )
        })?;
        let controller = Some(controller_url.as_str());

        // Retryable: a transiently failing core download/install can recover
        // on the next attempt (mirrors runtime.rs:67).
        let binary = resolve_binary(
            self.vm,
            self.use_bundled,
            self.bundled_candidates,
            self.data_dir,
        )
        .await
        .map_err(|error| {
            AttemptFailure::new(
                anyhow!("resolve mihomo core binary: {error:#}"),
                controller,
                true,
            )
        })?;

        // Mirror of runtime.rs:70-77: service + session with a lazily
        // resolving endpoint source, so a rotated port is picked up by every
        // readiness probe without being passed around.
        let service_manager = ServiceManager::new(binary, config_path);
        let endpoints = Arc::new(ProfileEndpointSource::new(cm.clone()));
        let session = Arc::new(CoreSession::new(
            service_manager.core_process(),
            endpoints.clone(),
            Arc::new(MihomoVersionProbe::new(endpoints.clone())),
        ));

        // Mirror of runtime.rs:82-95: attach to a running instance by proving
        // it answers, or start fresh and wait for readiness (15s).
        let readiness: Result<(), AttemptFailure> = if service_manager.is_running().await {
            log::info!("boot: attaching to running mihomo service");
            session
                .wait_for_ready(session.generation(), READINESS_TIMEOUT)
                .await
                .map_err(|error| {
                    AttemptFailure::new(
                        anyhow!("running instance not ready: {error}"),
                        controller,
                        true,
                    )
                })
        } else {
            log::info!("boot: starting mihomo service");
            match session.start().await {
                Err(error) => Err(AttemptFailure::new(
                    anyhow!("start mihomo core: {error}"),
                    controller,
                    true,
                )),
                Ok(generation) => session
                    .wait_for_ready(generation, READINESS_TIMEOUT)
                    .await
                    .map_err(|error| {
                        AttemptFailure::new(
                            anyhow!("mihomo core not ready: {error}"),
                            controller,
                            true,
                        )
                    }),
            }
        };
        if let Err(failure) = readiness {
            // CoreSession has no Drop-stop: without this the core keeps the
            // controller port and every later attempt attaches to the same
            // zombie. Bounded stop; a wedged process is logged and left for
            // the next attempt (whose ensure_external_controller will dodge
            // its port).
            stop_session_quietly(&session).await;
            return Err(failure);
        }

        // Mirror of runtime.rs:97-100. Config-shaped and therefore fatal; the
        // core stays up (it is ready) for materialize to attach.
        endpoints.resolve().await.map_err(|error| {
            AttemptFailure::new(
                anyhow!("resolve controller endpoint: {error}"),
                controller,
                false,
            )
        })?;
        Ok(())
    }

    async fn rotate_controller_port(&self) -> anyhow::Result<u16> {
        let cm = app_config_manager().await?;
        let url = cm
            .rotate_external_controller()
            .await
            .map_err(|error| anyhow!("{error}"))?;
        controller_port_from_url(&url)
            .ok_or_else(|| anyhow!("rotated controller URL has no port: {url}"))
    }

    async fn materialize(&self) -> anyhow::Result<MihomoRuntime> {
        MihomoRuntime::bootstrap(
            self.vm,
            self.use_bundled,
            self.bundled_candidates,
            self.data_dir,
        )
        .await
    }
}

async fn stop_session_quietly(session: &Arc<CoreSession>) {
    match tokio::time::timeout(STOP_TIMEOUT, session.stop()).await {
        Ok(Ok(())) => log::info!("stopped mihomo core after failed boot attempt"),
        Ok(Err(error)) => {
            log::warn!("failed to stop mihomo core after failed attempt: {error}")
        }
        Err(_) => log::warn!(
            "timed out stopping mihomo core after failed attempt; it may linger until the next attempt"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::net::TcpListener;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const TEST_PORT: u16 = 9090;

    enum ScriptedAttempt {
        Ok,
        Fail {
            retryable: bool,
            port: Option<u16>,
            message: &'static str,
        },
    }

    struct MockEngine {
        script: Mutex<VecDeque<ScriptedAttempt>>,
        attempts_run: AtomicUsize,
        materialized: AtomicUsize,
        rotations: AtomicUsize,
        rotate_error: Option<&'static str>,
        materialize_error: Option<&'static str>,
    }

    impl MockEngine {
        fn new(script: Vec<ScriptedAttempt>) -> Self {
            Self {
                script: Mutex::new(script.into_iter().collect()),
                attempts_run: AtomicUsize::new(0),
                materialized: AtomicUsize::new(0),
                rotations: AtomicUsize::new(0),
                rotate_error: None,
                materialize_error: None,
            }
        }

        fn with_rotate_error(mut self, error: &'static str) -> Self {
            self.rotate_error = Some(error);
            self
        }

        fn with_materialize_error(mut self, error: &'static str) -> Self {
            self.materialize_error = Some(error);
            self
        }

        fn ok() -> Self {
            Self::new(vec![ScriptedAttempt::Ok])
        }
    }

    impl BootEngine for MockEngine {
        type Runtime = usize;

        async fn attempt(&self) -> Result<(), AttemptFailure> {
            self.attempts_run.fetch_add(1, Ordering::SeqCst);
            let step = self.script.lock().expect("script lock").pop_front();
            match step {
                None | Some(ScriptedAttempt::Ok) => Ok(()),
                Some(ScriptedAttempt::Fail {
                    retryable,
                    port,
                    message,
                }) => {
                    let url = port.map(|p| format!("http://127.0.0.1:{p}"));
                    Err(AttemptFailure::new(
                        anyhow!("{message}"),
                        url.as_deref(),
                        retryable,
                    ))
                }
            }
        }

        async fn rotate_controller_port(&self) -> anyhow::Result<u16> {
            let round = self.rotations.fetch_add(1, Ordering::SeqCst);
            match self.rotate_error {
                Some(error) => Err(anyhow!("{error}")),
                None => Ok(TEST_PORT + 1 + round as u16),
            }
        }

        async fn materialize(&self) -> anyhow::Result<usize> {
            match self.materialize_error {
                Some(error) => Err(anyhow!("{error}")),
                None => Ok(self.materialized.fetch_add(1, Ordering::SeqCst) + 1),
            }
        }
    }

    fn test_config(max_attempts: u32) -> RetryLoopConfig {
        RetryLoopConfig {
            max_attempts,
            retry_delay: Duration::from_millis(1),
            port_release_timeout: Duration::from_millis(10),
        }
    }

    fn always_free(port: u16) -> Option<u16> {
        Some(port)
    }

    async fn run(
        engine: &MockEngine,
        max_attempts: u32,
    ) -> anyhow::Result<(usize, Vec<BootAttempt>, bool)> {
        run_boot_retry(engine, test_config(max_attempts), &always_free).await
    }

    #[tokio::test]
    async fn retry_succeeds_after_rotation() {
        let engine = MockEngine::new(vec![
            ScriptedAttempt::Fail {
                retryable: true,
                port: Some(TEST_PORT),
                message: "controller not ready within 15.0s",
            },
            ScriptedAttempt::Ok,
        ]);

        let (runtime, attempts, rotated) = run(&engine, 3).await.expect("second attempt boots");

        assert_eq!(runtime, 1);
        assert_eq!(attempts.len(), 1);
        assert_eq!(
            attempts[0].controller_url.as_deref(),
            Some("http://127.0.0.1:9090")
        );
        assert!(attempts[0].error.contains("not ready"));
        assert!(rotated);
        assert_eq!(engine.rotations.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn all_attempts_fail_aggregates_boot_error() {
        let engine = MockEngine::new(vec![
            ScriptedAttempt::Fail {
                retryable: true,
                port: Some(9091),
                message: "timeout a",
            },
            ScriptedAttempt::Fail {
                retryable: true,
                port: Some(9092),
                message: "timeout b",
            },
            ScriptedAttempt::Fail {
                retryable: true,
                port: Some(9093),
                message: "timeout c",
            },
        ]);

        let error = run(&engine, 3).await.expect_err("all attempts fail");

        let boot_error = error.downcast_ref::<BootError>().expect("BootError");
        assert_eq!(boot_error.tried, vec![9091, 9092, 9093]);
        assert!(boot_error.source.to_string().contains("timeout c"));
        assert_eq!(engine.attempts_run.load(Ordering::SeqCst), 3);
        assert_eq!(engine.rotations.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn fatal_failure_stops_immediately() {
        let engine = MockEngine::new(vec![ScriptedAttempt::Fail {
            retryable: false,
            port: Some(TEST_PORT),
            message: "invalid profile YAML",
        }]);

        let error = run(&engine, 3).await.expect_err("fatal aborts");

        let boot_error = error.downcast_ref::<BootError>().expect("BootError");
        assert_eq!(boot_error.tried, vec![TEST_PORT]);
        assert_eq!(engine.attempts_run.load(Ordering::SeqCst), 1);
        assert_eq!(engine.rotations.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn rotation_failure_still_retries_without_rotated_flag() {
        let engine = MockEngine::new(vec![
            ScriptedAttempt::Fail {
                retryable: true,
                port: Some(TEST_PORT),
                message: "controller not ready",
            },
            ScriptedAttempt::Ok,
        ])
        .with_rotate_error("no available ports found");

        let (runtime, attempts, rotated) = run(&engine, 2)
            .await
            .expect("retry continues past rotation failure");

        assert_eq!(runtime, 1);
        assert_eq!(attempts.len(), 1);
        assert!(!rotated);
        assert_eq!(engine.rotations.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn materialize_failure_is_fatal_and_keeps_tried_ports() {
        // First attempt fails (retryable), second proves ready, materialize
        // blows up: the loop must surface the geoip error, not retry.
        let engine = MockEngine::new(vec![
            ScriptedAttempt::Fail {
                retryable: true,
                port: Some(TEST_PORT),
                message: "controller not ready",
            },
            ScriptedAttempt::Ok,
        ])
        .with_materialize_error("geoip download failed");

        let error = run(&engine, 3).await.expect_err("materialize fatal");
        let boot_error = error.downcast_ref::<BootError>().expect("BootError");
        assert_eq!(boot_error.tried, vec![TEST_PORT]);
        assert!(boot_error.source.to_string().contains("geoip"));
        assert_eq!(engine.attempts_run.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn zero_attempts_is_clamped_to_one() {
        let engine = MockEngine::ok();
        let (runtime, attempts, rotated) = run(&engine, 0).await.expect("clamped boot");
        assert_eq!(runtime, 1);
        assert!(attempts.is_empty());
        assert!(!rotated);
    }

    #[tokio::test]
    async fn port_release_wait_returns_once_picker_reports_free() {
        let polls = Arc::new(AtomicUsize::new(0));
        let observed = polls.clone();
        let picker = move |port: u16| -> Option<u16> {
            let seen = observed.fetch_add(1, Ordering::SeqCst);
            if seen < 2 { None } else { Some(port) }
        };

        let started = tokio::time::Instant::now();
        wait_for_port_release(TEST_PORT, Duration::from_secs(5), &picker).await;
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(polls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn port_release_wait_gives_up_after_timeout() {
        let picker = |_port: u16| -> Option<u16> { None };

        let started = tokio::time::Instant::now();
        wait_for_port_release(TEST_PORT, Duration::from_millis(50), &picker).await;
        assert!(started.elapsed() >= Duration::from_millis(50));
    }

    #[test]
    fn production_picker_respects_real_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
        let port = listener.local_addr().expect("addr").port();

        assert_eq!(production_port_picker(port), None);

        drop(listener);
        assert_eq!(production_port_picker(port), Some(port));
    }

    #[test]
    fn controller_port_from_url_parses_normalized_urls() {
        assert_eq!(
            controller_port_from_url("http://127.0.0.1:9090"),
            Some(9090)
        );
        assert_eq!(
            controller_port_from_url("https://host.example:8080/"),
            Some(8080)
        );
        assert_eq!(controller_port_from_url("127.0.0.1:9091"), Some(9091));
        assert_eq!(
            controller_port_from_url("http://127.0.0.1:9090/extra"),
            Some(9090)
        );
        assert_eq!(controller_port_from_url("[::1]:9092"), Some(9092));
        assert_eq!(controller_port_from_url("http://localhost"), None);
        assert_eq!(controller_port_from_url("http://127.0.0.1:notaport"), None);
    }

    #[test]
    fn boot_error_display_and_source() {
        let error = BootError {
            tried: vec![9090, 9091],
            source: anyhow!("controller not ready within 15.0s"),
        };

        let display = error.to_string();
        assert!(display.contains("2 attempt(s)"));
        assert!(display.contains("9090, 9091"));
        assert!(display.contains("controller not ready within 15.0s"));

        let source = std::error::Error::source(&error).expect("source");
        assert_eq!(source.to_string(), "controller not ready within 15.0s");
    }

    #[test]
    fn boot_error_display_without_ports() {
        let error = BootError {
            tried: vec![],
            source: anyhow!("load config manager: no home"),
        };
        let display = error.to_string();
        assert!(display.starts_with("mihomo bootstrap failed: "));
        assert!(display.contains("no home"));
    }

    #[test]
    fn default_options_match_ledger() {
        let options = BootRetryOptions::default();
        assert_eq!(options.max_attempts, 3);
        assert_eq!(options.retry_delay, Duration::from_millis(100));
        assert_eq!(options.port_release_timeout, Duration::from_secs(5));
        assert_eq!((options.port_picker)(TEST_PORT), Some(TEST_PORT));
    }
}

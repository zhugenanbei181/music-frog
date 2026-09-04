//! Unified mihomo core session (CORE-001/002).
//!
//! [`CoreSession`] is the single owner of core lifecycle state: the status
//! machine, the generation counter that invalidates stale async work, the
//! controller readiness probe, and the one place where the controller
//! endpoint/secret is resolved into an API client. Frontends (Iced, admin
//! web, Android) must consume a session instead of re-deriving any of this.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::core_process::CoreProcess;
use infiltrator_ports::endpoint::{ControllerEndpoint, EndpointSource};
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::client::MihomoClient;
use mihomo_config::manager::ConfigManager;
use tokio::time::{MissedTickBehavior, interval};
use yaml_rust2::YamlLoader;

use crate::error::InfiltratorError;

/// Interval between readiness probes while waiting for the controller.
pub const READINESS_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Default ceiling for [`CoreSession::wait_for_ready`].
pub const READINESS_TIMEOUT: Duration = Duration::from_secs(15);

/// Lifecycle state of the mihomo core process (CORE-002).
///
/// Transitions are driven exclusively through [`CoreSession`] methods so
/// every frontend observes the same machine instead of inferring its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreStatus {
    /// No core process is expected to be alive.
    Stopped,
    /// `start` accepted by the platform controller; readiness not yet proven.
    Starting,
    /// Process alive and controller probe succeeded.
    Ready,
    /// Serving traffic; promotion from `Ready` is the caller's decision
    /// (e.g. after proxy-port verification in the application facade).
    Running,
    /// `stop` accepted; waiting for the platform controller to confirm exit.
    Stopping,
    /// Terminal for the current generation; the reason is user-presentable.
    Failed(String),
}

impl CoreStatus {
    pub fn is_terminal_failure(&self) -> bool {
        matches!(self, CoreStatus::Failed(_))
    }
}

/// Errors with session semantics. More precise than [`InfiltratorError`]:
/// callers branch on stale generations and process exits, and those cases
/// must not collapse into a generic string error.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SessionError {
    /// The generation a task captured is no longer current; its result must
    /// be dropped and the task must re-read state from the session.
    #[error("stale session generation (captured {captured}, current {current})")]
    StaleGeneration { captured: u64, current: u64 },
    /// The core process died while waiting for controller readiness.
    #[error("core process exited before the controller became ready")]
    ProcessExited,
    /// Controller did not answer within the allotted timeout.
    #[error("controller not ready within {timeout_secs:.1}s")]
    ReadinessTimeout { timeout_secs: f64 },
    /// An action is not valid in the current status (e.g. `stop` on Stopped).
    #[error("operation {operation} not allowed in status {status:?}")]
    InvalidStatus {
        operation: &'static str,
        status: CoreStatus,
    },
    #[error("endpoint resolution failed: {0}")]
    Endpoint(String),
    #[error("controller probe failed: {0}")]
    Probe(String),
}

impl From<SessionError> for InfiltratorError {
    fn from(err: SessionError) -> Self {
        match err {
            SessionError::Endpoint(msg) | SessionError::Probe(msg) => InfiltratorError::Mihomo(msg),
            other => InfiltratorError::Internal(other.to_string()),
        }
    }
}

pub type SessionResult<T> = std::result::Result<T, SessionError>;

/// [`EndpointSource`] backed by [`ConfigManager`]: reads the current
/// profile's `external-controller` via the manager (normalization and port
/// fallback included) and parses the top-level `secret` key, which no
/// frontend resolved before this session layer existed.
pub struct ProfileEndpointSource<S: SecureStore> {
    config: Arc<ConfigManager<S>>,
}

impl<S: SecureStore> ProfileEndpointSource<S> {
    pub fn new(config: Arc<ConfigManager<S>>) -> Self {
        Self { config }
    }

    async fn current_profile_secret(&self) -> Result<Option<String>, PortError> {
        let profile = self
            .config
            .get_current()
            .await
            .map_err(|err| PortError::Io(err.to_string()))?;
        let content = self
            .config
            .load(&profile)
            .await
            .map_err(|err| PortError::Io(err.to_string()))?;
        let docs = YamlLoader::load_from_str(&content)
            .map_err(|err| PortError::Io(format!("invalid profile YAML: {err}")))?;
        let secret = docs
            .first()
            .and_then(|doc| doc["secret"].as_str())
            .map(str::to_string);
        Ok(secret)
    }
}

#[async_trait]
impl<S: SecureStore> EndpointSource for ProfileEndpointSource<S> {
    async fn resolve(&self) -> Result<ControllerEndpoint, PortError> {
        let url = self
            .config
            .get_external_controller()
            .await
            .map_err(|err| PortError::Io(err.to_string()))?;
        let secret = self.current_profile_secret().await?;
        Ok(ControllerEndpoint { url, secret })
    }
}

/// Readiness check executed against the live controller. A dedicated trait
/// because real-core probes stay out of ordinary unit tests (QA-002): tests
/// inject a mock instead of spawning mihomo.
#[async_trait]
pub trait ReadinessProbe: Send + Sync {
    async fn probe(&self) -> SessionResult<()>;
}

/// Production probe: resolve the endpoint, build a client, ask for the core
/// version. Endpoint resolution happens per probe so port rotation is picked
/// up immediately.
pub struct MihomoVersionProbe<S: SecureStore> {
    endpoints: Arc<ProfileEndpointSource<S>>,
}

impl<S: SecureStore> MihomoVersionProbe<S> {
    pub fn new(endpoints: Arc<ProfileEndpointSource<S>>) -> Self {
        Self { endpoints }
    }
}

#[async_trait]
impl<S: SecureStore> ReadinessProbe for MihomoVersionProbe<S> {
    async fn probe(&self) -> SessionResult<()> {
        let endpoint = self
            .endpoints
            .resolve()
            .await
            .map_err(|error| SessionError::Endpoint(error.to_string()))?;
        let client = MihomoClient::new(&endpoint.url, endpoint.secret)
            .map_err(|err| SessionError::Probe(err.to_string()))?;
        client
            .get_version()
            .await
            .map_err(|err| SessionError::Probe(err.to_string()))?;
        Ok(())
    }
}

/// One session generation: everything captured before a lifecycle bump.
#[derive(Clone, Debug, PartialEq)]
struct SessionState {
    status: CoreStatus,
    generation: u64,
}

/// Single owner of core lifecycle state for all frontends.
///
/// Platform process handling stays behind [`CoreProcess`]; everything
/// above it — status machine, generation protocol, readiness, endpoint
/// resolution — is shared here so no frontend re-derives core truth.
pub struct CoreSession {
    controller: Arc<dyn CoreProcess>,
    endpoints: Arc<dyn EndpointSource>,
    probe: Arc<dyn ReadinessProbe>,
    state: std::sync::RwLock<SessionState>,
}

impl CoreSession {
    pub fn new(
        controller: Arc<dyn CoreProcess>,
        endpoints: Arc<dyn EndpointSource>,
        probe: Arc<dyn ReadinessProbe>,
    ) -> Self {
        Self {
            controller,
            endpoints,
            probe,
            state: std::sync::RwLock::new(SessionState {
                status: CoreStatus::Stopped,
                generation: 0,
            }),
        }
    }

    pub fn status(&self) -> CoreStatus {
        self.state
            .read()
            .expect("session state lock")
            .status
            .clone()
    }

    pub fn generation(&self) -> u64 {
        self.state.read().expect("session state lock").generation
    }

    /// Validate a generation captured by an async task before applying its
    /// result. Every delayed write into UI/runtime state must pass here so
    /// work orphaned by a later start/stop cannot land.
    pub fn check_generation(&self, captured: u64) -> SessionResult<()> {
        let current = self.generation();
        if captured == current {
            Ok(())
        } else {
            Err(SessionError::StaleGeneration { captured, current })
        }
    }

    /// Resolve the controller endpoint for the current profile.
    pub async fn endpoint(&self) -> SessionResult<ControllerEndpoint> {
        self.endpoints
            .resolve()
            .await
            .map_err(|error| SessionError::Endpoint(error.to_string()))
    }

    /// Start the core: bump the generation (invalidating in-flight work),
    /// flip to [`CoreStatus::Starting`], and hand off to the platform
    /// controller. Readiness is *not* awaited here; follow with
    /// [`CoreSession::wait_for_ready`].
    pub async fn start(&self) -> SessionResult<u64> {
        let generation = self.transition(CoreStatus::Starting);
        self.controller
            .start()
            .await
            .map_err(|err| SessionError::Probe(err.to_string()))?;
        Ok(generation)
    }

    /// Stop the core and return to [`CoreStatus::Stopped`].
    pub async fn stop(&self) -> SessionResult<()> {
        self.transition(CoreStatus::Stopping);
        if let Err(err) = self.controller.stop().await {
            self.mark_failed(format!("stop failed: {err}"));
            return Err(SessionError::Probe(err.to_string()));
        }
        self.set_status(CoreStatus::Stopped);
        Ok(())
    }

    /// Restart = stop plus start under one generation bump per phase, so
    /// tasks from the previous run can never interleave with the new one.
    pub async fn restart(&self) -> SessionResult<u64> {
        if self.status() != CoreStatus::Stopped {
            self.stop().await?;
        }
        self.start().await
    }

    /// Poll until the controller answers, the process dies, or the timeout
    /// elapses. Succeeds once, leaving the session in [`CoreStatus::Ready`].
    pub async fn wait_for_ready(&self, generation: u64, timeout: Duration) -> SessionResult<()> {
        let mut ticks = interval(READINESS_POLL_INTERVAL);
        ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            self.check_generation(generation)?;

            let process_alive = matches!(
                self.controller.status().await,
                Ok(CoreLifecycle::Starting) | Ok(CoreLifecycle::Ready) | Ok(CoreLifecycle::Running)
            );
            if !process_alive {
                let reason = "core process exited while waiting for readiness".to_string();
                self.mark_failed(reason.clone());
                return Err(SessionError::ProcessExited);
            }

            match self.probe.probe().await {
                Ok(()) => {
                    self.set_status(CoreStatus::Ready);
                    return Ok(());
                }
                // Probe failures are expected while mihomo boots its
                // listener; only the timeout or a dead process ends the wait.
                Err(err) => {
                    log::trace!("readiness probe pending: {err}");
                }
            }

            if tokio::time::Instant::now() >= deadline {
                let err = SessionError::ReadinessTimeout {
                    timeout_secs: timeout.as_secs_f64(),
                };
                self.mark_failed(err.to_string());
                return Err(err);
            }

            ticks.tick().await;
        }
    }

    /// Promote `Ready` to `Running` once the caller has proven service
    /// (e.g. proxy port reachable). Refuses to promote a session that has
    /// moved on — callers treat that as a stale generation.
    pub fn mark_running(&self, generation: u64) -> SessionResult<()> {
        self.check_generation(generation)?;
        let mut state = self.state.write().expect("session state lock");
        if !matches!(state.status, CoreStatus::Ready) {
            return Err(SessionError::InvalidStatus {
                operation: "mark_running",
                status: state.status.clone(),
            });
        }
        state.status = CoreStatus::Running;
        Ok(())
    }

    /// Record a terminal failure for the current generation.
    pub fn mark_failed(&self, reason: impl Into<String>) {
        let mut state = self.state.write().expect("session state lock");
        state.status = CoreStatus::Failed(reason.into());
    }

    fn set_status(&self, status: CoreStatus) {
        self.state.write().expect("session state lock").status = status;
    }

    /// Bump the generation and install `status` atomically; every prior
    /// async task is now stale.
    fn transition(&self, status: CoreStatus) -> u64 {
        let mut state = self.state.write().expect("session state lock");
        state.generation += 1;
        state.status = status;
        state.generation
    }
}

#[async_trait]
impl CoreLifecyclePort for CoreSession {
    fn lifecycle(&self) -> CoreLifecycle {
        match self.status() {
            CoreStatus::Stopped => CoreLifecycle::Stopped,
            CoreStatus::Starting => CoreLifecycle::Starting,
            CoreStatus::Ready => CoreLifecycle::Ready,
            CoreStatus::Running => CoreLifecycle::Running,
            CoreStatus::Stopping => CoreLifecycle::Stopping,
            CoreStatus::Failed(_) => CoreLifecycle::Failed,
        }
    }

    fn generation(&self) -> u64 {
        CoreSession::generation(self)
    }

    async fn start(&self) -> Result<u64, PortError> {
        CoreSession::start(self)
            .await
            .map_err(map_session_port_error)
    }

    async fn stop(&self) -> Result<(), PortError> {
        CoreSession::stop(self)
            .await
            .map_err(map_session_port_error)
    }

    async fn restart(&self) -> Result<u64, PortError> {
        CoreSession::restart(self)
            .await
            .map_err(map_session_port_error)
    }

    async fn wait_for_ready(&self, generation: u64, timeout: Duration) -> Result<(), PortError> {
        CoreSession::wait_for_ready(self, generation, timeout)
            .await
            .map_err(map_session_port_error)
    }
}

fn map_session_port_error(error: SessionError) -> PortError {
    match error {
        SessionError::Endpoint(message) | SessionError::Probe(message) => {
            PortError::Network(message)
        }
        SessionError::ProcessExited => {
            PortError::Network("core process exited before readiness".to_string())
        }
        SessionError::ReadinessTimeout { timeout_secs } => {
            PortError::Network(format!("controller not ready within {timeout_secs:.1}s"))
        }
        SessionError::StaleGeneration { captured, current } => PortError::Failed(format!(
            "stale session generation (captured {captured}, current {current})"
        )),
        SessionError::InvalidStatus { operation, status } => PortError::Failed(format!(
            "operation {operation} not allowed in status {status:?}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    struct MockController {
        running: AtomicBool,
        fail_start: AtomicBool,
    }

    impl MockController {
        fn new() -> Self {
            Self {
                running: AtomicBool::new(false),
                fail_start: AtomicBool::new(false),
            }
        }
    }

    #[async_trait]
    impl CoreProcess for MockController {
        async fn start(&self) -> std::result::Result<(), infiltrator_ports::error::PortError> {
            if self.fail_start.load(Ordering::SeqCst) {
                return Err(infiltrator_ports::error::PortError::Failed(
                    "start rejected".into(),
                ));
            }
            self.running.store(true, Ordering::SeqCst);
            Ok(())
        }

        async fn stop(&self) -> std::result::Result<(), infiltrator_ports::error::PortError> {
            self.running.store(false, Ordering::SeqCst);
            Ok(())
        }

        async fn status(
            &self,
        ) -> std::result::Result<CoreLifecycle, infiltrator_ports::error::PortError> {
            Ok(if self.running.load(Ordering::SeqCst) {
                CoreLifecycle::Running
            } else {
                CoreLifecycle::Stopped
            })
        }

        fn controller_endpoint(&self) -> Option<String> {
            None
        }
    }

    struct MockProbe {
        /// Number of leading failures before the probe succeeds;
        /// `u64::MAX` means it never succeeds.
        failures_left: AtomicU64,
        calls: AtomicU64,
    }

    impl MockProbe {
        fn succeeding() -> Self {
            Self {
                failures_left: AtomicU64::new(0),
                calls: AtomicU64::new(0),
            }
        }

        fn failing_n_times(n: u64) -> Self {
            Self {
                failures_left: AtomicU64::new(n),
                calls: AtomicU64::new(0),
            }
        }

        fn never() -> Self {
            Self {
                failures_left: AtomicU64::new(u64::MAX),
                calls: AtomicU64::new(0),
            }
        }
    }

    #[async_trait]
    impl ReadinessProbe for MockProbe {
        async fn probe(&self) -> SessionResult<()> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            // load-then-fetch_sub: a bare fetch_sub wraps at zero and would
            // turn "fail N times" into "fail forever" after N+1 calls.
            if self.failures_left.load(Ordering::SeqCst) > 0 {
                self.failures_left.fetch_sub(1, Ordering::SeqCst);
                return Err(SessionError::Probe("not listening yet".into()));
            }
            Ok(())
        }
    }

    struct MockStore {
        entries: Mutex<HashMap<String, String>>,
    }

    impl MockStore {
        fn empty() -> Self {
            Self {
                entries: Mutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl SecureStore for MockStore {
        async fn get(
            &self,
            service: &str,
            key: &str,
        ) -> std::result::Result<Option<String>, infiltrator_ports::error::PortError> {
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
        ) -> std::result::Result<(), infiltrator_ports::error::PortError> {
            self.entries
                .lock()
                .expect("store lock")
                .insert(format!("{service}/{key}"), value.to_string());
            Ok(())
        }

        async fn delete(
            &self,
            service: &str,
            key: &str,
        ) -> std::result::Result<(), infiltrator_ports::error::PortError> {
            self.entries
                .lock()
                .expect("store lock")
                .remove(&format!("{service}/{key}"));
            Ok(())
        }
    }

    fn session_with(
        controller: Arc<dyn CoreProcess>,
        probe: Arc<dyn ReadinessProbe>,
    ) -> CoreSession {
        CoreSession::new(controller, Arc::new(StaticEndpoints), probe)
    }

    struct StaticEndpoints;

    #[async_trait]
    impl EndpointSource for StaticEndpoints {
        async fn resolve(&self) -> Result<ControllerEndpoint, PortError> {
            Ok(ControllerEndpoint {
                url: "http://127.0.0.1:9090".into(),
                secret: Some("test-secret".into()),
            })
        }
    }

    #[tokio::test]
    async fn start_then_ready_with_flaky_probe() {
        let controller = Arc::new(MockController::new());
        let session = session_with(controller.clone(), Arc::new(MockProbe::failing_n_times(2)));

        let generation = session.start().await.expect("start");
        assert_eq!(session.status(), CoreStatus::Starting);
        assert!(controller.running.load(Ordering::SeqCst));

        session
            .wait_for_ready(generation, Duration::from_secs(5))
            .await
            .expect("readiness");
        assert_eq!(session.status(), CoreStatus::Ready);

        session.mark_running(generation).expect("promote");
        assert_eq!(session.status(), CoreStatus::Running);
    }

    #[tokio::test]
    async fn process_death_fails_wait() {
        let controller = Arc::new(MockController::new());
        let session = session_with(controller.clone(), Arc::new(MockProbe::never()));

        let generation = session.start().await.expect("start");
        controller.running.store(false, Ordering::SeqCst);

        let err = session
            .wait_for_ready(generation, Duration::from_secs(5))
            .await
            .expect_err("process exit must fail the wait");
        assert_eq!(err, SessionError::ProcessExited);
        assert!(session.status().is_terminal_failure());
    }

    #[tokio::test]
    async fn readiness_timeout_marks_failed() {
        let session = session_with(
            Arc::new(MockController::new()),
            Arc::new(MockProbe::never()),
        );

        let generation = session.start().await.expect("start");
        let err = session
            .wait_for_ready(generation, Duration::from_millis(600))
            .await
            .expect_err("timeout expected");
        assert!(matches!(err, SessionError::ReadinessTimeout { .. }));
        assert!(session.status().is_terminal_failure());
    }

    #[tokio::test]
    async fn stop_invalidates_captured_generation() {
        let session = session_with(
            Arc::new(MockController::new()),
            Arc::new(MockProbe::succeeding()),
        );

        let generation = session.start().await.expect("start");
        session.stop().await.expect("stop");
        assert_eq!(session.status(), CoreStatus::Stopped);

        let err = session
            .check_generation(generation)
            .expect_err("old generation must be stale");
        assert!(matches!(err, SessionError::StaleGeneration { .. }));
        assert!(session.mark_running(generation).is_err());
    }

    #[tokio::test]
    async fn start_failure_bumps_generation_and_reports() {
        let controller = Arc::new(MockController::new());
        controller.fail_start.store(true, Ordering::SeqCst);
        let session = session_with(controller, Arc::new(MockProbe::succeeding()));

        let before = session.generation();
        let err = session
            .start()
            .await
            .expect_err("failing controller start must surface");
        assert!(matches!(err, SessionError::Probe(_)));
        assert_eq!(session.status(), CoreStatus::Starting);
        assert!(session.generation() > before);
    }

    #[tokio::test]
    async fn endpoint_resolution_reads_profile_url_and_secret() {
        let home = tempfile::tempdir().expect("tempdir");
        let config = Arc::new(
            ConfigManager::with_home_and_store(home.path().to_path_buf(), MockStore::empty())
                .expect("config manager"),
        );
        config
            .ensure_default_config()
            .await
            .expect("default config");
        config
            .save(
                "main",
                "port: 7890\nexternal-controller: 127.0.0.1:9091\nsecret: s3cret\n",
            )
            .await
            .expect("save profile");
        config.set_current("main").await.expect("set current");

        let source = ProfileEndpointSource::new(config);
        let endpoint = source.resolve().await.expect("resolve");
        assert_eq!(endpoint.url, "http://127.0.0.1:9091");
        assert_eq!(endpoint.secret.as_deref(), Some("s3cret"));
    }

    #[tokio::test]
    async fn endpoint_defaults_when_profile_has_no_controller() {
        let home = tempfile::tempdir().expect("tempdir");
        let config = Arc::new(
            ConfigManager::with_home_and_store(home.path().to_path_buf(), MockStore::empty())
                .expect("config manager"),
        );
        config.save("default", "mode: rule\n").await.expect("save");

        let source = ProfileEndpointSource::new(config);
        let endpoint = source.resolve().await.expect("resolve");
        assert_eq!(endpoint.url, "http://127.0.0.1:9090");
        assert_eq!(endpoint.secret, None);
    }
}

use self::execution::closed_failure;
use crate::command_application::CommandHandler;
use crate::log_application::LogApplication;
use crate::log_stream::LogPump;
use futures_util::lock;
use infiltrator_contract::command::{CommandIntent, CommandResult, ProxyMode, RequestId};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::{
    CoreEvent, CoreLifecycle, CoreLifecycleSnapshot, CoreSnapshot, CoreWatchdogSnapshot,
};
use infiltrator_domain::core_state;
use infiltrator_domain::core_state::{CoreState, CoreStateMachine};
use infiltrator_ports::application_runtime::ApplicationRuntime;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::error::PortError;
use infiltrator_ports::overview::OverviewReader;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, RwLock, Weak};
use std::thread::Builder;
use std::time;

#[path = "core_watchdog.rs"]
mod watchdog_runtime;

mod command_name;
mod dispatch;
mod execution;

const EVENT_CAPACITY: usize = 256;
const DISPATCH_CAPACITY: usize = 256;
const DEFAULT_READINESS_TIMEOUT: time::Duration = time::Duration::from_secs(15);
const DEFAULT_READINESS_POLL_INTERVAL: time::Duration = time::Duration::from_millis(250);
static NEXT_SESSION_NAMESPACE: AtomicU64 = AtomicU64::new(1);

/// Readiness retry policy expressed entirely in standard-library values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadinessPolicy {
    pub timeout: time::Duration,
    pub poll_interval: time::Duration,
}

impl Default for ReadinessPolicy {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_READINESS_TIMEOUT,
            poll_interval: DEFAULT_READINESS_POLL_INTERVAL,
        }
    }
}

struct StateMirror {
    state: CoreState,
    revision: u64,
}

struct Inner {
    logs: LogApplication,
    log_pump: Mutex<Option<LogPump>>,
    telemetry: lock::Mutex<telemetry::TelemetryState>,
    process: Arc<dyn CoreProcess>,
    readiness: Arc<dyn CoreReadiness>,
    overview: Option<Arc<dyn OverviewReader>>,
    readiness_policy: ReadinessPolicy,
    runtime: Arc<dyn ApplicationRuntime>,
    command_handler: RwLock<Option<Arc<dyn CommandHandler>>>,
    dispatch_tx: SyncSender<DispatchedCommand>,
    commands: lock::Mutex<()>,
    operation: lock::Mutex<()>,
    closed: AtomicBool,
    active_command: AtomicBool,
    state: RwLock<StateMirror>,
    watchdog: Mutex<watchdog_runtime::WatchdogRuntime>,
    next_request_id: AtomicU64,
    session_namespace: u64,
    next_session_sequence: AtomicU64,
    events: Mutex<VecDeque<CoreEvent>>,
}

struct DispatchedCommand {
    request_id: RequestId,
    intent: CommandIntent,
    reply: Option<Sender<CommandResult>>,
}

/// The single application owner of Core lifecycle operations.
///
/// `CoreApplication` serializes side-effecting commands, drives the pure
/// domain reducer, and publishes bounded contract events. It is intentionally
/// constructed with ports rather than `MihomoClient`, `ConfigManager`, or an
/// operating-system implementation.
#[derive(Clone)]
pub struct CoreApplication {
    inner: Arc<Inner>,
}

impl CoreApplication {
    pub fn new(
        process: Arc<dyn CoreProcess>,
        readiness: Arc<dyn CoreReadiness>,
        runtime: Arc<dyn ApplicationRuntime>,
    ) -> Self {
        Self::build(
            process,
            readiness,
            None,
            ReadinessPolicy::default(),
            runtime,
        )
    }

    /// Construct a lifecycle-only application with an explicit readiness
    /// policy. Tests and embedders can choose a smaller budget without
    /// changing the production default.
    pub fn new_with_policy(
        process: Arc<dyn CoreProcess>,
        readiness: Arc<dyn CoreReadiness>,
        readiness_policy: ReadinessPolicy,
        runtime: Arc<dyn ApplicationRuntime>,
    ) -> Self {
        Self::build(process, readiness, None, readiness_policy, runtime)
    }

    /// Construct the application with the optional Overview port wired in.
    /// This keeps mode-changing commands on the same application seam while
    /// allowing hosts that do not expose a controller to use lifecycle-only
    /// operation.
    pub fn new_with_overview(
        process: Arc<dyn CoreProcess>,
        readiness: Arc<dyn CoreReadiness>,
        overview: Arc<dyn OverviewReader>,
        runtime: Arc<dyn ApplicationRuntime>,
    ) -> Self {
        Self::build(
            process,
            readiness,
            Some(overview),
            ReadinessPolicy::default(),
            runtime,
        )
    }

    /// Construct an application with both an Overview port and an explicit
    /// readiness policy.
    pub fn new_with_overview_and_policy(
        process: Arc<dyn CoreProcess>,
        readiness: Arc<dyn CoreReadiness>,
        overview: Arc<dyn OverviewReader>,
        readiness_policy: ReadinessPolicy,
        runtime: Arc<dyn ApplicationRuntime>,
    ) -> Self {
        Self::build(
            process,
            readiness,
            Some(overview),
            readiness_policy,
            runtime,
        )
    }

    fn build(
        process: Arc<dyn CoreProcess>,
        readiness: Arc<dyn CoreReadiness>,
        overview: Option<Arc<dyn OverviewReader>>,
        readiness_policy: ReadinessPolicy,
        runtime: Arc<dyn ApplicationRuntime>,
    ) -> Self {
        let (dispatch_tx, dispatch_rx) = sync_channel(DISPATCH_CAPACITY);
        let session_namespace = NEXT_SESSION_NAMESPACE
            .fetch_add(1, Ordering::Relaxed)
            .max(1);
        let inner = Arc::new(Inner {
            logs: LogApplication::default(),
            log_pump: Mutex::new(None),
            telemetry: lock::Mutex::new(telemetry::TelemetryState::default()),
            process,
            readiness,
            overview,
            readiness_policy,
            runtime: Arc::clone(&runtime),
            command_handler: RwLock::new(None),
            dispatch_tx,
            commands: lock::Mutex::new(()),
            operation: lock::Mutex::new(()),
            closed: AtomicBool::new(false),
            active_command: AtomicBool::new(false),
            state: RwLock::new(StateMirror {
                state: CoreState::Idle { generation: 0 },
                revision: 0,
            }),
            watchdog: Mutex::new(watchdog_runtime::WatchdogRuntime::default()),
            next_request_id: AtomicU64::new(1),
            session_namespace,
            next_session_sequence: AtomicU64::new(1),
            events: Mutex::new(VecDeque::with_capacity(EVENT_CAPACITY)),
        });
        spawn_dispatch_worker(Arc::downgrade(&inner), dispatch_rx, runtime);
        Self { inner }
    }

    /// Install the non-lifecycle command facade used by a full product
    /// composition. Lifecycle and proxy-mode commands remain owned by this
    /// application; every other intent is delegated to the handler or
    /// rejected with a typed Unsupported failure.
    pub fn install_command_handler(&self, handler: Arc<dyn CommandHandler>) {
        let mut slot = self
            .inner
            .command_handler
            .write()
            .expect("command handler lock");
        assert!(
            !self.inner.closed.load(Ordering::Acquire),
            "cannot configure a closed application"
        );
        *slot = Some(handler);
    }

    pub fn log_application(&self) -> LogApplication {
        self.inner.logs.clone()
    }
    pub fn install_log_gateway(&self, gateway: Arc<dyn RuntimeGateway>) -> Result<(), Failure> {
        let mut slot = self.inner.log_pump.lock().expect("log driver");
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(closed_failure());
        }
        slot.take();
        let weak = Arc::downgrade(&self.inner);
        let pump = LogPump::spawn(
            self.inner.logs.clone(),
            gateway,
            self.inner.runtime.clone(),
            move || {
                weak.upgrade()
                    .map(|inner| CoreApplication { inner }.snapshot().lifecycle_snapshot())
            },
        )?;
        *slot = Some(pump);
        Ok(())
    }
    /// Execute a command and await its terminal result. The returned values
    /// contain no executor-specific types.
    pub async fn execute(&self, intent: CommandIntent) -> CommandResult {
        let request_id = self.allocate_request_id();
        self.execute_with_id(request_id, intent).await
    }

    /// Schedule a command on the application's single private executor and
    /// return only its correlation id. Completion/failure is observed through
    /// [`Self::drain_events`]. The caller never needs to own or name Tokio;
    /// every dispatched command is serialized by the same worker.
    pub fn dispatch(&self, intent: CommandIntent) -> RequestId {
        self.enqueue(intent, None)
    }

    /// Return the latest immutable contract projection.
    pub fn snapshot(&self) -> CoreSnapshot {
        let mirror = self.inner.state.read().expect("core state lock");
        snapshot_from_state(&mirror.state, mirror.revision, self.watchdog_snapshot())
    }

    /// Current lifecycle generation used to fence delayed surface work.
    pub fn generation(&self) -> u64 {
        self.snapshot().generation
    }

    /// Identity of the active core session. Unlike generation, this also
    /// separates sessions created by different application instances.
    pub fn session_token(&self) -> Option<SessionToken> {
        self.snapshot().session_token
    }

    /// Reject delayed work that belongs to a previous core session.
    pub fn check_session(&self, session_token: SessionToken) -> Result<(), Failure> {
        if self.session_token() == Some(session_token) {
            Ok(())
        } else {
            Err(Failure::new(
                ErrorCode::InvalidState,
                "stale core session token",
                false,
            ))
        }
    }

    /// Drain a bounded batch of application events for a frame-driven or FFI
    /// surface. The queue drops the oldest event when it reaches capacity.
    pub fn drain_events(&self) -> Vec<CoreEvent> {
        let mut events = self.inner.events.lock().expect("core event queue lock");
        events.drain(..).collect()
    }

    /// Adopt a host process that was started by another host lifecycle
    /// callback. This is the composition-root seam for Android `VpnService`
    /// and desktop boot attach flows; it never calls `start` on the process.
    pub async fn adopt_if_running(&self) -> Result<bool, Failure> {
        let _operation = self.inner.operation.lock().await;
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(closed_failure());
        }
        if !matches!(self.current_state(), CoreState::Idle { .. }) {
            return Ok(false);
        }

        let status = self.inner.process.status().await.map_err(Failure::from)?;
        if !matches!(status, CoreLifecycle::Running | CoreLifecycle::Ready) {
            return Ok(false);
        }

        let session_token = self.allocate_session_token();
        self.apply_domain_event(core_state::CoreEvent::StartRequested { session_token });
        match self
            .wait_for_readiness(session_token, self.inner.readiness_policy.timeout)
            .await
        {
            Ok(endpoint) => {
                self.apply_domain_event(core_state::CoreEvent::ReadinessSuccess {
                    session_token,
                    endpoint,
                });
                Ok(true)
            }
            Err(error) => {
                let message = error.to_string();
                self.apply_domain_event(core_state::CoreEvent::StartFailed {
                    session_token,
                    error: message,
                });
                Err(Failure::from(error))
            }
        }
    }

    fn allocate_request_id(&self) -> RequestId {
        RequestId::new(self.inner.next_request_id.fetch_add(1, Ordering::Relaxed))
    }

    fn allocate_session_token(&self) -> SessionToken {
        let sequence = self
            .inner
            .next_session_sequence
            .fetch_add(1, Ordering::Relaxed)
            .max(1);
        SessionToken::new((u128::from(self.inner.session_namespace) << 64) | u128::from(sequence))
    }

    async fn set_mode_locked(&self, wanted: ProxyMode) -> Result<(), Failure> {
        let Some(overview) = self.inner.overview.as_ref() else {
            return Err(Failure::unsupported(
                "proxy mode control is not configured for this host",
            ));
        };
        let actual = overview.set_mode(wanted).await.map_err(Failure::from)?;
        if actual == wanted {
            Ok(())
        } else {
            Err(Failure::new(
                ErrorCode::InvalidState,
                format!(
                    "controller retained proxy mode `{}` instead of `{}`",
                    actual.to_wire(),
                    wanted.to_wire()
                ),
                true,
            ))
        }
    }

    async fn start_locked(&self) -> Result<(), Failure> {
        if !matches!(
            self.current_state(),
            CoreState::Idle { .. } | CoreState::Failed { .. }
        ) {
            return Err(invalid_state_failure("start"));
        }

        // A manual start begins a fresh watchdog window. The recovery path
        // marks itself `Restarting` before calling this method, so its retry
        // counters remain intact until the new session is proven ready.
        self.reset_watchdog_for_start();

        let session_token = self.allocate_session_token();
        self.apply_domain_event(core_state::CoreEvent::StartRequested { session_token });
        if let Err(error) = self.inner.process.cleanup_orphaned().await {
            let message = error.to_string();
            self.apply_domain_event(core_state::CoreEvent::StartFailed {
                session_token,
                error: message,
            });
            return Err(Failure::from(error));
        }
        if let Err(error) = self.inner.process.start().await {
            let message = error.to_string();
            self.apply_domain_event(core_state::CoreEvent::StartFailed {
                session_token,
                error: message.clone(),
            });
            return Err(Failure::from(error));
        }

        match self
            .wait_for_readiness(session_token, self.inner.readiness_policy.timeout)
            .await
        {
            Ok(endpoint) => {
                self.apply_domain_event(core_state::CoreEvent::ReadinessSuccess {
                    session_token,
                    endpoint,
                });
                Ok(())
            }
            Err(error) => {
                let message = error.to_string();
                self.apply_domain_event(core_state::CoreEvent::StartFailed {
                    session_token,
                    error: message,
                });
                Err(Failure::from(error))
            }
        }
    }

    async fn stop_locked(&self) -> Result<(), Failure> {
        if matches!(self.current_state(), CoreState::Idle { .. }) {
            return Err(invalid_state_failure("stop"));
        }

        let Some(session_token) = self.session_token() else {
            return Err(invalid_state_failure("stop"));
        };
        self.apply_domain_event(core_state::CoreEvent::StopRequested { session_token });
        if let Err(error) = self.inner.process.stop().await {
            let message = error.to_string();
            self.apply_domain_event(core_state::CoreEvent::StopFailed {
                session_token,
                error: message,
            });
            return Err(Failure::from(error));
        }

        self.apply_domain_event(core_state::CoreEvent::StopCompleted { session_token });
        self.reset_watchdog_after_stop();
        Ok(())
    }

    async fn restart_locked(&self) -> Result<(), Failure> {
        if matches!(
            self.current_state(),
            CoreState::Starting { .. }
                | CoreState::Running { .. }
                | CoreState::Reloading { .. }
                | CoreState::Stopping { .. }
        ) {
            self.stop_locked().await?;
        }
        self.start_locked().await
    }

    fn current_state(&self) -> CoreState {
        self.inner
            .state
            .read()
            .expect("core state lock")
            .state
            .clone()
    }

    fn apply_domain_event(&self, event: core_state::CoreEvent) {
        let snapshot = {
            let mut mirror = self.inner.state.write().expect("core state lock");
            let (state, warning) = CoreStateMachine::step(&mirror.state, event);
            mirror.state = state;
            mirror.revision = mirror.revision.saturating_add(1);
            if let Some(warning) = warning {
                log::warn!(target: "infiltrator-application", "core domain transition warning: {warning}");
            }
            snapshot_from_state(&mirror.state, mirror.revision, self.watchdog_snapshot())
        };
        self.inner.logs.observe_core(&snapshot);
        self.push_event(CoreEvent::SnapshotUpdated(snapshot));
    }

    fn push_event(&self, event: CoreEvent) {
        let mut events = self.inner.events.lock().expect("core event queue lock");
        if events.len() == EVENT_CAPACITY {
            events.pop_front();
        }
        events.push_back(event);
    }

    async fn wait_for_readiness(
        &self,
        session_token: SessionToken,
        timeout: time::Duration,
    ) -> Result<String, PortError> {
        let deadline = time::Instant::now() + timeout;
        loop {
            if self.session_token() != Some(session_token) {
                return Err(PortError::Failed("stale core session token".to_string()));
            }
            match self.inner.process.status().await {
                Ok(CoreLifecycle::Starting)
                | Ok(CoreLifecycle::Ready)
                | Ok(CoreLifecycle::Running) => {}
                Ok(_) => {
                    return Err(PortError::Failed(
                        "core process exited before readiness".to_string(),
                    ));
                }
                Err(error) => return Err(error),
            }

            let probe_error = match self.inner.readiness.probe().await {
                Ok(endpoint) => return Ok(endpoint),
                Err(error) => error,
            };

            if time::Instant::now() >= deadline {
                return Err(probe_error);
            }
            self.inner
                .runtime
                .sleep(self.inner.readiness_policy.poll_interval)
                .await;
        }
    }
}

#[async_trait::async_trait]
impl CoreLifecyclePort for CoreApplication {
    fn lifecycle(&self) -> CoreLifecycle {
        self.snapshot().lifecycle
    }

    fn generation(&self) -> u64 {
        CoreApplication::generation(self)
    }

    fn session_token(&self) -> Option<SessionToken> {
        CoreApplication::session_token(self)
    }

    fn lifecycle_snapshot(&self) -> CoreLifecycleSnapshot {
        self.snapshot().lifecycle_snapshot()
    }

    async fn start(&self) -> Result<u64, PortError> {
        match self
            .execute_lifecycle_with_id(self.allocate_request_id(), CommandIntent::StartCore)
            .await
        {
            CommandResult::Completed { .. } => Ok(self.generation()),
            CommandResult::Rejected { failure, .. } => Err(PortError::Failed(failure.message)),
            CommandResult::Produced { .. } => Err(PortError::Failed(
                "Unexpected typed result for a lifecycle/control command".into(),
            )),
            CommandResult::Accepted { .. } => Err(PortError::Failed(
                "application start unexpectedly returned Accepted".to_string(),
            )),
        }
    }

    async fn stop(&self) -> Result<(), PortError> {
        match self
            .execute_lifecycle_with_id(self.allocate_request_id(), CommandIntent::StopCore)
            .await
        {
            CommandResult::Completed { .. } => Ok(()),
            CommandResult::Rejected { failure, .. } => Err(PortError::Failed(failure.message)),
            CommandResult::Produced { .. } => Err(PortError::Failed(
                "Unexpected typed result for a lifecycle/control command".into(),
            )),
            CommandResult::Accepted { .. } => Err(PortError::Failed(
                "application stop unexpectedly returned Accepted".to_string(),
            )),
        }
    }

    async fn restart(&self) -> Result<u64, PortError> {
        match self
            .execute_lifecycle_with_id(self.allocate_request_id(), CommandIntent::RestartCore)
            .await
        {
            CommandResult::Completed { .. } => Ok(self.generation()),
            CommandResult::Rejected { failure, .. } => Err(PortError::Failed(failure.message)),
            CommandResult::Produced { .. } => Err(PortError::Failed(
                "Unexpected typed result for a lifecycle/control command".into(),
            )),
            CommandResult::Accepted { .. } => Err(PortError::Failed(
                "application restart unexpectedly returned Accepted".to_string(),
            )),
        }
    }

    fn begin_reload(&self) -> Result<SessionToken, PortError> {
        let CoreState::Running { session_token, .. } = self.current_state() else {
            return Err(PortError::Failed(
                "core is not running for hot reload".to_string(),
            ));
        };
        self.apply_domain_event(core_state::CoreEvent::ReloadRequested { session_token });
        Ok(session_token)
    }

    fn complete_reload(&self, session_token: SessionToken) -> Result<(), PortError> {
        self.check_session(session_token)
            .map_err(|failure| PortError::Failed(failure.message))?;
        self.apply_domain_event(core_state::CoreEvent::ReloadSuccess { session_token });
        Ok(())
    }

    fn fail_reload(&self, session_token: SessionToken, error: String) -> Result<(), PortError> {
        self.check_session(session_token)
            .map_err(|failure| PortError::Failed(failure.message))?;
        self.apply_domain_event(core_state::CoreEvent::ReloadFailed {
            session_token,
            error,
        });
        Ok(())
    }

    async fn wait_for_ready(
        &self,
        generation: u64,
        timeout: time::Duration,
    ) -> Result<(), PortError> {
        if self.generation() != generation {
            return Err(PortError::Failed(
                "stale application generation".to_string(),
            ));
        }
        let Some(session_token) = self.session_token() else {
            return Err(PortError::Failed("core session is not active".to_string()));
        };
        self.wait_for_readiness(session_token, timeout)
            .await
            .map(|_| ())
    }

    async fn wait_for_ready_session(
        &self,
        generation: u64,
        session_token: SessionToken,
        timeout: time::Duration,
    ) -> Result<(), PortError> {
        if self.generation() != generation {
            return Err(PortError::Failed(
                "stale application generation".to_string(),
            ));
        }
        self.check_session(session_token)
            .map_err(|failure| PortError::Failed(failure.message))?;
        self.wait_for_readiness(session_token, timeout)
            .await
            .map(|_| ())
    }
}

fn spawn_dispatch_worker(
    inner: Weak<Inner>,
    dispatch_rx: Receiver<DispatchedCommand>,
    runtime: Arc<dyn ApplicationRuntime>,
) {
    let _ = Builder::new()
        .name("infiltrator-application".to_owned())
        .spawn(move || {
            while let Ok(command) = dispatch_rx.recv() {
                let Some(inner) = inner.upgrade() else {
                    return;
                };
                runtime.block_on(Box::pin(async move {
                    let result = CoreApplication { inner }
                        .execute_with_id(command.request_id, command.intent)
                        .await;
                    if let Some(reply) = command.reply {
                        let _ = reply.send(result);
                    }
                }));
            }
        });
}

fn invalid_state_failure(operation: &str) -> Failure {
    Failure::new(
        ErrorCode::InvalidState,
        format!("operation `{operation}` is not valid in the current core state"),
        false,
    )
}

fn snapshot_from_state(
    state: &CoreState,
    revision: u64,
    watchdog: CoreWatchdogSnapshot,
) -> CoreSnapshot {
    let (lifecycle, generation, session_token, failure) = match state {
        CoreState::Idle { generation } => (CoreLifecycle::Stopped, *generation, None, None),
        CoreState::Starting {
            generation,
            session_token,
        } => (
            CoreLifecycle::Starting,
            *generation,
            Some(*session_token),
            None,
        ),
        CoreState::Running {
            generation,
            session_token,
            ..
        } => (
            CoreLifecycle::Running,
            *generation,
            Some(*session_token),
            None,
        ),
        CoreState::Reloading {
            generation,
            session_token,
            ..
        } => (
            CoreLifecycle::Ready,
            *generation,
            Some(*session_token),
            None,
        ),
        CoreState::Stopping {
            generation,
            session_token,
        } => (
            CoreLifecycle::Stopping,
            *generation,
            Some(*session_token),
            None,
        ),
        CoreState::Failed {
            generation,
            session_token,
            error,
        } => (
            CoreLifecycle::Failed,
            *generation,
            Some(*session_token),
            Some(Failure::new(ErrorCode::Internal, error.clone(), false)),
        ),
    };

    CoreSnapshot {
        lifecycle,
        generation,
        session_token,
        revision,
        proxy_mode: None,
        core_version: None,
        sampled_at_epoch_ms: None,
        failure,
        upload_bps: 0.0,
        download_bps: 0.0,
        active_connections: 0,
        memory_bytes: None,
        watchdog,
    }
}

#[path = "core_telemetry.rs"]
mod telemetry;
#[cfg(test)]
#[path = "core_application_tests.rs"]
mod tests;

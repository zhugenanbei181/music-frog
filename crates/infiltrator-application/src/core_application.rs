use infiltrator_contract::command::{CommandIntent, CommandResult, RequestId};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::{CoreEvent, CoreLifecycle, CoreSnapshot};
use infiltrator_domain::core_state::{CoreState, CoreStateMachine};
use infiltrator_ports::application_runtime::ApplicationRuntime;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::overview::OverviewReader;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, RwLock};

use crate::command_application::CommandHandler;

const EVENT_CAPACITY: usize = 256;
const DISPATCH_CAPACITY: usize = 256;
const DEFAULT_READINESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
const DEFAULT_READINESS_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);

/// Readiness retry policy expressed entirely in standard-library values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadinessPolicy {
    pub timeout: std::time::Duration,
    pub poll_interval: std::time::Duration,
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
    process: Arc<dyn CoreProcess>,
    readiness: Arc<dyn CoreReadiness>,
    overview: Option<Arc<dyn OverviewReader>>,
    readiness_policy: ReadinessPolicy,
    runtime: Arc<dyn ApplicationRuntime>,
    command_handler: RwLock<Option<Arc<dyn CommandHandler>>>,
    dispatch_tx: std::sync::mpsc::SyncSender<DispatchedCommand>,
    operation: futures_util::lock::Mutex<()>,
    state: RwLock<StateMirror>,
    next_request_id: AtomicU64,
    events: Mutex<VecDeque<CoreEvent>>,
}

struct DispatchedCommand {
    request_id: RequestId,
    intent: CommandIntent,
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
        let (dispatch_tx, dispatch_rx) = std::sync::mpsc::sync_channel(DISPATCH_CAPACITY);
        let inner = Arc::new(Inner {
            process,
            readiness,
            overview,
            readiness_policy,
            runtime: Arc::clone(&runtime),
            command_handler: RwLock::new(None),
            dispatch_tx,
            operation: futures_util::lock::Mutex::new(()),
            state: RwLock::new(StateMirror {
                state: CoreState::Idle { generation: 0 },
                revision: 0,
            }),
            next_request_id: AtomicU64::new(1),
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
        *slot = Some(handler);
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
        let request_id = self.allocate_request_id();
        let kind = intent.kind();
        match self
            .inner
            .dispatch_tx
            .try_send(DispatchedCommand { request_id, intent })
        {
            Ok(()) => {}
            Err(std::sync::mpsc::TrySendError::Full(_)) => {
                self.push_event(CoreEvent::CommandFailed {
                    request_id,
                    kind,
                    failure: Failure::new(
                        ErrorCode::Internal,
                        "application command queue is full",
                        true,
                    ),
                });
            }
            Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                self.push_event(CoreEvent::CommandFailed {
                    request_id,
                    kind,
                    failure: Failure::new(
                        ErrorCode::Internal,
                        "application worker is no longer available",
                        true,
                    ),
                });
            }
        }
        request_id
    }

    /// Return the latest immutable contract projection.
    pub fn snapshot(&self) -> CoreSnapshot {
        let mirror = self.inner.state.read().expect("core state lock");
        snapshot_from_state(&mirror.state, mirror.revision)
    }

    /// Current lifecycle generation used to fence delayed surface work.
    pub fn generation(&self) -> u64 {
        self.snapshot().generation
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
        if !matches!(self.current_state(), CoreState::Idle { .. }) {
            return Ok(false);
        }

        let status = self.inner.process.status().await.map_err(Failure::from)?;
        if !matches!(status, CoreLifecycle::Running | CoreLifecycle::Ready) {
            return Ok(false);
        }

        self.apply_domain_event(infiltrator_domain::core_state::CoreEvent::StartRequested);
        match self
            .wait_for_readiness(self.inner.readiness_policy.timeout)
            .await
        {
            Ok(endpoint) => {
        self.apply_domain_event(
            infiltrator_domain::core_state::CoreEvent::ReadinessSuccess(endpoint),
        );
                Ok(true)
            }
            Err(error) => {
                let message = error.to_string();
        self.apply_domain_event(infiltrator_domain::core_state::CoreEvent::StartFailed(message));
                Err(Failure::from(error))
            }
        }
    }

    fn allocate_request_id(&self) -> RequestId {
        RequestId::new(self.inner.next_request_id.fetch_add(1, Ordering::Relaxed))
    }

    async fn execute_with_id(&self, request_id: RequestId, intent: CommandIntent) -> CommandResult {
        let kind = intent.kind();
        self.push_event(CoreEvent::CommandAccepted { request_id, kind });

        let _operation = self.inner.operation.lock().await;
        let outcome = match intent {
            CommandIntent::StartCore => self.start_locked().await,
            CommandIntent::StopCore => self.stop_locked().await,
            CommandIntent::RestartCore => self.restart_locked().await,
            CommandIntent::SetProxyMode { mode } => self.set_mode_locked(mode).await,
            unsupported => {
                let handler = self
                    .inner
                    .command_handler
                    .read()
                    .expect("command handler lock")
                    .clone();
                match handler {
                    Some(handler) => handler.handle(unsupported).await,
                    None => Err(Failure::unsupported(format!(
                        "command `{}` has no application command handler",
                        command_name(&unsupported)
                    ))),
                }
            }
        };

        match outcome {
            Ok(()) => {
                self.push_event(CoreEvent::CommandCompleted { request_id, kind });
                CommandResult::Completed { request_id }
            }
            Err(failure) => {
                self.push_event(CoreEvent::CommandFailed {
                    request_id,
                    kind,
                    failure: failure.clone(),
                });
                CommandResult::Rejected {
                    request_id,
                    failure,
                }
            }
        }
    }

    async fn set_mode_locked(
        &self,
        wanted: infiltrator_contract::command::ProxyMode,
    ) -> Result<(), Failure> {
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
                false,
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

        self.apply_domain_event(infiltrator_domain::core_state::CoreEvent::StartRequested);
        if let Err(error) = self.inner.process.start().await {
            let message = error.to_string();
            self.apply_domain_event(
                infiltrator_domain::core_state::CoreEvent::StartFailed(message.clone()),
            );
            return Err(Failure::from(error));
        }

        match self
            .wait_for_readiness(self.inner.readiness_policy.timeout)
            .await
        {
            Ok(endpoint) => {
                self.apply_domain_event(
                    infiltrator_domain::core_state::CoreEvent::ReadinessSuccess(endpoint),
                );
                Ok(())
            }
            Err(error) => {
                let message = error.to_string();
                self.apply_domain_event(
                    infiltrator_domain::core_state::CoreEvent::StartFailed(message),
                );
                Err(Failure::from(error))
            }
        }
    }

    async fn stop_locked(&self) -> Result<(), Failure> {
        if matches!(self.current_state(), CoreState::Idle { .. }) {
            return Err(invalid_state_failure("stop"));
        }

        self.apply_domain_event(infiltrator_domain::core_state::CoreEvent::StopRequested);
        if let Err(error) = self.inner.process.stop().await {
            let message = error.to_string();
            self.apply_domain_event(infiltrator_domain::core_state::CoreEvent::StopFailed(message));
            return Err(Failure::from(error));
        }

        self.apply_domain_event(infiltrator_domain::core_state::CoreEvent::StopCompleted);
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

    fn apply_domain_event(&self, event: infiltrator_domain::core_state::CoreEvent) {
        let snapshot = {
            let mut mirror = self.inner.state.write().expect("core state lock");
            let (state, warning) = CoreStateMachine::step(&mirror.state, event);
            mirror.state = state;
            mirror.revision = mirror.revision.saturating_add(1);
            if let Some(warning) = warning {
                log::warn!(target: "infiltrator-application", "core domain transition warning: {warning}");
            }
            snapshot_from_state(&mirror.state, mirror.revision)
        };
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
        timeout: std::time::Duration,
    ) -> Result<String, infiltrator_ports::error::PortError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            match self.inner.process.status().await {
                Ok(CoreLifecycle::Starting)
                | Ok(CoreLifecycle::Ready)
                | Ok(CoreLifecycle::Running) => {}
                Ok(_) => {
                    return Err(infiltrator_ports::error::PortError::Failed(
                        "core process exited before readiness".to_string(),
                    ));
                }
                Err(error) => return Err(error),
            }

            let probe_error = match self.inner.readiness.probe().await {
                Ok(endpoint) => return Ok(endpoint),
                Err(error) => error,
            };

            if std::time::Instant::now() >= deadline {
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

    async fn start(&self) -> Result<u64, infiltrator_ports::error::PortError> {
        match self.execute(CommandIntent::StartCore).await {
            CommandResult::Completed { .. } => Ok(self.generation()),
            CommandResult::Rejected { failure, .. } => {
                Err(infiltrator_ports::error::PortError::Failed(failure.message))
            }
            CommandResult::Accepted { .. } => Err(infiltrator_ports::error::PortError::Failed(
                "application start unexpectedly returned Accepted".to_string(),
            )),
        }
    }

    async fn stop(&self) -> Result<(), infiltrator_ports::error::PortError> {
        match self.execute(CommandIntent::StopCore).await {
            CommandResult::Completed { .. } => Ok(()),
            CommandResult::Rejected { failure, .. } => {
                Err(infiltrator_ports::error::PortError::Failed(failure.message))
            }
            CommandResult::Accepted { .. } => Err(infiltrator_ports::error::PortError::Failed(
                "application stop unexpectedly returned Accepted".to_string(),
            )),
        }
    }

    async fn restart(&self) -> Result<u64, infiltrator_ports::error::PortError> {
        match self.execute(CommandIntent::RestartCore).await {
            CommandResult::Completed { .. } => Ok(self.generation()),
            CommandResult::Rejected { failure, .. } => {
                Err(infiltrator_ports::error::PortError::Failed(failure.message))
            }
            CommandResult::Accepted { .. } => Err(infiltrator_ports::error::PortError::Failed(
                "application restart unexpectedly returned Accepted".to_string(),
            )),
        }
    }

    async fn wait_for_ready(
        &self,
        generation: u64,
        timeout: std::time::Duration,
    ) -> Result<(), infiltrator_ports::error::PortError> {
        if self.generation() != generation {
            return Err(infiltrator_ports::error::PortError::Failed(
                "stale application generation".to_string(),
            ));
        }
        self.wait_for_readiness(timeout).await.map(|_| ())
    }
}

fn spawn_dispatch_worker(
    inner: std::sync::Weak<Inner>,
    dispatch_rx: Receiver<DispatchedCommand>,
    runtime: Arc<dyn ApplicationRuntime>,
) {
    let _ = std::thread::Builder::new()
        .name("infiltrator-application".to_owned())
        .spawn(move || {
            while let Ok(command) = dispatch_rx.recv() {
                let Some(inner) = inner.upgrade() else {
                    return;
                };
                let application = CoreApplication { inner };
                runtime.block_on(Box::pin(async move {
                    let _ = application
                        .execute_with_id(command.request_id, command.intent)
                        .await;
                }));
            }
        });
}

fn command_name(intent: &CommandIntent) -> &'static str {
    match intent {
        CommandIntent::StartCore => "start_core",
        CommandIntent::StopCore => "stop_core",
        CommandIntent::RestartCore => "restart_core",
        CommandIntent::SwitchProfile { .. } => "switch_profile",
        CommandIntent::SetProxyMode { .. } => "set_proxy_mode",
        CommandIntent::SelectProxyNode { .. } => "select_proxy_node",
        CommandIntent::TestDelay { .. } => "test_delay",
        CommandIntent::UpdateProfile { .. } => "update_profile",
        CommandIntent::DeleteProfile { .. } => "delete_profile",
        CommandIntent::RefreshRuleProviders => "refresh_rule_providers",
        CommandIntent::CloseConnection { .. } => "close_connection",
        CommandIntent::CloseAllConnections => "close_all_connections",
        CommandIntent::ClearLogs => "clear_logs",
        CommandIntent::SetLogLevelFilter { .. } => "set_log_level_filter",
        CommandIntent::ClearDnsCache => "clear_dns_cache",
        CommandIntent::TestDnsLatency => "test_dns_latency",
        CommandIntent::RunDoctorDiagnostics => "run_doctor_diagnostics",
        CommandIntent::RepairDoctorIssue { .. } => "repair_doctor_issue",
        CommandIntent::RepairAllDoctorIssues => "repair_all_doctor_issues",
        CommandIntent::ToggleTun { .. } => "toggle_tun",
        CommandIntent::SetSystemProxy { .. } => "set_system_proxy",
        CommandIntent::ToggleAppRouting { .. } => "toggle_app_routing",
        CommandIntent::SetAppRoutingMode { .. } => "set_app_routing_mode",
        CommandIntent::ToggleIncludeSystemApps { .. } => "toggle_include_system_apps",
        CommandIntent::SetAppRule { .. } => "set_app_rule",
        CommandIntent::SyncNow => "sync_now",
        CommandIntent::CreateBackupSnapshot => "create_backup_snapshot",
        CommandIntent::ResolveConflictKeepLocal => "resolve_conflict_keep_local",
        CommandIntent::ResolveConflictTakeRemote => "resolve_conflict_take_remote",
        CommandIntent::RestoreSnapshot { .. } => "restore_snapshot",
        CommandIntent::UpdateSetting { .. } => "update_setting",
        CommandIntent::CheckUpdates => "check_updates",
    }
}

fn invalid_state_failure(operation: &str) -> Failure {
    Failure::new(
        ErrorCode::InvalidState,
        format!("operation `{operation}` is not valid in the current core state"),
        false,
    )
}

fn snapshot_from_state(state: &CoreState, revision: u64) -> CoreSnapshot {
    let (lifecycle, generation, failure) = match state {
        CoreState::Idle { generation } => (CoreLifecycle::Stopped, *generation, None),
        CoreState::Starting { generation } => (CoreLifecycle::Starting, *generation, None),
        CoreState::Running { generation, .. } => (CoreLifecycle::Running, *generation, None),
        CoreState::Reloading { generation, .. } => (CoreLifecycle::Ready, *generation, None),
        CoreState::Stopping { generation } => (CoreLifecycle::Stopping, *generation, None),
        CoreState::Failed { generation, error } => (
            CoreLifecycle::Failed,
            *generation,
            Some(Failure::new(ErrorCode::Internal, error.clone(), false)),
        ),
    };

    CoreSnapshot {
        lifecycle,
        generation,
        revision,
        proxy_mode: None,
        core_version: None,
        sampled_at_epoch_ms: None,
        failure,
        upload_bps: 0.0,
        download_bps: 0.0,
        active_connections: 0,
        memory_bytes: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::command::{CommandResult, ProxyMode};
    use infiltrator_ports::application_runtime::{
        ApplicationFuture, ApplicationRuntime, ApplicationSleep,
    };
    use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
    use infiltrator_ports::error::PortError;
    use infiltrator_ports::overview::{OverviewReader, OverviewSample};
    use std::sync::atomic::AtomicBool;

    struct FakeProcess {
        running: AtomicBool,
        fail_start: bool,
        fail_stop: bool,
    }

    #[async_trait::async_trait]
    impl CoreProcess for FakeProcess {
        async fn start(&self) -> Result<(), PortError> {
            if self.fail_start {
                return Err(PortError::Failed("fake start failure".to_string()));
            }
            self.running.store(true, Ordering::SeqCst);
            Ok(())
        }

        async fn stop(&self) -> Result<(), PortError> {
            if self.fail_stop {
                return Err(PortError::PermissionDenied(
                    "fake stop permission denied".to_string(),
                ));
            }
            self.running.store(false, Ordering::SeqCst);
            Ok(())
        }

        async fn status(&self) -> Result<CoreLifecycle, PortError> {
            Ok(if self.running.load(Ordering::SeqCst) {
                CoreLifecycle::Running
            } else {
                CoreLifecycle::Stopped
            })
        }

        fn controller_endpoint(&self) -> Option<String> {
            Some("http://127.0.0.1:9090".to_string())
        }
    }

    struct FakeReadiness {
        endpoint: Result<String, PortError>,
    }

    #[async_trait::async_trait]
    impl CoreReadiness for FakeReadiness {
        async fn probe(&self) -> Result<String, PortError> {
            match &self.endpoint {
                Ok(endpoint) => Ok(endpoint.clone()),
                Err(error) => Err(error.clone()),
            }
        }
    }

    struct FakeOverview {
        mode: Result<ProxyMode, PortError>,
    }

    struct FakeCommandHandler;

    impl crate::command_application::CommandHandler for FakeCommandHandler {
        fn handle(
            &self,
            intent: infiltrator_contract::command::CommandIntent,
        ) -> crate::command_application::CommandFuture {
            Box::pin(async move {
                if matches!(intent, infiltrator_contract::command::CommandIntent::ClearLogs) {
                    Ok(())
                } else {
                    Err(Failure::unsupported("fake handler rejected command"))
                }
            })
        }
    }

    struct TestRuntime;

    impl ApplicationRuntime for TestRuntime {
        fn block_on(&self, future: ApplicationFuture) {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("test runtime")
                .block_on(future);
        }

        fn sleep(&self, duration: std::time::Duration) -> ApplicationSleep<'_> {
            Box::pin(tokio::time::sleep(duration))
        }
    }

    fn runtime() -> Arc<dyn ApplicationRuntime> {
        Arc::new(TestRuntime)
    }

    #[async_trait::async_trait]
    impl OverviewReader for FakeOverview {
        async fn sample(&self) -> Result<OverviewSample, PortError> {
            Err(PortError::Failed(
                "sample not needed in this test".to_string(),
            ))
        }

        async fn set_mode(&self, _mode: ProxyMode) -> Result<ProxyMode, PortError> {
            self.mode.clone()
        }
    }

    fn application(process: FakeProcess, readiness: Result<String, PortError>) -> CoreApplication {
        CoreApplication::new_with_policy(
            Arc::new(process),
            Arc::new(FakeReadiness {
                endpoint: readiness,
            }),
            ReadinessPolicy {
                timeout: std::time::Duration::from_millis(100),
                poll_interval: std::time::Duration::from_millis(1),
            },
            runtime(),
        )
    }

    #[tokio::test]
    async fn lifecycle_commands_publish_only_contract_values() {
        let app = application(
            FakeProcess {
                running: AtomicBool::new(false),
                fail_start: false,
                fail_stop: false,
            },
            Ok("http://127.0.0.1:9090".to_string()),
        );

        let started = app.execute(CommandIntent::StartCore).await;
        assert_eq!(
            started,
            CommandResult::Completed {
                request_id: RequestId(1)
            }
        );
        assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Running);
        assert_eq!(app.snapshot().generation, 1);
        assert_eq!(app.snapshot().revision, 2);

        let events = app.drain_events();
        assert!(matches!(
            events.first(),
            Some(CoreEvent::CommandAccepted { .. })
        ));
        assert!(matches!(events.get(1), Some(CoreEvent::SnapshotUpdated(_))));
        assert!(matches!(events.get(2), Some(CoreEvent::SnapshotUpdated(_))));
        assert!(matches!(
            events.get(3),
            Some(CoreEvent::CommandCompleted { .. })
        ));

        let stopped = app.execute(CommandIntent::StopCore).await;
        assert_eq!(
            stopped,
            CommandResult::Completed {
                request_id: RequestId(2)
            }
        );
        assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Stopped);
        assert_eq!(app.snapshot().generation, 1);
    }

    #[tokio::test]
    async fn adapter_failures_become_typed_contract_failures() {
        let app = application(
            FakeProcess {
                running: AtomicBool::new(false),
                fail_start: true,
                fail_stop: false,
            },
            Ok("http://127.0.0.1:9090".to_string()),
        );

        let result = app.execute(CommandIntent::StartCore).await;
        let CommandResult::Rejected { failure, .. } = result else {
            panic!("expected rejected command");
        };
        assert_eq!(failure.code, ErrorCode::Internal);
        assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Failed);
        assert_eq!(app.snapshot().generation, 1);

        let events = app.drain_events();
        assert!(events.iter().any(|event| matches!(
            event,
            CoreEvent::CommandFailed { failure, .. } if failure.code == ErrorCode::Internal
        )));
    }

    #[tokio::test]
    async fn readiness_failures_never_report_running() {
        let app = application(
            FakeProcess {
                running: AtomicBool::new(false),
                fail_start: false,
                fail_stop: false,
            },
            Err(PortError::Network("controller unavailable".to_string())),
        );

        let result = app.execute(CommandIntent::StartCore).await;
        let CommandResult::Rejected { failure, .. } = result else {
            panic!("expected rejected command");
        };
        assert_eq!(failure.code, ErrorCode::Network);
        assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Failed);
        assert!(
            !app.drain_events()
                .iter()
                .any(|event| matches!(event, CoreEvent::CommandCompleted { .. }))
        );
    }

    #[tokio::test]
    async fn dispatch_returns_id_and_completes_through_event_queue() {
        let app = application(
            FakeProcess {
                running: AtomicBool::new(false),
                fail_start: false,
                fail_stop: false,
            },
            Ok("http://127.0.0.1:9090".to_string()),
        );

        let request_id = app.dispatch(CommandIntent::StartCore);
        assert_eq!(request_id, RequestId(1));

        for _ in 0..16 {
            if app
                .drain_events()
                .iter()
                .any(|event| matches!(event, CoreEvent::CommandCompleted { .. }))
            {
                assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Running);
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        panic!("dispatched command did not complete");
    }

    #[tokio::test]
    async fn adopt_checks_host_status_without_starting_the_process() {
        let process = FakeProcess {
            running: AtomicBool::new(true),
            fail_start: false,
            fail_stop: false,
        };
        let app = application(process, Ok("http://127.0.0.1:9090".to_string()));

        assert!(app.adopt_if_running().await.expect("adopt succeeds"));
        assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Running);
        assert_eq!(app.snapshot().generation, 1);
    }

    #[tokio::test]
    async fn mode_commands_use_the_injected_overview_port() {
        let app = CoreApplication::new_with_overview(
            Arc::new(FakeProcess {
                running: AtomicBool::new(true),
                fail_start: false,
                fail_stop: false,
            }),
            Arc::new(FakeReadiness {
                endpoint: Ok("http://127.0.0.1:9090".to_string()),
            }),
            Arc::new(FakeOverview {
                mode: Ok(ProxyMode::Global),
            }),
            runtime(),
        );

        let result = app
            .execute(CommandIntent::SetProxyMode {
                mode: ProxyMode::Global,
            })
            .await;
        assert!(matches!(result, CommandResult::Completed { .. }));
    }

    #[tokio::test]
    async fn non_lifecycle_commands_use_the_installed_handler() {
        let app = application(
            FakeProcess {
                running: AtomicBool::new(false),
                fail_start: false,
                fail_stop: false,
            },
            Ok("http://127.0.0.1:9090".to_string()),
        );
        app.install_command_handler(Arc::new(FakeCommandHandler));

        let result = app.execute(CommandIntent::ClearLogs).await;
        assert_eq!(
            result,
            CommandResult::Completed {
                request_id: RequestId(1)
            }
        );
    }

    #[test]
    fn dispatch_does_not_require_a_caller_owned_runtime() {
        let app = application(
            FakeProcess {
                running: AtomicBool::new(false),
                fail_start: false,
                fail_stop: false,
            },
            Ok("http://127.0.0.1:9090".to_string()),
        );

        assert_eq!(app.dispatch(CommandIntent::StartCore), RequestId(1));
        for _ in 0..100 {
            if app
                .drain_events()
                .iter()
                .any(|event| matches!(event, CoreEvent::CommandCompleted { .. }))
            {
                assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Running);
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("dispatched command did not complete without a caller runtime");
    }
}

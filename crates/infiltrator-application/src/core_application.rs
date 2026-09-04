use infiltrator_contract::command::{CommandIntent, CommandResult, RequestId};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::{CoreEvent, CoreLifecycle, CoreSnapshot};
use infiltrator_domain::core_state::{CoreEvent as DomainEvent, CoreState, CoreStateMachine};
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

const EVENT_CAPACITY: usize = 256;

struct StateMirror {
    state: CoreState,
    revision: u64,
}

struct Inner {
    process: Arc<dyn CoreProcess>,
    readiness: Arc<dyn CoreReadiness>,
    operation: tokio::sync::Mutex<()>,
    state: RwLock<StateMirror>,
    next_request_id: AtomicU64,
    events: Mutex<VecDeque<CoreEvent>>,
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
    pub fn new(process: Arc<dyn CoreProcess>, readiness: Arc<dyn CoreReadiness>) -> Self {
        Self {
            inner: Arc::new(Inner {
                process,
                readiness,
                operation: tokio::sync::Mutex::new(()),
                state: RwLock::new(StateMirror {
                    state: CoreState::Idle { generation: 0 },
                    revision: 0,
                }),
                next_request_id: AtomicU64::new(1),
                events: Mutex::new(VecDeque::with_capacity(EVENT_CAPACITY)),
            }),
        }
    }

    /// Execute a command and await its terminal result. The returned values
    /// contain no executor-specific types.
    pub async fn execute(&self, intent: CommandIntent) -> CommandResult {
        let request_id = self.allocate_request_id();
        self.execute_with_id(request_id, intent).await
    }

    /// Schedule a command on the application's Tokio runtime and return only
    /// its correlation id. Completion/failure is observed through
    /// [`Self::drain_events`]. This method must be called from a running Tokio
    /// runtime; frontends that own the runtime can instead use [`Self::execute`].
    pub fn dispatch(&self, intent: CommandIntent) -> RequestId {
        let request_id = self.allocate_request_id();
        let application = self.clone();
        tokio::spawn(async move {
            let _ = application.execute_with_id(request_id, intent).await;
        });
        request_id
    }

    /// Return the latest immutable contract projection.
    pub fn snapshot(&self) -> CoreSnapshot {
        let mirror = self.inner.state.read().expect("core state lock");
        snapshot_from_state(&mirror.state, mirror.revision)
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

        self.apply_domain_event(DomainEvent::StartRequested);
        match self.inner.readiness.probe().await {
            Ok(endpoint) => {
                self.apply_domain_event(DomainEvent::ReadinessSuccess(endpoint));
                Ok(true)
            }
            Err(error) => {
                let message = error.to_string();
                self.apply_domain_event(DomainEvent::StartFailed(message));
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
            unsupported => Err(Failure::unsupported(format!(
                "command `{}` is not wired into the 0.30 application yet",
                command_name(&unsupported)
            ))),
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

    async fn start_locked(&self) -> Result<(), Failure> {
        if !matches!(
            self.current_state(),
            CoreState::Idle { .. } | CoreState::Failed { .. }
        ) {
            return Err(invalid_state_failure("start"));
        }

        self.apply_domain_event(DomainEvent::StartRequested);
        if let Err(error) = self.inner.process.start().await {
            let message = error.to_string();
            self.apply_domain_event(DomainEvent::StartFailed(message.clone()));
            return Err(Failure::from(error));
        }

        match self.inner.readiness.probe().await {
            Ok(endpoint) => {
                self.apply_domain_event(DomainEvent::ReadinessSuccess(endpoint));
                Ok(())
            }
            Err(error) => {
                let message = error.to_string();
                self.apply_domain_event(DomainEvent::StartFailed(message));
                Err(Failure::from(error))
            }
        }
    }

    async fn stop_locked(&self) -> Result<(), Failure> {
        if matches!(self.current_state(), CoreState::Idle { .. }) {
            return Err(invalid_state_failure("stop"));
        }

        self.apply_domain_event(DomainEvent::StopRequested);
        if let Err(error) = self.inner.process.stop().await {
            let message = error.to_string();
            self.apply_domain_event(DomainEvent::StopFailed(message));
            return Err(Failure::from(error));
        }

        self.apply_domain_event(DomainEvent::StopCompleted);
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

    fn apply_domain_event(&self, event: DomainEvent) {
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
}

fn command_name(intent: &CommandIntent) -> &'static str {
    match intent {
        CommandIntent::StartCore => "start_core",
        CommandIntent::StopCore => "stop_core",
        CommandIntent::RestartCore => "restart_core",
        CommandIntent::SwitchProfile { .. } => "switch_profile",
        CommandIntent::SetProxyMode { .. } => "set_proxy_mode",
        CommandIntent::SelectProxyNode { .. } => "select_proxy_node",
        CommandIntent::UpdateProfile { .. } => "update_profile",
        CommandIntent::RefreshRuleProviders => "refresh_rule_providers",
        CommandIntent::CloseConnection { .. } => "close_connection",
        CommandIntent::CloseAllConnections => "close_all_connections",
        CommandIntent::ClearDnsCache => "clear_dns_cache",
        CommandIntent::ToggleTun { .. } => "toggle_tun",
        CommandIntent::SetSystemProxy { .. } => "set_system_proxy",
        CommandIntent::SyncNow => "sync_now",
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::command::CommandResult;
    use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
    use infiltrator_ports::error::PortError;
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

    fn application(process: FakeProcess, readiness: Result<String, PortError>) -> CoreApplication {
        CoreApplication::new(
            Arc::new(process),
            Arc::new(FakeReadiness {
                endpoint: readiness,
            }),
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
            tokio::task::yield_now().await;
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
}

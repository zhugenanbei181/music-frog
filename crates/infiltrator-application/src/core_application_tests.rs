use super::*;
use infiltrator_contract::command::{CommandResult, ProxyMode};
use infiltrator_contract::snapshot::CoreWatchdogState;
use infiltrator_domain::watchdog::WatchdogConfig;
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep,
};
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::core_watchdog::{CoreWatchdogPort, WatchdogTick};
use infiltrator_ports::error::PortError;
use infiltrator_ports::overview::{OverviewReader, OverviewSample};
use std::sync::atomic::AtomicBool;

struct FakeProcess {
    running: AtomicBool,
    fail_start: bool,
    fail_stop: bool,
}

struct WatchdogProcess {
    running: Arc<AtomicBool>,
    starts: Arc<AtomicU64>,
    fail_starts: Arc<AtomicU64>,
}

#[async_trait::async_trait]
impl CoreProcess for WatchdogProcess {
    async fn start(&self) -> Result<(), PortError> {
        self.starts.fetch_add(1, Ordering::SeqCst);
        if self.fail_starts.load(Ordering::SeqCst) > 0 {
            self.fail_starts.fetch_sub(1, Ordering::SeqCst);
            return Err(PortError::Failed("watchdog restart rejected".to_string()));
        }
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn stop(&self) -> Result<(), PortError> {
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

struct CleanupTrackingProcess {
    running: AtomicBool,
    cleanup_called: AtomicBool,
}

#[async_trait::async_trait]
impl CoreProcess for CleanupTrackingProcess {
    async fn start(&self) -> Result<(), PortError> {
        assert!(
            self.cleanup_called.load(Ordering::SeqCst),
            "application must reconcile orphan state before starting"
        );
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn stop(&self) -> Result<(), PortError> {
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

    async fn cleanup_orphaned(&self) -> Result<Option<u32>, PortError> {
        self.cleanup_called.store(true, Ordering::SeqCst);
        Ok(Some(4242))
    }
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
            if matches!(
                intent,
                infiltrator_contract::command::CommandIntent::ClearLogs
            ) {
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
    let lifecycle = app.lifecycle_snapshot();
    assert_eq!(lifecycle.lifecycle, CoreLifecycle::Running);
    assert_eq!(lifecycle.generation, 1);
    assert_eq!(lifecycle.session_token, app.snapshot().session_token);
    assert_eq!(lifecycle.revision, app.snapshot().revision);
    let first_session = app
        .snapshot()
        .session_token
        .expect("running core must expose a session token");
    assert!(first_session.is_valid());
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
    assert!(app.snapshot().session_token.is_none());

    let restarted = app.execute(CommandIntent::StartCore).await;
    assert!(matches!(restarted, CommandResult::Completed { .. }));
    let second_session = app
        .snapshot()
        .session_token
        .expect("restarted core must expose a session token");
    assert_ne!(first_session, second_session);
    assert_eq!(app.snapshot().generation, 2);
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
    assert!(app.snapshot().session_token.is_some());
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

#[tokio::test]
async fn stale_session_tokens_are_rejected_after_stop_and_restart() {
    let app = application(
        FakeProcess {
            running: AtomicBool::new(false),
            fail_start: false,
            fail_stop: false,
        },
        Ok("http://127.0.0.1:9090".to_string()),
    );

    app.execute(CommandIntent::StartCore).await;
    let old_token = app.session_token().expect("session token after start");
    assert!(app.check_session(old_token).is_ok());

    app.execute(CommandIntent::StopCore).await;
    assert!(app.check_session(old_token).is_err());

    app.execute(CommandIntent::StartCore).await;
    let new_token = app.session_token().expect("session token after restart");
    assert_ne!(old_token, new_token);
    assert!(app.check_session(old_token).is_err());
    assert!(app.check_session(new_token).is_ok());
}

#[tokio::test]
async fn start_reconciles_orphan_state_before_spawning_a_new_session() {
    let process = Arc::new(CleanupTrackingProcess {
        running: AtomicBool::new(false),
        cleanup_called: AtomicBool::new(false),
    });
    let app = CoreApplication::new_with_policy(
        process.clone(),
        Arc::new(FakeReadiness {
            endpoint: Ok("http://127.0.0.1:9090".to_string()),
        }),
        ReadinessPolicy {
            timeout: std::time::Duration::from_millis(100),
            poll_interval: std::time::Duration::from_millis(1),
        },
        runtime(),
    );

    let result = app.execute(CommandIntent::StartCore).await;
    assert!(matches!(result, CommandResult::Completed { .. }));
    assert!(process.cleanup_called.load(Ordering::SeqCst));
    assert_eq!(app.snapshot().generation, 1);
    assert!(app.snapshot().session_token.is_some());
}

#[tokio::test]
async fn hot_reload_fences_to_the_current_session_without_bumping_generation() {
    let app = application(
        FakeProcess {
            running: AtomicBool::new(false),
            fail_start: false,
            fail_stop: false,
        },
        Ok("http://127.0.0.1:9090".to_string()),
    );
    app.execute(CommandIntent::StartCore).await;
    let before = app.snapshot();
    let token = CoreLifecyclePort::begin_reload(&app).expect("begin reload");
    assert_eq!(Some(token), before.session_token);
    assert_eq!(app.snapshot().generation, before.generation);
    assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Ready);

    CoreLifecyclePort::complete_reload(&app, token).expect("complete reload");
    assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Running);
    assert_eq!(app.snapshot().generation, before.generation);

    let token = CoreLifecyclePort::begin_reload(&app).expect("begin second reload");
    CoreLifecyclePort::fail_reload(&app, token, "invalid config".to_string())
        .expect("record reload failure");
    assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Running);
    assert_eq!(app.snapshot().session_token, Some(token));
}

#[tokio::test]
async fn watchdog_detects_exit_and_restarts_with_a_new_session() {
    let running = Arc::new(AtomicBool::new(false));
    let starts = Arc::new(AtomicU64::new(0));
    let process = Arc::new(WatchdogProcess {
        running: running.clone(),
        starts: starts.clone(),
        fail_starts: Arc::new(AtomicU64::new(0)),
    });
    let app = CoreApplication::new_with_policy(
        process,
        Arc::new(FakeReadiness {
            endpoint: Ok("http://127.0.0.1:9090".to_string()),
        }),
        ReadinessPolicy {
            timeout: std::time::Duration::from_millis(100),
            poll_interval: std::time::Duration::from_millis(1),
        },
        runtime(),
    );
    app.configure_watchdog(WatchdogConfig {
        initial_retry_delay: std::time::Duration::ZERO,
        max_retry_delay: std::time::Duration::from_millis(1),
        max_restart_attempts: 2,
    });

    assert!(matches!(
        app.execute(CommandIntent::StartCore).await,
        CommandResult::Completed { .. }
    ));
    let before = app.snapshot();
    running.store(false, Ordering::SeqCst);

    assert!(matches!(
        CoreWatchdogPort::watchdog_tick(&app)
            .await
            .expect("detect exit"),
        WatchdogTick::Waiting {
            attempt: 1,
            retry_in_ms: 0,
            ..
        }
    ));
    assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Failed);

    assert!(matches!(
        CoreWatchdogPort::watchdog_tick(&app)
            .await
            .expect("recover exit"),
        WatchdogTick::Recovered { attempts: 1, .. }
    ));
    let after = app.snapshot();
    assert_eq!(after.lifecycle, CoreLifecycle::Running);
    assert!(after.generation > before.generation);
    assert_ne!(after.session_token, before.session_token);
    assert!(matches!(
        after.watchdog.state,
        CoreWatchdogState::Recovered { attempts: 1 }
    ));
    assert_eq!(starts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn watchdog_opens_the_circuit_after_repeated_restart_failures() {
    let running = Arc::new(AtomicBool::new(false));
    let starts = Arc::new(AtomicU64::new(0));
    let fail_starts = Arc::new(AtomicU64::new(0));
    let process = Arc::new(WatchdogProcess {
        running: running.clone(),
        starts: starts.clone(),
        fail_starts: fail_starts.clone(),
    });
    let app = CoreApplication::new_with_policy(
        process,
        Arc::new(FakeReadiness {
            endpoint: Ok("http://127.0.0.1:9090".to_string()),
        }),
        ReadinessPolicy {
            timeout: std::time::Duration::from_millis(100),
            poll_interval: std::time::Duration::from_millis(1),
        },
        runtime(),
    );
    app.configure_watchdog(WatchdogConfig {
        initial_retry_delay: std::time::Duration::ZERO,
        max_retry_delay: std::time::Duration::from_millis(1),
        max_restart_attempts: 2,
    });

    assert!(matches!(
        app.execute(CommandIntent::StartCore).await,
        CommandResult::Completed { .. }
    ));
    fail_starts.store(2, Ordering::SeqCst);
    let before = app.snapshot();
    running.store(false, Ordering::SeqCst);

    assert!(matches!(
        CoreWatchdogPort::watchdog_tick(&app)
            .await
            .expect("detect exit"),
        WatchdogTick::Waiting { attempt: 1, .. }
    ));
    assert!(matches!(
        CoreWatchdogPort::watchdog_tick(&app)
            .await
            .expect("first failed restart"),
        WatchdogTick::Waiting { attempt: 2, .. }
    ));
    assert!(matches!(
        CoreWatchdogPort::watchdog_tick(&app)
            .await
            .expect("second failed restart"),
        WatchdogTick::Tripped { attempts: 2, .. }
    ));
    assert!(matches!(
        app.snapshot().watchdog.state,
        CoreWatchdogState::Tripped { attempts: 2 }
    ));
    assert_eq!(app.snapshot().generation, before.generation + 2);
    assert_eq!(starts.load(Ordering::SeqCst), 3);
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

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

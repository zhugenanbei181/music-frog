use super::*;
use crate::command_application::handler::{
    CommandFuture, CommandHandler, CommandOutputFuture, ValidatedCoreCommandScope,
};
use infiltrator_contract::command::{CommandIntent, CommandKind, CommandResult};
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, AdapterRevision, CoreCapabilityEntry, CoreCapabilityMatrix,
    CoreCapabilityScope, CoreLifecycleOperation, CoreOperation, EngineCapabilityDecision,
    HostCapabilityDecision,
};
use infiltrator_contract::core_identity::{
    CoreCommandRequest, CoreIdentity, CoreInstanceId, CoreVersion, EngineId,
};
use infiltrator_contract::outbound::{
    identity::{OutboundId, SnapshotRevision},
    probe::OutboundSelectionRequest,
};
use infiltrator_contract::routing_intent::RoutingIntent;
use infiltrator_contract::surface::HostKind;
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep,
};
use infiltrator_ports::core_capability_provider::{
    CoreCapabilityProvider, CoreCapabilityProviderError,
};
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::error::PortError;
use infiltrator_ports::outbound::OutboundScopeSource;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::{thread, time};
use tokio::runtime::Builder;
use tokio::time::sleep;

#[path = "core_application/ownership_tests.rs"]
mod ownership_tests;
#[path = "core_application/speedtest_actor_tests.rs"]
mod speedtest_actor_tests;
#[path = "core_application/telemetry_tests.rs"]
mod telemetry_tests;
#[path = "core_application/transaction_tests.rs"]
mod transaction_tests;
#[path = "core_application/watchdog_tests.rs"]
mod watchdog_tests;

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
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new("mihomo").ok()?,
            CoreInstanceId::new("watchdog-test").ok()?,
            None,
        ))
    }

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
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new("fake-core").ok()?,
            CoreInstanceId::new("test-instance").ok()?,
            None,
        ))
    }

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

struct ReadinessRollbackProcess {
    state: Arc<Mutex<CoreLifecycle>>,
    stop_calls: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl CoreProcess for ReadinessRollbackProcess {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new("mihomo").ok()?,
            CoreInstanceId::new("readiness-rollback-test").ok()?,
            None,
        ))
    }

    async fn start(&self) -> Result<(), PortError> {
        *self.state.lock().expect("process state lock") = CoreLifecycle::Running;
        Ok(())
    }

    async fn stop(&self) -> Result<(), PortError> {
        self.stop_calls.fetch_add(1, Ordering::SeqCst);
        *self.state.lock().expect("process state lock") = CoreLifecycle::Stopped;
        Ok(())
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(self.state.lock().expect("process state lock").clone())
    }

    fn controller_endpoint(&self) -> Option<String> {
        Some("http://127.0.0.1:9090".to_owned())
    }
}

struct CleanupTrackingProcess {
    running: AtomicBool,
    cleanup_called: AtomicBool,
}

#[async_trait::async_trait]
impl CoreProcess for CleanupTrackingProcess {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new("mihomo").ok()?,
            CoreInstanceId::new("cleanup-test").ok()?,
            None,
        ))
    }

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

    async fn active_core_version(&self) -> Result<Option<CoreVersion>, PortError> {
        Ok(Some(
            CoreVersion::new("1.19.12").expect("valid reported test version"),
        ))
    }
}

struct VersionUnavailableReadiness;

#[async_trait::async_trait]
impl CoreReadiness for VersionUnavailableReadiness {
    async fn probe(&self) -> Result<String, PortError> {
        Ok("http://127.0.0.1:9090".to_string())
    }
}

struct FakeCommandHandler;

impl CommandHandler for FakeCommandHandler {
    fn handle(&self, intent: CommandIntent) -> CommandFuture {
        Box::pin(async move {
            if matches!(intent, CommandIntent::ClearLogs) {
                Ok(())
            } else {
                Err(Failure::unsupported("fake handler rejected command"))
            }
        })
    }

    fn handle_output_scoped(
        &self,
        scope: ValidatedCoreCommandScope,
        intent: CommandIntent,
    ) -> CommandOutputFuture {
        if scope.as_ref().is_some() && matches!(intent, CommandIntent::ClearLogs) {
            let handled = self.handle(intent);
            Box::pin(async move { handled.await.map(|()| CommandOutput::Unit) })
        } else {
            Box::pin(async {
                Err(Failure::unsupported(
                    "test fake only handles scoped log clear",
                ))
            })
        }
    }
}

struct TestRuntime;

impl ApplicationRuntime for TestRuntime {
    fn block_on(&self, future: ApplicationFuture) {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
            .block_on(future);
    }

    fn sleep(&self, duration: time::Duration) -> ApplicationSleep<'_> {
        Box::pin(sleep(duration))
    }
}

fn runtime() -> Arc<dyn ApplicationRuntime> {
    Arc::new(TestRuntime)
}

struct TestLifecycleCapabilityProvider;

impl CoreCapabilityProvider for TestLifecycleCapabilityProvider {
    fn matrix_for(
        &self,
        engine_id: &EngineId,
        core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        let scope = CoreCapabilityScope::new(
            engine_id.clone(),
            core_version.cloned(),
            HostKind::Desktop,
            AdapterRevision::new(1),
        );
        let entries = [
            CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
            CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
            CoreOperation::Lifecycle(CoreLifecycleOperation::Restart),
        ]
        .into_iter()
        .map(|operation| {
            CoreCapabilityEntry::new(
                operation,
                EngineCapabilityDecision::Supported,
                HostCapabilityDecision::Supported,
                AdapterCapabilityDecision::Supported,
            )
        })
        .collect();
        CoreCapabilityMatrix::new(scope, entries).map_err(|error| {
            CoreCapabilityProviderError::MatrixUnavailable {
                reason: error.to_string(),
            }
        })
    }
}

pub(crate) fn install_lifecycle_capabilities(application: &CoreApplication) {
    application
        .install_core_capability_application(CoreCapabilityApplication::new(Arc::new(
            TestLifecycleCapabilityProvider,
        )))
        .expect("test lifecycle capabilities install before use");
}

fn application(process: FakeProcess, readiness: Result<String, PortError>) -> CoreApplication {
    let application = CoreApplication::new_with_policy(
        Arc::new(process),
        Arc::new(FakeReadiness {
            endpoint: readiness,
        }),
        ReadinessPolicy {
            timeout: time::Duration::from_millis(100),
            poll_interval: time::Duration::from_millis(1),
        },
        runtime(),
    );
    install_lifecycle_capabilities(&application);
    application
}

fn readiness_rollback_application(
    process_state: Arc<Mutex<CoreLifecycle>>,
    stop_calls: Arc<AtomicUsize>,
) -> CoreApplication {
    let application = CoreApplication::new_with_policy(
        Arc::new(ReadinessRollbackProcess {
            state: process_state,
            stop_calls,
        }),
        Arc::new(FakeReadiness {
            endpoint: Err(PortError::Network("controller unavailable".to_owned())),
        }),
        ReadinessPolicy {
            timeout: time::Duration::from_millis(10),
            poll_interval: time::Duration::from_millis(1),
        },
        runtime(),
    );
    install_lifecycle_capabilities(&application);
    application
}

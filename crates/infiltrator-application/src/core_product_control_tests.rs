use infiltrator_contract::core_process::CoreProcessStartContext;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time;

use crate::android_control_application::AndroidControlApplication;
use crate::command_application::handler::{
    CommandFuture, CommandHandler, CommandOutputFuture, CoreLifecycleChildPermit,
    ValidatedCoreCommandScope,
};
use crate::core_application::ReadinessPolicy;
use crate::core_application::{CoreApplication, test_execute_result_at_current_scope};
use crate::core_capability_application::CoreCapabilityApplication;
use infiltrator_contract::android_control::AndroidProductControlIntent;
use infiltrator_contract::command::{CommandIntent, CommandResult};
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, AdapterRevision, CoreCapabilityEntry, CoreCapabilityMatrix,
    CoreCapabilityScope, CoreLifecycleOperation, CoreOperation, EngineBinaryTarget,
    EngineCapabilityDecision, HostCapabilityDecision,
};
use infiltrator_contract::core_identity::{
    CoreCommandRequest, CoreIdentity, CoreInstanceId, CoreVersion, EngineId,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};
use infiltrator_contract::surface::HostKind;
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep,
};
use infiltrator_ports::core_capability_provider::{
    CoreCapabilityProvider, CoreCapabilityProviderError,
};
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::error::PortError;
use tokio::runtime::Builder;
use tokio::time::sleep;

struct FakeReadiness {
    endpoint: Result<String, PortError>,
}

struct VersionUnavailableReadiness;

#[async_trait::async_trait]
impl CoreReadiness for VersionUnavailableReadiness {
    async fn probe(&self) -> Result<String, PortError> {
        Ok("http://127.0.0.1:9090".to_owned())
    }
}

#[async_trait::async_trait]
impl CoreReadiness for FakeReadiness {
    async fn probe(&self) -> Result<String, PortError> {
        self.endpoint.clone()
    }

    async fn active_core_version(&self) -> Result<Option<CoreVersion>, PortError> {
        Ok(Some(
            CoreVersion::new("1.19.12").expect("valid reported test version"),
        ))
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProductControlStep {
    Authorization,
    CoreStart,
    VpnStart,
    VpnStop,
    CoreStop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VpnAuthorizationOutcome {
    Granted,
    Denied,
}

type ProductControlApplicationParts = (
    Arc<CoreApplication>,
    AndroidControlApplication,
    Arc<AtomicBool>,
    Arc<Mutex<Vec<ProductControlStep>>>,
    Arc<Mutex<Vec<Option<CoreVersion>>>>,
);

struct ProductControlProcess {
    running: Arc<AtomicBool>,
    steps: Arc<Mutex<Vec<ProductControlStep>>>,
}

#[async_trait::async_trait]
impl CoreProcess for ProductControlProcess {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new("mihomo").expect("valid test engine"),
            CoreInstanceId::new("product-control-test").expect("valid test instance"),
            None,
        ))
    }

    async fn start(&self, _context: CoreProcessStartContext) -> Result<(), PortError> {
        self.steps
            .lock()
            .expect("product control steps")
            .push(ProductControlStep::CoreStart);
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn stop(&self) -> Result<(), PortError> {
        self.steps
            .lock()
            .expect("product control steps")
            .push(ProductControlStep::CoreStop);
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
        Some("http://127.0.0.1:9090".to_owned())
    }
}

struct ProductControlCapabilities {
    engine_id: EngineId,
    operations: Vec<CoreOperation>,
    observed_versions: Arc<Mutex<Vec<Option<CoreVersion>>>>,
}

impl ProductControlCapabilities {
    fn product_control(observed_versions: Arc<Mutex<Vec<Option<CoreVersion>>>>) -> Self {
        Self {
            engine_id: EngineId::new("mihomo").expect("valid test engine"),
            observed_versions,
            operations: vec![
                CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
                CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
                CoreOperation::Lifecycle(CoreLifecycleOperation::Restart),
            ],
        }
    }
}

impl CoreCapabilityProvider for ProductControlCapabilities {
    fn matrix_for(
        &self,
        engine_id: &EngineId,
        core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        self.observed_versions
            .lock()
            .expect("observed capability versions")
            .push(core_version.cloned());
        let binary_target = EngineBinaryTarget::current();
        let host = binary_target.host_kind().unwrap_or(HostKind::Desktop);
        let scope = CoreCapabilityScope::new_for_target(
            self.engine_id.clone(),
            core_version.cloned(),
            host,
            binary_target,
            AdapterRevision::new(1),
        );
        let _ = engine_id;
        let entries = self
            .operations
            .iter()
            .copied()
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

struct ProductControlHandler {
    authorization: VpnAuthorizationOutcome,
    running: Arc<AtomicBool>,
    steps: Arc<Mutex<Vec<ProductControlStep>>>,
}

impl CommandHandler for ProductControlHandler {
    fn handle(&self, intent: CommandIntent) -> CommandFuture {
        let authorization = self.authorization;
        let running = self.running.clone();
        let steps = self.steps.clone();
        Box::pin(async move {
            match intent {
                CommandIntent::CheckVpnAuthorization => {
                    steps
                        .lock()
                        .expect("product control steps")
                        .push(ProductControlStep::Authorization);
                    match authorization {
                        VpnAuthorizationOutcome::Granted => Ok(()),
                        VpnAuthorizationOutcome::Denied => Err(Failure::new(
                            ErrorCode::Permission,
                            "VPN consent required",
                            false,
                        )),
                    }
                }
                CommandIntent::StartVpn => {
                    if !running.load(Ordering::SeqCst) {
                        return Err(Failure::new(
                            ErrorCode::NotReady,
                            "core must be ready before the VPN data path starts",
                            false,
                        ));
                    }
                    steps
                        .lock()
                        .expect("product control steps")
                        .push(ProductControlStep::VpnStart);
                    Ok(())
                }
                CommandIntent::StopVpn => {
                    steps
                        .lock()
                        .expect("product control steps")
                        .push(ProductControlStep::VpnStop);
                    Ok(())
                }
                _ => Err(Failure::unsupported("unexpected product control command")),
            }
        })
    }

    fn handle_lifecycle_child_output(
        &self,
        _permit: CoreLifecycleChildPermit,
        _scope: ValidatedCoreCommandScope,
        intent: CommandIntent,
    ) -> CommandOutputFuture {
        if matches!(
            intent,
            CommandIntent::CheckVpnAuthorization | CommandIntent::StartVpn | CommandIntent::StopVpn
        ) {
            let handled = self.handle(intent);
            Box::pin(async move { handled.await.map(|()| CommandOutput::Unit) })
        } else {
            Box::pin(async {
                Err(Failure::unsupported(
                    "unexpected lifecycle child test command",
                ))
            })
        }
    }
}

fn product_control_application(
    authorization: VpnAuthorizationOutcome,
) -> ProductControlApplicationParts {
    let (core, running, steps) = raw_core();
    let observed_versions = Arc::new(Mutex::new(Vec::new()));
    core.install_core_capability_application(CoreCapabilityApplication::new(Arc::new(
        ProductControlCapabilities::product_control(observed_versions.clone()),
    )))
    .expect("product control capabilities install before use");
    core.install_command_handler(Arc::new(ProductControlHandler {
        authorization,
        running: running.clone(),
        steps: steps.clone(),
    }));
    (
        core.clone(),
        AndroidControlApplication::new(core.as_ref().clone()),
        running,
        steps,
        observed_versions,
    )
}

fn product_control_application_without_version() -> ProductControlApplicationParts {
    let running = Arc::new(AtomicBool::new(false));
    let steps = Arc::new(Mutex::new(Vec::new()));
    let core = Arc::new(CoreApplication::new_with_policy(
        Arc::new(ProductControlProcess {
            running: running.clone(),
            steps: steps.clone(),
        }),
        Arc::new(VersionUnavailableReadiness),
        ReadinessPolicy {
            timeout: time::Duration::from_millis(100),
            poll_interval: time::Duration::from_millis(1),
        },
        runtime(),
    ));
    let observed_versions = Arc::new(Mutex::new(Vec::new()));
    core.install_core_capability_application(CoreCapabilityApplication::new(Arc::new(
        ProductControlCapabilities::product_control(observed_versions.clone()),
    )))
    .expect("product control capabilities install before use");
    core.install_command_handler(Arc::new(ProductControlHandler {
        authorization: VpnAuthorizationOutcome::Granted,
        running: running.clone(),
        steps: steps.clone(),
    }));
    (
        core.clone(),
        AndroidControlApplication::new(core.as_ref().clone()),
        running,
        steps,
        observed_versions,
    )
}

fn raw_core() -> (
    Arc<CoreApplication>,
    Arc<AtomicBool>,
    Arc<Mutex<Vec<ProductControlStep>>>,
) {
    let running = Arc::new(AtomicBool::new(false));
    let steps = Arc::new(Mutex::new(Vec::new()));
    let core = Arc::new(CoreApplication::new_with_policy(
        Arc::new(ProductControlProcess {
            running: running.clone(),
            steps: steps.clone(),
        }),
        Arc::new(FakeReadiness {
            endpoint: Ok("http://127.0.0.1:9090".to_owned()),
        }),
        ReadinessPolicy {
            timeout: time::Duration::from_millis(100),
            poll_interval: time::Duration::from_millis(1),
        },
        runtime(),
    ));
    (core, running, steps)
}

async fn execute_control_at_observed_scope(
    core: &CoreApplication,
    control: &AndroidControlApplication,
    intent: AndroidProductControlIntent,
) -> Result<CoreLifecycleSnapshot, Failure> {
    let scope = core
        .snapshot()
        .scope()
        .expect("product-control test observes the owner scope");
    control.execute(intent, scope).await
}

#[tokio::test]
async fn missing_lifecycle_provider_denies_start_before_process_side_effect() {
    let (core, running, steps) = raw_core();

    let result = test_execute_result_at_current_scope(&core, CommandIntent::StartCore).await;

    assert!(matches!(
        result,
        CommandResult::Rejected { failure, .. } if failure.code == ErrorCode::NotReady
    ));
    assert!(!running.load(Ordering::SeqCst));
    assert!(steps.lock().expect("product control steps").is_empty());
}

#[tokio::test]
async fn proxy_start_requires_core_lifecycle_capability_before_host_steps() {
    let (core, running, steps) = raw_core();
    let observed_versions = Arc::new(Mutex::new(Vec::new()));
    core.install_core_capability_application(CoreCapabilityApplication::new(Arc::new(
        ProductControlCapabilities {
            engine_id: EngineId::new("mihomo").expect("valid test engine"),
            operations: vec![
                CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
                CoreOperation::Lifecycle(CoreLifecycleOperation::Restart),
            ],
            observed_versions,
        },
    )))
    .expect("test provider installs before use");
    core.install_command_handler(Arc::new(ProductControlHandler {
        authorization: VpnAuthorizationOutcome::Granted,
        running: running.clone(),
        steps: steps.clone(),
    }));
    let control = AndroidControlApplication::new(core.as_ref().clone());

    let failure =
        execute_control_at_observed_scope(&core, &control, AndroidProductControlIntent::StartProxy)
            .await
            .expect_err("missing CoreLifecycle Start capability must deny admission");

    assert_eq!(failure.code, ErrorCode::Unsupported);
    assert!(!running.load(Ordering::SeqCst));
    assert!(steps.lock().expect("product control steps").is_empty());
}

#[tokio::test]
async fn mismatched_and_unregistered_lifecycle_capabilities_fail_without_starting_core() {
    let (core, running, steps) = raw_core();
    let start = CoreOperation::Lifecycle(CoreLifecycleOperation::Start);
    core.install_core_capability_application(CoreCapabilityApplication::new(Arc::new(
        ProductControlCapabilities {
            engine_id: EngineId::new("sing-box").expect("valid foreign engine"),
            operations: vec![start],
            observed_versions: Arc::new(Mutex::new(Vec::new())),
        },
    )))
    .expect("foreign test provider installs before use");

    let mismatch = test_execute_result_at_current_scope(&core, CommandIntent::StartCore).await;
    assert!(matches!(
        mismatch,
        CommandResult::Rejected { failure, .. } if failure.code == ErrorCode::InvalidState
    ));
    assert!(steps.lock().expect("product control steps").is_empty());

    core.install_core_capability_application(CoreCapabilityApplication::new(Arc::new(
        ProductControlCapabilities::product_control(Arc::new(Mutex::new(Vec::new()))),
    )))
    .expect("current engine provider installs before use");
    assert!(!running.load(Ordering::SeqCst));
    assert!(steps.lock().expect("product control steps").is_empty());
}

#[tokio::test]
async fn versionless_core_can_still_stop_and_clean_up() {
    let (core, _, running, steps, _) =
        product_control_application(VpnAuthorizationOutcome::Granted);
    assert!(core.snapshot().identity.unwrap().version.is_none());

    let result = test_execute_result_at_current_scope(&core, CommandIntent::StopCore).await;

    assert!(matches!(result, CommandResult::Completed { .. }));
    assert!(!running.load(Ordering::SeqCst));
    assert_eq!(
        *steps.lock().expect("product control steps"),
        vec![ProductControlStep::CoreStop]
    );
}

#[tokio::test]
async fn proxy_start_requires_vpn_authorization_before_starting_core() {
    let (core, control, running, steps, observed_versions) =
        product_control_application(VpnAuthorizationOutcome::Denied);

    let failure =
        execute_control_at_observed_scope(&core, &control, AndroidProductControlIntent::StartProxy)
            .await
            .expect_err("missing VPN consent must reject the proxy start");

    assert_eq!(failure.code, ErrorCode::Permission);
    assert!(!running.load(Ordering::SeqCst));
    assert_eq!(
        *observed_versions
            .lock()
            .expect("observed capability versions"),
        vec![None]
    );
    assert_eq!(
        *steps.lock().expect("product control steps"),
        vec![ProductControlStep::Authorization]
    );
}

#[tokio::test]
async fn proxy_start_without_owner_version_uses_lifecycle_and_vpn_children() {
    let (core, control, running, steps, observed_versions) =
        product_control_application_without_version();

    execute_control_at_observed_scope(&core, &control, AndroidProductControlIntent::StartProxy)
        .await
        .expect("Core lifecycle and VPN host operations do not need an active controller version");

    assert!(running.load(Ordering::SeqCst));
    assert!(core.snapshot().identity.unwrap().version.is_none());
    assert_eq!(core.snapshot().lifecycle, CoreLifecycle::Running);
    assert_eq!(
        *observed_versions
            .lock()
            .expect("observed capability versions"),
        vec![None, None]
    );
    assert_eq!(
        *steps.lock().expect("product control steps"),
        vec![
            ProductControlStep::Authorization,
            ProductControlStep::CoreStart,
            ProductControlStep::VpnStart,
        ]
    );
}

#[tokio::test]
async fn proxy_and_kernel_pause_keep_their_explicit_order_and_independence() {
    let (core, control, running, steps, observed_versions) =
        product_control_application(VpnAuthorizationOutcome::Granted);

    execute_control_at_observed_scope(&core, &control, AndroidProductControlIntent::StartProxy)
        .await
        .expect("authorized proxy start completes");
    assert_eq!(
        *steps.lock().expect("product control steps"),
        vec![
            ProductControlStep::Authorization,
            ProductControlStep::CoreStart,
            ProductControlStep::VpnStart,
        ]
    );
    assert_eq!(
        *observed_versions
            .lock()
            .expect("observed capability versions"),
        vec![
            None,
            Some(CoreVersion::new("1.19.12").expect("valid test version")),
        ]
    );

    steps.lock().expect("product control steps").clear();
    let stop_scope = core
        .snapshot()
        .scope()
        .expect("shared core command observes the owner scope");
    let stop_proxy = core
        .execute_scoped(CoreCommandRequest {
            scope: stop_scope,
            intent: CommandIntent::StopProxy,
        })
        .await;
    assert!(matches!(stop_proxy.result, CommandResult::Completed { .. }));
    assert!(running.load(Ordering::SeqCst));
    assert_eq!(
        *steps.lock().expect("product control steps"),
        vec![ProductControlStep::VpnStop]
    );
    assert_eq!(
        *observed_versions
            .lock()
            .expect("observed capability versions"),
        vec![
            None,
            Some(CoreVersion::new("1.19.12").expect("valid test version")),
        ]
    );

    steps.lock().expect("product control steps").clear();
    execute_control_at_observed_scope(&core, &control, AndroidProductControlIntent::PauseKernel)
        .await
        .expect("pausing the kernel stops VPN before the process");
    assert!(!running.load(Ordering::SeqCst));
    assert_eq!(
        *steps.lock().expect("product control steps"),
        vec![ProductControlStep::VpnStop, ProductControlStep::CoreStop]
    );
}

#[tokio::test]
async fn pause_and_restart_reconcile_an_idle_application_with_the_host_owner() {
    let (core, control, running, steps, _) =
        product_control_application(VpnAuthorizationOutcome::Granted);

    execute_control_at_observed_scope(&core, &control, AndroidProductControlIntent::PauseKernel)
        .await
        .expect("pause is idempotent when the host is already stopped");
    assert!(!running.load(Ordering::SeqCst));
    assert_eq!(
        *steps.lock().expect("product control steps"),
        vec![ProductControlStep::VpnStop, ProductControlStep::CoreStop]
    );

    steps.lock().expect("product control steps").clear();
    execute_control_at_observed_scope(&core, &control, AndroidProductControlIntent::RestartKernel)
        .await
        .expect("restart reconciles the host before starting a fresh core session");
    assert!(running.load(Ordering::SeqCst));
    assert_eq!(
        *steps.lock().expect("product control steps"),
        vec![
            ProductControlStep::VpnStop,
            ProductControlStep::CoreStop,
            ProductControlStep::CoreStart,
        ]
    );
}

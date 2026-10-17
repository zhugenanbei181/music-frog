//! End-to-end mihomo lifecycle through the engine-neutral ports.
//!
//! The example resolves `MIHOMO_BINARY`, `MIHOMO_CONFIG`, `MIHOMO_CONTROLLER`
//! and the optional `MIHOMO_SECRET` from the environment. Without a binary and
//! config document it prints the plan it would execute and exits successfully,
//! so `cargo run -p mihomo-driver --example lifecycle` works in a clean
//! checkout and in CI. With them it starts the process through `CoreProcess`,
//! waits for controller readiness through `CoreReadiness`, reads the live
//! version and outbound inventory through the driver's ports, and finally
//! stops the process and verifies the exact terminal readback.
//!
//! It contains no product path, no product crate and no UI dependency.

#[path = "lifecycle/plan.rs"]
mod plan;

use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, CoreScope};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::outbound::inventory::OutboundInventoryState;
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use lurker_core_ports::error::PortError;
use lurker_core_ports::outbound::OutboundPort;
use mihomo_driver::api::client::MihomoClient;
use mihomo_driver::driver::{ControllerReadiness, ProcessCoreController};
use mihomo_driver::outbound::MihomoOutboundAdapter;
use std::env::var;
use std::error::Error;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{Instant, sleep};

type ExampleResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const READINESS_TIMEOUT: Duration = Duration::from_secs(30);
const READINESS_RETRY_INTERVAL: Duration = Duration::from_millis(200);

/// One fixed owner scope for the outbound observation of this example.
struct FixedScope(CoreScope);

impl CoreScopeSource for FixedScope {
    fn scope_observation(&self) -> CoreScopeObservation {
        CoreScopeObservation::Observed(self.0.clone())
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{} example failed: {error}", plan::EXAMPLE_NAME);
            ExitCode::FAILURE
        }
    }
}

async fn run() -> ExampleResult<()> {
    match plan::resolve_plan(|key| var(key).ok())? {
        plan::LifecyclePlan::NotConfigured { missing } => {
            print_not_configured(&missing);
            Ok(())
        }
        plan::LifecyclePlan::Run(target) => run_configured(&target).await,
    }
}

fn print_not_configured(missing: &[&str]) {
    println!("{} example: NOT CONFIGURED", plan::EXAMPLE_NAME);
    println!("missing environment: {}", missing.join(", "));
    println!("this example would:");
    for (index, step) in plan::PLAN_STEPS.iter().enumerate() {
        println!("  {}. {step}", index + 1);
    }
    println!(
        "required environment: {}",
        plan::REQUIRED_ENV_KEYS.join(", ")
    );
    println!(
        "optional environment: {}",
        plan::OPTIONAL_ENV_KEYS.join(", ")
    );
    println!("run: {}", plan::RUN_COMMAND);
    println!("nothing was started; exiting successfully.");
}

async fn run_configured(target: &plan::EngineTarget) -> ExampleResult<()> {
    println!("{} example: configured", plan::EXAMPLE_NAME);

    // The driver never resolves a product home itself; the example owns this
    // throwaway state directory for the PID record and engine logs.
    let home = tempfile::tempdir()?;
    let controller = ProcessCoreController::with_home(
        target.binary().to_path_buf(),
        target.config().to_path_buf(),
        home.path().to_path_buf(),
    );
    let identity = controller
        .core_identity()
        .ok_or_else(|| example_error("the driver published no core identity"))?;
    println!(
        "core identity: {} / {}",
        identity.engine.as_str(),
        identity.instance.as_str()
    );

    controller
        .start(CoreProcessStartContext::standalone(identity.clone()))
        .await?;
    println!("started process pid: {:?}", controller.pid().await);

    let readiness =
        ControllerReadiness::new(target.controller(), target.secret().map(str::to_owned));

    // Always stop, even when the observation fails, so the example never
    // leaves an engine behind.
    let observed = observe_running(&readiness, target, identity).await;
    let stopped = controller.stop().await;
    let terminal = verify_terminal(&controller).await;

    observed?;
    stopped?;
    terminal?;
    Ok(())
}

async fn observe_running(
    readiness: &ControllerReadiness,
    target: &plan::EngineTarget,
    identity: CoreIdentity,
) -> ExampleResult<()> {
    let endpoint = wait_until_ready(readiness, READINESS_TIMEOUT).await?;
    println!("readiness endpoint: {endpoint}");

    let version = readiness
        .active_core_version()
        .await?
        .ok_or_else(|| example_error("readiness did not report a live core version"))?;
    println!("live version: {}", version.raw());

    let scope = CoreScope {
        identity: CoreIdentity::new(
            identity.engine.clone(),
            identity.instance.clone(),
            Some(version),
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    };
    let client = Arc::new(MihomoClient::new(
        target.controller(),
        target.secret().map(str::to_owned),
    )?);
    let adapter = MihomoOutboundAdapter::new(client, Arc::new(FixedScope(scope.clone())));
    let observation = adapter.observation(scope).await?;

    print_inventory(observation.inventory().inventory())?;
    println!("outbound detail state: {:?}", observation.details().state());
    Ok(())
}

fn print_inventory(inventory: &OutboundInventoryState) -> ExampleResult<()> {
    match inventory {
        OutboundInventoryState::Ready {
            entries,
            collections,
        } => {
            println!(
                "outbound inventory: {} entries, {} selection sets",
                entries.len(),
                collections.len()
            );
            Ok(())
        }
        OutboundInventoryState::Empty => {
            println!("outbound inventory: empty");
            Ok(())
        }
        OutboundInventoryState::Unknown => Err(example_error("outbound inventory is unknown")),
        OutboundInventoryState::Unsupported => {
            Err(example_error("outbound inventory is unsupported"))
        }
        OutboundInventoryState::Failed { failure: observed } => Err(example_error(format!(
            "outbound inventory failed: {}",
            observed.message
        ))),
    }
}

async fn verify_terminal(controller: &ProcessCoreController) -> ExampleResult<()> {
    let lifecycle = controller.status().await?;
    let pid = controller.pid().await;
    println!("terminal readback: status={lifecycle:?} pid={pid:?}");

    if lifecycle != CoreLifecycle::Stopped {
        return Err(example_error(format!(
            "expected CoreLifecycle::Stopped after stop, got {lifecycle:?}"
        )));
    }
    if pid.is_some() {
        return Err(example_error("expected no PID readback after stop"));
    }
    Ok(())
}

async fn wait_until_ready(
    readiness: &dyn CoreReadiness,
    timeout: Duration,
) -> Result<String, PortError> {
    let deadline = Instant::now() + timeout;
    loop {
        match readiness.probe().await {
            Ok(endpoint) => return Ok(endpoint),
            Err(error) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(error);
                }
                sleep(remaining.min(READINESS_RETRY_INTERVAL)).await;
            }
        }
    }
}

fn example_error(message: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    message.into().into()
}

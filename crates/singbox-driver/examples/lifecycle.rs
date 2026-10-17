//! End-to-end sing-box lifecycle through the engine-neutral ports.
//!
//! The example resolves `SINGBOX_BINARY`, `SINGBOX_CONFIG`, `SINGBOX_CONTROLLER`
//! and the optional `SINGBOX_SECRET` from the environment. Without a binary and
//! config document it prints the plan it would execute and exits successfully,
//! so `cargo run -p singbox-driver --example lifecycle` works in a clean
//! checkout and in CI. With them it starts the process through `CoreProcess`,
//! waits for Clash API readiness through `CoreReadiness`, reads the live
//! version and outbound inventory through the driver's ports, and finally
//! stops the process and verifies the exact terminal readback.
//!
//! It contains no product path, no product crate and no UI dependency.

#[path = "lifecycle/plan.rs"]
mod plan;

use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::outbound::inventory::OutboundInventoryState;
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use lurker_core_ports::error::PortError;
use lurker_core_ports::outbound::OutboundPort;
use singbox_driver::api::clash_api::SingBoxClashApiClient;
use singbox_driver::api::controller_configuration::SingBoxControllerConfiguration;
use singbox_driver::api::http::SingBoxControllerAuthorization;
use singbox_driver::driver::SingBoxProcess;
use singbox_driver::outbound::SingBoxOutboundAdapter;
use singbox_driver::readiness::SingBoxControllerReadiness;
use std::env::var;
use std::error::Error;
use std::fs::read;
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

    // The profile document, not this example, is the authority for the Clash
    // API address and secret. The environment values must agree with it.
    let controller = load_controller(target)?;

    let process = SingBoxProcess::with_tokio(
        target.binary().to_path_buf(),
        target.config().to_path_buf(),
        CoreInstanceId::new(plan::EXAMPLE_NAME)?,
        target.controller(),
    )?;
    let identity = process
        .core_identity()
        .ok_or_else(|| example_error("the driver published no core identity"))?;
    println!(
        "core identity: {} / {}",
        identity.engine.as_str(),
        identity.instance.as_str()
    );

    process
        .start(CoreProcessStartContext::standalone(identity.clone()))
        .await?;
    println!("started process pid: {:?}", process.pid().await);
    println!(
        "process controller endpoint: {:?}",
        process.controller_endpoint()
    );

    let readiness = SingBoxControllerReadiness::with_reqwest(controller.clone())?;

    // Always stop, even when the observation fails, so the example never
    // leaves an engine behind.
    let observed = observe_running(&readiness, &controller, identity).await;
    let stopped = process.stop().await;
    let terminal = verify_terminal(&process).await;

    observed?;
    stopped?;
    terminal?;
    Ok(())
}

fn load_controller(target: &plan::EngineTarget) -> ExampleResult<SingBoxControllerConfiguration> {
    let raw = read(target.config())?;
    let controller = SingBoxControllerConfiguration::from_profile_json(&raw)?;

    if controller.address() != target.controller() {
        return Err(example_error(format!(
            "SINGBOX_CONTROLLER {} does not match the profile external_controller {}",
            target.controller(),
            controller.address()
        )));
    }

    let expected = match target.secret() {
        Some(secret) => SingBoxControllerAuthorization::BearerToken(secret.to_owned()),
        None => SingBoxControllerAuthorization::NoSecret,
    };
    if controller.authorization() != &expected {
        return Err(example_error(
            "SINGBOX_SECRET does not match the profile experimental.clash_api.secret",
        ));
    }

    Ok(controller)
}

async fn observe_running(
    readiness: &SingBoxControllerReadiness,
    controller: &SingBoxControllerConfiguration,
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
        identity: CoreIdentity::new(identity.engine, identity.instance, Some(version)),
        generation: CoreGeneration::new(1),
        session_token: None,
    };
    let api = SingBoxClashApiClient::with_reqwest(controller.clone())?;
    let adapter = SingBoxOutboundAdapter::new(api, Arc::new(FixedScope(scope.clone())));
    let inventory = adapter.inventory(scope).await?;

    print_inventory(inventory.inventory())
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

async fn verify_terminal(process: &SingBoxProcess) -> ExampleResult<()> {
    let lifecycle = process.status().await?;
    let pid = process.pid().await;
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

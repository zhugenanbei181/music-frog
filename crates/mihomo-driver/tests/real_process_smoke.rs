//! Explicit real-binary acceptance harness for the Mihomo driver.
//!
//! This is the evidence required by PUB-002.1 / PUB-002.3. The default gate only
//! covers mock/fake adapters, so this test is `#[ignore]`d and never runs unless
//! it is requested explicitly:
//!
//! ```text
//! MIHOMO_REAL_BINARY=... MIHOMO_REAL_CONFIG=... MIHOMO_REAL_HOME=... \
//! MIHOMO_REAL_CONTROLLER=http://127.0.0.1:9090 \
//! cargo nextest run -p mihomo-driver --run-ignored -E 'test(real_process_smoke)'
//! ```
//!
//! The harness builds the public `MihomoEngineBackendProvider`, starts one real
//! process through `CoreProcess`, waits for `CoreReadiness`, reads a non-empty
//! `OutboundPort` inventory, stops the process and requires the exact terminal
//! `CoreLifecycle::Stopped` readback with no PID. It carries no product crate,
//! no UI dependency and no hardcoded engine path: every path comes from the
//! environment, and a missing variable fails the test loudly.

#![cfg(feature = "backend")]

use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, CoreScope, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::outbound::inventory::OutboundInventoryState;
use lurker_core_ports::core_process::{CoreProcess, CoreProcessStartAdmission, CoreReadiness};
use lurker_core_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use lurker_core_ports::engine_backend::{EngineBackendPorts, EngineBackendProvider};
use mihomo_driver::backend::{MihomoBackendOptions, MihomoEngineBackendProvider};
use std::env::var;
use std::fs::create_dir_all;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{Instant, sleep};

const READINESS_TIMEOUT: Duration = Duration::from_secs(30);
const READINESS_RETRY_INTERVAL: Duration = Duration::from_millis(200);

/// One fixed owner scope: the harness itself is the only Core owner here.
struct FixedScope(CoreScope);

impl CoreScopeSource for FixedScope {
    fn scope_observation(&self) -> CoreScopeObservation {
        CoreScopeObservation::Observed(self.0.clone())
    }
}

#[tokio::test]
#[ignore = "requires MIHOMO_REAL_BINARY / MIHOMO_REAL_CONFIG / MIHOMO_REAL_HOME / MIHOMO_REAL_CONTROLLER"]
async fn real_process_smoke() {
    let binary = var("MIHOMO_REAL_BINARY")
        .expect("set MIHOMO_REAL_BINARY to an installed mihomo executable");
    let config =
        var("MIHOMO_REAL_CONFIG").expect("set MIHOMO_REAL_CONFIG to a mihomo config document");
    let home = var("MIHOMO_REAL_HOME")
        .expect("set MIHOMO_REAL_HOME to the data directory that owns the PID record and logs");
    let controller = var("MIHOMO_REAL_CONTROLLER")
        .expect("set MIHOMO_REAL_CONTROLLER to a loopback controller endpoint");
    let secret = var("MIHOMO_REAL_SECRET")
        .ok()
        .filter(|value| !value.trim().is_empty());

    create_dir_all(&home).expect("MIHOMO_REAL_HOME must be a usable data directory");

    let options = MihomoBackendOptions::new(binary, config, home, controller, secret)
        .expect("the MIHOMO_REAL_* environment must describe a runnable mihomo backend");
    let backend = MihomoEngineBackendProvider::new(options)
        .create_backend()
        .expect("the mihomo backend bundle must be creatable from the environment");

    let process = backend.process();
    let readiness = backend.readiness();
    let identity = process
        .core_identity()
        .expect("the mihomo process adapter must publish its core identity");
    assert_eq!(identity.engine, EngineId::mihomo());

    let context = CoreProcessStartContext::standalone(identity.clone());
    let admission = process
        .prepare_start(&context)
        .await
        .expect("preparing the real mihomo launch must succeed");
    assert_eq!(
        admission,
        CoreProcessStartAdmission::LaunchNewProcess,
        "the harness owns a fresh controller and must launch a new process"
    );

    process
        .start(context)
        .await
        .expect("the real mihomo process must start");
    println!(
        "started mihomo pid {:?}",
        process
            .pid()
            .await
            .expect("a started mihomo process must publish its pid")
    );

    // Observe first, then always stop: a failed observation must never leave an
    // engine process behind.
    let observed = observe_running(&backend, readiness.as_ref(), &identity).await;
    let stopped = process.stop().await;
    let terminal = verify_terminal(process.as_ref()).await;

    if let Err(message) = observed {
        panic!("{message}");
    }
    stopped.expect("stopping the real mihomo process must confirm its exit");
    if let Err(message) = terminal {
        panic!("{message}");
    }
}

async fn observe_running(
    backend: &EngineBackendPorts,
    readiness: &dyn CoreReadiness,
    identity: &CoreIdentity,
) -> Result<(), String> {
    let endpoint = wait_until_ready(readiness, READINESS_TIMEOUT).await?;
    println!("mihomo controller ready at {endpoint}");

    let version = readiness
        .active_core_version()
        .await
        .map_err(|error| format!("reading the live mihomo version failed: {error}"))?
        .ok_or_else(|| "mihomo readiness reported no live core version".to_owned())?;
    println!("live mihomo version {}", version.raw());

    let scope = CoreScope {
        identity: CoreIdentity::new(
            identity.engine.clone(),
            identity.instance.clone(),
            Some(version),
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    };
    backend
        .bind_owner_core_scope(Arc::new(FixedScope(scope.clone())))
        .map_err(|error| format!("binding the outbound owner scope failed: {error}"))?;

    let outbound = backend
        .operations()
        .outbound()
        .map_err(|error| format!("the mihomo backend exposes no outbound port: {error}"))?;
    let observation = outbound
        .observation(scope)
        .await
        .map_err(|error| format!("observing the live mihomo outbound inventory failed: {error}"))?;

    match observation.inventory().inventory() {
        OutboundInventoryState::Ready { entries, .. } if !entries.is_empty() => {
            println!("outbound inventory: {} entries", entries.len());
            Ok(())
        }
        other => Err(format!(
            "expected a non-empty mihomo outbound inventory, got {other:?}"
        )),
    }
}

async fn wait_until_ready(
    readiness: &dyn CoreReadiness,
    timeout: Duration,
) -> Result<String, String> {
    let deadline = Instant::now() + timeout;
    loop {
        match readiness.probe().await {
            Ok(endpoint) => return Ok(endpoint),
            Err(error) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(format!(
                        "MIHOMO_REAL_CONTROLLER was not ready within {timeout:?}: {error}"
                    ));
                }
                sleep(remaining.min(READINESS_RETRY_INTERVAL)).await;
            }
        }
    }
}

async fn verify_terminal(process: &dyn CoreProcess) -> Result<(), String> {
    let lifecycle = process
        .status()
        .await
        .map_err(|error| format!("reading the terminal mihomo lifecycle failed: {error}"))?;
    let pid = process.pid().await;
    if lifecycle != CoreLifecycle::Stopped {
        return Err(format!(
            "expected the terminal CoreLifecycle::Stopped after stop, got {lifecycle:?}"
        ));
    }
    if pid.is_some() {
        return Err(format!(
            "expected no mihomo pid readback after stop, got {pid:?}"
        ));
    }
    println!("terminal readback: status={lifecycle:?} pid={pid:?}");
    Ok(())
}

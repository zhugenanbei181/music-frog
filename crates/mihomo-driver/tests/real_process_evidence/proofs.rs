//! The four typed proofs the Mihomo driver runs against a real process.
//!
//! Every proof returns a typed outcome: the proof either produced the outcome
//! its engine contract declares, the engine's typed port declares the
//! capability unsupported, or the proof failed. A proof never reports success
//! without the typed evidence that backs it, and the proofs only use the
//! public API of the driver and of the neutral core ports.

use crate::contract::RealProcessTarget;
use crate::receipt::{Proof, ProofOutcome, ProofRecord};
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, CoreScope, CoreVersion};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::outbound::inventory::{
    OutboundInventorySnapshot, OutboundInventoryState, OutboundProbeState, OutboundSelectionState,
    SelectionAvailability,
};
use lurker_core_ports::core_process::{CoreProcess, CoreProcessStartAdmission, CoreReadiness};
use lurker_core_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use lurker_core_ports::engine_backend::{EngineBackendPorts, EngineBackendProvider};
use lurker_core_ports::outbound::{OutboundPort, OutboundProbeRequest, ProbeTarget};
use mihomo_driver::api::client::MihomoClient;
use mihomo_driver::backend::{MihomoBackendOptions, MihomoEngineBackendProvider};
use serde_json::json;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep};

/// The budget the controller handshake may take.
const READINESS_TIMEOUT: Duration = Duration::from_secs(30);
/// The budget the outbound inventory may take to become observable.
const INVENTORY_TIMEOUT: Duration = Duration::from_secs(30);
/// The interval between two readiness or inventory observations.
const RETRY_INTERVAL: Duration = Duration::from_millis(200);
/// The timeout of the typed node speed test.
const PROBE_TIMEOUT_MS: u64 = 5_000;
/// The budget the traffic dispatch may take while the engine binds its
/// listeners.
const DISPATCH_TIMEOUT: Duration = Duration::from_secs(15);
/// The budget the typed node speed test may take while the engine settles.
const PROBE_TIMEOUT: Duration = Duration::from_secs(15);
/// The payload the traffic dispatch proof sends through the engine.
const DISPATCH_PAYLOAD: &[u8] = b"music-frog-real-process-evidence";

/// The evidence collected by one real-binary run.
pub struct Evidence {
    /// The typed outcome of every proof that ran.
    pub records: Vec<ProofRecord>,
    /// The live version the running engine reported.
    pub engine_version: Option<String>,
}

/// One fixed owner scope: the harness itself is the only Core owner here.
struct FixedScope(CoreScope);

impl CoreScopeSource for FixedScope {
    fn scope_observation(&self) -> CoreScopeObservation {
        CoreScopeObservation::Observed(self.0.clone())
    }
}

/// Runs the four proofs against one real Mihomo process.
pub async fn run(target: &RealProcessTarget) -> Result<Evidence, String> {
    // The fixture answers the engine's outbound and receives the dispatched
    // traffic, so it must listen before the engine starts.
    let _fixture = Fixture::start(target.fixture()).await?;

    let options = MihomoBackendOptions::new(
        target.binary().clone(),
        target.config().clone(),
        target.home().clone(),
        target.controller().to_owned(),
        target.secret().map(str::to_owned),
    )
    .map_err(|error| format!("the Mihomo backend options are invalid: {error}"))?;

    let backend = MihomoEngineBackendProvider::new(options)
        .create_backend()
        .map_err(|error| format!("the Mihomo backend bundle could not be created: {error}"))?;

    let process = backend.process();
    let readiness = backend.readiness();
    let identity = process
        .core_identity()
        .ok_or_else(|| "the Mihomo process adapter published no core identity".to_owned())?;

    let context = CoreProcessStartContext::standalone(identity.clone());
    let admission = process
        .prepare_start(&context)
        .await
        .map_err(|error| format!("preparing the real Mihomo launch failed: {error}"))?;
    if admission != CoreProcessStartAdmission::LaunchNewProcess {
        return Err(
            "the evidence harness owns a fresh controller and must launch a new process".to_owned(),
        );
    }
    process
        .start(context)
        .await
        .map_err(|error| format!("the real Mihomo process did not start: {error}"))?;

    // Observe first, then always stop: a failed observation must never leave
    // an engine process behind.
    let observed = observe(&backend, readiness.as_ref(), target, &identity).await;
    let stopped = process.stop().await;
    let terminal = verify_terminal(process.as_ref()).await;

    let evidence = observed?;
    stopped.map_err(|error| format!("stopping the real Mihomo process failed: {error}"))?;
    terminal?;
    Ok(evidence)
}

/// Runs the four proofs against the running engine.
async fn observe(
    backend: &EngineBackendPorts,
    readiness: &dyn CoreReadiness,
    target: &RealProcessTarget,
    identity: &CoreIdentity,
) -> Result<Evidence, String> {
    let mut records: Vec<ProofRecord> = Vec::new();

    let (endpoint, version) = match prove_api_handshake(readiness).await {
        Ok(observed) => observed,
        Err(reason) => {
            records.push(ProofRecord::new(
                Proof::ApiHandshake,
                ProofOutcome::Failed {
                    reason: reason.clone(),
                },
            ));
            return Ok(Evidence {
                records,
                engine_version: None,
            });
        }
    };
    records.push(ProofRecord::new(
        Proof::ApiHandshake,
        ProofOutcome::Proven {
            detail: format!(
                "the controller at {endpoint} reported version {}",
                version.raw()
            ),
        },
    ));

    let scope = CoreScope {
        identity: CoreIdentity::new(
            identity.engine.clone(),
            identity.instance.clone(),
            Some(version.clone()),
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    };
    backend
        .bind_owner_core_scope(Arc::new(FixedScope(scope.clone())))
        .map_err(|error| format!("binding the Mihomo owner scope failed: {error}"))?;

    let outbound = backend
        .operations()
        .outbound()
        .map_err(|error| format!("the Mihomo backend exposes no outbound port: {error}"))?;

    let speed = prove_node_speed_test(outbound.as_ref(), &scope, target).await;
    if !record(&mut records, Proof::NodeSpeedTest, speed) {
        return Ok(Evidence {
            records,
            engine_version: Some(version.raw().to_owned()),
        });
    }

    let dispatch = prove_traffic_dispatch(target).await;
    if !record(&mut records, Proof::TrafficDispatch, dispatch) {
        return Ok(Evidence {
            records,
            engine_version: Some(version.raw().to_owned()),
        });
    }

    let configuration = prove_configuration_write(outbound.as_ref(), &scope, target).await;
    record(&mut records, Proof::ConfigurationWrite, configuration);

    Ok(Evidence {
        records,
        engine_version: Some(version.raw().to_owned()),
    })
}

/// Records the typed outcome of one proof and reports whether the run may
/// continue.
fn record(
    records: &mut Vec<ProofRecord>,
    proof: Proof,
    outcome: Result<ProofOutcome, String>,
) -> bool {
    let (outcome, failed) = match outcome {
        Ok(outcome) => {
            let failed = matches!(&outcome, ProofOutcome::Failed { .. });
            (outcome, failed)
        }
        Err(reason) => (ProofOutcome::Failed { reason }, true),
    };
    records.push(ProofRecord::new(proof, outcome));
    !failed
}

/// Proves the controller handshake and reads the live core version.
async fn prove_api_handshake(
    readiness: &dyn CoreReadiness,
) -> Result<(String, CoreVersion), String> {
    let endpoint = wait_until_ready(readiness, READINESS_TIMEOUT).await?;
    let version = readiness
        .active_core_version()
        .await
        .map_err(|error| format!("reading the live Mihomo version failed: {error}"))?
        .ok_or_else(|| "the Mihomo readiness reported no live core version".to_owned())?;
    Ok((endpoint, version))
}

/// Proves the typed node speed test of the configured outbound.
///
/// The engine publishes its outbounds before they are reachable, so the typed
/// probe is retried inside a bounded budget instead of failing on the startup
/// race. A typed `Unsupported` outcome is returned as it is: it is a declared
/// capability gap, not a transient failure.
async fn prove_node_speed_test(
    outbound: &dyn OutboundPort,
    scope: &CoreScope,
    target: &RealProcessTarget,
) -> Result<ProofOutcome, String> {
    let deadline = Instant::now() + PROBE_TIMEOUT;
    let mut last_reason = String::new();
    loop {
        match probe_once(outbound, scope, target).await {
            Ok(outcome) => return Ok(outcome),
            Err(reason) => {
                last_reason = reason;
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(format!(
                        "the typed node speed test did not succeed: {last_reason}"
                    ));
                }
                sleep(remaining.min(RETRY_INTERVAL)).await;
            }
        }
    }
}

/// Probes the configured outbound once.
async fn probe_once(
    outbound: &dyn OutboundPort,
    scope: &CoreScope,
    target: &RealProcessTarget,
) -> Result<ProofOutcome, String> {
    let inventory = wait_for_inventory(outbound, scope.clone(), INVENTORY_TIMEOUT).await?;
    let outbound_id = OutboundId::new(scope.identity.engine.clone(), target.probe_outbound())
        .map_err(|error| format!("the probe outbound identity is invalid: {error}"))?;
    let request = OutboundProbeRequest::new(
        ProbeTarget::http_url(target.fixture_url())
            .map_err(|error| format!("the probe target is invalid: {error}"))?,
        PROBE_TIMEOUT_MS,
    )
    .map_err(|error| format!("the probe request is invalid: {error}"))?;
    let snapshot = outbound
        .probe(scope.clone(), inventory.revision(), outbound_id, request)
        .await
        .map_err(|error| format!("the typed outbound probe failed: {error}"))?;
    match snapshot.state() {
        OutboundProbeState::Measured { delay_ms } => Ok(ProofOutcome::Proven {
            detail: format!(
                "the typed probe measured {delay_ms} ms for {}",
                target.probe_outbound()
            ),
        }),
        OutboundProbeState::TimedOut => Err("the typed outbound probe timed out".to_owned()),
        OutboundProbeState::Unsupported => Ok(ProofOutcome::Unsupported {
            reason: "the Mihomo outbound port reports the node speed test as unsupported"
                .to_owned(),
        }),
        OutboundProbeState::Unknown | OutboundProbeState::Probing => {
            Err("the typed outbound probe did not reach a terminal state".to_owned())
        }
        OutboundProbeState::Failed { failure } => Err(format!(
            "the typed outbound probe failed: {}",
            failure.message
        )),
    }
}

/// Proves that the engine dispatches real traffic to the loopback fixture.
///
/// The engine publishes its controller before its listeners are bound, so the
/// dispatch is retried inside a bounded budget instead of failing on the
/// startup race.
async fn prove_traffic_dispatch(target: &RealProcessTarget) -> Result<ProofOutcome, String> {
    let deadline = Instant::now() + DISPATCH_TIMEOUT;
    let mut last_reason = String::new();
    loop {
        match dispatch_once(target).await {
            Ok(detail) => return Ok(ProofOutcome::Proven { detail }),
            Err(reason) => {
                last_reason = reason;
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(format!(
                        "the engine did not dispatch the traffic to the fixture: {last_reason}"
                    ));
                }
                sleep(remaining.min(RETRY_INTERVAL)).await;
            }
        }
    }
}

/// Dispatches one payload through the engine's own SOCKS5 listener.
async fn dispatch_once(target: &RealProcessTarget) -> Result<String, String> {
    let fixture: SocketAddr = target
        .fixture()
        .parse()
        .map_err(|error| format!("the fixture address is invalid: {error}"))?;

    let mut stream = TcpStream::connect(target.proxy())
        .await
        .map_err(|error| format!("the engine SOCKS5 listener refused the connection: {error}"))?;
    stream
        .write_all(&[0x05, 0x01, 0x00])
        .await
        .map_err(|error| format!("the SOCKS5 greeting could not be sent: {error}"))?;
    let mut greeting = [0u8; 2];
    stream
        .read_exact(&mut greeting)
        .await
        .map_err(|error| format!("the SOCKS5 greeting was not answered: {error}"))?;
    if greeting != [0x05, 0x00] {
        return Err(format!(
            "the engine SOCKS5 listener refused the no-auth greeting: {greeting:?}"
        ));
    }

    let mut request = vec![0x05, 0x01, 0x00];
    match fixture.ip() {
        IpAddr::V4(address) => {
            request.push(0x01);
            request.extend_from_slice(&address.octets());
        }
        IpAddr::V6(address) => {
            request.push(0x04);
            request.extend_from_slice(&address.octets());
        }
    }
    request.extend_from_slice(&fixture.port().to_be_bytes());
    stream
        .write_all(&request)
        .await
        .map_err(|error| format!("the SOCKS5 connect request could not be sent: {error}"))?;

    let mut head = [0u8; 4];
    stream
        .read_exact(&mut head)
        .await
        .map_err(|error| format!("the SOCKS5 connect reply was not received: {error}"))?;
    if head[1] != 0x00 {
        return Err(format!(
            "the engine refused the SOCKS5 connect request: {head:?}"
        ));
    }
    let mut tail = Vec::new();
    match head[3] {
        0x01 => tail.resize(4 + 2, 0),
        0x04 => tail.resize(16 + 2, 0),
        0x03 => {
            let mut length = [0u8; 1];
            stream
                .read_exact(&mut length)
                .await
                .map_err(|error| format!("the SOCKS5 domain reply was truncated: {error}"))?;
            tail.resize(usize::from(length[0]) + 2, 0);
        }
        other => {
            return Err(format!(
                "the engine SOCKS5 reply used an unknown address type {other}"
            ));
        }
    }
    stream
        .read_exact(&mut tail)
        .await
        .map_err(|error| format!("the SOCKS5 connect reply was truncated: {error}"))?;

    stream
        .write_all(DISPATCH_PAYLOAD)
        .await
        .map_err(|error| format!("the dispatched payload could not be sent: {error}"))?;
    let mut echoed = vec![0u8; DISPATCH_PAYLOAD.len()];
    stream
        .read_exact(&mut echoed)
        .await
        .map_err(|error| format!("the dispatched payload was not echoed: {error}"))?;
    if echoed != DISPATCH_PAYLOAD {
        return Err("the loopback fixture did not receive the dispatched payload".to_owned());
    }

    Ok(format!(
        "the engine dispatched {} bytes to the loopback fixture through its own SOCKS5 listener",
        DISPATCH_PAYLOAD.len()
    ))
}

/// Proves a configuration write to the running engine and its readback.
async fn prove_configuration_write(
    outbound: &dyn OutboundPort,
    scope: &CoreScope,
    target: &RealProcessTarget,
) -> Result<ProofOutcome, String> {
    let inventory = wait_for_inventory(outbound, scope.clone(), INVENTORY_TIMEOUT).await?;
    let collection_id = OutboundId::new(scope.identity.engine.clone(), target.selector())
        .map_err(|error| format!("the selector identity is invalid: {error}"))?;
    let (original, alternative) = select_pair(&inventory, &collection_id)?;

    let switched = outbound
        .select(
            scope.clone(),
            inventory.revision(),
            collection_id.clone(),
            alternative.clone(),
        )
        .await
        .map_err(|error| format!("the typed selector write failed: {error}"))?;
    if selected_member(switched.inventory(), &collection_id) != Some(&alternative) {
        return Err("the typed selector readback did not confirm the requested member".to_owned());
    }
    let restored = outbound
        .select(
            scope.clone(),
            switched.revision(),
            collection_id.clone(),
            original.clone(),
        )
        .await
        .map_err(|error| format!("restoring the typed selector failed: {error}"))?;
    if selected_member(restored.inventory(), &collection_id) != Some(&original) {
        return Err("the typed selector readback did not confirm the restored member".to_owned());
    }

    let client = MihomoClient::new(target.controller(), target.secret().map(str::to_owned))
        .map_err(|error| format!("the Mihomo controller client is invalid: {error}"))?;
    client
        .patch_config(json!({"log-level": "error"}))
        .await
        .map_err(|error| format!("the live configuration write failed: {error}"))?;
    let config = client
        .get_config()
        .await
        .map_err(|error| format!("the live configuration readback failed: {error}"))?;
    if config.log_level != "error" {
        return Err(format!(
            "the live configuration readback reported the log level `{}`, expected `error`",
            config.log_level
        ));
    }

    Ok(ProofOutcome::Proven {
        detail: "the typed selector write, its restore and the live log-level write were read back"
            .to_owned(),
    })
}

/// Returns the observed member and one alternative member of the collection.
fn select_pair(
    inventory: &OutboundInventorySnapshot,
    collection_id: &OutboundId,
) -> Result<(OutboundId, OutboundId), String> {
    let OutboundInventoryState::Ready { collections, .. } = inventory.inventory() else {
        return Err("the Mihomo outbound inventory is not ready".to_owned());
    };
    let Some(collection) = collections
        .iter()
        .find(|collection| &collection.id == collection_id)
    else {
        return Err(format!(
            "the configured selector `{}` was not observed in the inventory",
            collection_id.value()
        ));
    };
    if collection.availability != SelectionAvailability::Selectable {
        return Err(format!(
            "the configured selector `{}` is not selectable",
            collection_id.value()
        ));
    }
    let OutboundSelectionState::Ready { selected } = &collection.selection else {
        return Err(format!(
            "the configured selector `{}` has no observed member",
            collection_id.value()
        ));
    };
    let Some(alternative) = collection
        .member_ids
        .iter()
        .find(|member| *member != selected)
    else {
        return Err(format!(
            "the configured selector `{}` has no alternative member",
            collection_id.value()
        ));
    };
    Ok((selected.clone(), alternative.clone()))
}

/// Returns the currently selected member of one collection.
fn selected_member<'a>(
    inventory: &'a OutboundInventoryState,
    collection_id: &OutboundId,
) -> Option<&'a OutboundId> {
    let OutboundInventoryState::Ready { collections, .. } = inventory else {
        return None;
    };
    collections
        .iter()
        .find(|collection| &collection.id == collection_id)
        .and_then(|collection| match &collection.selection {
            OutboundSelectionState::Ready { selected } => Some(selected),
            _ => None,
        })
}

/// Waits until the controller answers and reports the live endpoint.
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
                        "the Mihomo controller was not ready within {timeout:?}: {error}"
                    ));
                }
                sleep(remaining.min(RETRY_INTERVAL)).await;
            }
        }
    }
}

/// Waits until the engine published a non-empty outbound inventory.
async fn wait_for_inventory(
    outbound: &dyn OutboundPort,
    scope: CoreScope,
    timeout: Duration,
) -> Result<OutboundInventorySnapshot, String> {
    let deadline = Instant::now() + timeout;
    let mut last_error = String::new();
    loop {
        match outbound.inventory(scope.clone()).await {
            Ok(snapshot) => {
                if let OutboundInventoryState::Ready { entries, .. } = snapshot.inventory()
                    && !entries.is_empty()
                {
                    return Ok(snapshot);
                }
                last_error = "the engine published no outbound yet".to_owned();
            }
            Err(error) => {
                last_error = format!("the outbound inventory could not be read: {error}");
            }
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(format!(
                "the Mihomo outbound inventory was not observable within {timeout:?}: {last_error}"
            ));
        }
        sleep(remaining.min(RETRY_INTERVAL)).await;
    }
}

/// Requires the exact terminal lifecycle readback of a stopped process.
async fn verify_terminal(process: &dyn CoreProcess) -> Result<(), String> {
    let lifecycle = process
        .status()
        .await
        .map_err(|error| format!("reading the terminal Mihomo lifecycle failed: {error}"))?;
    let pid = process.pid().await;
    if lifecycle != CoreLifecycle::Stopped {
        return Err(format!(
            "expected the terminal CoreLifecycle::Stopped after stop, got {lifecycle:?}"
        ));
    }
    if pid.is_some() {
        return Err(format!(
            "expected no Mihomo pid readback after stop, got {pid:?}"
        ));
    }
    Ok(())
}

/// The loopback fixture the engine's outbound and the dispatch proof use.
struct Fixture {
    task: JoinHandle<()>,
}

impl Fixture {
    /// Binds the fixture and serves the loopback traffic in the background.
    async fn start(address: &str) -> Result<Self, String> {
        let address: SocketAddr = address
            .parse()
            .map_err(|error| format!("the fixture address is invalid: {error}"))?;
        let listener = TcpListener::bind(address)
            .await
            .map_err(|error| format!("the loopback fixture could not bind {address}: {error}"))?;
        let task = tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buffer = vec![0u8; 4096];
                    let Ok(count) = stream.read(&mut buffer).await else {
                        return;
                    };
                    if count == 0 {
                        return;
                    }
                    let head = buffer[..count].to_vec();
                    if head.starts_with(b"CONNECT ") {
                        // The engine probes its HTTP proxy outbound with a
                        // CONNECT tunnel: answer the tunnel, then the request
                        // inside it.
                        if stream
                            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                            .await
                            .is_err()
                        {
                            return;
                        }
                        let mut tunneled = vec![0u8; 4096];
                        if stream.read(&mut tunneled).await.is_err() {
                            return;
                        }
                        let _ = stream
                            .write_all(
                                b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                            )
                            .await;
                        return;
                    }
                    if head.starts_with(b"GET ") || head.starts_with(b"HEAD ") {
                        let _ = stream
                            .write_all(
                                b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                            )
                            .await;
                        return;
                    }
                    let _ = stream.write_all(&head).await;
                });
            }
        });
        Ok(Self { task })
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

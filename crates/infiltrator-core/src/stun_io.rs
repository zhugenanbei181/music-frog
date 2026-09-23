//! DUAL-14-09 (re-scoped): the real UDP STUN egress-probe adapter.
//!
//! For a configured STUN server the adapter resolves the endpoint, binds a UDP
//! socket connected to it (so the kernel drops datagrams from any other
//! source), sends one RFC 5389 Binding Request and parses the
//! XOR-MAPPED-ADDRESS from the Binding Success Response. The mapping is what
//! the STUN server observed for *this host/process's* UDP flow; it is not a
//! browser WebRTC measurement.
//!
//! Every network outcome is a typed result: a timeout, a non-STUN datagram, a
//! server error response or a socket/DNS failure never becomes a fabricated
//! public address.

use infiltrator_contract::stun_probe::{StunProbeObservation, StunProbeRequest};
use infiltrator_ports::error::PortError;
use infiltrator_ports::stun_probe::StunProbePort;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::time::timeout;

use crate::dns_latency_io::{resolve_target, split_host_port};
use crate::stun_wire::{self, MAX_RESPONSE_BYTES};

/// The IANA STUN/TURN default port, used when the configured server omits one.
const DEFAULT_STUN_PORT: u16 = 3478;

/// The real host adapter: one UDP Binding Request per `observe`.
#[derive(Debug, Default, Clone, Copy)]
pub struct UdpStunProbe;

impl UdpStunProbe {
    pub fn new() -> Self {
        Self
    }

    /// Run one exchange; every network outcome is a typed observation.
    async fn observe_server(&self, request: &StunProbeRequest) -> StunProbeObservation {
        let server = request.server.trim();
        if server.is_empty() {
            return StunProbeObservation::failed(server, "the configured STUN server is empty");
        }
        let Some((host, port)) = split_host_port(server, DEFAULT_STUN_PORT) else {
            return StunProbeObservation::failed(
                server,
                format!("the configured STUN server {server} has no usable host:port"),
            );
        };
        let target = match resolve_target(&host, port).await {
            Ok(target) => target,
            Err(message) => return StunProbeObservation::failed(server, message),
        };
        let transaction = stun_wire::new_transaction_id();
        let query = stun_wire::encode_binding_request(&transaction);
        let deadline = Duration::from_millis(u64::from(request.timeout_ms));
        exchange(target, server, &transaction, &query, deadline).await
    }
}

#[async_trait::async_trait]
impl StunProbePort for UdpStunProbe {
    async fn observe(&self, request: StunProbeRequest) -> Result<StunProbeObservation, PortError> {
        Ok(self.observe_server(&request).await)
    }
}

/// One connected UDP Binding Request exchange, bounded by `deadline`.
async fn exchange(
    target: SocketAddr,
    server: &str,
    transaction: &stun_wire::TransactionId,
    query: &[u8],
    deadline: Duration,
) -> StunProbeObservation {
    let bind = if target.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let socket = match UdpSocket::bind(bind).await {
        Ok(socket) => socket,
        Err(error) => {
            return StunProbeObservation::failed(
                server,
                format!("cannot open a UDP socket: {error}"),
            );
        }
    };
    if let Err(error) = socket.connect(target).await {
        return StunProbeObservation::failed(
            server,
            format!("cannot connect the STUN socket to {target}: {error}"),
        );
    }
    if let Err(outcome) = send_with_deadline(&socket, query, deadline).await {
        return StunProbeObservation::failed(server, outcome);
    }

    let started = Instant::now();
    let mut buffer = vec![0u8; MAX_RESPONSE_BYTES];
    loop {
        let remaining = deadline.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return StunProbeObservation::timed_out(server);
        }
        match timeout(remaining, socket.recv(&mut buffer)).await {
            Err(_) => return StunProbeObservation::timed_out(server),
            Ok(Err(error)) => {
                return StunProbeObservation::failed(
                    server,
                    format!("cannot read the STUN answer from {target}: {error}"),
                );
            }
            Ok(Ok(len)) => match stun_wire::parse_binding_response(&buffer[..len], transaction) {
                Ok(response) => {
                    if let Some(mapping) = response.mapped {
                        return StunProbeObservation::observed(server, mapping);
                    }
                    let code = response
                        .error_code
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "unknown".to_owned());
                    let reason = response.error_reason.unwrap_or_default();
                    return StunProbeObservation::failed(
                        server,
                        format!("the STUN server answered error {code} {reason}")
                            .trim_end()
                            .to_owned(),
                    );
                }
                // A datagram that is not part of this transaction (a stray or
                // an off-path packet) is discarded; the loop keeps waiting for
                // the real answer until the deadline.
                Err(stun_wire::StunWireError::TransactionMismatch) => continue,
                Err(error) => {
                    return StunProbeObservation::failed(
                        server,
                        format!("the STUN answer failed validation: {error}"),
                    );
                }
            },
        }
    }
}

async fn send_with_deadline(
    socket: &UdpSocket,
    query: &[u8],
    deadline: Duration,
) -> Result<(), String> {
    match timeout(deadline, socket.send(query)).await {
        Err(_) => Err("the STUN Binding Request timed out before it was sent".to_owned()),
        Ok(Err(error)) => Err(format!("cannot send the STUN Binding Request: {error}")),
        Ok(Ok(_)) => Ok(()),
    }
}

#[cfg(test)]
#[path = "stun_io_test.rs"]
mod stun_io_test;

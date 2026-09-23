//! DUAL-14-08: the real echo-authority probe adapter.
//!
//! For every requested probe the adapter resolves the given freshly generated
//! subdomain through the configured resolver and reads the identity the echo
//! authority observed:
//!
//! * plain entries (`ip:port`, `ip`, `udp://host:port`) go over UDP with the
//!   socket connected to the resolver, so the kernel drops datagrams from any
//!   other source;
//! * `https://` / `http://` / `doh://` entries are sent as a wire-format DoH
//!   POST through the shared HTTP client;
//! * `system` is the platform resolver path (a host name lookup), which is the
//!   path a DNS leak would actually flow through.
//!
//! The observation is the first `A` record of the validated answer: a
//! controlled echo authority answers `random.<zone>` with the resolver's
//! source address as it observed it. Transports this host cannot drive (DoT,
//! DoQ, DNSCrypt, DHCP, HTTP/3) are reported as `NotProbed` with the typed
//! reason, and an answer without an `A` record is `InvalidResponse` — never a
//! fabricated identity. The address decoding and the wire transaction id are
//! shared with the DUAL-14-10 latency prober.

use infiltrator_contract::dns_leak::{
    DnsLeakEchoProbe, DnsLeakEchoReport, DnsLeakEchoRequest, DnsLeakObservation,
    DnsLeakObservationOutcome, DnsLeakProbeTransport,
};
use infiltrator_http::HttpClient;
use infiltrator_ports::dns_leak::DnsLeakEchoPort;
use infiltrator_ports::error::PortError;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::timeout;

use crate::dns_latency_io::{ProbePlan, next_query_id, plan_for, resolve_target};
use crate::dns_wire::{self, DnsQuestion, MAX_RESPONSE_BYTES};

/// The mihomo `Content-Type`/`Accept` for a wire-format DoH exchange.
const DOH_CONTENT_TYPE: &str = "application/dns-message";

pub struct HttpDnsLeakEchoProbe {
    client: HttpClient,
}

impl Default for HttpDnsLeakEchoProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpDnsLeakEchoProbe {
    pub fn new() -> Self {
        Self {
            client: infiltrator_http::build_http_client(),
        }
    }

    /// Probe with a caller-supplied client (used by the deterministic tests).
    pub fn with_client(client: HttpClient) -> Self {
        Self { client }
    }

    /// Observe one source; every failure is a result, never an error.
    pub async fn observe_probe(
        &self,
        probe: &DnsLeakEchoProbe,
        request: &DnsLeakEchoRequest,
    ) -> DnsLeakObservation {
        let (transport, outcome) = match plan_for(&probe.resolver) {
            ProbePlan::Undrivable { reason } => (
                DnsLeakProbeTransport::Undrivable {
                    reason: reason.clone(),
                },
                DnsLeakObservationOutcome::NotProbed { reason },
            ),
            ProbePlan::System => (
                DnsLeakProbeTransport::System,
                self.observe_system(probe, request).await,
            ),
            ProbePlan::Udp { host, port } => (
                DnsLeakProbeTransport::Udp,
                self.observe_udp(&host, port, probe, request).await,
            ),
            ProbePlan::Doh { url } => (
                DnsLeakProbeTransport::Doh,
                self.observe_doh(&url, probe, request).await,
            ),
        };
        DnsLeakObservation {
            resolver: probe.resolver.clone(),
            authority: probe.authority.clone(),
            question: probe.question.clone(),
            transport,
            outcome,
        }
    }

    async fn observe_udp(
        &self,
        host: &str,
        port: u16,
        probe: &DnsLeakEchoProbe,
        request: &DnsLeakEchoRequest,
    ) -> DnsLeakObservationOutcome {
        let question = echo_question(&probe.question);
        let query = match dns_wire::encode_query(&question) {
            Ok(query) => query,
            Err(error) => {
                return DnsLeakObservationOutcome::Failed {
                    message: error.to_string(),
                };
            }
        };
        let target = match resolve_target(host, port).await {
            Ok(target) => target,
            Err(message) => return DnsLeakObservationOutcome::Failed { message },
        };
        let bind = if target.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        };
        let socket = match UdpSocket::bind(bind).await {
            Ok(socket) => socket,
            Err(error) => {
                return DnsLeakObservationOutcome::Failed {
                    message: format!("cannot open a UDP socket: {error}"),
                };
            }
        };
        // Connecting the datagram socket pins the peer: the kernel discards
        // an answer from anything but the resolver under observation.
        if let Err(error) = socket.connect(target).await {
            return DnsLeakObservationOutcome::Failed {
                message: format!("cannot connect the probe socket to {target}: {error}"),
            };
        }
        let deadline = echo_deadline(request);
        if let Err(outcome) = send_with_deadline(&socket, &query, deadline).await {
            return outcome;
        }
        let mut buffer = vec![0u8; MAX_RESPONSE_BYTES];
        let answer = match timeout(deadline, socket.recv(&mut buffer)).await {
            Err(_) => return DnsLeakObservationOutcome::TimedOut,
            Ok(Err(error)) => {
                return DnsLeakObservationOutcome::Failed {
                    message: format!("cannot read the echo answer from {target}: {error}"),
                };
            }
            Ok(Ok(len)) => &buffer[..len],
        };
        read_observation(answer, &question, &target.to_string())
    }

    async fn observe_doh(
        &self,
        url: &str,
        probe: &DnsLeakEchoProbe,
        request: &DnsLeakEchoRequest,
    ) -> DnsLeakObservationOutcome {
        let question = echo_question(&probe.question);
        let query = match dns_wire::encode_query(&question) {
            Ok(query) => query,
            Err(error) => {
                return DnsLeakObservationOutcome::Failed {
                    message: error.to_string(),
                };
            }
        };
        let deadline = echo_deadline(request);
        let response = match self
            .client
            .post(url)
            .header(
                infiltrator_http::reqwest::header::CONTENT_TYPE,
                DOH_CONTENT_TYPE,
            )
            .header(infiltrator_http::reqwest::header::ACCEPT, DOH_CONTENT_TYPE)
            .timeout(deadline)
            .body(query)
            .send()
            .await
        {
            Ok(response) => response,
            Err(error) if error.is_timeout() => return DnsLeakObservationOutcome::TimedOut,
            Err(error) => {
                return DnsLeakObservationOutcome::Failed {
                    message: format!("DoH request to {url} failed: {error}"),
                };
            }
        };
        if !response.status().is_success() {
            return DnsLeakObservationOutcome::Failed {
                message: format!("DoH endpoint {url} returned HTTP {}", response.status()),
            };
        }
        let body = match response.bytes().await {
            Ok(body) => body,
            Err(error) => {
                return DnsLeakObservationOutcome::Failed {
                    message: format!("cannot read the DoH answer from {url}: {error}"),
                };
            }
        };
        read_observation(&body, &question, url)
    }

    async fn observe_system(
        &self,
        probe: &DnsLeakEchoProbe,
        request: &DnsLeakEchoRequest,
    ) -> DnsLeakObservationOutcome {
        let deadline = echo_deadline(request);
        // The platform resolver is the path a leak actually flows through,
        // but the OS may answer from its own cache; a cached answer is still
        // an identity some resolver observed, never an invented one.
        match timeout(
            deadline,
            tokio::net::lookup_host((probe.question.as_str(), 53)),
        )
        .await
        {
            Err(_) => DnsLeakObservationOutcome::TimedOut,
            Ok(Err(error)) => DnsLeakObservationOutcome::Failed {
                message: format!(
                    "the platform resolver cannot resolve {}: {error}",
                    probe.question
                ),
            },
            Ok(Ok(mut addresses)) => match addresses.next() {
                Some(address) => DnsLeakObservationOutcome::Observed {
                    identity: address.ip().to_string(),
                },
                None => DnsLeakObservationOutcome::InvalidResponse {
                    reason: "the platform resolver returned no address".to_owned(),
                },
            },
        }
    }
}

#[async_trait::async_trait]
impl DnsLeakEchoPort for HttpDnsLeakEchoProbe {
    async fn observe(&self, request: DnsLeakEchoRequest) -> Result<DnsLeakEchoReport, PortError> {
        if request.probes.is_empty() {
            return Err(PortError::Failed(
                "the DNS leak echo probe needs at least one configured source".to_owned(),
            ));
        }
        // Sequential on purpose: every observation stays attributable to
        // exactly one exchange with one configured source.
        let mut observations = Vec::with_capacity(request.probes.len());
        for probe in &request.probes {
            observations.push(self.observe_probe(probe, &request).await);
        }
        Ok(DnsLeakEchoReport::new(observations))
    }
}

/// The single `A` question of one echo probe, with a fresh transaction id.
fn echo_question(qname: &str) -> DnsQuestion {
    DnsQuestion::a_record(next_query_id(), qname)
}

fn echo_deadline(request: &DnsLeakEchoRequest) -> Duration {
    Duration::from_millis(u64::from(request.timeout_ms))
}

async fn send_with_deadline(
    socket: &UdpSocket,
    query: &[u8],
    deadline: Duration,
) -> Result<(), DnsLeakObservationOutcome> {
    match timeout(deadline, socket.send(query)).await {
        Err(_) => Err(DnsLeakObservationOutcome::TimedOut),
        Ok(Err(error)) => Err(DnsLeakObservationOutcome::Failed {
            message: format!("cannot send the echo query: {error}"),
        }),
        Ok(Ok(_)) => Ok(()),
    }
}

/// Validate the answer and read the echoed resolver identity from it.
fn read_observation(
    answer: &[u8],
    question: &DnsQuestion,
    peer: &str,
) -> DnsLeakObservationOutcome {
    if let Err(error) = dns_wire::validate_response(answer, question) {
        return DnsLeakObservationOutcome::InvalidResponse {
            reason: error.to_string(),
        };
    }
    let rcode = dns_wire::response_rcode(answer).unwrap_or(0);
    if rcode != 0 {
        return DnsLeakObservationOutcome::InvalidResponse {
            reason: format!("the echo authority answered {peer} with rcode {rcode}"),
        };
    }
    match dns_wire::extract_a_record(answer, question) {
        Ok(Some(address)) => DnsLeakObservationOutcome::Observed {
            identity: address.to_string(),
        },
        Ok(None) => DnsLeakObservationOutcome::InvalidResponse {
            reason:
                "the echo authority answered without an A record to read the observed resolver from"
                    .to_owned(),
        },
        Err(error) => DnsLeakObservationOutcome::InvalidResponse {
            reason: error.to_string(),
        },
    }
}

#[cfg(test)]
#[path = "dns_leak_io_test.rs"]
mod dns_leak_io_test;

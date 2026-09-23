//! DUAL-14-10: the real per-nameserver latency prober.
//!
//! The adapter encodes a minimal DNS query, sends it to each configured
//! nameserver, times the answer and validates the response id before
//! publishing a number:
//!
//! * plain entries (`ip:port`, `ip`, `udp://host:port`) go over UDP with the
//!   socket connected to the target, so the kernel drops datagrams from any
//!   other source;
//! * `https://` / `http://` / `doh://` entries are sent as a wire-format DoH
//!   POST through the shared HTTP client.
//!
//! Transports this host cannot drive (DoT, DoQ, DNSCrypt, DHCP, `system`,
//! HTTP/3) are reported as `NotProbed` with the typed reason instead of a
//! fabricated latency.

use infiltrator_contract::dns_latency::{
    DnsLatencyProbeRequest, DnsLatencyReport, DnsProbeOutcome, DnsProbeTarget, DnsProbeTransport,
    DnsServerLatency, MAX_REPORTED_RTT_MS,
};
use infiltrator_http::HttpClient;
use infiltrator_ports::dns_latency::DnsLatencyProbePort;
use infiltrator_ports::error::PortError;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant, SystemTime};
use tokio::net::UdpSocket;
use tokio::time::timeout;

use crate::dns_wire::{self, DnsQuestion, MAX_RESPONSE_BYTES};

/// The mihomo `Content-Type`/`Accept` for a wire-format DoH exchange.
const DOH_CONTENT_TYPE: &str = "application/dns-message";

/// The latency prober cannot time the platform resolver keyword: there is no
/// endpoint to address. The DNS leak echo prober drives the same `system`
/// keyword through a real host name lookup (see `dns_leak_io`).
const SYSTEM_RESOLVER_REASON: &str = "the system resolver is not an upstream this host can probe";

pub struct HttpDnsLatencyProber {
    client: HttpClient,
}

impl Default for HttpDnsLatencyProber {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpDnsLatencyProber {
    pub fn new() -> Self {
        Self {
            client: infiltrator_http::build_http_client(),
        }
    }

    /// Probe with a caller-supplied client (used by the deterministic tests).
    pub fn with_client(client: HttpClient) -> Self {
        Self { client }
    }

    /// Probe one target; every failure is a result, never an error.
    pub async fn probe_target(
        &self,
        target: &DnsProbeTarget,
        request: &DnsLatencyProbeRequest,
    ) -> DnsServerLatency {
        let (transport, outcome) = match plan_for(&target.address) {
            ProbePlan::Undrivable { reason } => (
                DnsProbeTransport::Undrivable {
                    reason: reason.clone(),
                },
                DnsProbeOutcome::NotProbed { reason },
            ),
            // A latency number for "whatever the platform resolver does" is
            // not a fact this host can attribute, so 14-10 keeps the typed
            // refusal while the leak echo prober drives the same path.
            ProbePlan::System => {
                let reason = SYSTEM_RESOLVER_REASON.to_owned();
                (
                    DnsProbeTransport::Undrivable {
                        reason: reason.clone(),
                    },
                    DnsProbeOutcome::NotProbed { reason },
                )
            }
            ProbePlan::Udp { host, port } => (
                DnsProbeTransport::Udp,
                self.probe_udp(&host, port, request).await,
            ),
            ProbePlan::Doh { url } => (DnsProbeTransport::Doh, self.probe_doh(&url, request).await),
        };
        DnsServerLatency {
            address: target.address.clone(),
            is_fallback: target.is_fallback,
            transport,
            outcome,
        }
    }

    async fn probe_udp(
        &self,
        host: &str,
        port: u16,
        request: &DnsLatencyProbeRequest,
    ) -> DnsProbeOutcome {
        let question = new_question(&request.question);
        let query = match dns_wire::encode_query(&question) {
            Ok(query) => query,
            Err(error) => {
                return DnsProbeOutcome::Failed {
                    message: error.to_string(),
                };
            }
        };
        let target = match resolve_target(host, port).await {
            Ok(target) => target,
            Err(message) => return DnsProbeOutcome::Failed { message },
        };
        let bind = if target.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        };
        let socket = match UdpSocket::bind(bind).await {
            Ok(socket) => socket,
            Err(error) => {
                return DnsProbeOutcome::Failed {
                    message: format!("cannot open a UDP socket: {error}"),
                };
            }
        };
        // Connecting the datagram socket pins the peer: the kernel discards
        // any answer that does not come from the probed nameserver.
        if let Err(error) = socket.connect(target).await {
            return DnsProbeOutcome::Failed {
                message: format!("cannot connect the probe socket to {target}: {error}"),
            };
        }
        let deadline = Duration::from_millis(u64::from(request.timeout_ms));
        let started = Instant::now();
        match timeout(deadline, socket.send(&query)).await {
            Err(_) => return DnsProbeOutcome::TimedOut,
            Ok(Err(error)) => {
                return DnsProbeOutcome::Failed {
                    message: format!("cannot send the probe query: {error}"),
                };
            }
            Ok(Ok(_)) => {}
        }
        let mut buffer = vec![0u8; MAX_RESPONSE_BYTES];
        match timeout(deadline, socket.recv(&mut buffer)).await {
            Err(_) => DnsProbeOutcome::TimedOut,
            Ok(Err(error)) => DnsProbeOutcome::Failed {
                message: format!("cannot read the probe answer from {target}: {error}"),
            },
            Ok(Ok(len)) => match dns_wire::validate_response(&buffer[..len], &question) {
                Ok(()) => DnsProbeOutcome::Measured {
                    rtt_ms: clamp_rtt(started.elapsed()),
                },
                Err(error) => DnsProbeOutcome::InvalidResponse {
                    reason: error.to_string(),
                },
            },
        }
    }

    async fn probe_doh(&self, url: &str, request: &DnsLatencyProbeRequest) -> DnsProbeOutcome {
        let question = new_question(&request.question);
        let query = match dns_wire::encode_query(&question) {
            Ok(query) => query,
            Err(error) => {
                return DnsProbeOutcome::Failed {
                    message: error.to_string(),
                };
            }
        };
        let deadline = Duration::from_millis(u64::from(request.timeout_ms));
        let started = Instant::now();
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
            Err(error) if error.is_timeout() => return DnsProbeOutcome::TimedOut,
            Err(error) => {
                return DnsProbeOutcome::Failed {
                    message: format!("DoH request to {url} failed: {error}"),
                };
            }
        };
        if !response.status().is_success() {
            return DnsProbeOutcome::Failed {
                message: format!("DoH endpoint {url} returned HTTP {}", response.status()),
            };
        }
        let body = match response.bytes().await {
            Ok(body) => body,
            Err(error) => {
                return DnsProbeOutcome::Failed {
                    message: format!("cannot read the DoH answer from {url}: {error}"),
                };
            }
        };
        match dns_wire::validate_response(&body, &question) {
            Ok(()) => DnsProbeOutcome::Measured {
                rtt_ms: clamp_rtt(started.elapsed()),
            },
            Err(error) => DnsProbeOutcome::InvalidResponse {
                reason: error.to_string(),
            },
        }
    }
}

#[async_trait::async_trait]
impl DnsLatencyProbePort for HttpDnsLatencyProber {
    async fn probe(&self, request: DnsLatencyProbeRequest) -> Result<DnsLatencyReport, PortError> {
        if request.targets.is_empty() {
            return Err(PortError::Failed(
                "the latency probe needs at least one nameserver".to_owned(),
            ));
        }
        // Sequential on purpose: one stalled nameserver must not overlap its
        // deadline with another probe's timing, so every measured round trip
        // is attributable to exactly one exchange.
        let mut results = Vec::with_capacity(request.targets.len());
        for target in &request.targets {
            results.push(self.probe_target(target, &request).await);
        }
        Ok(DnsLatencyReport::measured(&request.question, results))
    }
}

/// The transport this host can drive for one configured address.
#[derive(Debug)]
pub(crate) enum ProbePlan {
    Udp {
        host: String,
        port: u16,
    },
    Doh {
        url: String,
    },
    /// The platform resolver keyword `system`. The latency prober cannot
    /// time it (there is no endpoint); the echo prober drives it through a
    /// host name lookup.
    System,
    Undrivable {
        reason: String,
    },
}

pub(crate) fn plan_for(address: &str) -> ProbePlan {
    let raw = address.trim();
    if raw.is_empty() {
        return undrivable("the nameserver entry is empty");
    }
    let (scheme, rest) = match raw.split_once("://") {
        Some((scheme, rest)) => (scheme.to_ascii_lowercase(), rest),
        None => (String::new(), raw),
    };
    // `system` is mihomo's keyword for "use the platform resolver": there is
    // no endpoint to send a query to, but the resolver path itself exists.
    if scheme.is_empty() && raw.eq_ignore_ascii_case("system") {
        return ProbePlan::System;
    }
    match scheme.as_str() {
        "" | "udp" => match split_host_port(rest, 53) {
            Some((host, port)) => ProbePlan::Udp { host, port },
            None => undrivable("the plain nameserver entry has no usable host:port"),
        },
        "https" | "http" => ProbePlan::Doh {
            url: strip_tag(raw).to_owned(),
        },
        "doh" => ProbePlan::Doh {
            url: format!("https://{}", strip_tag(rest)),
        },
        "h3" => undrivable("HTTP/3 DoH needs an h3 client this host does not provide"),
        "tls" | "dot" => undrivable("DNS over TLS is not probed by this host"),
        "quic" | "doq" => undrivable("DNS over QUIC is not probed by this host"),
        "sdns" => undrivable("DNSCrypt stamps are not probed by this host"),
        "dhcp" => undrivable("the DHCP-provided resolver is not an upstream this host can probe"),
        other => undrivable(&format!("unsupported nameserver scheme {other}://")),
    }
}

fn undrivable(reason: &str) -> ProbePlan {
    ProbePlan::Undrivable {
        reason: reason.to_owned(),
    }
}

/// mihomo allows a `#tag` suffix on upstream entries; it is an interface/proxy
/// selector, not part of the endpoint.
fn strip_tag(value: &str) -> &str {
    value.split('#').next().unwrap_or(value).trim()
}

/// Split `host:port` / `host` / `[v6]:port` / `[v6]`, defaulting the port.
pub(crate) fn split_host_port(value: &str, default_port: u16) -> Option<(String, u16)> {
    let value = strip_tag(value);
    if value.is_empty() {
        return None;
    }
    if value.parse::<IpAddr>().is_ok() {
        return Some((value.to_owned(), default_port));
    }
    if let Some(rest) = value.strip_prefix('[') {
        let (host, after) = rest.split_once(']')?;
        if host.parse::<IpAddr>().is_err() {
            return None;
        }
        return match after.strip_prefix(':') {
            Some(port) => port.parse::<u16>().ok().map(|port| (host.to_owned(), port)),
            None if after.is_empty() => Some((host.to_owned(), default_port)),
            None => None,
        };
    }
    match value.rsplit_once(':') {
        Some((host, port))
            if !host.is_empty()
                && !port.is_empty()
                && port.chars().all(|value| value.is_ascii_digit()) =>
        {
            port.parse::<u16>().ok().map(|port| (host.to_owned(), port))
        }
        _ => Some((value.to_owned(), default_port)),
    }
}

pub(crate) async fn resolve_target(host: &str, port: u16) -> Result<SocketAddr, String> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, port));
    }
    match tokio::net::lookup_host((host, port)).await {
        Ok(mut addresses) => addresses
            .next()
            .ok_or_else(|| format!("{host} resolves to no address")),
        Err(error) => Err(format!(
            "cannot resolve the nameserver host {host}: {error}"
        )),
    }
}

fn new_question(qname: &str) -> DnsQuestion {
    DnsQuestion::a_record(next_query_id(), qname)
}

/// A distinct transaction id per probe: the process start time seeds a
/// counter, so two probes in the same session never share an id and an
/// unrelated answer cannot validate by accident. Shared with the DUAL-14-08
/// echo prober so neither adapter can collide with the other's ids.
pub(crate) fn next_query_id() -> u16 {
    static NEXT: AtomicU16 = AtomicU16::new(0);
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|elapsed| elapsed.subsec_nanos() as u16)
        .unwrap_or(0);
    NEXT.fetch_add(1, Ordering::Relaxed).wrapping_add(seed)
}

fn clamp_rtt(elapsed: Duration) -> u32 {
    u32::try_from(elapsed.as_millis())
        .unwrap_or(MAX_REPORTED_RTT_MS)
        .min(MAX_REPORTED_RTT_MS)
}

#[cfg(test)]
#[path = "dns_latency_io_test.rs"]
mod dns_latency_io_test;

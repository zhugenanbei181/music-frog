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
//! The observation is read with the source's *declared* rule: an `A` record
//! ([`DnsLeakEchoRecord::FirstAddress`]) or one of the TXT rules
//! ([`DnsLeakEchoRecord::TxtFirstValue`] /
//! [`DnsLeakEchoRecord::TxtFirstIpAddress`] /
//! [`DnsLeakEchoRecord::TxtKeyedValue`]).
//! A controlled echo authority answers the probe with the resolver's source
//! address as it observed it. Transports this host cannot drive (DoT, DoQ,
//! DNSCrypt, DHCP, HTTP/3) are reported as `NotProbed` with the typed reason,
//! and an answer that does not carry the declared record — or carries more
//! than one distinct candidate — is `InvalidResponse`: never a fabricated
//! identity. The address decoding and the wire transaction id are shared with
//! the DUAL-14-10 latency prober.

use infiltrator_contract::dns_leak::{
    DnsLeakEchoProbe, DnsLeakEchoRecord, DnsLeakEchoReport, DnsLeakEchoRequest, DnsLeakObservation,
    DnsLeakObservationOutcome, DnsLeakProbeTransport,
};
use infiltrator_http::HttpClient;
use infiltrator_ports::dns_leak::DnsLeakEchoPort;
use infiltrator_ports::error::PortError;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};
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
        let question = echo_question(&probe.question, &probe.record);
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
        let deadline = echo_deadline(request);
        let answer = match exchange_udp(target, &query, deadline).await {
            Ok(answer) => answer,
            Err(outcome) => return outcome,
        };
        read_observation(&answer, &question, &target.to_string(), &probe.record)
    }

    async fn observe_doh(
        &self,
        url: &str,
        probe: &DnsLeakEchoProbe,
        request: &DnsLeakEchoRequest,
    ) -> DnsLeakObservationOutcome {
        let question = echo_question(&probe.question, &probe.record);
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
        read_observation(&body, &question, url, &probe.record)
    }

    async fn observe_system(
        &self,
        probe: &DnsLeakEchoProbe,
        request: &DnsLeakEchoRequest,
    ) -> DnsLeakObservationOutcome {
        match probe.record {
            DnsLeakEchoRecord::FirstAddress => self.observe_system_address(probe, request).await,
            DnsLeakEchoRecord::TxtFirstValue
            | DnsLeakEchoRecord::TxtFirstIpAddress
            | DnsLeakEchoRecord::TxtKeyedValue { .. } => {
                self.observe_system_txt(probe, request).await
            }
        }
    }

    /// The platform resolver path for an `A` question: a host name lookup,
    /// which is the path a leak actually flows through.
    async fn observe_system_address(
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

    /// The platform resolver path for a `TXT` question. The standard name
    /// lookup only carries `A`/`AAAA`, so a TXT probe reads the resolver
    /// addresses the OS is actually configured with and asks them directly.
    /// A host with no readable platform resolver is a typed failure.
    async fn observe_system_txt(
        &self,
        probe: &DnsLeakEchoProbe,
        request: &DnsLeakEchoRequest,
    ) -> DnsLeakObservationOutcome {
        let nameservers = system_nameservers();
        if nameservers.is_empty() {
            return DnsLeakObservationOutcome::Failed {
                message:
                    "the platform resolver is not readable on this host, so no TXT echo can be observed"
                        .to_owned(),
            };
        }
        let question = echo_question(&probe.question, &probe.record);
        let query = match dns_wire::encode_query(&question) {
            Ok(query) => query,
            Err(error) => {
                return DnsLeakObservationOutcome::Failed {
                    message: error.to_string(),
                };
            }
        };
        let deadline = echo_deadline(request);
        let started = Instant::now();
        let mut last: Option<DnsLeakObservationOutcome> = None;
        for nameserver in nameservers {
            let remaining = deadline.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                break;
            }
            let target = SocketAddr::new(nameserver, 53);
            match exchange_udp(target, &query, remaining).await {
                Ok(answer) => {
                    return read_observation(
                        &answer,
                        &question,
                        &target.to_string(),
                        &probe.record,
                    );
                }
                Err(outcome) => last = Some(outcome),
            }
        }
        last.unwrap_or(DnsLeakObservationOutcome::TimedOut)
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

/// The question of one echo probe: the record type is the source's declared
/// extraction rule, and the transaction id is fresh per probe.
fn echo_question(qname: &str, record: &DnsLeakEchoRecord) -> DnsQuestion {
    if record.is_txt() {
        DnsQuestion::txt(next_query_id(), qname)
    } else {
        DnsQuestion::a_record(next_query_id(), qname)
    }
}

fn echo_deadline(request: &DnsLeakEchoRequest) -> Duration {
    Duration::from_millis(u64::from(request.timeout_ms))
}

/// One connected UDP exchange with a resolver, bounded by `deadline`. The
/// connected socket pins the peer, so the kernel discards an answer from
/// anything but the resolver under observation.
async fn exchange_udp(
    target: SocketAddr,
    query: &[u8],
    deadline: Duration,
) -> Result<Vec<u8>, DnsLeakObservationOutcome> {
    let bind = if target.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let socket =
        UdpSocket::bind(bind)
            .await
            .map_err(|error| DnsLeakObservationOutcome::Failed {
                message: format!("cannot open a UDP socket: {error}"),
            })?;
    socket
        .connect(target)
        .await
        .map_err(|error| DnsLeakObservationOutcome::Failed {
            message: format!("cannot connect the probe socket to {target}: {error}"),
        })?;
    send_with_deadline(&socket, query, deadline).await?;
    let mut buffer = vec![0u8; MAX_RESPONSE_BYTES];
    match timeout(deadline, socket.recv(&mut buffer)).await {
        Err(_) => Err(DnsLeakObservationOutcome::TimedOut),
        Ok(Err(error)) => Err(DnsLeakObservationOutcome::Failed {
            message: format!("cannot read the echo answer from {target}: {error}"),
        }),
        Ok(Ok(len)) => Ok(buffer[..len].to_vec()),
    }
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

/// Validate the answer and read the echoed resolver identity from it with the
/// source's declared rule. A missing or ambiguous record is a typed
/// `InvalidResponse`, never a guess.
fn read_observation(
    answer: &[u8],
    question: &DnsQuestion,
    peer: &str,
    record: &DnsLeakEchoRecord,
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
    match extract_identity(answer, question, record) {
        Ok(identity) => DnsLeakObservationOutcome::Observed { identity },
        Err(reason) => DnsLeakObservationOutcome::InvalidResponse { reason },
    }
}

/// Read the resolver identity from an answer using the declared rule. Every
/// rule is explicit: nothing is inferred from whatever the answer contains.
fn extract_identity(
    answer: &[u8],
    question: &DnsQuestion,
    record: &DnsLeakEchoRecord,
) -> Result<String, String> {
    match record {
        DnsLeakEchoRecord::FirstAddress => match dns_wire::extract_a_record(answer, question) {
            Ok(Some(address)) => Ok(address.to_string()),
            Ok(None) => Err(
                "the echo authority answered without an A record to read the observed resolver from"
                    .to_owned(),
            ),
            Err(error) => Err(error.to_string()),
        },
        DnsLeakEchoRecord::TxtFirstValue => {
            let records = txt_records(answer, question)?;
            let value = records
                .iter()
                .find_map(|strings| strings.first())
                .ok_or_else(|| {
                    "the echo authority answered with an empty TXT record to read the observed resolver from"
                        .to_owned()
                })?;
            validate_resolver_identity(value)
        }
        DnsLeakEchoRecord::TxtFirstIpAddress => {
            let records = txt_records(answer, question)?;
            first_ip_txt_value(&records)
        }
        DnsLeakEchoRecord::TxtKeyedValue { key } => {
            let records = txt_records(answer, question)?;
            let value = keyed_txt_value(&records, key)?;
            validate_resolver_identity(value)
        }
    }
}

/// The first TXT character-string that parses as an IP address. Zero addresses
/// is missing; more than one distinct address is ambiguous; both are typed
/// errors instead of an arbitrary pick. This is a declared rule, not a guess:
/// the authority is configured as one that answers with its observed address
/// plus unrelated TXT metadata.
fn first_ip_txt_value(records: &[Vec<String>]) -> Result<String, String> {
    let mut distinct: Vec<&str> = Vec::new();
    for strings in records {
        for value in strings {
            let value = value.trim();
            if value.parse::<IpAddr>().is_ok() && !distinct.contains(&value) {
                distinct.push(value);
            }
        }
    }
    match distinct.as_slice() {
        [] => Err(
            "the TXT answer carried no IP address to read the observed resolver from".to_owned(),
        ),
        [value] => Ok((*value).to_owned()),
        values => Err(format!(
            "the TXT answer carried {} distinct IP addresses, so the observed resolver is ambiguous",
            values.len()
        )),
    }
}

/// Every TXT record of the answer, or a typed reason when there is none.
fn txt_records(answer: &[u8], question: &DnsQuestion) -> Result<Vec<Vec<String>>, String> {
    match dns_wire::extract_txt_records(answer, question) {
        Ok(Some(records)) => Ok(records),
        Ok(None) => Err(
            "the echo authority answered without a TXT record to read the observed resolver from"
                .to_owned(),
        ),
        Err(error) => Err(error.to_string()),
    }
}

/// The value that follows the exact `key` string in a TXT record. Zero matches
/// is missing; more than one distinct value is ambiguous; both are typed
/// errors instead of an arbitrary pick.
fn keyed_txt_value<'a>(records: &'a [Vec<String>], key: &str) -> Result<&'a str, String> {
    let mut candidates: Vec<&str> = Vec::new();
    for strings in records {
        for pair in strings.windows(2) {
            if pair[0] == key {
                candidates.push(&pair[1]);
            }
        }
    }
    let mut distinct: Vec<&str> = Vec::new();
    for candidate in candidates {
        if !distinct.contains(&candidate) {
            distinct.push(candidate);
        }
    }
    match distinct.as_slice() {
        [] => Err(format!(
            "the TXT answer carried no {key:?} entry to read the observed resolver from"
        )),
        [value] => Ok(*value),
        values => Err(format!(
            "the TXT answer carried {} distinct {key:?} entries, so the observed resolver is ambiguous",
            values.len()
        )),
    }
}

/// A resolver identity is an IP address: a TXT value that is not one is a
/// malformed echo, never an identity to publish.
fn validate_resolver_identity(value: &str) -> Result<String, String> {
    let value = value.trim();
    match value.parse::<IpAddr>() {
        Ok(_) => Ok(value.to_owned()),
        Err(_) => Err(format!(
            "the TXT answer value {value:?} is not a resolver IP address"
        )),
    }
}

/// The resolver addresses the platform resolver is configured with, read from
/// `/etc/resolv.conf`. An unreadable or empty configuration is an honest "no
/// addressable platform resolver", never a fallback to a guessed resolver.
fn system_nameservers() -> Vec<IpAddr> {
    match std::fs::read_to_string("/etc/resolv.conf") {
        Ok(text) => parse_resolv_conf(&text),
        Err(_) => Vec::new(),
    }
}

/// Parse `nameserver` lines from a `resolv.conf` document. Split out so the
/// parsing is testable without touching the host file.
fn parse_resolv_conf(text: &str) -> Vec<IpAddr> {
    let mut servers = Vec::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        if parts.next() != Some("nameserver") {
            continue;
        }
        let Some(token) = parts.next() else {
            continue;
        };
        // A link-local IPv6 nameserver can carry a zone id; the address
        // without the zone is what a socket targets.
        let token = token.split('%').next().unwrap_or(token);
        if let Ok(address) = token.parse::<IpAddr>()
            && !servers.contains(&address)
        {
            servers.push(address);
        }
    }
    servers
}

#[cfg(test)]
#[path = "dns_leak_io_test.rs"]
mod dns_leak_io_test;

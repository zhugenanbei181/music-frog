//! DUAL-14-08: the shared DNS leak cross-source probe use-case.
//!
//! One instance is shared by the command handler and the surface reader, so
//! the last real cross-source probe reaches both surfaces. The application
//! owns the probe policy: it generates a fresh subdomain per configured
//! source (the authority is configured by the host), queries every source
//! through the injected host echo prober and compares the observed resolver
//! identities. A host without a fact source (no prober, or no configured echo
//! authority) stays a typed unsupported status; a divergent result lists the
//! observed facts and never guesses which one is a leak.

use futures_util::lock;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::dns_leak::{
    DEFAULT_ECHO_TIMEOUT_MS, DnsLeakEchoProbe, DnsLeakEchoRecord, DnsLeakEchoRequest,
    DnsLeakObservation, DnsLeakObservationOutcome, DnsLeakOperation, DnsLeakProbeName,
    DnsLeakProbeSource, DnsLeakProbeTransport, DnsLeakReport, DnsLeakStatus, MAX_ECHO_TIMEOUT_MS,
    MAX_PROBE_NAME_LEN,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_ports::dns_leak::{DnsLeakEchoPort, DnsLeakProbePort};
use infiltrator_ports::error::PortError;
use std::fmt;
use std::process::id;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// The typed reason a host without an echo prober publishes.
pub const NO_ECHO_PORT_REASON: &str = "this host injected no DNS leak echo prober";

/// The typed reason a host with a prober but no configured source publishes.
pub const NO_ECHO_SOURCE_REASON: &str =
    "no controlled DNS leak echo authority is configured on this host";

/// The typed reason one source is skipped before any exchange.
const INVALID_AUTHORITY_REASON: &str = "the configured echo authority is not a valid DNS zone name";

/// The typed reason a host prober answered fewer observations than asked.
const MISSING_OBSERVATION_REASON: &str =
    "the host echo prober returned no observation for this source";

/// DUAL-14-08: the real default echo authorities this host cross-checks.
///
/// Two independent third-party public TXT echo services are asked through the
/// platform resolver (`system`): `whoami.ds.akahelp.net` declares the
/// `"ip" "<resolver-ip>"` pair rule, and `o-o.myaddr.l.google.com` declares
/// the first-IP TXT rule (it also answers an EDNS client-subnet string). A
/// leak conclusion is real cross-source corroboration only when both
/// observations agree; the application still compares them and never
/// hardcodes a verdict. If either service is unreachable the report is a
/// typed failure, never a guess.
pub fn default_echo_sources() -> Vec<DnsLeakProbeSource> {
    vec![
        DnsLeakProbeSource::exact(
            "system",
            "whoami.ds.akahelp.net",
            DnsLeakEchoRecord::TxtKeyedValue {
                key: "ip".to_owned(),
            },
        ),
        DnsLeakProbeSource::exact(
            "system",
            "o-o.myaddr.l.google.com",
            DnsLeakEchoRecord::TxtFirstIpAddress,
        ),
    ]
}

#[derive(Clone)]
pub struct DnsLeakApplication {
    port: Option<Arc<dyn DnsLeakEchoPort>>,
    sources: Vec<DnsLeakProbeSource>,
    timeout_ms: u32,
    last: Arc<Mutex<DnsLeakReport>>,
    operation: Arc<lock::Mutex<()>>,
}

impl fmt::Debug for DnsLeakApplication {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DnsLeakApplication")
            .finish_non_exhaustive()
    }
}

impl DnsLeakApplication {
    pub fn new(port: Option<Arc<dyn DnsLeakEchoPort>>, sources: Vec<DnsLeakProbeSource>) -> Self {
        Self {
            port,
            sources,
            timeout_ms: DEFAULT_ECHO_TIMEOUT_MS,
            last: Arc::new(Mutex::new(DnsLeakReport::default())),
            operation: Arc::new(lock::Mutex::new(())),
        }
    }

    /// A host that cannot cross-check keeps the honest typed refusal.
    pub fn unconfigured() -> Self {
        Self::new(None, Vec::new())
    }

    /// Probe with a caller-supplied deadline (used by the deterministic tests).
    pub fn with_timeout_ms(mut self, timeout_ms: u32) -> Self {
        if timeout_ms > 0 {
            self.timeout_ms = timeout_ms.min(MAX_ECHO_TIMEOUT_MS);
        }
        self
    }

    pub fn has_echo_prober(&self) -> bool {
        self.port.is_some()
    }

    pub fn sources(&self) -> &[DnsLeakProbeSource] {
        &self.sources
    }

    /// Whether this host can produce real cross-source leak facts.
    pub fn status(&self) -> DnsLeakStatus {
        if self.port.is_none() {
            return DnsLeakStatus::unsupported(NO_ECHO_PORT_REASON);
        }
        if self.sources.is_empty() {
            return DnsLeakStatus::unsupported(NO_ECHO_SOURCE_REASON);
        }
        DnsLeakStatus::Ready
    }

    /// The last probe, with the status and configured sources re-derived from
    /// the application so a host can never publish a stale readiness.
    pub fn last_report(&self) -> DnsLeakReport {
        let mut report = match self.last.lock() {
            Ok(report) => report.clone(),
            Err(_) => DnsLeakReport {
                operation: DnsLeakOperation::Failed {
                    failure: Failure::new(
                        ErrorCode::InvalidState,
                        "DNS leak report lock is poisoned",
                        false,
                    ),
                },
                ..Default::default()
            },
        };
        report.status = self.status();
        report.sources = self.sources.clone();
        report
    }

    /// Ask the host echo prober for every configured source, then publish the
    /// one shared report. The random subdomains are generated here, so two
    /// surfaces can never probe different questions.
    async fn run_probe(&self, port: &Arc<dyn DnsLeakEchoPort>) -> Result<DnsLeakReport, PortError> {
        let mut pending: Vec<(usize, DnsLeakEchoProbe)> = Vec::new();
        let mut observations: Vec<Option<DnsLeakObservation>> =
            (0..self.sources.len()).map(|_| None).collect();
        for (index, source) in self.sources.iter().enumerate() {
            let question = probe_question(source);
            if !is_valid_probe_name(&question) {
                observations[index] = Some(DnsLeakObservation {
                    resolver: source.resolver.clone(),
                    authority: source.authority.clone(),
                    question,
                    transport: DnsLeakProbeTransport::Undrivable {
                        reason: INVALID_AUTHORITY_REASON.to_owned(),
                    },
                    outcome: DnsLeakObservationOutcome::NotProbed {
                        reason: INVALID_AUTHORITY_REASON.to_owned(),
                    },
                });
                continue;
            }
            pending.push((
                index,
                DnsLeakEchoProbe::new(
                    &source.resolver,
                    &source.authority,
                    &question,
                    source.record.clone(),
                ),
            ));
        }

        if !pending.is_empty() {
            let proposal: Vec<DnsLeakEchoProbe> =
                pending.iter().map(|(_, probe)| probe.clone()).collect();
            let request = DnsLeakEchoRequest::new(proposal).with_timeout_ms(self.timeout_ms);
            let echo = port.observe(request).await?;
            let mut returned = echo.observations;
            for (index, probe) in pending {
                let position = returned.iter().position(|observation| {
                    observation.question == probe.question
                        && observation.resolver == probe.resolver
                        && observation.authority == probe.authority
                });
                observations[index] = Some(match position {
                    Some(position) => returned.remove(position),
                    None => DnsLeakObservation {
                        resolver: probe.resolver.clone(),
                        authority: probe.authority.clone(),
                        question: probe.question.clone(),
                        transport: DnsLeakProbeTransport::Undrivable {
                            reason: MISSING_OBSERVATION_REASON.to_owned(),
                        },
                        outcome: DnsLeakObservationOutcome::Failed {
                            message: MISSING_OBSERVATION_REASON.to_owned(),
                        },
                    },
                });
            }
        }

        let report = DnsLeakReport::observed(
            self.sources.clone(),
            observations.into_iter().flatten().collect(),
        );
        *self.last.lock().map_err(|_| {
            PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "DNS leak report lock is poisoned",
                false,
            ))
        })? = report.clone();
        Ok(report)
    }
}

struct PendingLeakProbe(Arc<Mutex<DnsLeakReport>>);
impl Drop for PendingLeakProbe {
    fn drop(&mut self) {
        if let Ok(mut report) = self.0.lock()
            && matches!(report.operation, DnsLeakOperation::Running)
        {
            report.operation = DnsLeakOperation::Failed {
                failure: Failure::new(
                    ErrorCode::Canceled,
                    "DNS cross-source probe was canceled",
                    true,
                ),
            };
        }
    }
}
#[async_trait::async_trait]
impl DnsLeakProbePort for DnsLeakApplication {
    async fn probe(&self) -> Result<DnsLeakReport, PortError> {
        let _operation = self.operation.lock().await;
        {
            let mut report = self
                .last
                .lock()
                .map_err(|_| PortError::Failed("DNS leak report lock is poisoned".into()))?;
            report.operation = DnsLeakOperation::Running;
        }
        let _pending = PendingLeakProbe(self.last.clone());
        let result = match self.port.as_ref() {
            None => Err(PortError::unsupported(Capability::Dns, NO_ECHO_PORT_REASON)),
            Some(_) if self.sources.is_empty() => Err(PortError::unsupported(
                Capability::Dns,
                NO_ECHO_SOURCE_REASON,
            )),
            Some(port) => self.run_probe(port).await,
        };
        if let Err(error) = &result {
            let mut report = self
                .last
                .lock()
                .map_err(|_| PortError::Failed("DNS leak report lock is poisoned".into()))?;
            report.status = self.status();
            report.sources = self.sources.clone();
            report.operation = DnsLeakOperation::Failed {
                failure: Failure::from(error.clone()),
            };
        }
        result
    }
}

/// The question of a valid probe candidate: a fresh subdomain under the
/// configured authority.
pub fn random_probe_question(authority: &str) -> String {
    let authority = authority.trim().trim_end_matches('.');
    if authority.is_empty() {
        return String::new();
    }
    format!("{}.{authority}", random_probe_label())
}

/// The exact question one source is asked. A wildcard authority gets a fresh
/// cache-busting subdomain; a fixed-name public echo service is asked exactly,
/// because a generated label under it is only ever NXDOMAIN.
pub fn probe_question(source: &DnsLeakProbeSource) -> String {
    match source.probe_name {
        DnsLeakProbeName::FreshSubdomain => random_probe_question(&source.authority),
        DnsLeakProbeName::ExactAuthority => {
            source.authority.trim().trim_end_matches('.').to_owned()
        }
    }
}

/// A cache-busting nonce for the probe label. It only has to be unique enough
/// that a resolver cache cannot answer the previous probe; it is not a
/// security token, so the workspace CSPRNG dependency is not pulled in.
pub fn random_probe_label() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos() as u64)
        .unwrap_or(0);
    let counter = NEXT.fetch_add(1, Ordering::Relaxed);
    let mixed = splitmix64(nanos ^ counter.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let mixed = splitmix64(mixed ^ u64::from(id()));
    format!("l{mixed:016x}")
}

fn splitmix64(value: u64) -> u64 {
    let mut state = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    state = (state ^ (state >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    state = (state ^ (state >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    state ^ (state >> 31)
}

/// Whether a generated name can be asked of a real authority.
pub fn is_valid_probe_name(name: &str) -> bool {
    if name.is_empty() || name.len() > MAX_PROBE_NAME_LEN {
        return false;
    }
    name.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label.chars().all(|character| {
                character.is_ascii_alphanumeric() || character == '-' || character == '_'
            })
    })
}

#[cfg(test)]
#[path = "dns_leak_application_test.rs"]
mod tests;

#[cfg(test)]
#[path = "dns_leak_application_state_test.rs"]
mod state_tests;

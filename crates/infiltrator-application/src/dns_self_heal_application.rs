//! DUAL-14-13: the shared DNS self-heal use-case.
//!
//! Three real observations are folded into one typed snapshot:
//!
//! * the `dns.listen` port of the existing port-conflict probe (a taken port
//!   is a hard failure: the kernel cannot serve DNS);
//! * the last per-nameserver latency report (every attempted upstream failing
//!   is the "upstream cannot resolve" signal);
//! * the domain `validate_dns_topology` audit of the configured profile.
//!
//! A fact this host could not observe stays `Unknown`; it is never reported as
//! healthy just because nothing was measured.

use infiltrator_contract::dns_latency::{DnsLatencyReport, DnsLatencySummary};
use infiltrator_contract::dns_self_heal::{
    DnsSelfHealCheck, DnsSelfHealFix, DnsSelfHealKind, DnsSelfHealSnapshot, DnsSelfHealState,
};
use infiltrator_contract::port_conflict::{PortBinding, PortConflict, PortConflictSnapshot};
use infiltrator_domain::dns::DnsConfig;
use infiltrator_domain::dns_topology::{DnsTopologyDiagnostic, TopologySeverity};

/// The `dns.listen` observation inside a port-conflict snapshot, when the host
/// probe covered it.
pub fn dns_listen_conflict(snapshot: &PortConflictSnapshot) -> Option<&PortConflict> {
    snapshot
        .conflicts
        .iter()
        .find(|conflict| conflict.binding == PortBinding::DnsListen)
}

/// Build the shared self-heal read model from the configured profile and the
/// two host observations.
pub fn dns_self_heal_snapshot(
    config: &DnsConfig,
    listen: Option<&PortConflict>,
    latency: &DnsLatencyReport,
) -> DnsSelfHealSnapshot {
    let topology = infiltrator_domain::dns_topology::validate_dns_topology(config);
    DnsSelfHealSnapshot::new(vec![
        listen_port_check(listen),
        upstream_check(latency),
        topology_check(&topology),
    ])
}

fn listen_port_check(listen: Option<&PortConflict>) -> DnsSelfHealCheck {
    let kind = DnsSelfHealKind::ListenPort;
    let Some(conflict) = listen else {
        return DnsSelfHealCheck {
            kind,
            state: DnsSelfHealState::Unknown,
            detail: "dns.listen is not configured, so no listener port was observed".to_owned(),
            fix: None,
        };
    };
    if conflict.available {
        return DnsSelfHealCheck {
            kind,
            state: DnsSelfHealState::Healthy,
            detail: format!("dns.listen port {} is available", conflict.port),
            fix: None,
        };
    }
    let owner = match (&conflict.owner_pid, &conflict.owner_name) {
        (Some(pid), Some(name)) => format!(" by {name} (pid {pid})"),
        _ => String::new(),
    };
    DnsSelfHealCheck {
        kind,
        state: DnsSelfHealState::Critical,
        detail: format!(
            "dns.listen port {} is already bound{owner}; the core cannot serve DNS",
            conflict.port
        ),
        fix: Some(DnsSelfHealFix::RepairDnsListenPort),
    }
}

fn upstream_check(latency: &DnsLatencyReport) -> DnsSelfHealCheck {
    let kind = DnsSelfHealKind::UpstreamResolution;
    let (state, detail, fix) = match latency.summary() {
        DnsLatencySummary::Unsupported { reason } => (
            DnsSelfHealState::Unknown,
            format!("no per-nameserver prober observed upstream reachability ({reason})"),
            None,
        ),
        DnsLatencySummary::NotProbed => (
            DnsSelfHealState::Unknown,
            "the configured upstreams have not been probed in this session".to_owned(),
            None,
        ),
        DnsLatencySummary::AllMeasured {
            count,
            best_ms,
            worst_ms,
        } => (
            DnsSelfHealState::Healthy,
            format!("all {count} upstreams answered ({best_ms}-{worst_ms} ms)"),
            None,
        ),
        DnsLatencySummary::Partial { measured, total } => (
            DnsSelfHealState::Warning,
            format!("only {measured} of {total} upstreams answered"),
            Some(DnsSelfHealFix::RecheckUpstreams),
        ),
        DnsLatencySummary::NoneReachable { total } => (
            DnsSelfHealState::Critical,
            format!("none of the {total} upstreams answered; resolution is unreachable"),
            Some(DnsSelfHealFix::RecheckUpstreams),
        ),
    };
    DnsSelfHealCheck {
        kind,
        state,
        detail,
        fix,
    }
}

fn topology_check(diagnostics: &[DnsTopologyDiagnostic]) -> DnsSelfHealCheck {
    let kind = DnsSelfHealKind::Topology;
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut advisories = 0usize;
    for diagnostic in diagnostics {
        match diagnostic.severity {
            TopologySeverity::Error => errors.push(diagnostic.code.clone()),
            TopologySeverity::Warning => warnings.push(diagnostic.code.clone()),
            TopologySeverity::Info => advisories += 1,
        }
    }
    if !errors.is_empty() {
        return DnsSelfHealCheck {
            kind,
            state: DnsSelfHealState::Critical,
            detail: format!("topology audit errors: {}", errors.join(", ")),
            fix: Some(DnsSelfHealFix::ApplyDnsSettings),
        };
    }
    if !warnings.is_empty() {
        return DnsSelfHealCheck {
            kind,
            state: DnsSelfHealState::Warning,
            detail: format!("topology audit warnings: {}", warnings.join(", ")),
            fix: Some(DnsSelfHealFix::ApplyDnsSettings),
        };
    }
    DnsSelfHealCheck {
        kind,
        state: DnsSelfHealState::Healthy,
        detail: format!("the configured topology has no finding ({advisories} advisories)"),
        fix: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::dns_latency::{
        DEFAULT_PROBE_QUESTION, DnsProbeOutcome, DnsProbeTransport, DnsServerLatency,
    };
    use infiltrator_domain::dns::FallbackFilter;

    fn conflict(available: bool, owner: Option<(&str, u32)>) -> PortConflict {
        PortConflict {
            binding: PortBinding::DnsListen,
            port: 1053,
            available,
            owner_pid: owner.map(|(_, pid)| pid),
            owner_name: owner.map(|(name, _)| name.to_owned()),
            can_release: false,
        }
    }

    fn probe(results: Vec<DnsServerLatency>) -> DnsLatencyReport {
        DnsLatencyReport::measured(DEFAULT_PROBE_QUESTION, results)
    }

    fn measured(address: &str, rtt_ms: u32) -> DnsServerLatency {
        DnsServerLatency {
            address: address.to_owned(),
            is_fallback: false,
            transport: DnsProbeTransport::Udp,
            outcome: DnsProbeOutcome::Measured { rtt_ms },
        }
    }

    fn timed_out(address: &str) -> DnsServerLatency {
        DnsServerLatency {
            address: address.to_owned(),
            is_fallback: false,
            transport: DnsProbeTransport::Udp,
            outcome: DnsProbeOutcome::TimedOut,
        }
    }

    fn healthy_config() -> DnsConfig {
        DnsConfig {
            enable: Some(true),
            default_nameserver: Some(vec!["223.5.5.5".to_owned()]),
            direct_nameserver: Some(vec!["223.5.5.5".to_owned()]),
            proxy_server_nameserver: Some(vec!["223.5.5.5".to_owned()]),
            ..DnsConfig::default()
        }
    }

    #[test]
    fn an_unobserved_fact_is_unknown_and_never_healthy() {
        let snapshot = dns_self_heal_snapshot(
            &healthy_config(),
            None,
            &DnsLatencyReport::unsupported("host injected no DNS latency prober"),
        );
        assert_eq!(
            snapshot.check(DnsSelfHealKind::ListenPort).map(|c| c.state),
            Some(DnsSelfHealState::Unknown)
        );
        assert_eq!(
            snapshot
                .check(DnsSelfHealKind::UpstreamResolution)
                .map(|c| c.state),
            Some(DnsSelfHealState::Unknown)
        );
        assert_eq!(
            snapshot.overall_state(),
            DnsSelfHealState::Healthy,
            "the clean topology audit is the only observed fact"
        );
        assert!(!snapshot.needs_repair());
        assert!(
            !snapshot
                .check(DnsSelfHealKind::ListenPort)
                .unwrap()
                .detail
                .is_empty()
        );
    }

    #[test]
    fn a_taken_listen_port_is_critical_and_suggests_the_real_repair() {
        let listen = conflict(false, Some(("dig", 4242)));
        let snapshot = dns_self_heal_snapshot(
            &healthy_config(),
            Some(&listen),
            &probe(vec![measured("223.5.5.5", 9)]),
        );
        let check = snapshot.check(DnsSelfHealKind::ListenPort).unwrap();
        assert_eq!(check.state, DnsSelfHealState::Critical);
        assert_eq!(check.fix, Some(DnsSelfHealFix::RepairDnsListenPort));
        assert!(check.detail.contains("1053"));
        assert!(check.detail.contains("dig"));
        assert!(check.detail.contains("pid 4242"));
        assert_eq!(snapshot.overall_state(), DnsSelfHealState::Critical);
        assert!(snapshot.needs_repair());

        let free = conflict(true, None);
        let healthy = dns_self_heal_snapshot(
            &healthy_config(),
            Some(&free),
            &probe(vec![measured("223.5.5.5", 9)]),
        );
        let check = healthy.check(DnsSelfHealKind::ListenPort).unwrap();
        assert_eq!(check.state, DnsSelfHealState::Healthy);
        assert_eq!(check.fix, None);
        assert_eq!(healthy.overall_state(), DnsSelfHealState::Healthy);
    }

    #[test]
    fn an_unreachable_upstream_list_is_a_critical_self_heal_finding() {
        let listen = conflict(true, None);
        let snapshot = dns_self_heal_snapshot(
            &healthy_config(),
            Some(&listen),
            &probe(vec![timed_out("223.5.5.5"), timed_out("1.1.1.1")]),
        );
        let check = snapshot.check(DnsSelfHealKind::UpstreamResolution).unwrap();
        assert_eq!(check.state, DnsSelfHealState::Critical);
        assert_eq!(check.fix, Some(DnsSelfHealFix::RecheckUpstreams));
        assert!(check.detail.contains("none of the 2 upstreams answered"));
        assert_eq!(
            snapshot.fixes(),
            vec![DnsSelfHealFix::RecheckUpstreams],
            "the port is free, so only the upstream finding needs action"
        );
    }

    #[test]
    fn a_partial_probe_is_a_warning_not_a_health_claim() {
        let snapshot = dns_self_heal_snapshot(
            &healthy_config(),
            Some(&conflict(true, None)),
            &probe(vec![measured("223.5.5.5", 9), timed_out("1.1.1.1")]),
        );
        let check = snapshot.check(DnsSelfHealKind::UpstreamResolution).unwrap();
        assert_eq!(check.state, DnsSelfHealState::Warning);
        assert!(check.detail.contains("only 1 of 2 upstreams answered"));
        assert_eq!(snapshot.overall_state(), DnsSelfHealState::Warning);
    }

    #[test]
    fn the_static_topology_audit_drives_its_own_fix() {
        let unconfigured = DnsConfig {
            enable: Some(true),
            ..DnsConfig::default()
        };
        let snapshot = dns_self_heal_snapshot(
            &unconfigured,
            Some(&conflict(true, None)),
            &probe(vec![measured("223.5.5.5", 9)]),
        );
        let check = snapshot.check(DnsSelfHealKind::Topology).unwrap();
        assert_eq!(check.state, DnsSelfHealState::Warning);
        assert_eq!(check.fix, Some(DnsSelfHealFix::ApplyDnsSettings));
        assert!(check.detail.contains("BOOTSTRAP_DNS_MISSING"));

        // A fallback-filter without fallback servers is a real warning too.
        let mismatched = DnsConfig {
            enable: Some(true),
            default_nameserver: Some(vec!["223.5.5.5".to_owned()]),
            direct_nameserver: Some(vec!["223.5.5.5".to_owned()]),
            proxy_server_nameserver: Some(vec!["223.5.5.5".to_owned()]),
            fallback_filter: Some(FallbackFilter {
                geoip: Some(true),
                ..FallbackFilter::default()
            }),
            ..DnsConfig::default()
        };
        let snapshot = dns_self_heal_snapshot(
            &mismatched,
            Some(&conflict(true, None)),
            &probe(vec![measured("223.5.5.5", 9)]),
        );
        assert!(
            snapshot
                .check(DnsSelfHealKind::Topology)
                .unwrap()
                .detail
                .contains("FALLBACK_FILTER_WITHOUT_FALLBACK")
        );
    }

    #[test]
    fn the_listen_observation_is_taken_from_its_own_binding() {
        let snapshot = PortConflictSnapshot {
            revision: 1,
            conflicts: vec![
                PortConflict {
                    binding: PortBinding::MixedProxy,
                    port: 7890,
                    available: false,
                    owner_pid: None,
                    owner_name: None,
                    can_release: false,
                },
                conflict(true, None),
            ],
        };
        assert_eq!(dns_listen_conflict(&snapshot).map(|c| c.port), Some(1053));
        assert!(dns_listen_conflict(&PortConflictSnapshot::default()).is_none());
    }
}

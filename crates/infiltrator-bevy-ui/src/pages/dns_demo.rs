//! DUAL-14: the deterministic Bevy DNS demo fixture.
//!
//! Split out of [`crate::pages::dns`] to keep that module inside the source
//! budget. This is the explicit screenshot/demo fixture behind
//! `DemoSurfaceSource`; it is never published as a runtime probe result.

use infiltrator_contract::dns::{
    DnsCacheFlushReport, DnsCoreSwitches, DnsEnhancedMode, DnsFakeIpFilterMode, DnsHostEntry,
    DnsServerTag, FakeIpMappingEntry, FakeIpMappingPool, FakeIpMappingSource,
};
use infiltrator_contract::dns_latency::{
    DEFAULT_PROBE_QUESTION, DnsLatencyReport, DnsProbeOutcome, DnsProbeTransport, DnsServerLatency,
};
use infiltrator_contract::dns_leak::{
    DnsLeakObservation, DnsLeakObservationOutcome, DnsLeakProbeSource, DnsLeakProbeTransport,
    DnsLeakReport,
};
use infiltrator_contract::dns_self_heal::{
    DnsSelfHealCheck, DnsSelfHealFix, DnsSelfHealKind, DnsSelfHealSnapshot, DnsSelfHealState,
};
use infiltrator_contract::stun_probe::{StunMappedAddress, StunProbeObservation, StunProbeReport};

use crate::pages::dns::{DnsProjection, DnsServerItem};

impl DnsProjection {
    /// Believable demo fixture for the DNS page.
    ///
    /// The pinned latency values below are the demo's screenshot content, not
    /// a runtime probe result: the runtime report can only come from an
    /// injected host prober.
    pub fn demo() -> Self {
        Self {
            mode: DnsEnhancedMode::FakeIp,
            cache_entries: 342,
            fake_ip_range: "198.18.0.1/16".to_owned(),
            switches: DnsCoreSwitches {
                enable: true,
                ipv6: true,
                cache: true,
                use_hosts: true,
                use_system_hosts: true,
                respect_rules: false,
            },
            filter_mode: DnsFakeIpFilterMode::Blacklist,
            form: infiltrator_contract::dns_form::DnsWorkbenchForm {
                switches: DnsCoreSwitches {
                    enable: true,
                    ipv6: true,
                    cache: true,
                    use_hosts: true,
                    use_system_hosts: true,
                    respect_rules: false,
                },
                enhanced_mode: DnsEnhancedMode::FakeIp,
                filter_mode: DnsFakeIpFilterMode::Blacklist,
                bootstrap_nameserver: "223.5.5.5".to_owned(),
                nameserver: "https://1.1.1.1/dns-query, tls://8.8.8.8:853".to_owned(),
                fallback: "https://cloudflare-dns.com/dns-query".to_owned(),
                fallback_policy: infiltrator_contract::dns_form::DnsFallbackPolicyDraft {
                    geoip: true,
                    geoip_code: "CN".to_owned(),
                    trigger_ipcidr: "240.0.0.0/4".to_owned(),
                },
                fake_ip_range: "198.18.0.1/16".to_owned(),
                fake_ip_filter: "*.lan, localhost.ptlogin2.qq.com".to_owned(),
                proxy_server_nameserver: "tls://223.5.5.5:853".to_owned(),
                direct_nameserver: "system".to_owned(),
            },
            cache_flush: DnsCacheFlushReport::default(),
            fake_ip_pool: FakeIpMappingPool {
                source: FakeIpMappingSource::LiveConnections,
                range: "198.18.0.1/16".to_owned(),
                total: 2,
                entries: vec![
                    FakeIpMappingEntry {
                        domain: "music.example.org".to_owned(),
                        address: "198.18.0.5".to_owned(),
                    },
                    FakeIpMappingEntry {
                        domain: "cdn.example.net".to_owned(),
                        address: "198.18.0.7".to_owned(),
                    },
                ],
            },
            latency: demo_latency_report(),
            leak: demo_leak_report(),
            stun: demo_stun_report(),
            self_heal: demo_self_heal_snapshot(),
            hosts: vec![
                DnsHostEntry {
                    domain: "router.lan".to_owned(),
                    address: "192.168.1.1".to_owned(),
                },
                DnsHostEntry {
                    domain: "nas.lan".to_owned(),
                    address: "192.168.1.20".to_owned(),
                },
            ],
            servers: vec![
                DnsServerItem {
                    address: "https://1.1.1.1/dns-query".to_owned(),
                    protocol: "DoH (HTTPS)".to_owned(),
                    latency_ms: Some(28),
                    is_fallback: false,
                    tags: vec![DnsServerTag::Encrypted],
                },
                DnsServerItem {
                    address: "tls://8.8.8.8:853".to_owned(),
                    protocol: "DoT (TLS)".to_owned(),
                    latency_ms: Some(45),
                    is_fallback: false,
                    tags: vec![DnsServerTag::Encrypted],
                },
                DnsServerItem {
                    address: "https://dns.alidns.com/dns-query".to_owned(),
                    protocol: "DoH (Domestic)".to_owned(),
                    latency_ms: Some(18),
                    is_fallback: false,
                    tags: vec![DnsServerTag::Domestic, DnsServerTag::Encrypted],
                },
                DnsServerItem {
                    address: "https://cloudflare-dns.com/dns-query".to_owned(),
                    protocol: "DoH (Fallback)".to_owned(),
                    latency_ms: Some(35),
                    is_fallback: true,
                    tags: vec![DnsServerTag::Fallback, DnsServerTag::Encrypted],
                },
            ],
        }
    }
}

/// The demo's pinned screenshot latencies, mirrored into the shared report
/// type so the fixture page is internally consistent. `DemoSurfaceSource` is
/// the only consumer; no runtime path can construct this.
fn demo_latency_report() -> DnsLatencyReport {
    DnsLatencyReport::measured(
        DEFAULT_PROBE_QUESTION,
        vec![
            DnsServerLatency {
                address: "https://1.1.1.1/dns-query".to_owned(),
                is_fallback: false,
                transport: DnsProbeTransport::Doh,
                outcome: DnsProbeOutcome::Measured { rtt_ms: 28 },
            },
            DnsServerLatency {
                address: "tls://8.8.8.8:853".to_owned(),
                is_fallback: false,
                transport: DnsProbeTransport::Undrivable {
                    reason: "DNS over TLS is not probed by this host".to_owned(),
                },
                outcome: DnsProbeOutcome::NotProbed {
                    reason: "DNS over TLS is not probed by this host".to_owned(),
                },
            },
            DnsServerLatency {
                address: "https://dns.alidns.com/dns-query".to_owned(),
                is_fallback: false,
                transport: DnsProbeTransport::Doh,
                outcome: DnsProbeOutcome::Measured { rtt_ms: 18 },
            },
            DnsServerLatency {
                address: "https://cloudflare-dns.com/dns-query".to_owned(),
                is_fallback: true,
                transport: DnsProbeTransport::Doh,
                outcome: DnsProbeOutcome::Measured { rtt_ms: 35 },
            },
        ],
    )
}

/// DUAL-14-08: the demo's pinned cross-source fixture. Two authorities report
/// different resolver identities so the screenshot exercises the fact list;
/// this is fixture content, not a runtime probe result.
fn demo_leak_report() -> DnsLeakReport {
    DnsLeakReport::observed(
        vec![
            DnsLeakProbeSource::new("223.5.5.5", "echo-a.example.org"),
            DnsLeakProbeSource::new("223.5.5.5", "echo-b.example.org"),
        ],
        vec![
            DnsLeakObservation {
                resolver: "223.5.5.5".to_owned(),
                authority: "echo-a.example.org".to_owned(),
                question: "lf31.echo-a.example.org".to_owned(),
                transport: DnsLeakProbeTransport::Udp,
                outcome: DnsLeakObservationOutcome::Observed {
                    identity: "203.0.113.9".to_owned(),
                },
            },
            DnsLeakObservation {
                resolver: "223.5.5.5".to_owned(),
                authority: "echo-b.example.org".to_owned(),
                question: "lf32.echo-b.example.org".to_owned(),
                transport: DnsLeakProbeTransport::Udp,
                outcome: DnsLeakObservationOutcome::Observed {
                    identity: "198.51.100.7".to_owned(),
                },
            },
        ],
    )
}

/// DUAL-14-09: the demo's pinned STUN UDP-egress observation, explicitly a
/// fixture (`DemoSurfaceSource` is its only consumer; no runtime path can
/// construct it). The mapping is a documentation address and the expected
/// egress matches it, so the screenshot never implies a measured leak.
fn demo_stun_report() -> StunProbeReport {
    StunProbeReport::from_observation(
        StunProbeObservation::observed(
            infiltrator_contract::stun_probe::DEFAULT_STUN_SERVER,
            StunMappedAddress::new("203.0.113.9", 51234),
        ),
        Some(StunMappedAddress::new("203.0.113.9", 0)),
    )
}

fn demo_self_heal_snapshot() -> DnsSelfHealSnapshot {
    DnsSelfHealSnapshot::new(vec![
        DnsSelfHealCheck {
            kind: DnsSelfHealKind::ListenPort,
            state: DnsSelfHealState::Healthy,
            detail: "dns.listen port 1053 is available".to_owned(),
            fix: None,
        },
        DnsSelfHealCheck {
            kind: DnsSelfHealKind::UpstreamResolution,
            state: DnsSelfHealState::Warning,
            detail: "only 3 of 4 upstreams answered".to_owned(),
            fix: Some(DnsSelfHealFix::RecheckUpstreams),
        },
        DnsSelfHealCheck {
            kind: DnsSelfHealKind::Topology,
            state: DnsSelfHealState::Healthy,
            detail: "the configured topology has no finding (0 advisories)".to_owned(),
            fix: None,
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_dns_fixture() {
        let proj = DnsProjection::demo();
        assert_eq!(proj.mode, DnsEnhancedMode::FakeIp);
        assert_eq!(proj.cache_entries, 342);
        assert_eq!(proj.fake_ip_range, "198.18.0.1/16");
        assert_eq!(proj.servers.len(), 4);
        assert_eq!(proj.servers[0].address, "https://1.1.1.1/dns-query");
        assert_eq!(proj.servers[0].protocol, "DoH (HTTPS)");
        assert_eq!(proj.servers[0].latency_ms, Some(28));
        assert!(proj.switches.enable);
        assert_eq!(proj.filter_mode, DnsFakeIpFilterMode::Blacklist);
        assert_eq!(proj.fake_ip_pool.total, 2);
        assert_eq!(proj.latency.results.len(), 4);
        assert_eq!(
            proj.latency.latency_of("https://1.1.1.1/dns-query"),
            Some(28)
        );
        assert_eq!(proj.hosts.len(), 2);
    }

    #[test]
    fn the_demo_fixture_is_never_published_as_a_runtime_probe() {
        // The runtime path always builds its report from a host port; the
        // fixture only exists for screenshots and is documented as such.
        let proj = DnsProjection::demo();
        assert!(proj.latency.is_probed());
        assert_eq!(proj.latency.question, DEFAULT_PROBE_QUESTION);
        assert_eq!(proj.self_heal.checks.len(), DnsSelfHealKind::ALL.len());
        assert_eq!(proj.self_heal.overall_state(), DnsSelfHealState::Warning);
        assert!(proj.self_heal.needs_repair());
        // DUAL-14-08: the leak fixture is a pinned screenshot too; it never
        // claims a runtime fact (only `DemoSurfaceSource` reads it).
        assert!(proj.leak.is_probed());
        assert!(proj.leak.conclusion().is_divergent());
        assert_eq!(proj.leak.observed_facts().len(), 2);
    }
}

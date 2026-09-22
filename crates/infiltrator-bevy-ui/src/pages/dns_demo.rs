//! DUAL-14: the deterministic Bevy DNS demo fixture.
//!
//! Split out of [`crate::pages::dns`] to keep that module inside the source
//! budget. This is the explicit screenshot/demo fixture behind
//! `DemoSurfaceSource`; it is never published as a runtime probe result.

use infiltrator_contract::dns::{
    DnsCacheFlushReport, DnsCoreSwitches, DnsEnhancedMode, DnsFakeIpFilterMode, DnsHostEntry,
    DnsLatencyStatus, DnsServerTag, FakeIpMappingEntry, FakeIpMappingPool, FakeIpMappingSource,
};

use crate::pages::dns::{DnsProjection, DnsServerItem};

impl DnsProjection {
    /// Believable demo fixture for the DNS page.
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
            latency: DnsLatencyStatus::Unsupported,
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
        assert_eq!(proj.latency, DnsLatencyStatus::Unsupported);
        assert_eq!(proj.hosts.len(), 2);
    }
}

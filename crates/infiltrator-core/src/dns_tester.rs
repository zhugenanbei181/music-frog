use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::{Ipv4Addr, Ipv6Addr};

/// Supported DNS protocols for leak, resolution, and latency testing.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DnsProtocol {
    Udp,
    Tcp,
    DoH,
    DoH3,
    DoT,
    DoQ,
    DnsCrypt,
    FakeIp,
}

impl DnsProtocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Udp => "udp",
            Self::Tcp => "tcp",
            Self::DoH => "doh",
            Self::DoH3 => "doh3",
            Self::DoT => "dot",
            Self::DoQ => "doq",
            Self::DnsCrypt => "dnscrypt",
            Self::FakeIp => "fakeip",
        }
    }

    pub fn default_port(&self) -> u16 {
        match self {
            Self::DoT | Self::DoQ => 853,
            Self::DoH | Self::DoH3 | Self::DnsCrypt => 443,
            Self::Udp | Self::Tcp => 53,
            Self::FakeIp => 0,
        }
    }
}

/// A target endpoint for DNS testing.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DnsTestTarget {
    pub endpoint: String,
    pub protocol: DnsProtocol,
    pub ecs_subnet: Option<String>,
    pub proxy_outbound: Option<String>,
    pub sni: Option<String>,
}

impl DnsTestTarget {
    pub fn simple(endpoint: impl Into<String>, protocol: DnsProtocol) -> Self {
        Self {
            endpoint: endpoint.into(),
            protocol,
            ecs_subnet: None,
            proxy_outbound: None,
            sni: None,
        }
    }
}

/// The result of a DNS test execution.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DnsTestResult {
    pub target: DnsTestTarget,
    pub latency_ms: Option<u64>,
    pub tls_handshake_ms: Option<u64>,
    pub resolved_ips: Vec<String>,
    pub is_hijacked: bool,
    pub is_bogus: bool,
    pub is_fake_ip: bool,
    pub ecs_reflected: Option<String>,
    pub error: Option<String>,
}

/// DNS Leak scenarios for privacy auditing.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnsLeakScenario {
    /// Query routed directly to local ISP DNS bypassing proxy.
    DirectLeak,
    /// Untrusted fallback DNS queried when primary was healthy.
    FallbackLeak,
    /// Destination bypassed Fake-IP pool and requested direct IP.
    FakeIpBypassLeak,
    /// Domestic client subnet exposed via EDNS0 Client Subnet.
    EcsPrivacyLeak,
    /// WebRTC STUN/TURN direct UDP query leaked real host address.
    WebRtcStunLeak,
}

/// Known poisoned / bogus IP addresses returned by GFW/censorship middleboxes.
pub struct BogusIpDetector;

impl BogusIpDetector {
    /// Returns static list of well-known hijacked IP addresses.
    pub fn known_bogus_ips() -> HashSet<&'static str> {
        [
            // GFW standard bogus IPs
            "243.185.187.39",
            "37.61.54.158",
            "46.82.174.68",
            "78.16.49.15",
            "93.46.8.89",
            "159.106.121.75",
            "203.98.7.65",
            "211.139.144.11",
            "216.234.234.30",
            "209.145.54.50",
            "69.63.187.12",
            "12.129.206.14",
            "64.233.160.1",
            "8.7.198.45",
            "74.125.127.102",
            // Zero / Loopback bogus returns
            "0.0.0.0",
            "127.0.0.1",
        ]
        .into_iter()
        .collect()
    }

    /// Checks whether an IP string is a known hijacked IP or in RFC 5735 reserved space.
    pub fn is_bogus_ip(ip_str: &str) -> bool {
        let trimmed = ip_str.trim();
        if Self::known_bogus_ips().contains(trimmed) {
            return true;
        }

        if let Ok(ipv4) = trimmed.parse::<Ipv4Addr>() {
            let num = u32::from(ipv4);
            // 240.0.0.0/4 (Class E reserved space)
            if (num & 0xF0000000) == 0xF0000000 {
                return true;
            }
            // 0.0.0.0/8
            if (num & 0xFF000000) == 0 {
                return true;
            }
        }

        false
    }
}

/// Diagnostic tester for DNS leaks and recursive resolution checking.
pub struct DnsTester;

impl DnsTester {
    /// Evaluates the health of a DNS resolution by checking for hijacked IP addresses.
    /// If `expected_ip_prefix` is provided, resolved IPs must start with this prefix.
    pub fn evaluate_dns_health(
        target: &DnsTestTarget,
        resolved_ips: &[String],
        expected_ip_prefix: Option<&str>,
    ) -> DnsTestResult {
        Self::evaluate_dns_health_comprehensive(target, resolved_ips, expected_ip_prefix, None)
    }

    /// Evaluates health with bogus IP detection and Fake-IP verification.
    pub fn evaluate_dns_health_comprehensive(
        target: &DnsTestTarget,
        resolved_ips: &[String],
        expected_ip_prefix: Option<&str>,
        fake_ip_cidr: Option<&str>,
    ) -> DnsTestResult {
        let mut is_hijacked = false;
        let mut is_bogus = false;
        let mut is_fake_ip = false;

        if let Some(prefix) = expected_ip_prefix
            && !resolved_ips.is_empty()
        {
            is_hijacked = !resolved_ips.iter().all(|ip| ip.starts_with(prefix));
        }

        for ip in resolved_ips {
            if BogusIpDetector::is_bogus_ip(ip) {
                is_bogus = true;
                is_hijacked = true;
            }
            if let Some(cidr) = fake_ip_cidr {
                if Self::check_fake_ip_range(ip, cidr) {
                    is_fake_ip = true;
                }
            }
        }

        DnsTestResult {
            target: target.clone(),
            latency_ms: None,
            tls_handshake_ms: None,
            resolved_ips: resolved_ips.to_vec(),
            is_hijacked,
            is_bogus,
            is_fake_ip,
            ecs_reflected: None,
            error: None,
        }
    }

    /// Checks if a given IPv4 string falls within a specified CIDR range.
    /// Commonly used to detect if an IP is part of a Fake-IP pool (e.g., `198.18.0.0/15`).
    pub fn check_fake_ip_range(ip_str: &str, fake_ip_cidr: &str) -> bool {
        let ip: Ipv4Addr = match ip_str.parse() {
            Ok(v) => v,
            Err(_) => return false,
        };

        let parts: Vec<&str> = fake_ip_cidr.split('/').collect();
        if parts.len() != 2 {
            return false;
        }

        let cidr_ip: Ipv4Addr = match parts[0].parse() {
            Ok(v) => v,
            Err(_) => return false,
        };

        let prefix_len: u32 = match parts[1].parse() {
            Ok(v) => v,
            Err(_) => return false,
        };

        if prefix_len > 32 {
            return false;
        }

        let ip_num = u32::from(ip);
        let cidr_num = u32::from(cidr_ip);

        let mask = if prefix_len == 0 {
            0
        } else {
            !((1u32 << (32 - prefix_len)) - 1)
        };

        (ip_num & mask) == (cidr_num & mask)
    }

    /// Checks if a given IPv6 string falls within a specified IPv6 CIDR range.
    pub fn check_ipv6_fake_ip_range(ip_str: &str, fake_ip_cidr: &str) -> bool {
        let ip: Ipv6Addr = match ip_str.parse() {
            Ok(v) => v,
            Err(_) => return false,
        };

        let parts: Vec<&str> = fake_ip_cidr.split('/').collect();
        if parts.len() != 2 {
            return false;
        }

        let cidr_ip: Ipv6Addr = match parts[0].parse() {
            Ok(v) => v,
            Err(_) => return false,
        };

        let prefix_len: u32 = match parts[1].parse() {
            Ok(v) => v,
            Err(_) => return false,
        };

        if prefix_len > 128 {
            return false;
        }

        let ip_num = u128::from(ip);
        let cidr_num = u128::from(cidr_ip);

        let mask = if prefix_len == 0 {
            0
        } else {
            !((1u128 << (128 - prefix_len)) - 1)
        };

        (ip_num & mask) == (cidr_num & mask)
    }

    /// Detects potential privacy leak for a given scenario.
    pub fn detect_dns_leak(
        scenario: DnsLeakScenario,
        resolved_ips: &[String],
        local_isp_ip_prefix: Option<&str>,
    ) -> bool {
        match scenario {
            DnsLeakScenario::DirectLeak => {
                if let Some(isp_prefix) = local_isp_ip_prefix {
                    resolved_ips.iter().any(|ip| ip.starts_with(isp_prefix))
                } else {
                    false
                }
            }
            DnsLeakScenario::FallbackLeak => {
                // If fallback returns poisoned IP, it leaked
                resolved_ips.iter().any(|ip| BogusIpDetector::is_bogus_ip(ip))
            }
            DnsLeakScenario::FakeIpBypassLeak => {
                // Resolved IP is not in Fake-IP space when Fake-IP mode expected
                resolved_ips.iter().all(|ip| !Self::check_fake_ip_range(ip, "198.18.0.0/15"))
            }
            DnsLeakScenario::EcsPrivacyLeak => {
                // Handled via ecs_reflected validation
                false
            }
            DnsLeakScenario::WebRtcStunLeak => {
                // If STUN test resolves directly to real ISP range
                if let Some(isp_prefix) = local_isp_ip_prefix {
                    resolved_ips.iter().any(|ip| ip.starts_with(isp_prefix))
                } else {
                    false
                }
            }
        }
    }

    /// Parses protocol enum from URI scheme string.
    pub fn parse_protocol_from_uri(uri: &str) -> DnsProtocol {
        let u = uri.trim().to_ascii_lowercase();
        if u.starts_with("https://") || u.starts_with("doh://") {
            if u.contains("h3=true") {
                DnsProtocol::DoH3
            } else {
                DnsProtocol::DoH
            }
        } else if u.starts_with("tls://") || u.starts_with("dot://") {
            DnsProtocol::DoT
        } else if u.starts_with("quic://") || u.starts_with("doq://") {
            DnsProtocol::DoQ
        } else if u.starts_with("sdns://") {
            DnsProtocol::DnsCrypt
        } else if u.starts_with("tcp://") {
            DnsProtocol::Tcp
        } else {
            DnsProtocol::Udp
        }
    }

    /// Ranks successful DNS test results by latency in ascending order.
    pub fn rank_fastest_dns(results: &[DnsTestResult]) -> Vec<&DnsTestResult> {
        let mut valid_results: Vec<&DnsTestResult> = results
            .iter()
            .filter(|r| r.latency_ms.is_some() && r.error.is_none() && !r.is_bogus)
            .collect();

        valid_results.sort_by_key(|r| r.latency_ms.unwrap());
        valid_results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fake_ip_range() {
        // Matches exact prefix
        assert!(DnsTester::check_fake_ip_range(
            "198.18.0.5",
            "198.18.0.0/16"
        ));
        assert!(DnsTester::check_fake_ip_range(
            "198.18.255.255",
            "198.18.0.0/16"
        ));

        // Out of range
        assert!(!DnsTester::check_fake_ip_range(
            "198.19.0.1",
            "198.18.0.0/16"
        ));

        // Matches wider prefix
        assert!(DnsTester::check_fake_ip_range(
            "198.19.0.1",
            "198.18.0.0/15"
        ));
        assert!(!DnsTester::check_fake_ip_range(
            "198.20.0.1",
            "198.18.0.0/15"
        ));

        // Invalid inputs
        assert!(!DnsTester::check_fake_ip_range("invalid", "198.18.0.0/16"));
        assert!(!DnsTester::check_fake_ip_range("198.18.0.1", "invalid/16"));
        assert!(!DnsTester::check_fake_ip_range(
            "198.18.0.1",
            "198.18.0.0/33"
        ));
    }

    #[test]
    fn test_ipv6_fake_ip_range() {
        assert!(DnsTester::check_ipv6_fake_ip_range(
            "fc00::1",
            "fc00::/18"
        ));
        assert!(DnsTester::check_ipv6_fake_ip_range(
            "fc00:3fff:ffff:ffff:ffff:ffff:ffff:ffff",
            "fc00::/18"
        ));
        assert!(!DnsTester::check_ipv6_fake_ip_range(
            "fc00:4000::1",
            "fc00::/18"
        ));
    }

    #[test]
    fn test_bogus_ip_detection() {
        assert!(BogusIpDetector::is_bogus_ip("243.185.187.39"));
        assert!(BogusIpDetector::is_bogus_ip("37.61.54.158"));
        assert!(BogusIpDetector::is_bogus_ip("0.0.0.0"));
        assert!(BogusIpDetector::is_bogus_ip("240.10.20.30")); // Class E
        assert!(!BogusIpDetector::is_bogus_ip("1.1.1.1"));
        assert!(!BogusIpDetector::is_bogus_ip("142.250.190.46"));
    }

    #[test]
    fn test_evaluate_dns_health() {
        let target = DnsTestTarget::simple("8.8.8.8", DnsProtocol::Udp);

        // Healthy evaluation (no expected prefix)
        let res_no_prefix = DnsTester::evaluate_dns_health(&target, &["1.2.3.4".to_string()], None);
        assert!(!res_no_prefix.is_hijacked);
        assert!(!res_no_prefix.is_bogus);

        // Healthy evaluation (matches prefix)
        let res_healthy =
            DnsTester::evaluate_dns_health(&target, &["104.18.2.1".to_string()], Some("104.18"));
        assert!(!res_healthy.is_hijacked);

        // Hijacked evaluation (doesn't match prefix)
        let res_hijacked =
            DnsTester::evaluate_dns_health(&target, &["192.168.1.1".to_string()], Some("104.18"));
        assert!(res_hijacked.is_hijacked);

        // Bogus evaluation
        let res_bogus = DnsTester::evaluate_dns_health(
            &target,
            &["243.185.187.39".to_string()],
            Some("104.18"),
        );
        assert!(res_bogus.is_bogus);
        assert!(res_bogus.is_hijacked);
    }

    #[test]
    fn test_parse_protocol_from_uri() {
        assert_eq!(
            DnsTester::parse_protocol_from_uri("https://dns.google/dns-query"),
            DnsProtocol::DoH
        );
        assert_eq!(
            DnsTester::parse_protocol_from_uri("https://cloudflare-dns.com/dns-query?h3=true"),
            DnsProtocol::DoH3
        );
        assert_eq!(
            DnsTester::parse_protocol_from_uri("tls://1.1.1.1:853"),
            DnsProtocol::DoT
        );
        assert_eq!(
            DnsTester::parse_protocol_from_uri("quic://dns.adguard.com:853"),
            DnsProtocol::DoQ
        );
        assert_eq!(
            DnsTester::parse_protocol_from_uri("sdns://..."),
            DnsProtocol::DnsCrypt
        );
        assert_eq!(
            DnsTester::parse_protocol_from_uri("tcp://1.1.1.1:53"),
            DnsProtocol::Tcp
        );
        assert_eq!(
            DnsTester::parse_protocol_from_uri("223.5.5.5"),
            DnsProtocol::Udp
        );
    }

    #[test]
    fn test_rank_fastest_dns() {
        let t1 = DnsTestResult {
            target: DnsTestTarget::simple("A", DnsProtocol::Udp),
            latency_ms: Some(100),
            tls_handshake_ms: None,
            resolved_ips: vec![],
            is_hijacked: false,
            is_bogus: false,
            is_fake_ip: false,
            ecs_reflected: None,
            error: None,
        };
        let t2 = DnsTestResult {
            target: DnsTestTarget::simple("B", DnsProtocol::Tcp),
            latency_ms: Some(50),
            tls_handshake_ms: None,
            resolved_ips: vec![],
            is_hijacked: false,
            is_bogus: false,
            is_fake_ip: false,
            ecs_reflected: None,
            error: None,
        };
        let t3 = DnsTestResult {
            target: DnsTestTarget::simple("C", DnsProtocol::DoH),
            latency_ms: Some(20),
            tls_handshake_ms: None,
            resolved_ips: vec![],
            is_hijacked: true,
            is_bogus: true, // Should be excluded because bogus
            is_fake_ip: false,
            ecs_reflected: None,
            error: None,
        };

        let results = vec![t1, t2, t3];
        let ranked = DnsTester::rank_fastest_dns(&results);

        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].target.endpoint, "B");
        assert_eq!(ranked[1].target.endpoint, "A");
    }
}

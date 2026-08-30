use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

/// Supported DNS protocols for leak and resolution testing.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum DnsProtocol {
    Udp,
    Tcp,
    DoH,
    DoT,
    FakeIp,
}

/// A target endpoint for DNS testing.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DnsTestTarget {
    pub endpoint: String,
    pub protocol: DnsProtocol,
}

/// The result of a DNS test execution.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DnsTestResult {
    pub target: DnsTestTarget,
    pub latency_ms: Option<u64>,
    pub resolved_ips: Vec<String>,
    pub is_hijacked: bool,
    pub error: Option<String>,
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
        let mut is_hijacked = false;

        if let Some(prefix) = expected_ip_prefix
            && !resolved_ips.is_empty() {
                is_hijacked = !resolved_ips.iter().all(|ip| ip.starts_with(prefix));
            }

        DnsTestResult {
            target: target.clone(),
            latency_ms: None, // Populated externally after timing
            resolved_ips: resolved_ips.to_vec(),
            is_hijacked,
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

    /// Ranks successful DNS test results by latency in ascending order.
    pub fn rank_fastest_dns(results: &[DnsTestResult]) -> Vec<&DnsTestResult> {
        let mut valid_results: Vec<&DnsTestResult> = results
            .iter()
            .filter(|r| r.latency_ms.is_some() && r.error.is_none())
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
        assert!(DnsTester::check_fake_ip_range("198.18.0.5", "198.18.0.0/16"));
        assert!(DnsTester::check_fake_ip_range("198.18.255.255", "198.18.0.0/16"));
        
        // Out of range
        assert!(!DnsTester::check_fake_ip_range("198.19.0.1", "198.18.0.0/16"));
        
        // Matches wider prefix
        assert!(DnsTester::check_fake_ip_range("198.19.0.1", "198.18.0.0/15"));
        assert!(!DnsTester::check_fake_ip_range("198.20.0.1", "198.18.0.0/15"));

        // Invalid inputs
        assert!(!DnsTester::check_fake_ip_range("invalid", "198.18.0.0/16"));
        assert!(!DnsTester::check_fake_ip_range("198.18.0.1", "invalid/16"));
        assert!(!DnsTester::check_fake_ip_range("198.18.0.1", "198.18.0.0/33"));
    }

    #[test]
    fn test_evaluate_dns_health() {
        let target = DnsTestTarget {
            endpoint: "8.8.8.8".to_string(),
            protocol: DnsProtocol::Udp,
        };

        // Healthy evaluation (no expected prefix)
        let res_no_prefix = DnsTester::evaluate_dns_health(&target, &["1.2.3.4".to_string()], None);
        assert!(!res_no_prefix.is_hijacked);
        
        // Healthy evaluation (matches prefix)
        let res_healthy = DnsTester::evaluate_dns_health(&target, &["104.18.2.1".to_string()], Some("104.18"));
        assert!(!res_healthy.is_hijacked);

        // Hijacked evaluation (doesn't match prefix)
        let res_hijacked = DnsTester::evaluate_dns_health(&target, &["192.168.1.1".to_string()], Some("104.18"));
        assert!(res_hijacked.is_hijacked);
    }

    #[test]
    fn test_rank_fastest_dns() {
        let t1 = DnsTestResult {
            target: DnsTestTarget { endpoint: "A".into(), protocol: DnsProtocol::Udp },
            latency_ms: Some(100),
            resolved_ips: vec![],
            is_hijacked: false,
            error: None,
        };
        let t2 = DnsTestResult {
            target: DnsTestTarget { endpoint: "B".into(), protocol: DnsProtocol::Tcp },
            latency_ms: Some(50),
            resolved_ips: vec![],
            is_hijacked: false,
            error: None,
        };
        let t3 = DnsTestResult {
            target: DnsTestTarget { endpoint: "C".into(), protocol: DnsProtocol::DoH },
            latency_ms: None,
            resolved_ips: vec![],
            is_hijacked: false,
            error: Some("Timeout".into()),
        };

        let results = vec![t1, t2, t3];
        let ranked = DnsTester::rank_fastest_dns(&results);
        
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].target.endpoint, "B");
        assert_eq!(ranked[1].target.endpoint, "A");
    }
}

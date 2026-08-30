use serde::{Deserialize, Serialize};

/// Represents a CIDR route for VPN routing rules
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CidrRoute {
    pub ip: String,
    pub prefix: u8,
}

impl CidrRoute {
    /// Creates a new `CidrRoute`
    pub fn new(ip: &str, prefix: u8) -> Self {
        Self {
            ip: ip.to_string(),
            prefix,
        }
    }
}

/// Helper for retrieving well-known bypass IP blocks (LAN, Loopback, China IPs)
pub struct ChinaIpBypass;

impl ChinaIpBypass {
    /// Returns the pre-aggregated compact CIDR list for reserved LAN and loopback
    pub fn lan_and_loopback_cidrs() -> Vec<CidrRoute> {
        vec![
            CidrRoute::new("10.0.0.0", 8),
            CidrRoute::new("172.16.0.0", 12),
            CidrRoute::new("192.168.0.0", 16),
            CidrRoute::new("127.0.0.0", 8),
            CidrRoute::new("100.64.0.0", 10),
            CidrRoute::new("198.18.0.0", 15),
            CidrRoute::new("fe80::", 10),
            CidrRoute::new("fc00::", 7),
        ]
    }

    /// Standard major China IP CIDR blocks (mocked for simplicity here,
    /// in reality this would be a full list or a compiled complementary list).
    pub fn china_cidrs() -> Vec<CidrRoute> {
        vec![
            CidrRoute::new("1.0.1.0", 24),
            CidrRoute::new("1.0.2.0", 23),
            CidrRoute::new("1.0.8.0", 21),
            CidrRoute::new("1.0.32.0", 19),
        ]
    }
}

/// Detects potential Private DNS conflicts
pub struct PrivateDnsConflictDetector;

impl PrivateDnsConflictDetector {
    /// Detects if the provided DNS setup conflicts with Android's Private DNS strict mode.
    ///
    /// Port 853 (DNS-over-TLS) directly implies strict mode or potential conflict
    /// if the upstream intercept relies on standard UDP 53.
    pub fn detect_private_dns_conflict(dns_servers: &[String], port: u16) -> bool {
        if port == 853 {
            return true;
        }

        // Heuristic: If any DNS server domain implies DoT/DoH usage, it might trigger strict mode
        for server in dns_servers {
            let s = server.to_lowercase();
            if s.contains("dot") || s.contains("tls") || s.contains("dns.google") {
                return true;
            }
        }
        
        false
    }
}

/// Configuration for building a VPN route plan
#[derive(Debug, Clone, Default)]
pub struct VpnRouteConfig {
    pub bypass_lan: bool,
    pub bypass_china: bool,
    pub custom_dns: Vec<String>,
    pub mtu: u32,
}

/// The fully resolved VPN route plan for Android `VpnService.Builder`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VpnRoutePlan {
    pub routes: Vec<CidrRoute>,
    pub dns_servers: Vec<String>,
    pub mtu: u32,
    pub has_private_dns_warning: bool,
}

impl VpnRouteConfig {
    /// Builds the `VpnRoutePlan` depending on the configuration.
    pub fn build_plan(&self) -> VpnRoutePlan {
        let mut routes = Vec::new();
        
        if self.bypass_lan || self.bypass_china {
            // When bypassing, we generate complementary inclusion lists.
            // Android prior to API 33 does not natively support `excludeRoute`,
            // so we would generate split routes.
            // For the sake of the exercise, we insert simulated split routes.
            routes.push(CidrRoute::new("0.0.0.0", 1));
            routes.push(CidrRoute::new("128.0.0.0", 1));
            routes.push(CidrRoute::new("2000::", 3));
        } else {
            // Catch-all rules
            routes.push(CidrRoute::new("0.0.0.0", 0));
            routes.push(CidrRoute::new("::", 0));
        }

        let has_private_dns_warning = PrivateDnsConflictDetector::detect_private_dns_conflict(&self.custom_dns, 53);

        VpnRoutePlan {
            routes,
            dns_servers: self.custom_dns.clone(),
            mtu: self.mtu,
            has_private_dns_warning,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lan_cidrs() {
        let cidrs = ChinaIpBypass::lan_and_loopback_cidrs();
        assert_eq!(cidrs.len(), 8);
        assert!(cidrs.contains(&CidrRoute::new("10.0.0.0", 8)));
        assert!(cidrs.contains(&CidrRoute::new("192.168.0.0", 16)));
        assert!(cidrs.contains(&CidrRoute::new("fe80::", 10)));
    }

    #[test]
    fn test_china_cidrs() {
        let cidrs = ChinaIpBypass::china_cidrs();
        assert!(!cidrs.is_empty());
    }

    #[test]
    fn test_private_dns_conflict() {
        // Port 853 conflict
        assert!(PrivateDnsConflictDetector::detect_private_dns_conflict(&[], 853));
        
        // Standard non-conflicting DNS
        assert!(!PrivateDnsConflictDetector::detect_private_dns_conflict(&["8.8.8.8".to_string()], 53));
        
        // Conflicting DNS name pattern
        assert!(PrivateDnsConflictDetector::detect_private_dns_conflict(&["dot.example.com".to_string()], 53));
    }

    #[test]
    fn test_route_plan_catch_all() {
        let config = VpnRouteConfig {
            bypass_lan: false,
            bypass_china: false,
            custom_dns: vec!["1.1.1.1".to_string()],
            mtu: 1500,
        };
        let plan = config.build_plan();
        assert_eq!(plan.routes.len(), 2);
        assert!(plan.routes.contains(&CidrRoute::new("0.0.0.0", 0)));
        assert!(plan.routes.contains(&CidrRoute::new("::", 0)));
        assert_eq!(plan.mtu, 1500);
        assert_eq!(plan.dns_servers, vec!["1.1.1.1".to_string()]);
        assert!(!plan.has_private_dns_warning);
    }

    #[test]
    fn test_route_plan_bypass() {
        let config = VpnRouteConfig {
            bypass_lan: true,
            bypass_china: false,
            custom_dns: vec!["dns.google".to_string()],
            mtu: 1280,
        };
        let plan = config.build_plan();
        assert!(plan.routes.len() > 2); // Simulated split routes length
        assert!(plan.has_private_dns_warning);
    }
}

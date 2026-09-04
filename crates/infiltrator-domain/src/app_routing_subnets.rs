//! Corporate/private subnet classification and bypass rule generation.

use crate::rules::RuleEntry;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use super::{CorporateSubnetDetector, SubnetCategory, matches_cidr, parse_cidr};

impl Default for CorporateSubnetDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl CorporateSubnetDetector {
    pub const DEFAULT_SUBNETS: [&'static str; 6] = [
        "10.0.0.0/8",
        "172.16.0.0/12",
        "192.168.0.0/16",
        "100.64.0.0/10",
        "fd00::/8",
        "fe80::/10",
    ];

    pub fn new() -> Self {
        Self {
            custom_subnets: Vec::new(),
        }
    }

    pub fn with_custom_subnets<I, S>(subnets: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut detector = Self::new();
        for subnet in subnets {
            let _ = detector.add_custom_subnet(subnet.as_ref());
        }
        detector
    }

    pub fn add_custom_subnet(&mut self, cidr: &str) -> anyhow::Result<()> {
        let trimmed = cidr.trim();
        if parse_cidr(trimmed).is_none() {
            anyhow::bail!("Invalid CIDR subnet notation: '{}'", cidr);
        }
        if !self.custom_subnets.iter().any(|s| s == trimmed) {
            self.custom_subnets.push(trimmed.to_string());
        }
        Ok(())
    }

    pub fn custom_subnets(&self) -> &[String] {
        &self.custom_subnets
    }

    pub fn is_private_v4(ip: &Ipv4Addr) -> bool {
        let octets = ip.octets();
        octets[0] == 10
            || (octets[0] == 172 && (16..=31).contains(&octets[1]))
            || (octets[0] == 192 && octets[1] == 168)
    }

    pub fn is_cgnat_v4(ip: &Ipv4Addr) -> bool {
        let octets = ip.octets();
        octets[0] == 100 && (64..=127).contains(&octets[1])
    }

    pub fn is_ula_v6(ip: &Ipv6Addr) -> bool {
        let octets = ip.octets();
        octets[0] == 0xfd
    }

    pub fn is_link_local_v6(ip: &Ipv6Addr) -> bool {
        let octets = ip.octets();
        octets[0] == 0xfe && (octets[1] & 0xc0) == 0x80
    }

    pub fn classify_ip(&self, ip: &IpAddr) -> Option<SubnetCategory> {
        for custom in &self.custom_subnets {
            if matches_cidr(custom, *ip) {
                return Some(SubnetCategory::CustomCorporate);
            }
        }

        match ip {
            IpAddr::V4(v4) => {
                if Self::is_private_v4(v4) {
                    return Some(SubnetCategory::PrivateIpv4);
                }
                if Self::is_cgnat_v4(v4) {
                    return Some(SubnetCategory::Cgnat);
                }
            }
            IpAddr::V6(v6) => {
                if Self::is_ula_v6(v6) {
                    return Some(SubnetCategory::PrivateIpv6);
                }
                if Self::is_link_local_v6(v6) {
                    return Some(SubnetCategory::LinkLocalIpv6);
                }
            }
        }

        None
    }

    pub fn classify_ip_str(&self, ip_str: &str) -> Option<SubnetCategory> {
        let ip = ip_str.trim().parse::<IpAddr>().ok()?;
        self.classify_ip(&ip)
    }

    pub fn is_corporate_or_private(&self, ip: &IpAddr) -> bool {
        self.classify_ip(ip).is_some()
    }

    pub fn is_corporate_or_private_str(&self, ip_str: &str) -> bool {
        self.classify_ip_str(ip_str).is_some()
    }

    pub fn matching_subnet(&self, ip: &IpAddr) -> Option<String> {
        for default_cidr in Self::DEFAULT_SUBNETS {
            if matches_cidr(default_cidr, *ip) {
                return Some(default_cidr.to_string());
            }
        }
        for custom in &self.custom_subnets {
            if matches_cidr(custom, *ip) {
                return Some(custom.clone());
            }
        }
        None
    }

    pub fn generate_direct_bypass_rules(&self) -> Vec<RuleEntry> {
        let mut rules = Vec::with_capacity(Self::DEFAULT_SUBNETS.len() + self.custom_subnets.len());

        for cidr in Self::DEFAULT_SUBNETS {
            let rule_type = if cidr.contains(':') {
                "IP-CIDR6"
            } else {
                "IP-CIDR"
            };
            rules.push(RuleEntry {
                rule: format!("{},{},DIRECT,no-resolve", rule_type, cidr),
                enabled: true,
            });
        }

        for custom in &self.custom_subnets {
            let rule_type = if custom.contains(':') {
                "IP-CIDR6"
            } else {
                "IP-CIDR"
            };
            rules.push(RuleEntry {
                rule: format!("{},{},DIRECT,no-resolve", rule_type, custom),
                enabled: true,
            });
        }

        rules
    }
}

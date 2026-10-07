use super::RuleEntry;
use super::types::{ParsedRule, RuleType};
use crate::sub_rules::format_ast;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};

pub mod context;
pub mod decision_chain;
pub mod evaluation;
pub mod named;
use infiltrator_contract::rule_condition::RuleTraceIssue;
use infiltrator_contract::rule_location::{RuleLocation, RulePathEntry};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrafficContext {
    pub domain: Option<String>,
    pub ip: Option<IpAddr>,
    pub port: Option<u16>,
    pub src_ip: Option<IpAddr>,
    pub src_port: Option<u16>,
    pub in_port: Option<u16>,
    pub in_type: Option<String>,
    pub in_name: Option<String>,
    pub in_user: Option<String>,
    pub process_name: Option<String>,
    pub process_path: Option<String>,
    pub network: Option<String>,
    pub dscp: Option<u8>,
    pub uid: Option<u32>,
    pub package_name: Option<String>,
    pub client_ip: Option<IpAddr>,
}

impl TrafficContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_domain(domain: impl Into<String>) -> Self {
        Self {
            domain: Some(domain.into()),
            ..Default::default()
        }
    }

    pub fn from_ip(ip: IpAddr) -> Self {
        Self {
            ip: Some(ip),
            ..Default::default()
        }
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn with_src_ip(mut self, ip: IpAddr) -> Self {
        self.src_ip = Some(ip);
        self
    }

    pub fn with_src_port(mut self, port: u16) -> Self {
        self.src_port = Some(port);
        self
    }

    pub fn with_in_port(mut self, port: u16) -> Self {
        self.in_port = Some(port);
        self
    }

    pub fn with_in_type(mut self, in_type: impl Into<String>) -> Self {
        self.in_type = Some(in_type.into());
        self
    }

    pub fn with_in_name(mut self, in_name: impl Into<String>) -> Self {
        self.in_name = Some(in_name.into());
        self
    }

    pub fn with_in_user(mut self, in_user: impl Into<String>) -> Self {
        self.in_user = Some(in_user.into());
        self
    }

    pub fn with_process(mut self, process: impl Into<String>) -> Self {
        self.process_name = Some(process.into());
        self
    }

    pub fn with_process_path(mut self, path: impl Into<String>) -> Self {
        self.process_path = Some(path.into());
        self
    }

    pub fn with_network(mut self, network: impl Into<String>) -> Self {
        self.network = Some(network.into());
        self
    }

    pub fn with_dscp(mut self, dscp: u8) -> Self {
        self.dscp = Some(dscp);
        self
    }

    pub fn with_uid(mut self, uid: u32) -> Self {
        self.uid = Some(uid);
        self
    }

    pub fn with_package_name(mut self, pkg: impl Into<String>) -> Self {
        self.package_name = Some(pkg.into());
        self
    }

    pub fn with_client_ip(mut self, ip: IpAddr) -> Self {
        self.client_ip = Some(ip);
        self
    }

    /// Parse a query string which can be a domain, an IP address, or `host:port`.
    pub fn from_query(query: &str) -> Self {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Self::default();
        }

        // SocketAddr: e.g. 1.2.3.4:80 or [::1]:80
        if let Ok(socket_addr) = trimmed.parse::<SocketAddr>() {
            return Self {
                ip: Some(socket_addr.ip()),
                port: Some(socket_addr.port()),
                ..Default::default()
            };
        }

        // IP address: e.g. 1.1.1.1 or ::1
        if let Ok(ip) = trimmed.parse::<IpAddr>() {
            return Self {
                ip: Some(ip),
                ..Default::default()
            };
        }

        // Host:Port format
        if let Some((host, port_str)) = trimmed.rsplit_once(':')
            && let Ok(port) = port_str.parse::<u16>()
        {
            if let Ok(ip) = host.parse::<IpAddr>() {
                return Self {
                    ip: Some(ip),
                    port: Some(port),
                    ..Default::default()
                };
            } else if !host.is_empty() {
                return Self {
                    domain: Some(host.to_string()),
                    port: Some(port),
                    ..Default::default()
                };
            }
        }

        // Plain domain or process
        Self {
            domain: Some(trimmed.to_string()),
            ..Default::default()
        }
    }
}

impl From<&str> for TrafficContext {
    fn from(s: &str) -> Self {
        Self::from_query(s)
    }
}

impl From<String> for TrafficContext {
    fn from(s: String) -> Self {
        Self::from_query(&s)
    }
}

impl From<&String> for TrafficContext {
    fn from(s: &String) -> Self {
        Self::from_query(s.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleTraceMatch {
    pub index: usize,
    pub rule: String,
    pub target: String,
    pub location: RuleLocation,
    pub raw: String,
    pub path: Vec<RulePathEntry>,
}

impl From<RuleTraceMatch> for (usize, String, String) {
    fn from(m: RuleTraceMatch) -> Self {
        (m.index, m.rule, m.target)
    }
}

fn parse_cidr(cidr_str: &str) -> Option<(IpAddr, u8)> {
    let trimmed = cidr_str.trim();
    if let Some((ip_str, prefix_str)) = trimmed.split_once('/') {
        let ip = ip_str.trim().parse::<IpAddr>().ok()?;
        let prefix = prefix_str.trim().parse::<u8>().ok()?;
        Some((ip, prefix))
    } else {
        let ip = trimmed.parse::<IpAddr>().ok()?;
        let prefix = match ip {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        };
        Some((ip, prefix))
    }
}

fn matches_cidr(cidr_str: &str, ip: IpAddr) -> bool {
    let Some((net_ip, prefix)) = parse_cidr(cidr_str) else {
        return false;
    };
    match (net_ip, ip) {
        (IpAddr::V4(net), IpAddr::V4(target)) => {
            if prefix > 32 {
                return false;
            }
            if prefix == 0 {
                return true;
            }
            let mask = !0u32 << (32 - prefix);
            (u32::from(net) & mask) == (u32::from(target) & mask)
        }
        (IpAddr::V6(net), IpAddr::V6(target)) => {
            if prefix > 128 {
                return false;
            }
            if prefix == 0 {
                return true;
            }
            let mask = !0u128 << (128 - prefix);
            (u128::from(net) & mask) == (u128::from(target) & mask)
        }
        _ => false,
    }
}

fn matches_port(port_spec: &str, port: u16) -> bool {
    let spec = port_spec.trim();
    for part in spec.split(['/', ',']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((start_s, end_s)) = part.split_once('-').or_else(|| part.split_once(':')) {
            if let (Ok(start), Ok(end)) =
                (start_s.trim().parse::<u16>(), end_s.trim().parse::<u16>())
                && port >= start
                && port <= end
            {
                return true;
            }
        } else if let Ok(p) = part.parse::<u16>()
            && port == p
        {
            return true;
        }
    }
    false
}

fn format_matched_rule_desc(parsed: &ParsedRule) -> String {
    match &parsed.rule_type {
        RuleType::Match => "MATCH".to_string(),
        RuleType::Logical(logical) => format_ast(&logical.payload),
        other => {
            if let Some(payload) = other.payload() {
                format!("{},{}", other.name(), payload)
            } else {
                other.name().to_string()
            }
        }
    }
}

/// Pure rule tracer: evaluates a traffic context against a rule list in order,
/// returning the matched rule match record (index, rule string/pattern, and target).
pub fn trace_rules(
    rules: &[RuleEntry],
    context: &TrafficContext,
) -> Result<Option<RuleTraceMatch>, RuleTraceIssue> {
    named::trace_in_tables(rules, None, context).map_err(|mut issue| {
        issue.location = None;
        issue
    })
}

#[cfg(test)]
#[path = "tracer_tests.rs"]
mod tests;

use std::net::IpAddr;

use serde::{Deserialize, Serialize};

use super::RuleEntry;
use super::types::{ParsedRule, RuleType, parse_rule_str};
use crate::sub_rules::{LogicalRuleAst, format_ast};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrafficContext {
    pub domain: Option<String>,
    pub ip: Option<IpAddr>,
    pub port: Option<u16>,
    pub process_name: Option<String>,
    pub in_type: Option<String>,
    pub network: Option<String>,
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

    pub fn with_process(mut self, process: impl Into<String>) -> Self {
        self.process_name = Some(process.into());
        self
    }

    pub fn with_in_type(mut self, in_type: impl Into<String>) -> Self {
        self.in_type = Some(in_type.into());
        self
    }

    pub fn with_network(mut self, network: impl Into<String>) -> Self {
        self.network = Some(network.into());
        self
    }

    /// Parse a query string which can be a domain, an IP address, or `host:port`.
    pub fn from_query(query: &str) -> Self {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Self::default();
        }

        // SocketAddr: e.g. 1.2.3.4:80 or [::1]:80
        if let Ok(socket_addr) = trimmed.parse::<std::net::SocketAddr>() {
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

fn eval_single_rule_type(rule_type: &RuleType, context: &TrafficContext) -> bool {
    match rule_type {
        RuleType::Domain(domain) => {
            if let Some(ref d) = context.domain {
                d.eq_ignore_ascii_case(domain)
            } else {
                false
            }
        }
        RuleType::DomainSuffix(suffix) => {
            if let Some(ref d) = context.domain {
                let s = suffix.trim_start_matches('.').to_ascii_lowercase();
                let d_lower = d.trim_start_matches('.').to_ascii_lowercase();
                d_lower == s || d_lower.ends_with(&format!(".{s}"))
            } else {
                false
            }
        }
        RuleType::DomainKeyword(kw) => {
            if let Some(ref d) = context.domain {
                d.to_ascii_lowercase().contains(&kw.to_ascii_lowercase())
            } else {
                false
            }
        }
        RuleType::DomainRegex(pattern) => {
            if let Some(ref d) = context.domain {
                regex::Regex::new(pattern)
                    .map(|re| re.is_match(d))
                    .unwrap_or(false)
            } else {
                false
            }
        }
        RuleType::Geosite(code) => {
            if let Some(ref d) = context.domain {
                d.to_ascii_lowercase().contains(&code.to_ascii_lowercase())
            } else {
                false
            }
        }
        RuleType::IpCidr(cidr) | RuleType::IpCidr6(cidr) | RuleType::SrcIpCidr(cidr) => {
            if let Some(ip) = context.ip {
                matches_cidr(cidr, ip)
            } else if let Some(ref d) = context.domain
                && let Ok(ip) = d.parse::<IpAddr>()
            {
                matches_cidr(cidr, ip)
            } else {
                false
            }
        }
        RuleType::IpSuffix(suffix) => {
            if let Some(ip) = context.ip {
                ip.to_string().ends_with(suffix)
            } else {
                false
            }
        }
        RuleType::IpAsn(asn) | RuleType::SrcIpAsn(asn) => {
            if let Some(ref d) = context.domain {
                d.eq_ignore_ascii_case(asn)
            } else {
                false
            }
        }
        RuleType::GeoIp(country) | RuleType::SrcGeoIp(country) => {
            if let Some(ref d) = context.domain {
                d.eq_ignore_ascii_case(country)
            } else {
                false
            }
        }
        RuleType::DstPort(port_spec)
        | RuleType::SrcPort(port_spec)
        | RuleType::InPort(port_spec) => {
            if let Some(port) = context.port {
                matches_port(port_spec, port)
            } else {
                false
            }
        }
        RuleType::InType(in_type) => {
            if let Some(ref t) = context.in_type {
                t.eq_ignore_ascii_case(in_type)
            } else {
                false
            }
        }
        RuleType::InName(name) | RuleType::InUser(name) => {
            if let Some(ref t) = context.in_type {
                t.eq_ignore_ascii_case(name)
            } else {
                false
            }
        }
        RuleType::ProcessPath(path) | RuleType::ProcessName(path) => {
            if let Some(ref p) = context.process_name {
                p.eq_ignore_ascii_case(path)
                    || p.ends_with(path)
                    || p.to_ascii_lowercase()
                        .ends_with(&format!("/{}", path.to_ascii_lowercase()))
                    || p.to_ascii_lowercase()
                        .ends_with(&format!("\\{}", path.to_ascii_lowercase()))
            } else {
                false
            }
        }
        RuleType::ProcessPathRegex(pattern) | RuleType::ProcessNameRegex(pattern) => {
            if let Some(ref p) = context.process_name {
                regex::Regex::new(pattern)
                    .map(|re| re.is_match(p))
                    .unwrap_or(false)
            } else {
                false
            }
        }
        RuleType::Network(net) => {
            if let Some(ref n) = context.network {
                n.eq_ignore_ascii_case(net)
            } else {
                false
            }
        }
        RuleType::Dscp(_) | RuleType::Uid(_) => false,
        RuleType::PackageName(pkg) => {
            if let Some(ref p) = context.process_name {
                p.eq_ignore_ascii_case(pkg)
            } else {
                false
            }
        }
        RuleType::RuleSet(name) => {
            if let Some(ref d) = context.domain {
                d.eq_ignore_ascii_case(name)
            } else {
                false
            }
        }
        RuleType::Match => true,
        RuleType::Logical(logical) => eval_logical_ast(&logical.payload, context),
        RuleType::Unknown(_, payload) => {
            if let Some(ref d) = context.domain {
                d.to_ascii_lowercase()
                    .contains(&payload.to_ascii_lowercase())
            } else if let Some(ref p) = context.process_name {
                p.to_ascii_lowercase()
                    .contains(&payload.to_ascii_lowercase())
            } else {
                false
            }
        }
    }
}

fn eval_sub_rule(leaf_str: &str, context: &TrafficContext) -> bool {
    let trimmed = leaf_str
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')');
    let parts: Vec<&str> = trimmed.split(',').map(str::trim).collect();
    if parts.is_empty() {
        return false;
    }

    let type_name = parts[0].to_ascii_uppercase();
    if type_name == "MATCH" {
        return true;
    }

    let payload = if parts.len() >= 2 {
        parts[1].to_string()
    } else {
        return false;
    };

    let rule_type = match type_name.as_str() {
        "DOMAIN" => RuleType::Domain(payload),
        "DOMAIN-SUFFIX" => RuleType::DomainSuffix(payload),
        "DOMAIN-KEYWORD" => RuleType::DomainKeyword(payload),
        "DOMAIN-REGEX" => RuleType::DomainRegex(payload),
        "GEOSITE" => RuleType::Geosite(payload),
        "IP-CIDR" | "IP-CIDR4" => RuleType::IpCidr(payload),
        "IP-CIDR6" => RuleType::IpCidr6(payload),
        "DST-PORT" => RuleType::DstPort(payload),
        "SRC-PORT" => RuleType::SrcPort(payload),
        "IN-PORT" => RuleType::InPort(payload),
        "IN-TYPE" => RuleType::InType(payload),
        "PROCESS-NAME" => RuleType::ProcessName(payload),
        "PROCESS-PATH" => RuleType::ProcessPath(payload),
        "NETWORK" => RuleType::Network(payload),
        "PACKAGE-NAME" => RuleType::PackageName(payload),
        _ => RuleType::Unknown(type_name, payload),
    };

    eval_single_rule_type(&rule_type, context)
}

fn eval_logical_ast(ast: &LogicalRuleAst, context: &TrafficContext) -> bool {
    ast.evaluate(&|leaf| eval_sub_rule(leaf, context))
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
pub fn trace_rules(rules: &[RuleEntry], context: &TrafficContext) -> Option<RuleTraceMatch> {
    for (index, entry) in rules.iter().enumerate() {
        if !entry.enabled {
            continue;
        }

        let Ok(parsed) = parse_rule_str(&entry.rule) else {
            continue;
        };

        let matched = eval_single_rule_type(&parsed.rule_type, context);
        if matched {
            let rule = format_matched_rule_desc(&parsed);
            return Some(RuleTraceMatch {
                index,
                rule,
                target: parsed.target,
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_domain_rules() {
        let rules = vec![
            RuleEntry {
                rule: "DOMAIN,special.com,DIRECT".into(),
                enabled: true,
            },
            RuleEntry {
                rule: "DOMAIN-SUFFIX,google.com,Proxy-Group".into(),
                enabled: true,
            },
            RuleEntry {
                rule: "DOMAIN-KEYWORD,youtube,Video-Group".into(),
                enabled: true,
            },
            RuleEntry {
                rule: "MATCH,Final-Group".into(),
                enabled: true,
            },
        ];

        let ctx1 = TrafficContext::from_query("www.google.com");
        let res1 = trace_rules(&rules, &ctx1).unwrap();
        assert_eq!(res1.index, 1);
        assert_eq!(res1.rule, "DOMAIN-SUFFIX,google.com");
        assert_eq!(res1.target, "Proxy-Group");

        let ctx2 = TrafficContext::from_query("my-youtube-video.org");
        let res2 = trace_rules(&rules, &ctx2).unwrap();
        assert_eq!(res2.index, 2);
        assert_eq!(res2.rule, "DOMAIN-KEYWORD,youtube");
        assert_eq!(res2.target, "Video-Group");

        let ctx3 = TrafficContext::from_query("unknown-site.net");
        let res3 = trace_rules(&rules, &ctx3).unwrap();
        assert_eq!(res3.index, 3);
        assert_eq!(res3.rule, "MATCH");
        assert_eq!(res3.target, "Final-Group");
    }

    #[test]
    fn test_trace_ip_cidr_and_port() {
        let rules = vec![
            RuleEntry {
                rule: "IP-CIDR,192.168.1.0/24,LAN".into(),
                enabled: true,
            },
            RuleEntry {
                rule: "DST-PORT,80/443,WEB".into(),
                enabled: true,
            },
            RuleEntry {
                rule: "MATCH,DEFAULT".into(),
                enabled: true,
            },
        ];

        let ctx_ip = TrafficContext::from_ip("192.168.1.50".parse().unwrap());
        let res1 = trace_rules(&rules, &ctx_ip).unwrap();
        assert_eq!(res1.index, 0);
        assert_eq!(res1.target, "LAN");

        let ctx_port = TrafficContext::new().with_port(443);
        let res2 = trace_rules(&rules, &ctx_port).unwrap();
        assert_eq!(res2.index, 1);
        assert_eq!(res2.target, "WEB");
    }

    #[test]
    fn test_trace_process_and_logical() {
        let rules = vec![
            RuleEntry {
                rule: "PROCESS-NAME,steam.exe,GAME".into(),
                enabled: true,
            },
            RuleEntry {
                rule: "AND((DOMAIN,example.com),(DST-PORT,443),SECURE)".into(),
                enabled: true,
            },
            RuleEntry {
                rule: "MATCH,DIRECT".into(),
                enabled: true,
            },
        ];

        let ctx_proc = TrafficContext::new().with_process("steam.exe");
        let res1 = trace_rules(&rules, &ctx_proc).unwrap();
        assert_eq!(res1.index, 0);
        assert_eq!(res1.target, "GAME");

        let ctx_and = TrafficContext::from_domain("example.com").with_port(443);
        let res2 = trace_rules(&rules, &ctx_and).unwrap();
        assert_eq!(res2.index, 1);
        assert_eq!(res2.target, "SECURE");

        // Port doesn't match for example.com (port 80)
        let ctx_and_fail = TrafficContext::from_domain("example.com").with_port(80);
        let res3 = trace_rules(&rules, &ctx_and_fail).unwrap();
        assert_eq!(res3.index, 2);
        assert_eq!(res3.target, "DIRECT");
    }

    #[test]
    fn test_disabled_rules_skipped() {
        let rules = vec![
            RuleEntry {
                rule: "DOMAIN,google.com,DISABLED_TARGET".into(),
                enabled: false,
            },
            RuleEntry {
                rule: "DOMAIN,google.com,ACTIVE_TARGET".into(),
                enabled: true,
            },
        ];

        let ctx = TrafficContext::from_domain("google.com");
        let res = trace_rules(&rules, &ctx).unwrap();
        assert_eq!(res.index, 1);
        assert_eq!(res.target, "ACTIVE_TARGET");
    }
}

use std::net::IpAddr;

use serde::{Deserialize, Serialize};

use super::RuleEntry;
use super::types::{ParsedRule, RuleType, parse_rule_str};
use crate::sub_rules::{LogicalRuleAst, format_ast};

pub mod decision_chain;

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

/// Comprehensive evaluation against all 28+ rule types.
pub fn eval_single_rule_type(rule_type: &RuleType, context: &TrafficContext) -> bool {
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
        RuleType::IpCidr(cidr) | RuleType::IpCidr6(cidr) => {
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
        RuleType::SrcIpCidr(cidr) => {
            if let Some(ip) = context.src_ip.or(context.client_ip) {
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
        RuleType::IpAsn(asn) => {
            if let Some(ref d) = context.domain {
                d.eq_ignore_ascii_case(asn)
            } else {
                false
            }
        }
        RuleType::SrcIpAsn(asn) => {
            if let Some(ref d) = context.domain {
                d.eq_ignore_ascii_case(asn)
            } else {
                false
            }
        }
        RuleType::GeoIp(country) => {
            if let Some(ref d) = context.domain {
                d.eq_ignore_ascii_case(country)
            } else {
                false
            }
        }
        RuleType::SrcGeoIp(country) => {
            if let Some(ref d) = context.domain {
                d.eq_ignore_ascii_case(country)
            } else {
                false
            }
        }
        RuleType::DstPort(port_spec) => {
            if let Some(port) = context.port {
                matches_port(port_spec, port)
            } else {
                false
            }
        }
        RuleType::SrcPort(port_spec) => {
            if let Some(port) = context.src_port {
                matches_port(port_spec, port)
            } else {
                false
            }
        }
        RuleType::InPort(port_spec) => {
            if let Some(port) = context.in_port {
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
        RuleType::InName(name) => {
            if let Some(ref n) = context.in_name {
                n.eq_ignore_ascii_case(name)
            } else {
                false
            }
        }
        RuleType::InUser(user) => {
            if let Some(ref u) = context.in_user {
                u.eq_ignore_ascii_case(user)
            } else {
                false
            }
        }
        RuleType::ProcessPath(path) => {
            if let Some(ref p) = context.process_path {
                p.eq_ignore_ascii_case(path)
                    || p.ends_with(path)
                    || p.to_ascii_lowercase()
                        .ends_with(&format!("/{}", path.to_ascii_lowercase()))
                    || p.to_ascii_lowercase()
                        .ends_with(&format!("\\{}", path.to_ascii_lowercase()))
            } else if let Some(ref p) = context.process_name {
                p.eq_ignore_ascii_case(path)
            } else {
                false
            }
        }
        RuleType::ProcessPathRegex(pattern) => {
            let target = context
                .process_path
                .as_ref()
                .or(context.process_name.as_ref());
            if let Some(p) = target {
                regex::Regex::new(pattern)
                    .map(|re| re.is_match(p))
                    .unwrap_or(false)
            } else {
                false
            }
        }
        RuleType::ProcessName(name) => {
            if let Some(ref p) = context.process_name {
                p.eq_ignore_ascii_case(name)
                    || p.ends_with(name)
                    || p.to_ascii_lowercase()
                        .ends_with(&format!("/{}", name.to_ascii_lowercase()))
                    || p.to_ascii_lowercase()
                        .ends_with(&format!("\\{}", name.to_ascii_lowercase()))
            } else {
                false
            }
        }
        RuleType::ProcessNameRegex(pattern) => {
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
        RuleType::Dscp(val_str) => {
            if let Some(dscp) = context.dscp {
                val_str
                    .trim()
                    .parse::<u8>()
                    .map(|v| v == dscp)
                    .unwrap_or(false)
            } else {
                false
            }
        }
        RuleType::Uid(val_str) => {
            if let Some(uid) = context.uid {
                val_str
                    .trim()
                    .parse::<u32>()
                    .map(|v| v == uid)
                    .unwrap_or(false)
            } else {
                false
            }
        }
        RuleType::PackageName(pkg) => {
            if let Some(ref p) = context.package_name {
                p.eq_ignore_ascii_case(pkg)
            } else if let Some(ref p) = context.process_name {
                p.eq_ignore_ascii_case(pkg)
            } else {
                false
            }
        }
        RuleType::RuleSet(name) => {
            if let Some(ref d) = context.domain {
                d.to_ascii_lowercase().contains(&name.to_ascii_lowercase())
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

/// Evaluate an individual leaf inside an AND / OR / NOT / SUB-RULE clause.
/// Supports ANY rule syntax supported in standard rules by parsing through `parse_rule_str`.
fn eval_sub_rule(leaf_str: &str, context: &TrafficContext) -> bool {
    let trimmed = leaf_str
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')');
    if trimmed.is_empty() {
        return false;
    }

    let synth = if trimmed.eq_ignore_ascii_case("MATCH") {
        "MATCH,DIRECT".to_string()
    } else if trimmed.contains(',') {
        format!("{trimmed},_DUMMY_TARGET_")
    } else {
        format!("{trimmed},,_DUMMY_TARGET_")
    };

    if let Ok(parsed) = parse_rule_str(&synth) {
        eval_single_rule_type(&parsed.rule_type, context)
    } else {
        false
    }
}

fn eval_logical_ast(ast: &LogicalRuleAst, context: &TrafficContext) -> bool {
    ast.evaluate(&|leaf| eval_sub_rule(leaf, context))
}

/// Recursively explain a logical rule AST against the current traffic context.
pub fn explain_logical_ast(ast: &LogicalRuleAst, context: &TrafficContext) -> Vec<String> {
    let mut explanations = Vec::new();
    explain_ast_recursive(ast, context, &mut explanations, 0);
    explanations
}

fn explain_ast_recursive(
    ast: &LogicalRuleAst,
    context: &TrafficContext,
    out: &mut Vec<String>,
    depth: usize,
) {
    let indent = "  ".repeat(depth);
    match ast {
        LogicalRuleAst::Leaf(payload) => {
            let matched = eval_sub_rule(&payload.0, context);
            let status = if matched { "[PASS]" } else { "[FAIL]" };
            out.push(format!("{indent}{status} {}", payload.0));
        }
        LogicalRuleAst::And(children) => {
            let all_pass = children
                .iter()
                .all(|c| c.evaluate(&|l| eval_sub_rule(l, context)));
            let status = if all_pass { "[AND PASS]" } else { "[AND FAIL]" };
            out.push(format!("{indent}{status}"));
            for c in children {
                explain_ast_recursive(c, context, out, depth + 1);
            }
        }
        LogicalRuleAst::Or(children) => {
            let any_pass = children
                .iter()
                .any(|c| c.evaluate(&|l| eval_sub_rule(l, context)));
            let status = if any_pass { "[OR PASS]" } else { "[OR FAIL]" };
            out.push(format!("{indent}{status}"));
            for c in children {
                explain_ast_recursive(c, context, out, depth + 1);
            }
        }
        LogicalRuleAst::Not(child) => {
            let inner_pass = child.evaluate(&|l| eval_sub_rule(l, context));
            let status = if !inner_pass {
                "[NOT PASS]"
            } else {
                "[NOT FAIL]"
            };
            out.push(format!(
                "{indent}{status} (子条件取反: 原值为 {inner_pass})"
            ));
            explain_ast_recursive(child, context, out, depth + 1);
        }
        LogicalRuleAst::SubRule(children) => {
            let any_pass = children
                .iter()
                .any(|c| c.evaluate(&|l| eval_sub_rule(l, context)));
            let status = if any_pass {
                "[SUB-RULE PASS]"
            } else {
                "[SUB-RULE FAIL]"
            };
            out.push(format!("{indent}{status}"));
            for c in children {
                explain_ast_recursive(c, context, out, depth + 1);
            }
        }
    }
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
#[path = "tracer_tests.rs"]
mod tests;

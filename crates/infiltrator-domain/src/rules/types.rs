use crate::sub_rules;
use crate::sub_rules::LogicalRule;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleType {
    Domain(String),
    DomainSuffix(String),
    DomainKeyword(String),
    DomainRegex(String),
    Geosite(String),
    IpCidr(String),
    IpCidr6(String),
    IpSuffix(String),
    SrcIpSuffix(String),
    IpAsn(String),
    GeoIp(String),
    SrcGeoIp(String),
    SrcIpCidr(String),
    SrcIpAsn(String),
    DstPort(String),
    SrcPort(String),
    InPort(String),
    InType(String),
    InName(String),
    InUser(String),
    ProcessPath(String),
    ProcessPathRegex(String),
    ProcessName(String),
    ProcessNameRegex(String),
    Network(String),
    Dscp(String),
    Uid(String),
    PackageName(String),
    RuleSet(String),
    Match,
    Logical(LogicalRule),
    Unknown(String, String),
}

impl RuleType {
    pub fn name(&self) -> &str {
        match self {
            Self::Domain(_) => "DOMAIN",
            Self::DomainSuffix(_) => "DOMAIN-SUFFIX",
            Self::DomainKeyword(_) => "DOMAIN-KEYWORD",
            Self::DomainRegex(_) => "DOMAIN-REGEX",
            Self::Geosite(_) => "GEOSITE",
            Self::IpCidr(_) => "IP-CIDR",
            Self::IpCidr6(_) => "IP-CIDR6",
            Self::IpSuffix(_) => "IP-SUFFIX",
            Self::SrcIpSuffix(_) => "SRC-IP-SUFFIX",
            Self::IpAsn(_) => "IP-ASN",
            Self::GeoIp(_) => "GEOIP",
            Self::SrcGeoIp(_) => "SRC-GEOIP",
            Self::SrcIpCidr(_) => "SRC-IP-CIDR",
            Self::SrcIpAsn(_) => "SRC-IP-ASN",
            Self::DstPort(_) => "DST-PORT",
            Self::SrcPort(_) => "SRC-PORT",
            Self::InPort(_) => "IN-PORT",
            Self::InType(_) => "IN-TYPE",
            Self::InName(_) => "IN-NAME",
            Self::InUser(_) => "IN-USER",
            Self::ProcessPath(_) => "PROCESS-PATH",
            Self::ProcessPathRegex(_) => "PROCESS-PATH-REGEX",
            Self::ProcessName(_) => "PROCESS-NAME",
            Self::ProcessNameRegex(_) => "PROCESS-NAME-REGEX",
            Self::Network(_) => "NETWORK",
            Self::Dscp(_) => "DSCP",
            Self::Uid(_) => "UID",
            Self::PackageName(_) => "PACKAGE-NAME",
            Self::RuleSet(_) => "RULE-SET",
            Self::Match => "MATCH",
            Self::Logical(l) => match &l.payload {
                sub_rules::LogicalRuleAst::And(_) => "AND",
                sub_rules::LogicalRuleAst::Or(_) => "OR",
                sub_rules::LogicalRuleAst::Not(_) => "NOT",
                sub_rules::LogicalRuleAst::SubRule(_) => "SUB-RULE",
                sub_rules::LogicalRuleAst::Leaf(_) => "LOGICAL",
            },
            Self::Unknown(name, _) => name.as_str(),
        }
    }

    pub fn payload(&self) -> Option<&str> {
        match self {
            Self::Domain(p)
            | Self::DomainSuffix(p)
            | Self::DomainKeyword(p)
            | Self::DomainRegex(p)
            | Self::Geosite(p)
            | Self::IpCidr(p)
            | Self::IpCidr6(p)
            | Self::IpSuffix(p)
            | Self::SrcIpSuffix(p)
            | Self::IpAsn(p)
            | Self::GeoIp(p)
            | Self::SrcGeoIp(p)
            | Self::SrcIpCidr(p)
            | Self::SrcIpAsn(p)
            | Self::DstPort(p)
            | Self::SrcPort(p)
            | Self::InPort(p)
            | Self::InType(p)
            | Self::InName(p)
            | Self::InUser(p)
            | Self::ProcessPath(p)
            | Self::ProcessPathRegex(p)
            | Self::ProcessName(p)
            | Self::ProcessNameRegex(p)
            | Self::Network(p)
            | Self::Dscp(p)
            | Self::Uid(p)
            | Self::PackageName(p)
            | Self::RuleSet(p)
            | Self::Unknown(_, p) => Some(p.as_str()),
            Self::Match | Self::Logical(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedRule {
    pub rule_type: RuleType,
    pub target: String,
    pub no_resolve: bool,
    #[serde(default)]
    pub source_ip: bool,
}

pub fn parse_rule_str(source: &str) -> Result<ParsedRule> {
    let trimmed = source.trim();
    let kind = trimmed.split(',').next().unwrap_or("").to_ascii_uppercase();
    if ["AND", "OR", "NOT", "SUB-RULE"].contains(&kind.as_str()) {
        let logical = sub_rules::parse_logical_rule(&format!("{kind}{}", &trimmed[kind.len()..]))?;
        return Ok(ParsedRule {
            target: logical.target.clone(),
            no_resolve: false,
            source_ip: false,
            rule_type: RuleType::Logical(logical),
        });
    }
    parse_atomic_rule(trimmed, true)
}

pub fn parse_rule_condition(source: &str) -> Result<ParsedRule> {
    parse_atomic_rule(source.trim(), false)
}

fn parse_atomic_rule(source: &str, needs_target: bool) -> Result<ParsedRule> {
    let parts: Vec<&str> = source.split(',').map(str::trim).collect();
    let type_str = parts[0].to_ascii_uppercase();
    if type_str == "MATCH" {
        if !needs_target || parts.len() < 2 || parts[1].is_empty() {
            return Err(anyhow!("MATCH requires an outbound target"));
        }
        return Ok(ParsedRule {
            rule_type: RuleType::Match,
            target: parts[1].into(),
            no_resolve: false,
            source_ip: false,
        });
    }
    let minimum = if needs_target { 3 } else { 2 };
    if parts.len() < minimum || parts[0].is_empty() || parts[1].is_empty() {
        return Err(anyhow!(
            "Rule has a missing type, payload or target: {source}"
        ));
    }
    let regex =
        ["DOMAIN-REGEX", "PROCESS-NAME-REGEX", "PROCESS-PATH-REGEX"].contains(&type_str.as_str());
    let (target, payload, params) = if regex {
        let end = parts.len() - usize::from(needs_target);
        (
            if needs_target {
                parts[end].to_owned()
            } else {
                String::new()
            },
            parts[1..end].join(","),
            &parts[parts.len()..],
        )
    } else {
        let end = if needs_target { 3 } else { 2 };
        (
            if needs_target {
                parts[2].to_owned()
            } else {
                String::new()
            },
            parts[1].to_owned(),
            &parts[end..],
        )
    };
    if needs_target && target.is_empty() {
        return Err(anyhow!("Rule has an empty outbound target"));
    }
    let parameterized = [
        "IP-CIDR",
        "IP-CIDR6",
        "IP-SUFFIX",
        "IP-ASN",
        "GEOIP",
        "RULE-SET",
    ]
    .contains(&type_str.as_str());
    let source_ip = type_str.starts_with("SRC-")
        && ["SRC-IP-CIDR", "SRC-IP-SUFFIX", "SRC-IP-ASN", "SRC-GEOIP"].contains(&type_str.as_str())
        || parameterized && params.contains(&"src");
    let no_resolve = source_ip || parameterized && params.contains(&"no-resolve");
    let rule_type = match type_str.as_str() {
        "DOMAIN" => RuleType::Domain(payload),
        "DOMAIN-SUFFIX" => RuleType::DomainSuffix(payload),
        "DOMAIN-KEYWORD" => RuleType::DomainKeyword(payload),
        "DOMAIN-REGEX" => RuleType::DomainRegex(payload),
        "GEOSITE" => RuleType::Geosite(payload),
        "IP-CIDR" => RuleType::IpCidr(payload),
        "IP-CIDR6" => RuleType::IpCidr6(payload),
        "IP-SUFFIX" => RuleType::IpSuffix(payload),
        "SRC-IP-SUFFIX" => RuleType::SrcIpSuffix(payload),
        "IP-ASN" => RuleType::IpAsn(payload),
        "GEOIP" => RuleType::GeoIp(payload),
        "SRC-GEOIP" => RuleType::SrcGeoIp(payload),
        "SRC-IP-CIDR" => RuleType::SrcIpCidr(payload),
        "SRC-IP-ASN" => RuleType::SrcIpAsn(payload),
        "DST-PORT" => RuleType::DstPort(payload),
        "SRC-PORT" => RuleType::SrcPort(payload),
        "IN-PORT" => RuleType::InPort(payload),
        "IN-TYPE" => RuleType::InType(payload),
        "IN-NAME" => RuleType::InName(payload),
        "IN-USER" => RuleType::InUser(payload),
        "PROCESS-PATH" => RuleType::ProcessPath(payload),
        "PROCESS-PATH-REGEX" => RuleType::ProcessPathRegex(payload),
        "PROCESS-NAME" => RuleType::ProcessName(payload),
        "PROCESS-NAME-REGEX" => RuleType::ProcessNameRegex(payload),
        "NETWORK" => RuleType::Network(payload),
        "DSCP" => RuleType::Dscp(payload),
        "UID" => RuleType::Uid(payload),
        "PACKAGE-NAME" => RuleType::PackageName(payload),
        "RULE-SET" => RuleType::RuleSet(payload),
        _ => RuleType::Unknown(type_str, payload),
    };

    Ok(ParsedRule {
        rule_type,
        target,
        no_resolve,
        source_ip,
    })
}

#[cfg(test)]
mod tests {
    use super::super::RuleEntry;
    #[cfg(test)]
    use super::super::format_rule_entry;
    use super::*;

    #[test]
    fn test_parse_rule_entry() {
        let entry = RuleEntry {
            rule: "DOMAIN,example.com,DIRECT".to_string(),
            enabled: true,
        };
        assert!(entry.enabled);
        assert_eq!(entry.rule, "DOMAIN,example.com,DIRECT");
    }

    #[test]
    fn test_format_rule_entry() {
        let entry = RuleEntry {
            rule: "DIRECT".to_string(),
            enabled: false,
        };
        assert_eq!(format_rule_entry(&entry), "# DIRECT");
    }

    #[test]
    fn test_parse_standard_rules() {
        let r1 = parse_rule_str("DOMAIN-SUFFIX,google.com,Proxy").unwrap();
        assert_eq!(r1.rule_type, RuleType::DomainSuffix("google.com".into()));
        assert_eq!(r1.target, "Proxy");
        assert!(!r1.no_resolve);

        let r2 = parse_rule_str("IP-CIDR,1.1.1.1/32,DIRECT,no-resolve").unwrap();
        assert_eq!(r2.rule_type, RuleType::IpCidr("1.1.1.1/32".into()));
        assert_eq!(r2.target, "DIRECT");
        assert!(r2.no_resolve);

        let r3 = parse_rule_str("MATCH,FINAL").unwrap();
        assert_eq!(r3.rule_type, RuleType::Match);
        assert_eq!(r3.target, "FINAL");
    }

    #[test]
    fn test_parse_all_rule_types() {
        let types = [
            ("DOMAIN,a.com,T", RuleType::Domain("a.com".into())),
            (
                "DOMAIN-KEYWORD,key,T",
                RuleType::DomainKeyword("key".into()),
            ),
            ("GEOSITE,cn,T", RuleType::Geosite("cn".into())),
            ("GEOIP,CN,T", RuleType::GeoIp("CN".into())),
            ("SRC-GEOIP,US,T", RuleType::SrcGeoIp("US".into())),
            ("DST-PORT,443,T", RuleType::DstPort("443".into())),
            ("SRC-PORT,1234,T", RuleType::SrcPort("1234".into())),
            ("IN-TYPE,INNER,T", RuleType::InType("INNER".into())),
            (
                "PROCESS-NAME,curl.exe,T",
                RuleType::ProcessName("curl.exe".into()),
            ),
            ("NETWORK,tcp,T", RuleType::Network("tcp".into())),
            ("RULE-SET,apple,T", RuleType::RuleSet("apple".into())),
        ];

        for (raw, expected) in types {
            let parsed = parse_rule_str(raw).unwrap();
            assert_eq!(parsed.rule_type, expected);
            assert_eq!(parsed.target, "T");
        }
    }

    #[test]
    fn test_parse_logical_rules() {
        let r =
            parse_rule_str("AND,((DOMAIN,example.com),(IP-CIDR,192.0.2.0/24,no-resolve)),SECURE")
                .unwrap();
        assert_eq!(r.target, "SECURE");
        assert!(!r.no_resolve);
        assert!(matches!(r.rule_type, RuleType::Logical(_)));
    }
}

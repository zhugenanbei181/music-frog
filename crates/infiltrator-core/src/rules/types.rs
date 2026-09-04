use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

use infiltrator_domain::sub_rules::{self, LogicalRule};

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
}

pub fn parse_rule_str(rule_str: &str) -> Result<ParsedRule> {
    let trimmed = rule_str.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("Empty rule string"));
    }

    // Check logical rules first: AND, OR, NOT, SUB-RULE
    if trimmed.starts_with("AND(")
        || trimmed.starts_with("OR(")
        || trimmed.starts_with("NOT(")
        || trimmed.starts_with("SUB-RULE(")
    {
        let logical = sub_rules::parse_logical_rule(trimmed)?;
        return Ok(ParsedRule {
            target: logical.target.clone(),
            no_resolve: logical.no_resolve,
            rule_type: RuleType::Logical(logical),
        });
    }

    let mut parts: Vec<&str> = trimmed.split(',').map(str::trim).collect();
    let mut no_resolve = false;

    if let Some(last) = parts.last()
        && last.eq_ignore_ascii_case("no-resolve")
    {
        no_resolve = true;
        parts.pop();
    }

    if parts.is_empty() {
        return Err(anyhow!("Invalid rule: {}", rule_str));
    }

    let type_str = parts[0].to_ascii_uppercase();

    if type_str == "MATCH" {
        let target = if parts.len() >= 2 {
            parts[1].to_string()
        } else {
            "DIRECT".to_string()
        };
        return Ok(ParsedRule {
            rule_type: RuleType::Match,
            target,
            no_resolve,
        });
    }

    if parts.len() < 3 {
        return Err(anyhow!(
            "Rule requires at least TYPE,PAYLOAD,TARGET: {}",
            rule_str
        ));
    }

    let target = parts.pop().unwrap().to_string();
    let payload = parts[1..].join(",");

    let rule_type = match type_str.as_str() {
        "DOMAIN" => RuleType::Domain(payload),
        "DOMAIN-SUFFIX" => RuleType::DomainSuffix(payload),
        "DOMAIN-KEYWORD" => RuleType::DomainKeyword(payload),
        "DOMAIN-REGEX" => RuleType::DomainRegex(payload),
        "GEOSITE" => RuleType::Geosite(payload),
        "IP-CIDR" | "IP-CIDR4" => RuleType::IpCidr(payload),
        "IP-CIDR6" => RuleType::IpCidr6(payload),
        "IP-SUFFIX" => RuleType::IpSuffix(payload),
        "IP-ASN" => RuleType::IpAsn(payload),
        "GEOIP" => RuleType::GeoIp(payload),
        "SRC-GEOIP" => RuleType::SrcGeoIp(payload),
        "SRC-IP-CIDR" | "SRC-IP-CIDR4" => RuleType::SrcIpCidr(payload),
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
    })
}

#[cfg(test)]
mod tests {
    use super::super::RuleEntry;
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
        assert_eq!(super::super::format_rule_entry(&entry), "# DIRECT");
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
            parse_rule_str("AND((DOMAIN,example.com),(DST-PORT,443),SECURE,no-resolve)").unwrap();
        assert_eq!(r.target, "SECURE");
        assert!(r.no_resolve);
        assert!(matches!(r.rule_type, RuleType::Logical(_)));
    }
}

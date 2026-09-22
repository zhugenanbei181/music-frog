//! Shared rule-type catalogue (DUAL-11-01).
//!
//! Both surfaces label and color rule rows from one typed vocabulary instead of
//! re-deriving a display name and a semantic class per surface. The catalogue
//! covers every concrete [`RuleType`] variant plus the four logical operators,
//! and [`RuleType::spec`] is the exhaustive typed mapping from the parser model
//! into it, so adding a variant without a catalogue entry fails to compile.
//!
//! Lookup normalizes away `-`/`_` and case, so the historical spellings the
//! Iced editor already tolerated (`DOMAINSUFFIX`, `ipcidr6`) keep resolving.

use super::types::RuleType;

/// Semantic family of a rule type. Surfaces map this to their own chip/badge
/// colors; the domain never names a UI color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RuleTypeFamily {
    /// Domain-name matching (including rule sets).
    Host,
    /// Geo database lookups (GEOSITE / GEOIP).
    Geo,
    /// IP literal, CIDR and ASN matching.
    Address,
    /// Process identity matching.
    Process,
    /// Port matching.
    Port,
    /// Transport-level attributes (L4 protocol, DSCP).
    Transport,
    /// Inbound listener attributes.
    Inbound,
    /// Local identity of the requesting app/user.
    Identity,
    /// Logical composition of other rules.
    Composite,
    /// The terminal catch-all rule.
    Terminal,
    /// A spelling the catalogue does not know.
    Unknown,
}

impl RuleTypeFamily {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Geo => "geo",
            Self::Address => "address",
            Self::Process => "process",
            Self::Port => "port",
            Self::Transport => "transport",
            Self::Inbound => "inbound",
            Self::Identity => "identity",
            Self::Composite => "composite",
            Self::Terminal => "terminal",
            Self::Unknown => "unknown",
        }
    }

    /// Whether matching this family resolves a hostname, so the `no-resolve`
    /// flag is meaningful for its rules.
    pub const fn resolves_host(self) -> bool {
        matches!(self, Self::Address | Self::Geo)
    }
}

/// One entry of the rule-type catalogue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleTypeSpec {
    /// Canonical mihomo spelling (`DOMAIN-SUFFIX`).
    pub name: &'static str,
    /// Display label shared by both surfaces (`DomainSuffix`).
    pub label: &'static str,
    pub family: RuleTypeFamily,
    /// Whether the rule type composes other rules (`AND(...)`).
    pub is_logical: bool,
    /// Whether the parsed rule accepts a trailing `no-resolve` flag.
    pub accepts_no_resolve: bool,
}

/// Catalogue entry returned for a spelling the catalogue does not know.
pub const UNKNOWN_RULE_TYPE_SPEC: RuleTypeSpec = RuleTypeSpec {
    name: "",
    label: "Unknown",
    family: RuleTypeFamily::Unknown,
    is_logical: false,
    accepts_no_resolve: false,
};

/// Every rule-type spelling the dual surface renders, in presentation order.
pub const RULE_TYPE_MATRIX: [RuleTypeSpec; 33] = [
    spec("DOMAIN", "Domain", RuleTypeFamily::Host, false, false),
    spec(
        "DOMAIN-SUFFIX",
        "DomainSuffix",
        RuleTypeFamily::Host,
        false,
        false,
    ),
    spec(
        "DOMAIN-KEYWORD",
        "DomainKeyword",
        RuleTypeFamily::Host,
        false,
        false,
    ),
    spec(
        "DOMAIN-REGEX",
        "DomainRegex",
        RuleTypeFamily::Host,
        false,
        false,
    ),
    spec("RULE-SET", "RuleSet", RuleTypeFamily::Host, false, false),
    spec("GEOSITE", "GeoSite", RuleTypeFamily::Geo, false, false),
    spec("GEOIP", "GeoIP", RuleTypeFamily::Geo, false, true),
    spec("SRC-GEOIP", "SrcGeoIP", RuleTypeFamily::Geo, false, true),
    spec("IP-CIDR", "IPCIDR", RuleTypeFamily::Address, false, true),
    spec("IP-CIDR6", "IPCIDR6", RuleTypeFamily::Address, false, true),
    spec(
        "IP-SUFFIX",
        "IPSuffix",
        RuleTypeFamily::Address,
        false,
        true,
    ),
    spec("IP-ASN", "IPASN", RuleTypeFamily::Address, false, true),
    spec(
        "SRC-IP-CIDR",
        "SrcIPCIDR",
        RuleTypeFamily::Address,
        false,
        true,
    ),
    spec(
        "SRC-IP-ASN",
        "SrcIPASN",
        RuleTypeFamily::Address,
        false,
        true,
    ),
    spec("DST-PORT", "DstPort", RuleTypeFamily::Port, false, false),
    spec("SRC-PORT", "SrcPort", RuleTypeFamily::Port, false, false),
    spec("IN-PORT", "InPort", RuleTypeFamily::Inbound, false, false),
    spec(
        "NETWORK",
        "Network",
        RuleTypeFamily::Transport,
        false,
        false,
    ),
    spec("DSCP", "Dscp", RuleTypeFamily::Transport, false, false),
    spec("IN-TYPE", "InType", RuleTypeFamily::Inbound, false, false),
    spec("IN-NAME", "InName", RuleTypeFamily::Inbound, false, false),
    spec("IN-USER", "InUser", RuleTypeFamily::Inbound, false, false),
    spec("UID", "Uid", RuleTypeFamily::Identity, false, false),
    spec(
        "PACKAGE-NAME",
        "PackageName",
        RuleTypeFamily::Identity,
        false,
        false,
    ),
    spec(
        "PROCESS-PATH",
        "ProcessPath",
        RuleTypeFamily::Process,
        false,
        false,
    ),
    spec(
        "PROCESS-PATH-REGEX",
        "ProcessPathRegex",
        RuleTypeFamily::Process,
        false,
        false,
    ),
    spec(
        "PROCESS-NAME",
        "ProcessName",
        RuleTypeFamily::Process,
        false,
        false,
    ),
    spec(
        "PROCESS-NAME-REGEX",
        "ProcessNameRegex",
        RuleTypeFamily::Process,
        false,
        false,
    ),
    spec("AND", "And", RuleTypeFamily::Composite, true, true),
    spec("OR", "Or", RuleTypeFamily::Composite, true, true),
    spec("NOT", "Not", RuleTypeFamily::Composite, true, true),
    spec("SUB-RULE", "SubRule", RuleTypeFamily::Composite, true, true),
    spec("MATCH", "Match", RuleTypeFamily::Terminal, false, false),
];

const fn spec(
    name: &'static str,
    label: &'static str,
    family: RuleTypeFamily,
    is_logical: bool,
    accepts_no_resolve: bool,
) -> RuleTypeSpec {
    RuleTypeSpec {
        name,
        label,
        family,
        is_logical,
        accepts_no_resolve,
    }
}

/// Fold a spelling to its comparison form: upper case without separators.
pub fn normalize_rule_type_name(name: &str) -> String {
    name.trim()
        .chars()
        .filter(|c| *c != '-' && *c != '_')
        .collect::<String>()
        .to_ascii_uppercase()
}

/// Catalogue entry for a rule-type spelling, when known.
pub fn spec_for_name(name: &str) -> Option<&'static RuleTypeSpec> {
    let normalized = normalize_rule_type_name(name);
    if normalized.is_empty() {
        return None;
    }
    RULE_TYPE_MATRIX
        .iter()
        .find(|candidate| normalize_rule_type_name(candidate.name) == normalized)
}

/// Display label for a rule-type spelling. Unknown spellings keep their raw
/// text (the surfaces must not invent a label); an empty spelling reads as
/// `Rule`.
pub fn matrix_label(name: &str) -> String {
    if let Some(spec) = spec_for_name(name) {
        return spec.label.to_owned();
    }
    let trimmed = name.trim();
    if trimmed.is_empty() {
        "Rule".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// Semantic family for a rule-type spelling.
pub fn matrix_family(name: &str) -> RuleTypeFamily {
    spec_for_name(name)
        .map(|spec| spec.family)
        .unwrap_or(RuleTypeFamily::Unknown)
}

/// Whether the spelling can compose sub-rules (`AND`, `OR`, `NOT`, `SUB-RULE`).
pub fn matrix_is_logical(name: &str) -> bool {
    spec_for_name(name).is_some_and(|spec| spec.is_logical)
}

/// Whether a parsed rule of this spelling accepts a trailing `no-resolve`.
pub fn matrix_accepts_no_resolve(name: &str) -> bool {
    spec_for_name(name).is_some_and(|spec| spec.accepts_no_resolve)
}

impl RuleType {
    /// Catalogue entry for this parsed type. Every variant maps to a spec, so
    /// the surfaces never need a per-variant match of their own.
    pub fn spec(&self) -> &'static RuleTypeSpec {
        match self {
            Self::Domain(_) => &RULE_TYPE_MATRIX[0],
            Self::DomainSuffix(_) => &RULE_TYPE_MATRIX[1],
            Self::DomainKeyword(_) => &RULE_TYPE_MATRIX[2],
            Self::DomainRegex(_) => &RULE_TYPE_MATRIX[3],
            Self::RuleSet(_) => &RULE_TYPE_MATRIX[4],
            Self::Geosite(_) => &RULE_TYPE_MATRIX[5],
            Self::GeoIp(_) => &RULE_TYPE_MATRIX[6],
            Self::SrcGeoIp(_) => &RULE_TYPE_MATRIX[7],
            Self::IpCidr(_) => &RULE_TYPE_MATRIX[8],
            Self::IpCidr6(_) => &RULE_TYPE_MATRIX[9],
            Self::IpSuffix(_) => &RULE_TYPE_MATRIX[10],
            Self::IpAsn(_) => &RULE_TYPE_MATRIX[11],
            Self::SrcIpCidr(_) => &RULE_TYPE_MATRIX[12],
            Self::SrcIpAsn(_) => &RULE_TYPE_MATRIX[13],
            Self::DstPort(_) => &RULE_TYPE_MATRIX[14],
            Self::SrcPort(_) => &RULE_TYPE_MATRIX[15],
            Self::InPort(_) => &RULE_TYPE_MATRIX[16],
            Self::Network(_) => &RULE_TYPE_MATRIX[17],
            Self::Dscp(_) => &RULE_TYPE_MATRIX[18],
            Self::InType(_) => &RULE_TYPE_MATRIX[19],
            Self::InName(_) => &RULE_TYPE_MATRIX[20],
            Self::InUser(_) => &RULE_TYPE_MATRIX[21],
            Self::Uid(_) => &RULE_TYPE_MATRIX[22],
            Self::PackageName(_) => &RULE_TYPE_MATRIX[23],
            Self::ProcessPath(_) => &RULE_TYPE_MATRIX[24],
            Self::ProcessPathRegex(_) => &RULE_TYPE_MATRIX[25],
            Self::ProcessName(_) => &RULE_TYPE_MATRIX[26],
            Self::ProcessNameRegex(_) => &RULE_TYPE_MATRIX[27],
            Self::Logical(logical) => match &logical.payload {
                crate::sub_rules::LogicalRuleAst::And(_) => &RULE_TYPE_MATRIX[28],
                crate::sub_rules::LogicalRuleAst::Or(_) => &RULE_TYPE_MATRIX[29],
                crate::sub_rules::LogicalRuleAst::Not(_) => &RULE_TYPE_MATRIX[30],
                crate::sub_rules::LogicalRuleAst::SubRule(_) => &RULE_TYPE_MATRIX[31],
                crate::sub_rules::LogicalRuleAst::Leaf(_) => &RULE_TYPE_MATRIX[32],
            },
            Self::Match => &RULE_TYPE_MATRIX[32],
            Self::Unknown(_, _) => &UNKNOWN_RULE_TYPE_SPEC,
        }
    }

    /// Shared display label for this parsed type (see [`matrix_label`] for the
    /// string-spelling seam the surfaces use).
    pub fn display_label(&self) -> &'static str {
        self.spec().label
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::types::parse_rule_str;

    #[test]
    fn catalogue_names_and_labels_are_unique_and_canonical() {
        assert!(RULE_TYPE_MATRIX.len() >= 30);
        for (index, spec) in RULE_TYPE_MATRIX.iter().enumerate() {
            assert_eq!(
                spec.name,
                spec.name.trim().to_ascii_uppercase(),
                "canonical spelling {index}"
            );
            assert!(
                spec.name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
                "unexpected character in {}",
                spec.name
            );
            assert!(!spec.label.is_empty());
            assert_eq!(
                RULE_TYPE_MATRIX
                    .iter()
                    .filter(|candidate| candidate.name == spec.name)
                    .count(),
                1,
                "duplicate name {}",
                spec.name
            );
            assert_eq!(
                RULE_TYPE_MATRIX
                    .iter()
                    .filter(|candidate| candidate.label == spec.label)
                    .count(),
                1,
                "duplicate label {}",
                spec.label
            );
        }
    }

    #[test]
    fn every_catalogue_entry_parses_to_its_own_spelling() {
        for spec in RULE_TYPE_MATRIX.iter() {
            let parsed = if spec.is_logical {
                parse_rule_str(&format!("{}((DOMAIN,a.com),TARGET)", spec.name))
            } else if spec.name == "MATCH" {
                parse_rule_str("MATCH,TARGET")
            } else {
                parse_rule_str(&format!("{},payload,TARGET", spec.name))
            }
            .unwrap_or_else(|error| panic!("{} must parse: {error}", spec.name));
            assert_eq!(parsed.rule_type.name(), spec.name);
            assert_eq!(parsed.rule_type.spec(), spec);
            assert_eq!(parsed.rule_type.display_label(), spec.label);
        }
    }

    #[test]
    fn spelling_lookup_normalizes_separators_and_case() {
        assert_eq!(matrix_label("DOMAIN-SUFFIX"), "DomainSuffix");
        assert_eq!(matrix_label("domainsuffix"), "DomainSuffix");
        assert_eq!(matrix_label(" ip_cidr6 "), "IPCIDR6");
        assert_eq!(matrix_label("WEIRD-TYPE"), "WEIRD-TYPE");
        assert_eq!(matrix_label("  "), "Rule");
        assert_eq!(matrix_family("PROCESS-NAME"), RuleTypeFamily::Process);
        assert_eq!(matrix_family("WEIRD-TYPE"), RuleTypeFamily::Unknown);
        assert!(matrix_is_logical("and"));
        assert!(!matrix_is_logical("DOMAIN"));
        assert!(matrix_accepts_no_resolve("GEOIP"));
        assert!(!matrix_accepts_no_resolve("DOMAIN"));
    }

    #[test]
    fn unknown_variant_keeps_the_unknown_spec() {
        let unknown = RuleType::Unknown("FUTURE-TYPE".to_owned(), "payload".to_owned());
        assert_eq!(unknown.spec(), &UNKNOWN_RULE_TYPE_SPEC);
        assert_eq!(unknown.name(), "FUTURE-TYPE");
        assert_eq!(unknown.display_label(), "Unknown");
    }

    #[test]
    fn typed_variants_map_to_their_catalogue_spelling() {
        let payload = "payload".to_owned();
        let variants = [
            RuleType::Domain(payload.clone()),
            RuleType::DomainSuffix(payload.clone()),
            RuleType::DomainKeyword(payload.clone()),
            RuleType::DomainRegex(payload.clone()),
            RuleType::RuleSet(payload.clone()),
            RuleType::Geosite(payload.clone()),
            RuleType::GeoIp(payload.clone()),
            RuleType::SrcGeoIp(payload.clone()),
            RuleType::IpCidr(payload.clone()),
            RuleType::IpCidr6(payload.clone()),
            RuleType::IpSuffix(payload.clone()),
            RuleType::IpAsn(payload.clone()),
            RuleType::SrcIpCidr(payload.clone()),
            RuleType::SrcIpAsn(payload.clone()),
            RuleType::DstPort(payload.clone()),
            RuleType::SrcPort(payload.clone()),
            RuleType::InPort(payload.clone()),
            RuleType::Network(payload.clone()),
            RuleType::Dscp(payload.clone()),
            RuleType::InType(payload.clone()),
            RuleType::InName(payload.clone()),
            RuleType::InUser(payload.clone()),
            RuleType::Uid(payload.clone()),
            RuleType::PackageName(payload.clone()),
            RuleType::ProcessPath(payload.clone()),
            RuleType::ProcessPathRegex(payload.clone()),
            RuleType::ProcessName(payload.clone()),
            RuleType::ProcessNameRegex(payload.clone()),
            RuleType::Match,
        ];
        for variant in &variants {
            assert_eq!(variant.spec().name, variant.name(), "spelling drift");
            assert_eq!(matrix_label(variant.name()), variant.spec().label);
        }
        let logical = [
            ("AND((DOMAIN,a.com),T)", "AND"),
            ("OR((DOMAIN,a.com),T)", "OR"),
            ("NOT((DOMAIN,a.com),T)", "NOT"),
            ("SUB-RULE((DOMAIN,a.com),T)", "SUB-RULE"),
        ];
        for (raw, spelling) in logical {
            let parsed = parse_rule_str(raw).unwrap();
            assert_eq!(parsed.rule_type.spec().name, spelling);
            assert_eq!(parsed.rule_type.spec().family, RuleTypeFamily::Composite);
        }
    }
}

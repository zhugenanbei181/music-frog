//! Shared DNS workbench form model.
//!
//! Both surfaces edit the exact same draft: the raw editor strings, the six
//! core switches, the mapping/filter modes, and the fallback resolver policy.
//! Parsing the raw strings into the submitted [`DnsSettingsPatch`] and
//! validating the draft live in this module so Iced and Bevy cannot drift.

use crate::dns::{
    DnsCoreSwitches, DnsEnhancedMode, DnsFakeIpFilterMode, DnsFallbackPolicy, DnsSettingsPatch,
    DnsSwitchField, DnsUpstreamProtocol, join_server_list, parse_server_list,
};
use crate::surface_snapshot::DnsPageSnapshot;

/// One editable field of the shared DNS workbench form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DnsFormField {
    Enable,
    Ipv6,
    Cache,
    UseHosts,
    UseSystemHosts,
    RespectRules,
    EnhancedMode,
    FilterMode,
    /// Tier-1 bootstrap resolvers (`default-nameserver`, pure IPs only).
    BootstrapNameserver,
    /// Primary upstream resolvers (`nameserver`).
    Nameserver,
    /// Fallback resolvers (`fallback`).
    Fallback,
    /// `fallback-filter.geoip` trigger switch.
    FallbackGeoip,
    /// `fallback-filter.geoip-code`.
    FallbackGeoipCode,
    /// `fallback-filter.ipcidr` trigger list.
    FallbackTriggerIp,
    /// `dns.fake-ip-range`.
    FakeIpRange,
    /// `dns.fake-ip-filter`.
    FakeIpFilter,
    /// `dns.proxy-server-nameserver`.
    ProxyServerNameserver,
    /// `dns.direct-nameserver`.
    DirectNameserver,
}

impl DnsFormField {
    /// Every field the shared workbench form renders on both surfaces.
    pub const ALL: [Self; 18] = [
        Self::Enable,
        Self::Ipv6,
        Self::Cache,
        Self::UseHosts,
        Self::UseSystemHosts,
        Self::RespectRules,
        Self::EnhancedMode,
        Self::FilterMode,
        Self::BootstrapNameserver,
        Self::Nameserver,
        Self::Fallback,
        Self::FallbackGeoip,
        Self::FallbackGeoipCode,
        Self::FallbackTriggerIp,
        Self::FakeIpRange,
        Self::FakeIpFilter,
        Self::ProxyServerNameserver,
        Self::DirectNameserver,
    ];

    /// Stable field key for tests, guards and accesskit labels.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Enable => "enable",
            Self::Ipv6 => "ipv6",
            Self::Cache => "cache",
            Self::UseHosts => "use_hosts",
            Self::UseSystemHosts => "use_system_hosts",
            Self::RespectRules => "respect_rules",
            Self::EnhancedMode => "enhanced_mode",
            Self::FilterMode => "fake_ip_filter_mode",
            Self::BootstrapNameserver => "default_nameserver",
            Self::Nameserver => "nameserver",
            Self::Fallback => "fallback",
            Self::FallbackGeoip => "fallback_filter_geoip",
            Self::FallbackGeoipCode => "fallback_filter_geoip_code",
            Self::FallbackTriggerIp => "fallback_filter_ipcidr",
            Self::FakeIpRange => "fake_ip_range",
            Self::FakeIpFilter => "fake_ip_filter",
            Self::ProxyServerNameserver => "proxy_server_nameserver",
            Self::DirectNameserver => "direct_nameserver",
        }
    }

    /// The six switches the segmented switch form renders.
    pub const SWITCH_FIELDS: [DnsSwitchField; 6] = [
        DnsSwitchField::Enable,
        DnsSwitchField::Ipv6,
        DnsSwitchField::Cache,
        DnsSwitchField::UseHosts,
        DnsSwitchField::UseSystemHosts,
        DnsSwitchField::RespectRules,
    ];

    /// The workbench field backing a core switch.
    pub const fn from_switch(field: DnsSwitchField) -> Self {
        match field {
            DnsSwitchField::Enable => Self::Enable,
            DnsSwitchField::Ipv6 => Self::Ipv6,
            DnsSwitchField::Cache => Self::Cache,
            DnsSwitchField::UseHosts => Self::UseHosts,
            DnsSwitchField::UseSystemHosts => Self::UseSystemHosts,
            DnsSwitchField::RespectRules => Self::RespectRules,
        }
    }
}

/// A locally detected form issue. Surfaces localize these; the shared module
/// decides *what* is wrong so both surfaces agree on the error set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DnsFormIssue {
    /// A list entry uses a scheme the workbench cannot send to the host.
    UnsupportedScheme { field: DnsFormField, entry: String },
    /// A bootstrap resolver must be a pure IP address.
    BootstrapNotIp { entry: String },
    /// `fallback-filter.ipcidr` entry is not a CIDR network.
    InvalidTriggerCidr { entry: String },
    /// `geoip-code` is not a two-letter region code.
    InvalidGeoipCode { value: String },
}

impl DnsFormIssue {
    /// The field the issue belongs to.
    pub const fn field(&self) -> DnsFormField {
        match self {
            Self::UnsupportedScheme { field, .. } => *field,
            Self::BootstrapNotIp { .. } => DnsFormField::BootstrapNameserver,
            Self::InvalidTriggerCidr { .. } => DnsFormField::FallbackTriggerIp,
            Self::InvalidGeoipCode { .. } => DnsFormField::FallbackGeoipCode,
        }
    }
}

/// Raw editable fallback-policy fields.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DnsFallbackPolicyDraft {
    pub geoip: bool,
    pub geoip_code: String,
    /// Raw `ipcidr` trigger list.
    pub trigger_ipcidr: String,
}

impl DnsFallbackPolicyDraft {
    pub fn from_policy(policy: &DnsFallbackPolicy) -> Self {
        Self {
            geoip: policy.geoip,
            geoip_code: policy.geoip_code.clone(),
            trigger_ipcidr: policy.trigger_raw(),
        }
    }

    /// The typed policy submitted with the workbench patch.
    pub fn policy(&self) -> DnsFallbackPolicy {
        DnsFallbackPolicy::from_raw(self.geoip, &self.geoip_code, &self.trigger_ipcidr)
    }
}

/// The complete shared DNS workbench draft.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DnsWorkbenchForm {
    pub switches: DnsCoreSwitches,
    pub enhanced_mode: DnsEnhancedMode,
    pub filter_mode: DnsFakeIpFilterMode,
    /// Raw `default-nameserver` editor string.
    pub bootstrap_nameserver: String,
    /// Raw `nameserver` editor string.
    pub nameserver: String,
    /// Raw `fallback` editor string.
    pub fallback: String,
    pub fallback_policy: DnsFallbackPolicyDraft,
    pub fake_ip_range: String,
    /// Raw `fake-ip-filter` editor string.
    pub fake_ip_filter: String,
    /// Raw `proxy-server-nameserver` editor string.
    pub proxy_server_nameserver: String,
    /// Raw `direct-nameserver` editor string.
    pub direct_nameserver: String,
}

impl DnsWorkbenchForm {
    /// Seed the draft from the shared DNS page read model.
    pub fn from_snapshot(snapshot: &DnsPageSnapshot) -> Self {
        let primary: Vec<String> = snapshot
            .servers
            .iter()
            .filter(|server| !server.is_fallback)
            .map(|server| server.address.clone())
            .collect();
        let fallback: Vec<String> = snapshot
            .servers
            .iter()
            .filter(|server| server.is_fallback)
            .map(|server| server.address.clone())
            .collect();
        Self {
            switches: snapshot.switches,
            enhanced_mode: snapshot.enhanced_mode,
            filter_mode: snapshot.filter_mode,
            bootstrap_nameserver: join_server_list(&snapshot.default_nameserver),
            nameserver: join_server_list(&primary),
            fallback: join_server_list(&fallback),
            fallback_policy: DnsFallbackPolicyDraft::from_policy(&snapshot.fallback_policy),
            fake_ip_range: snapshot.fake_ip_range.clone(),
            fake_ip_filter: join_server_list(&snapshot.fake_ip_filter),
            proxy_server_nameserver: join_server_list(&snapshot.proxy_server_nameserver),
            direct_nameserver: join_server_list(&snapshot.direct_nameserver),
        }
    }

    /// Whether every switch, mode and list is still the seeded default.
    pub fn is_pristine(&self) -> bool {
        *self == Self::default()
    }

    /// Build the shared patch both surfaces submit.
    ///
    /// The patch always carries every form field: the workbench form is the
    /// source of truth, so clearing a list really clears it. An emptied
    /// `fake_ip_range` clears the key (the domain mirror of
    /// `clear_enhanced_mode`).
    pub fn patch(&self) -> DnsSettingsPatch {
        let fake_ip_range = self.fake_ip_range.trim();
        DnsSettingsPatch {
            switches: Some(self.switches),
            enhanced_mode: Some(self.enhanced_mode),
            filter_mode: Some(self.filter_mode),
            nameserver: Some(parse_server_list(&self.nameserver)),
            fallback: Some(parse_server_list(&self.fallback)),
            fallback_policy: Some(self.fallback_policy.policy()),
            fake_ip_range: (!fake_ip_range.is_empty()).then(|| fake_ip_range.to_owned()),
            clear_fake_ip_range: fake_ip_range.is_empty(),
            fake_ip_filter: Some(parse_server_list(&self.fake_ip_filter)),
            proxy_server_nameserver: Some(parse_server_list(&self.proxy_server_nameserver)),
            direct_nameserver: Some(parse_server_list(&self.direct_nameserver)),
            hosts: None,
            clear_hosts: false,
        }
    }

    /// Validate every editable field with the shared rules.
    pub fn validate(&self) -> Vec<DnsFormIssue> {
        let mut issues = Vec::new();
        for (field, raw) in self.list_fields() {
            for entry in parse_server_list(raw) {
                if DnsUpstreamProtocol::from_address(&entry) == DnsUpstreamProtocol::Unknown {
                    issues.push(DnsFormIssue::UnsupportedScheme { field, entry });
                }
            }
        }
        for entry in parse_server_list(&self.bootstrap_nameserver) {
            if !is_ip_literal(&entry) {
                issues.push(DnsFormIssue::BootstrapNotIp { entry });
            }
        }
        for entry in parse_server_list(&self.fallback_policy.trigger_ipcidr) {
            if !is_cidr_literal(&entry) {
                issues.push(DnsFormIssue::InvalidTriggerCidr { entry });
            }
        }
        let code = self.fallback_policy.geoip_code.trim();
        if !code.is_empty() && !is_region_code(code) {
            issues.push(DnsFormIssue::InvalidGeoipCode {
                value: code.to_owned(),
            });
        }
        issues
    }

    /// The workbench fields that hold a resolver list.
    pub const fn list_fields(&self) -> [(DnsFormField, &str); 6] {
        [
            (DnsFormField::Nameserver, self.nameserver.as_str()),
            (DnsFormField::Fallback, self.fallback.as_str()),
            (DnsFormField::FakeIpFilter, self.fake_ip_filter.as_str()),
            (
                DnsFormField::ProxyServerNameserver,
                self.proxy_server_nameserver.as_str(),
            ),
            (
                DnsFormField::DirectNameserver,
                self.direct_nameserver.as_str(),
            ),
            (
                DnsFormField::BootstrapNameserver,
                self.bootstrap_nameserver.as_str(),
            ),
        ]
    }

    /// The raw editor string of a text field, when the field has one.
    pub fn raw(&self, field: DnsFormField) -> Option<&str> {
        match field {
            DnsFormField::BootstrapNameserver => Some(&self.bootstrap_nameserver),
            DnsFormField::Nameserver => Some(&self.nameserver),
            DnsFormField::Fallback => Some(&self.fallback),
            DnsFormField::FallbackGeoipCode => Some(&self.fallback_policy.geoip_code),
            DnsFormField::FallbackTriggerIp => Some(&self.fallback_policy.trigger_ipcidr),
            DnsFormField::FakeIpRange => Some(&self.fake_ip_range),
            DnsFormField::FakeIpFilter => Some(&self.fake_ip_filter),
            DnsFormField::ProxyServerNameserver => Some(&self.proxy_server_nameserver),
            DnsFormField::DirectNameserver => Some(&self.direct_nameserver),
            _ => None,
        }
    }

    /// Write a raw editor string into its field. Returns whether the field is
    /// a text field this draft stores.
    pub fn set_raw(&mut self, field: DnsFormField, value: String) -> bool {
        match field {
            DnsFormField::BootstrapNameserver => self.bootstrap_nameserver = value,
            DnsFormField::Nameserver => self.nameserver = value,
            DnsFormField::Fallback => self.fallback = value,
            DnsFormField::FallbackGeoipCode => self.fallback_policy.geoip_code = value,
            DnsFormField::FallbackTriggerIp => self.fallback_policy.trigger_ipcidr = value,
            DnsFormField::FakeIpRange => self.fake_ip_range = value,
            DnsFormField::FakeIpFilter => self.fake_ip_filter = value,
            DnsFormField::ProxyServerNameserver => self.proxy_server_nameserver = value,
            DnsFormField::DirectNameserver => self.direct_nameserver = value,
            _ => return false,
        }
        true
    }
}

/// A pure-IP bootstrap entry: no scheme, no path, optional `#tag`/port.
pub fn is_ip_literal(entry: &str) -> bool {
    let trimmed = entry.trim();
    if trimmed.is_empty() {
        return false;
    }
    let (body, _tag) = match trimmed.split_once('#') {
        Some((body, _tag)) => (body.trim(), ()),
        None => (trimmed, ()),
    };
    let host = match body.split_once("://") {
        Some((_scheme, rest)) => rest,
        None => body,
    };
    let host = host.split('/').next().unwrap_or(host).trim();
    if host.starts_with('[') {
        let Some(closing) = host.find(']') else {
            return false;
        };
        return host[1..closing].parse::<std::net::IpAddr>().is_ok();
    }
    if host.parse::<std::net::IpAddr>().is_ok() {
        return true;
    }
    match host.rsplit_once(':') {
        Some((head, port)) if !head.contains(':') => {
            !port.is_empty()
                && port.parse::<u16>().is_ok()
                && head.parse::<std::net::Ipv4Addr>().is_ok()
        }
        // A bare IPv6 literal contains colons and is accepted by the host.
        Some(_) => true,
        None => false,
    }
}

/// An `ip/prefix` network with the prefix inside the address family range.
pub fn is_cidr_literal(entry: &str) -> bool {
    let Some((address, prefix)) = entry.trim().split_once('/') else {
        return false;
    };
    let Ok(prefix) = prefix.trim().parse::<u8>() else {
        return false;
    };
    match address.trim().parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(_)) => prefix <= 32,
        Ok(std::net::IpAddr::V6(_)) => prefix <= 128,
        Err(_) => false,
    }
}

/// ISO 3166-1 alpha-2 shaped region code.
pub fn is_region_code(value: &str) -> bool {
    value.len() == 2 && value.chars().all(|c| c.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface_snapshot::DnsServerSnapshot;

    fn sample_snapshot() -> DnsPageSnapshot {
        DnsPageSnapshot {
            enhanced_mode: DnsEnhancedMode::FakeIp,
            filter_mode: DnsFakeIpFilterMode::Whitelist,
            fake_ip_range: "198.18.0.1/16".to_owned(),
            default_nameserver: vec!["223.5.5.5".to_owned()],
            fake_ip_filter: vec!["*.lan".to_owned()],
            proxy_server_nameserver: vec!["tls://223.5.5.5:853".to_owned()],
            direct_nameserver: vec!["system".to_owned()],
            fallback_policy: DnsFallbackPolicy {
                geoip: true,
                geoip_code: "CN".to_owned(),
                trigger_ipcidr: vec!["192.168.0.0/16".to_owned()],
            },
            switches: DnsCoreSwitches {
                enable: true,
                ..DnsCoreSwitches::default()
            },
            servers: vec![
                DnsServerSnapshot {
                    address: "https://doh.pub/dns-query".to_owned(),
                    protocol: "DoH".to_owned(),
                    latency_ms: None,
                    is_fallback: false,
                    tags: Vec::new(),
                },
                DnsServerSnapshot {
                    address: "8.8.8.8".to_owned(),
                    protocol: "Plain".to_owned(),
                    latency_ms: None,
                    is_fallback: true,
                    tags: Vec::new(),
                },
            ],
            ..DnsPageSnapshot::default()
        }
    }

    #[test]
    fn form_round_trips_the_shared_snapshot() {
        let snapshot = sample_snapshot();
        let form = DnsWorkbenchForm::from_snapshot(&snapshot);
        assert_eq!(form.nameserver, "https://doh.pub/dns-query");
        assert_eq!(form.fallback, "8.8.8.8");
        assert_eq!(form.bootstrap_nameserver, "223.5.5.5");
        assert_eq!(form.fake_ip_filter, "*.lan");
        assert_eq!(form.proxy_server_nameserver, "tls://223.5.5.5:853");
        assert_eq!(form.direct_nameserver, "system");
        assert!(form.fallback_policy.geoip);
        assert_eq!(form.fallback_policy.geoip_code, "CN");
        assert_eq!(form.fallback_policy.trigger_ipcidr, "192.168.0.0/16");
        assert_eq!(form.enhanced_mode, DnsEnhancedMode::FakeIp);
        assert_eq!(form.filter_mode, DnsFakeIpFilterMode::Whitelist);
        assert!(form.validate().is_empty());
    }

    #[test]
    fn patch_carries_every_form_field_and_clears_an_emptied_range() {
        let mut form = DnsWorkbenchForm::from_snapshot(&sample_snapshot());
        form.fake_ip_range.clear();
        form.nameserver = "https://dns.google/dns-query, 1.1.1.1".to_owned();
        let patch = form.patch();
        assert_eq!(
            patch.nameserver.as_deref(),
            Some(
                &[
                    "https://dns.google/dns-query".to_owned(),
                    "1.1.1.1".to_owned()
                ][..]
            )
        );
        assert!(patch.clear_fake_ip_range);
        assert!(patch.fake_ip_range.is_none());
        assert_eq!(
            patch
                .fallback_policy
                .as_ref()
                .map(|policy| policy.geoip_code.clone()),
            Some("CN".to_owned())
        );
    }

    #[test]
    fn validate_reports_unknown_schemes_and_bad_triggers() {
        let mut form = DnsWorkbenchForm {
            nameserver: "ftp://dns.example".to_owned(),
            bootstrap_nameserver: "doh.pub".to_owned(),
            ..DnsWorkbenchForm::default()
        };
        form.fallback_policy.trigger_ipcidr = "192.168.0.0/33".to_owned();
        form.fallback_policy.geoip_code = "CHN".to_owned();
        let issues = form.validate();
        assert!(issues.iter().any(|issue| matches!(
            issue,
            DnsFormIssue::UnsupportedScheme { field, .. } if *field == DnsFormField::Nameserver
        )));
        assert!(issues.iter().any(
            |issue| matches!(issue, DnsFormIssue::BootstrapNotIp { entry } if entry == "doh.pub")
        ));
        assert!(
            issues
                .iter()
                .any(|issue| matches!(issue, DnsFormIssue::InvalidTriggerCidr { .. }))
        );
        assert!(
            issues
                .iter()
                .any(|issue| matches!(issue, DnsFormIssue::InvalidGeoipCode { .. }))
        );
        assert_eq!(issues[0].field(), DnsFormField::Nameserver);
    }

    #[test]
    fn every_shared_field_has_a_distinct_key() {
        let mut keys: Vec<&str> = DnsFormField::ALL.iter().map(|field| field.key()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), DnsFormField::ALL.len());
        assert_eq!(DnsFormField::ALL.len(), 18);
        for field in DnsFormField::SWITCH_FIELDS {
            assert!(DnsFormField::ALL.contains(&DnsFormField::from_switch(field)));
        }
    }

    #[test]
    fn protocol_and_literal_helpers_are_conservative() {
        assert!(is_ip_literal("223.5.5.5"));
        assert!(is_ip_literal("1.1.1.1:53"));
        assert!(is_ip_literal("udp://1.1.1.1:53#DIRECT"));
        assert!(is_ip_literal("[2400:3200::1]:53"));
        assert!(!is_ip_literal("doh.pub"));
        assert!(is_cidr_literal("10.0.0.0/8"));
        assert!(is_cidr_literal("2400:3200::/32"));
        assert!(!is_cidr_literal("10.0.0.0/33"));
        assert!(!is_cidr_literal("10.0.0.0"));
        assert!(is_region_code("CN"));
        assert!(!is_region_code("CHN"));
    }
}

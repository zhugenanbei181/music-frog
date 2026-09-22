//! Shared DNS workbench contract.
//!
//! Both surfaces consume the same typed DNS facts: the six core switches, the
//! domain-mapping (`enhanced-mode`) mode, the Fake-IP filter mode, nameserver
//! semantic tags, and the patch the surfaces submit when the user edits the
//! form. The host supports `fake-ip`/`redir-host` enhanced modes and
//! `blacklist`/`whitelist`/`rule` filter modes.

use serde::{Deserialize, Serialize};

/// Domain mapping mode (`dns.enhanced-mode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DnsEnhancedMode {
    FakeIp,
    RedirHost,
    /// The `enhanced-mode` key is cleared (no mapping mode configured).
    #[default]
    Unmapped,
}

impl DnsEnhancedMode {
    pub const ALL: [Self; 3] = [Self::FakeIp, Self::RedirHost, Self::Unmapped];

    /// Decode the raw profile value; unknown values are honestly `Unmapped`.
    pub fn from_config_value(value: Option<&str>) -> Self {
        match value.map(|raw| raw.trim().to_ascii_lowercase()) {
            Some(raw) if raw == "fake-ip" || raw == "fake_ip" => Self::FakeIp,
            Some(raw) if raw == "redir-host" || raw == "redir_host" => Self::RedirHost,
            _ => Self::Unmapped,
        }
    }

    /// The profile value, or `None` when the key must be cleared.
    pub const fn config_value(self) -> Option<&'static str> {
        match self {
            Self::FakeIp => Some("fake-ip"),
            Self::RedirHost => Some("redir-host"),
            Self::Unmapped => None,
        }
    }

    pub const fn to_index(self) -> usize {
        match self {
            Self::FakeIp => 0,
            Self::RedirHost => 1,
            Self::Unmapped => 2,
        }
    }

    pub const fn from_index(index: usize) -> Self {
        match index {
            0 => Self::FakeIp,
            1 => Self::RedirHost,
            _ => Self::Unmapped,
        }
    }
}

/// Fake-IP filter mode (`dns.fake-ip-filter-mode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DnsFakeIpFilterMode {
    #[default]
    Blacklist,
    Whitelist,
    /// Rule mode: `fake-ip-filter` holds DNS rules rather than domain patterns.
    Rules,
}

impl DnsFakeIpFilterMode {
    pub const ALL: [Self; 3] = [Self::Blacklist, Self::Whitelist, Self::Rules];

    pub fn from_config_value(value: Option<&str>) -> Self {
        match value.map(|raw| raw.trim().to_ascii_lowercase()) {
            Some(raw) if raw == "whitelist" => Self::Whitelist,
            Some(raw) if raw == "rule" || raw == "rules" => Self::Rules,
            _ => Self::Blacklist,
        }
    }

    pub const fn config_value(self) -> &'static str {
        match self {
            Self::Blacklist => "blacklist",
            Self::Whitelist => "whitelist",
            Self::Rules => "rule",
        }
    }

    pub const fn to_index(self) -> usize {
        match self {
            Self::Blacklist => 0,
            Self::Whitelist => 1,
            Self::Rules => 2,
        }
    }

    pub const fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Whitelist,
            2 => Self::Rules,
            _ => Self::Blacklist,
        }
    }
}

/// The six system-level DNS switches exposed by the workbench form.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DnsSwitchField {
    #[default]
    Enable,
    Ipv6,
    Cache,
    UseHosts,
    UseSystemHosts,
    RespectRules,
}

impl DnsSwitchField {
    pub const ALL: [Self; 6] = [
        Self::Enable,
        Self::Ipv6,
        Self::Cache,
        Self::UseHosts,
        Self::UseSystemHosts,
        Self::RespectRules,
    ];

    /// The profile key backing the switch.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Enable => "enable",
            Self::Ipv6 => "ipv6",
            Self::Cache => "cache",
            Self::UseHosts => "use_hosts",
            Self::UseSystemHosts => "use_system_hosts",
            Self::RespectRules => "respect_rules",
        }
    }
}

/// The value of the six DNS switches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsCoreSwitches {
    pub enable: bool,
    pub ipv6: bool,
    pub cache: bool,
    pub use_hosts: bool,
    pub use_system_hosts: bool,
    pub respect_rules: bool,
}

impl DnsCoreSwitches {
    pub fn value(self, field: DnsSwitchField) -> bool {
        match field {
            DnsSwitchField::Enable => self.enable,
            DnsSwitchField::Ipv6 => self.ipv6,
            DnsSwitchField::Cache => self.cache,
            DnsSwitchField::UseHosts => self.use_hosts,
            DnsSwitchField::UseSystemHosts => self.use_system_hosts,
            DnsSwitchField::RespectRules => self.respect_rules,
        }
    }

    pub fn with(mut self, field: DnsSwitchField, enabled: bool) -> Self {
        self.set(field, enabled);
        self
    }

    pub fn set(&mut self, field: DnsSwitchField, enabled: bool) {
        match field {
            DnsSwitchField::Enable => self.enable = enabled,
            DnsSwitchField::Ipv6 => self.ipv6 = enabled,
            DnsSwitchField::Cache => self.cache = enabled,
            DnsSwitchField::UseHosts => self.use_hosts = enabled,
            DnsSwitchField::UseSystemHosts => self.use_system_hosts = enabled,
            DnsSwitchField::RespectRules => self.respect_rules = enabled,
        }
    }
}

/// Semantic tag attached to a configured nameserver.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DnsServerTag {
    /// A known mainland resolver.
    Domestic,
    /// The server belongs to the fallback list.
    Fallback,
    /// DoH / DoT / DoQ / DoH3 / DNSCrypt transport.
    Encrypted,
    /// Plain UDP/TCP transport.
    Plain,
}

impl DnsServerTag {
    /// Derive the tags for one server from its address and list membership.
    pub fn classify(address: &str, is_fallback: bool) -> Vec<Self> {
        let mut tags = Vec::new();
        if is_fallback {
            tags.push(Self::Fallback);
        }
        if is_domestic_resolver(address) {
            tags.push(Self::Domestic);
        }
        if is_encrypted_resolver(address) {
            tags.push(Self::Encrypted);
        } else {
            tags.push(Self::Plain);
        }
        tags
    }
}

fn is_encrypted_resolver(address: &str) -> bool {
    let lower = address.trim().to_ascii_lowercase();
    lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with("h3://")
        || lower.starts_with("tls://")
        || lower.starts_with("dot://")
        || lower.starts_with("doh://")
        || lower.starts_with("quic://")
        || lower.starts_with("doq://")
        || lower.starts_with("sdns://")
}

fn is_domestic_resolver(address: &str) -> bool {
    const DOMESTIC: [&str; 10] = [
        "223.5.5.5",
        "223.6.6.6",
        "119.29.29.29",
        "114.114.114.114",
        "doh.pub",
        "alidns.com",
        "dnspod.cn",
        "360.cn",
        "114dns.com",
        "1.2.4.8",
    ];
    let lower = address.trim().to_ascii_lowercase();
    DOMESTIC.iter().any(|marker| lower.contains(marker))
}

/// A partial DNS settings patch submitted by either surface.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsSettingsPatch {
    pub switches: Option<DnsCoreSwitches>,
    pub enhanced_mode: Option<DnsEnhancedMode>,
    pub filter_mode: Option<DnsFakeIpFilterMode>,
}

impl DnsSettingsPatch {
    /// Build a patch that flips one switch while keeping the others intact.
    pub fn toggle(field: DnsSwitchField, current: DnsCoreSwitches) -> Self {
        let target = !current.value(field);
        Self {
            switches: Some(current.with(field, target)),
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enhanced_mode_round_trips_and_clears() {
        assert_eq!(
            DnsEnhancedMode::from_config_value(Some("fake-ip")),
            DnsEnhancedMode::FakeIp
        );
        assert_eq!(
            DnsEnhancedMode::from_config_value(Some("Redir-Host")),
            DnsEnhancedMode::RedirHost
        );
        assert_eq!(
            DnsEnhancedMode::from_config_value(None),
            DnsEnhancedMode::Unmapped
        );
        assert_eq!(DnsEnhancedMode::Unmapped.config_value(), None);
        for (index, mode) in DnsEnhancedMode::ALL.into_iter().enumerate() {
            assert_eq!(mode.to_index(), index);
            assert_eq!(DnsEnhancedMode::from_index(index), mode);
        }
    }

    #[test]
    fn filter_mode_supports_rule() {
        assert_eq!(
            DnsFakeIpFilterMode::from_config_value(Some("rule")),
            DnsFakeIpFilterMode::Rules
        );
        assert_eq!(DnsFakeIpFilterMode::Rules.config_value(), "rule");
        assert_eq!(
            DnsFakeIpFilterMode::from_config_value(None),
            DnsFakeIpFilterMode::Blacklist
        );
    }

    #[test]
    fn switch_patch_toggles_only_the_target() {
        let current = DnsCoreSwitches {
            enable: true,
            ..DnsCoreSwitches::default()
        };
        let patch = DnsSettingsPatch::toggle(DnsSwitchField::Ipv6, current);
        let updated = patch.switches.expect("switches");
        assert!(updated.enable);
        assert!(updated.ipv6);
        assert!(!updated.respect_rules);
    }

    #[test]
    fn server_tags_classify_fallback_domestic_encrypted() {
        let tags = DnsServerTag::classify("https://223.5.5.5/dns-query", false);
        assert!(tags.contains(&DnsServerTag::Domestic));
        assert!(tags.contains(&DnsServerTag::Encrypted));
        assert!(!tags.contains(&DnsServerTag::Fallback));

        let fallback = DnsServerTag::classify("8.8.8.8", true);
        assert_eq!(fallback, vec![DnsServerTag::Fallback, DnsServerTag::Plain]);
    }
}

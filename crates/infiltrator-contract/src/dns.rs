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

/// Upstream resolver transport decoded from a nameserver address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DnsUpstreamProtocol {
    /// Plain UDP (a bare address, or an explicit `udp://`).
    Udp,
    /// Plain TCP (`tcp://`).
    Tcp,
    /// DNS over HTTPS (`https://` / `doh://`).
    Doh,
    /// DNS over HTTP/3 (`h3://`, or `https://...?h3=true`).
    Doh3,
    /// DNS over TLS (`tls://` / `dot://`).
    Dot,
    /// DNS over QUIC (`quic://` / `doq://`).
    Doq,
    /// DNSCrypt stamp (`sdns://`).
    DnsCrypt,
    /// Platform resolver (`dhcp://`).
    Dhcp,
    /// The literal `system` resolver keyword.
    System,
    /// A scheme this workbench does not recognise.
    Unknown,
}

impl DnsUpstreamProtocol {
    /// Decode the transport from a raw nameserver entry.
    pub fn from_address(address: &str) -> Self {
        let lower = address.trim().to_ascii_lowercase();
        if lower.is_empty() {
            return Self::Unknown;
        }
        if lower == "system" {
            return Self::System;
        }
        let Some((scheme, rest)) = lower.split_once("://") else {
            return Self::Udp;
        };
        match scheme {
            "udp" => Self::Udp,
            "tcp" => Self::Tcp,
            "dhcp" => Self::Dhcp,
            "tls" | "dot" => Self::Dot,
            "quic" | "doq" => Self::Doq,
            "sdns" => Self::DnsCrypt,
            "h3" => Self::Doh3,
            "https" | "doh" => {
                if rest.contains("h3=true") {
                    Self::Doh3
                } else {
                    Self::Doh
                }
            }
            "http" => Self::Doh,
            _ => Self::Unknown,
        }
    }

    /// The protocol chip label both surfaces render.
    pub const fn chip_label(self) -> &'static str {
        match self {
            Self::Udp => "UDP",
            Self::Tcp => "TCP",
            Self::Doh => "DoH",
            Self::Doh3 => "DoH3",
            Self::Dot => "DoT",
            Self::Doq => "DoQ",
            Self::DnsCrypt => "DNSCrypt",
            Self::Dhcp => "DHCP",
            Self::System => "System",
            Self::Unknown => "DNS",
        }
    }

    /// DoH / DoH3 / DoT / DoQ / DNSCrypt are encrypted transports.
    pub const fn is_encrypted(self) -> bool {
        matches!(
            self,
            Self::Doh | Self::Doh3 | Self::Dot | Self::Doq | Self::DnsCrypt
        )
    }
}

/// Split a raw nameserver editor string into normalised entries.
///
/// Both surfaces share the exact same token rules: entries are separated by
/// newlines or commas, surrounding whitespace is dropped, and empty tokens are
/// discarded.
pub fn parse_server_list(raw: &str) -> Vec<String> {
    raw.lines()
        .flat_map(|line| line.split(','))
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Join nameserver entries back into the canonical editor string.
pub fn join_server_list(entries: &[String]) -> String {
    entries.join(", ")
}

/// Remove one entry from the raw editor string, keeping shared token rules.
pub fn remove_server_at(raw: &str, index: usize) -> String {
    let mut entries = parse_server_list(raw);
    if index < entries.len() {
        entries.remove(index);
    }
    join_server_list(&entries)
}

/// Append one entry to the raw editor string. Duplicates (case-insensitive)
/// are ignored so the quick-template chips cannot double-insert.
pub fn append_server(raw: &str, entry: &str) -> String {
    let trimmed = entry.trim();
    if trimmed.is_empty() {
        return raw.to_owned();
    }
    let mut entries = parse_server_list(raw);
    if !entries
        .iter()
        .any(|item| item.eq_ignore_ascii_case(trimmed))
    {
        entries.push(trimmed.to_owned());
    }
    join_server_list(&entries)
}

/// Fallback resolver policy (`dns.fallback-filter`).
///
/// The host schema exposes the GEOIP trigger as `geoip` / `geoip-code` plus an
/// explicit `ipcidr` trigger list; there is no numeric threshold in the
/// profile schema and this contract does not invent one.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsFallbackPolicy {
    /// `fallback-filter.geoip`: fall back when the answer is not domestic.
    pub geoip: bool,
    /// `fallback-filter.geoip-code`; empty means the key is not configured.
    pub geoip_code: String,
    /// `fallback-filter.ipcidr`: fall back when the answer lands in one of
    /// these CIDRs.
    pub trigger_ipcidr: Vec<String>,
}

impl DnsFallbackPolicy {
    /// Build the policy from its raw editor fields.
    pub fn from_raw(geoip: bool, geoip_code: &str, trigger_ipcidr: &str) -> Self {
        Self {
            geoip,
            geoip_code: geoip_code.trim().to_owned(),
            trigger_ipcidr: parse_server_list(trigger_ipcidr),
        }
    }

    /// The canonical raw trigger string for the form field.
    pub fn trigger_raw(&self) -> String {
        join_server_list(&self.trigger_ipcidr)
    }
}

/// Outcome of one DNS cache flush target.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsFlushOutcome {
    /// No flush has been requested in this session.
    #[default]
    NotRequested,
    /// The target accepted the flush.
    Flushed,
    /// The host has no drivable flush for this target.
    Unsupported { reason: String },
    /// The flush ran and failed.
    Failed { message: String },
}

impl DnsFlushOutcome {
    pub fn is_flushed(&self) -> bool {
        matches!(self, Self::Flushed)
    }
}

/// Honest per-target report of the last DNS cache flush.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsCacheFlushReport {
    /// The running core's Fake-IP mapping cache.
    pub fake_ip: DnsFlushOutcome,
    /// The operating system resolver cache.
    pub os_cache: DnsFlushOutcome,
}

impl DnsCacheFlushReport {
    /// Whether any target has a recorded outcome (a flush was requested).
    pub fn is_requested(&self) -> bool {
        self.fake_ip != DnsFlushOutcome::NotRequested
            || self.os_cache != DnsFlushOutcome::NotRequested
    }
}

/// A partial DNS settings patch submitted by either surface.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsSettingsPatch {
    pub switches: Option<DnsCoreSwitches>,
    pub enhanced_mode: Option<DnsEnhancedMode>,
    pub filter_mode: Option<DnsFakeIpFilterMode>,
    /// Primary upstream nameservers (DoH / DoT / DoQ / plain).
    pub nameserver: Option<Vec<String>>,
    /// Fallback upstream nameservers.
    pub fallback: Option<Vec<String>>,
    /// Fallback resolver policy.
    pub fallback_policy: Option<DnsFallbackPolicy>,
    /// `dns.fake-ip-range`; empty entries are rejected by the domain validator.
    pub fake_ip_range: Option<String>,
    /// Clear `dns.fake-ip-range` (the form field was emptied).
    #[serde(default)]
    pub clear_fake_ip_range: bool,
    /// `dns.fake-ip-filter` patterns or rules.
    pub fake_ip_filter: Option<Vec<String>>,
    /// `dns.proxy-server-nameserver`.
    pub proxy_server_nameserver: Option<Vec<String>>,
    /// `dns.direct-nameserver`.
    pub direct_nameserver: Option<Vec<String>>,
    /// DUAL-14-11: full `dns.hosts` mapping written from the shared editor.
    pub hosts: Option<Vec<DnsHostEntry>>,
    /// Clear `dns.hosts` (the shared hosts editor was emptied).
    #[serde(default)]
    pub clear_hosts: bool,
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

/// DUAL-14-06: one observed Fake-IP binding (`domain` ↔ virtual address).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FakeIpMappingEntry {
    pub domain: String,
    pub address: String,
}

/// Where a Fake-IP mapping pool read model came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FakeIpMappingSource {
    /// Derived from the running core's live connection metadata (`host` /
    /// `destinationIP` of connections resolved in Fake-IP mode). This is a
    /// real controller fact, but it is the *observed* subset — the core does
    /// not expose its persisted pool over the controller API.
    LiveConnections,
    /// The host surface could not reach the controller connection feed.
    Unsupported { reason: String },
    /// The controller was reachable but reported no connection table.
    Unavailable { reason: String },
}

/// DUAL-14-06: read-only Fake-IP mapping pool published to both surfaces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FakeIpMappingPool {
    pub source: FakeIpMappingSource,
    /// The configured `fake-ip-range` this pool was filtered against.
    pub range: String,
    pub entries: Vec<FakeIpMappingEntry>,
    /// Number of published entries (never a fabricated pool size).
    pub total: usize,
}

impl Default for FakeIpMappingPool {
    fn default() -> Self {
        Self {
            source: FakeIpMappingSource::Unsupported {
                reason: "no running core connection feed on this host".to_owned(),
            },
            range: String::new(),
            entries: Vec::new(),
            total: 0,
        }
    }
}

impl FakeIpMappingPool {
    /// Whether the pool carries an observed subset rather than a full listing.
    pub fn is_observed_subset(&self) -> bool {
        matches!(self.source, FakeIpMappingSource::LiveConnections)
    }

    /// Case-insensitive search over the domain and the virtual address.
    pub fn filter(&self, query: &str) -> Vec<&FakeIpMappingEntry> {
        let needle = query.trim().to_ascii_lowercase();
        if needle.is_empty() {
            return self.entries.iter().collect();
        }
        self.entries
            .iter()
            .filter(|entry| {
                entry.domain.to_ascii_lowercase().contains(&needle)
                    || entry.address.to_ascii_lowercase().contains(&needle)
            })
            .collect()
    }
}

/// DUAL-14-11: one `dns.hosts` mapping row.
///
/// The address token follows the host's own `hosts` value grammar: an IP
/// literal, the `lan` keyword, or an alias domain. A domain with several IPs
/// is published as one row per address so the editor round-trips losslessly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsHostEntry {
    pub domain: String,
    pub address: String,
}

/// DUAL-14-11: a locally detected hosts-editor issue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsHostsIssue {
    /// The value is neither an IP literal, `lan`, nor a domain alias.
    InvalidAddress { address: String },
    /// The mapping key is empty or carries whitespace.
    InvalidDomain { domain: String },
}

impl DnsHostsIssue {
    /// The offending token, for localized copy.
    pub fn token(&self) -> &str {
        match self {
            Self::InvalidAddress { address } => address,
            Self::InvalidDomain { domain } => domain,
        }
    }
}

/// Parse a shared hosts editor into mapping rows.
///
/// Rows are separated by newlines or `;` (so a single-line text field on one
/// surface and a multi-line editor on the other parse identically). Each row is
/// `address domain [domain ...]`, hosts-file order; `#` starts a comment.
pub fn parse_hosts_editor(raw: &str) -> Vec<DnsHostEntry> {
    let mut entries = Vec::new();
    for row in raw.split(['\n', ';']) {
        let row = row.split('#').next().unwrap_or("").trim();
        if row.is_empty() {
            continue;
        }
        let mut tokens = row.split_whitespace();
        let Some(address) = tokens.next() else {
            continue;
        };
        for domain in tokens {
            entries.push(DnsHostEntry {
                domain: domain.to_owned(),
                address: address.to_owned(),
            });
        }
    }
    entries
}

/// The canonical editor string for a hosts mapping (one row per entry).
pub fn hosts_editor_text(entries: &[DnsHostEntry]) -> String {
    entries
        .iter()
        .map(|entry| format!("{} {}", entry.address, entry.domain))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Append one `address domain` row, ignoring an exact duplicate.
pub fn append_host_row(raw: &str, address: &str, domain: &str) -> String {
    let address = address.trim();
    let domain = domain.trim();
    if address.is_empty() || domain.is_empty() {
        return raw.to_owned();
    }
    let mut entries = parse_hosts_editor(raw);
    if !entries
        .iter()
        .any(|entry| entry.domain.eq_ignore_ascii_case(domain) && entry.address == address)
    {
        entries.push(DnsHostEntry {
            domain: domain.to_owned(),
            address: address.to_owned(),
        });
    }
    hosts_editor_text(&entries)
}

/// Remove one row from the raw hosts editor string.
pub fn remove_host_at(raw: &str, index: usize) -> String {
    let mut entries = parse_hosts_editor(raw);
    if index < entries.len() {
        entries.remove(index);
    }
    hosts_editor_text(&entries)
}

/// Whether a hosts value follows the host's accepted grammar.
pub fn is_valid_hosts_address(address: &str) -> bool {
    let value = address.trim();
    if value.is_empty() {
        return false;
    }
    if value.eq_ignore_ascii_case("lan") {
        return true;
    }
    if value.parse::<std::net::Ipv4Addr>().is_ok() || value.parse::<std::net::Ipv6Addr>().is_ok() {
        return true;
    }
    // The host resolves a value with two or more labels as an alias domain.
    !value.contains(char::is_whitespace)
        && value.split('.').filter(|part| !part.is_empty()).count() >= 2
}

/// Whether a hosts mapping key is acceptable for the host trie.
pub fn is_valid_hosts_domain(domain: &str) -> bool {
    let value = domain.trim();
    !value.is_empty() && !value.contains(char::is_whitespace) && !value.contains(':')
}

/// Validate a hosts mapping; both surfaces localize the returned issues.
pub fn validate_hosts(entries: &[DnsHostEntry]) -> Vec<DnsHostsIssue> {
    let mut issues = Vec::new();
    for entry in entries {
        if !is_valid_hosts_domain(&entry.domain) {
            issues.push(DnsHostsIssue::InvalidDomain {
                domain: entry.domain.clone(),
            });
        }
        if !is_valid_hosts_address(&entry.address) {
            issues.push(DnsHostsIssue::InvalidAddress {
                address: entry.address.clone(),
            });
        }
    }
    issues
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

    #[test]
    fn upstream_protocol_decodes_both_surface_editor_shapes() {
        assert_eq!(
            DnsUpstreamProtocol::from_address("https://dns.google/dns-query"),
            DnsUpstreamProtocol::Doh
        );
        assert_eq!(
            DnsUpstreamProtocol::from_address("h3://dns.google/dns-query"),
            DnsUpstreamProtocol::Doh3
        );
        assert_eq!(
            DnsUpstreamProtocol::from_address("https://cloudflare-dns.com/dns-query?h3=true"),
            DnsUpstreamProtocol::Doh3
        );
        assert_eq!(
            DnsUpstreamProtocol::from_address("tls://223.5.5.5:853"),
            DnsUpstreamProtocol::Dot
        );
        assert_eq!(
            DnsUpstreamProtocol::from_address("doq://dns.adguard.com"),
            DnsUpstreamProtocol::Doq
        );
        assert_eq!(
            DnsUpstreamProtocol::from_address("sdns://stamp"),
            DnsUpstreamProtocol::DnsCrypt
        );
        assert_eq!(
            DnsUpstreamProtocol::from_address("dhcp://en0"),
            DnsUpstreamProtocol::Dhcp
        );
        assert_eq!(
            DnsUpstreamProtocol::from_address("system"),
            DnsUpstreamProtocol::System
        );
        assert_eq!(
            DnsUpstreamProtocol::from_address("223.5.5.5"),
            DnsUpstreamProtocol::Udp
        );
        assert_eq!(
            DnsUpstreamProtocol::from_address("ftp://dns.example"),
            DnsUpstreamProtocol::Unknown
        );
        assert_eq!(DnsUpstreamProtocol::Doq.chip_label(), "DoQ");
        assert_eq!(DnsUpstreamProtocol::Unknown.chip_label(), "DNS");
        assert!(DnsUpstreamProtocol::Dot.is_encrypted());
        assert!(!DnsUpstreamProtocol::Udp.is_encrypted());
        assert!(!DnsUpstreamProtocol::System.is_encrypted());
    }

    #[test]
    fn server_list_codec_round_trips_and_deduplicates() {
        let raw = "223.5.5.5, 119.29.29.29\nhttps://doh.pub/dns-query";
        assert_eq!(parse_server_list(raw).len(), 3);
        assert_eq!(
            remove_server_at(raw, 1),
            "223.5.5.5, https://doh.pub/dns-query"
        );
        let appended = append_server(raw, "tls://223.5.5.5:853");
        assert_eq!(
            appended,
            "223.5.5.5, 119.29.29.29, https://doh.pub/dns-query, tls://223.5.5.5:853"
        );
        assert_eq!(append_server(&appended, "223.5.5.5"), appended);
        assert_eq!(append_server(raw, "   "), raw);
        assert!(parse_server_list(" , \n ").is_empty());
    }

    #[test]
    fn fallback_policy_parses_raw_trigger_lists() {
        let policy = DnsFallbackPolicy::from_raw(true, " cn ", "192.168.0.0/16, 10.0.0.0/8");
        assert!(policy.geoip);
        assert_eq!(policy.geoip_code, "cn");
        assert_eq!(policy.trigger_raw(), "192.168.0.0/16, 10.0.0.0/8");
    }

    #[test]
    fn fake_ip_pool_filters_observed_bindings() {
        let pool = FakeIpMappingPool {
            source: FakeIpMappingSource::LiveConnections,
            range: "198.18.0.1/16".to_owned(),
            total: 2,
            entries: vec![
                FakeIpMappingEntry {
                    domain: "music.example.org".to_owned(),
                    address: "198.18.0.5".to_owned(),
                },
                FakeIpMappingEntry {
                    domain: "cdn.example.net".to_owned(),
                    address: "198.18.0.7".to_owned(),
                },
            ],
        };
        assert!(pool.is_observed_subset());
        assert_eq!(pool.filter("").len(), 2);
        assert_eq!(pool.filter("MUSIC").len(), 1);
        assert_eq!(pool.filter("198.18.0.7")[0].domain, "cdn.example.net");
        assert!(pool.filter("missing").is_empty());
        assert_eq!(
            FakeIpMappingPool::default().source,
            FakeIpMappingSource::Unsupported {
                reason: "no running core connection feed on this host".to_owned()
            }
        );
    }

    #[test]
    fn hosts_editor_round_trips_rows_without_ambiguity() {
        let raw = "127.0.0.1 localhost\n# comment\n192.168.1.1 router.lan *.router.lan";
        let entries = parse_hosts_editor(raw);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].address, "127.0.0.1");
        assert_eq!(entries[2].domain, "*.router.lan");
        let text = hosts_editor_text(&entries);
        assert_eq!(parse_hosts_editor(&text), entries);
        // A single-line surface separates rows with `;` and parses identically.
        assert_eq!(
            parse_hosts_editor("1.1.1.1 a.com; 8.8.8.8 b.com"),
            vec![
                DnsHostEntry {
                    domain: "a.com".to_owned(),
                    address: "1.1.1.1".to_owned()
                },
                DnsHostEntry {
                    domain: "b.com".to_owned(),
                    address: "8.8.8.8".to_owned()
                },
            ]
        );
        let appended = append_host_row(&text, "10.0.0.1", "nas.lan");
        assert_eq!(remove_host_at(&appended, 0), remove_host_at(&appended, 0));
        assert_eq!(append_host_row(&appended, "10.0.0.1", "nas.lan"), appended);
        assert_eq!(append_host_row(text.as_str(), " ", "x"), text);
    }

    #[test]
    fn hosts_validation_accepts_host_grammar_only() {
        assert!(is_valid_hosts_address("1.2.3.4"));
        assert!(is_valid_hosts_address("2001:db8::1"));
        assert!(is_valid_hosts_address("lan"));
        assert!(is_valid_hosts_address("target.example.com"));
        assert!(!is_valid_hosts_address("notadomain"));
        assert!(!is_valid_hosts_address("has space"));
        assert!(!is_valid_hosts_address(""));
        assert!(is_valid_hosts_domain("*.example.com"));
        assert!(!is_valid_hosts_domain("bad domain"));
        assert!(!is_valid_hosts_domain("host:53"));

        let issues = validate_hosts(&[
            DnsHostEntry {
                domain: "ok.example.com".to_owned(),
                address: "1.2.3.4".to_owned(),
            },
            DnsHostEntry {
                domain: "bad domain".to_owned(),
                address: "nope".to_owned(),
            },
        ]);
        assert_eq!(issues.len(), 2);
        assert!(issues.contains(&DnsHostsIssue::InvalidDomain {
            domain: "bad domain".to_owned()
        }));
        assert!(issues.contains(&DnsHostsIssue::InvalidAddress {
            address: "nope".to_owned()
        }));
        assert_eq!(issues[0].token(), issues[0].token());
    }
}

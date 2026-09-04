//! DNS configuration management for Mihomo / Clash.Meta profiles.
//!
//! Provides typed access to the `dns:` section of a profile, supporting
//! full anti-leak topologies (multi-tier upstreams, domain routing policies,
//! fallback filters, Fake-IP controls, ECS subnet governance, and DNS cache tuning).

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};
use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DnsConfig {
    pub enable: Option<bool>,
    pub listen: Option<String>,
    pub ipv6: Option<bool>,
    pub default_nameserver: Option<Vec<String>>,
    pub enhanced_mode: Option<String>,
    pub fake_ip_range: Option<String>,
    pub fake_ip_filter: Option<Vec<String>>,
    pub fake_ip_filter_mode: Option<String>,
    pub store_fake_ip: Option<bool>,
    pub nameserver: Option<Vec<String>>,
    pub fallback: Option<Vec<String>>,
    pub proxy_server_nameserver: Option<Vec<String>>,
    pub direct_nameserver: Option<Vec<String>>,
    pub nameserver_policy: Option<BTreeMap<String, serde_json::Value>>,
    pub fallback_filter: Option<FallbackFilter>,
    pub prefer_h3: Option<bool>,
    pub respect_rules: Option<bool>,
    pub use_system_hosts: Option<bool>,
    pub use_hosts: Option<bool>,
    pub cache: Option<bool>,
    pub edns_client_subnet: Option<String>,
    pub cache_algorithm: Option<String>,
    pub max_ttl: Option<u32>,
    pub min_ttl: Option<u32>,
    pub search_domains: Option<Vec<String>>,
    pub ecs_override_policy: Option<String>,
    pub bogus_nxdomain: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FallbackFilter {
    pub geoip: Option<bool>,
    pub geoip_code: Option<String>,
    pub ipcidr: Option<Vec<String>>,
    pub domain: Option<Vec<String>>,
    pub domain_suffix: Option<Vec<String>>,
    pub geosite: Option<Vec<String>>,
}

pub type DnsFallbackFilter = FallbackFilter;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DnsConfigPatch {
    pub enable: Option<bool>,
    pub listen: Option<String>,
    pub ipv6: Option<bool>,
    pub default_nameserver: Option<Vec<String>>,
    pub enhanced_mode: Option<String>,
    pub fake_ip_range: Option<String>,
    pub fake_ip_filter: Option<Vec<String>>,
    pub fake_ip_filter_mode: Option<String>,
    pub store_fake_ip: Option<bool>,
    pub nameserver: Option<Vec<String>>,
    pub fallback: Option<Vec<String>>,
    pub proxy_server_nameserver: Option<Vec<String>>,
    pub direct_nameserver: Option<Vec<String>>,
    pub nameserver_policy: Option<BTreeMap<String, serde_json::Value>>,
    pub fallback_filter: Option<FallbackFilter>,
    pub prefer_h3: Option<bool>,
    pub respect_rules: Option<bool>,
    pub use_system_hosts: Option<bool>,
    pub use_hosts: Option<bool>,
    pub cache: Option<bool>,
    pub edns_client_subnet: Option<String>,
    pub cache_algorithm: Option<String>,
    pub max_ttl: Option<u32>,
    pub min_ttl: Option<u32>,
    pub search_domains: Option<Vec<String>>,
    pub ecs_override_policy: Option<String>,
    pub bogus_nxdomain: Option<Vec<String>>,
}

pub type DnsConfigPayload = DnsConfigPatch;

impl From<DnsConfig> for DnsConfigPatch {
    fn from(c: DnsConfig) -> Self {
        Self {
            enable: c.enable,
            listen: c.listen,
            ipv6: c.ipv6,
            default_nameserver: c.default_nameserver,
            enhanced_mode: c.enhanced_mode,
            fake_ip_range: c.fake_ip_range,
            fake_ip_filter: c.fake_ip_filter,
            fake_ip_filter_mode: c.fake_ip_filter_mode,
            store_fake_ip: c.store_fake_ip,
            nameserver: c.nameserver,
            fallback: c.fallback,
            proxy_server_nameserver: c.proxy_server_nameserver,
            direct_nameserver: c.direct_nameserver,
            nameserver_policy: c.nameserver_policy,
            fallback_filter: c.fallback_filter,
            prefer_h3: c.prefer_h3,
            respect_rules: c.respect_rules,
            use_system_hosts: c.use_system_hosts,
            use_hosts: c.use_hosts,
            cache: c.cache,
            edns_client_subnet: c.edns_client_subnet,
            cache_algorithm: c.cache_algorithm,
            max_ttl: c.max_ttl,
            min_ttl: c.min_ttl,
            search_domains: c.search_domains,
            ecs_override_policy: c.ecs_override_policy,
            bogus_nxdomain: c.bogus_nxdomain,
        }
    }
}

impl From<DnsConfigPatch> for DnsConfig {
    fn from(p: DnsConfigPatch) -> Self {
        Self {
            enable: p.enable,
            listen: p.listen,
            ipv6: p.ipv6,
            default_nameserver: p.default_nameserver,
            enhanced_mode: p.enhanced_mode,
            fake_ip_range: p.fake_ip_range,
            fake_ip_filter: p.fake_ip_filter,
            fake_ip_filter_mode: p.fake_ip_filter_mode,
            store_fake_ip: p.store_fake_ip,
            nameserver: p.nameserver,
            fallback: p.fallback,
            proxy_server_nameserver: p.proxy_server_nameserver,
            direct_nameserver: p.direct_nameserver,
            nameserver_policy: p.nameserver_policy,
            fallback_filter: p.fallback_filter,
            prefer_h3: p.prefer_h3,
            respect_rules: p.respect_rules,
            use_system_hosts: p.use_system_hosts,
            use_hosts: p.use_hosts,
            cache: p.cache,
            edns_client_subnet: p.edns_client_subnet,
            cache_algorithm: p.cache_algorithm,
            max_ttl: p.max_ttl,
            min_ttl: p.min_ttl,
            search_domains: p.search_domains,
            ecs_override_policy: p.ecs_override_policy,
            bogus_nxdomain: p.bogus_nxdomain,
        }
    }
}

impl DnsConfig {
    pub fn apply_patch(&mut self, patch: DnsConfigPatch) {
        if let Some(v) = patch.enable {
            self.enable = Some(v);
        }
        if let Some(v) = patch.listen {
            self.listen = Some(v);
        }
        if let Some(v) = patch.ipv6 {
            self.ipv6 = Some(v);
        }
        if let Some(v) = patch.default_nameserver {
            self.default_nameserver = Some(v);
        }
        if let Some(v) = patch.enhanced_mode {
            self.enhanced_mode = Some(v);
        }
        if let Some(v) = patch.fake_ip_range {
            self.fake_ip_range = Some(v);
        }
        if let Some(v) = patch.fake_ip_filter {
            self.fake_ip_filter = Some(v);
        }
        if let Some(v) = patch.fake_ip_filter_mode {
            self.fake_ip_filter_mode = Some(v);
        }
        if let Some(v) = patch.store_fake_ip {
            self.store_fake_ip = Some(v);
        }
        if let Some(v) = patch.nameserver {
            self.nameserver = Some(v);
        }
        if let Some(v) = patch.fallback {
            self.fallback = Some(v);
        }
        if let Some(v) = patch.proxy_server_nameserver {
            self.proxy_server_nameserver = Some(v);
        }
        if let Some(v) = patch.direct_nameserver {
            self.direct_nameserver = Some(v);
        }
        if let Some(v) = patch.nameserver_policy {
            self.nameserver_policy = Some(v);
        }
        if let Some(v) = patch.fallback_filter {
            self.fallback_filter = Some(v);
        }
        if let Some(v) = patch.prefer_h3 {
            self.prefer_h3 = Some(v);
        }
        if let Some(v) = patch.respect_rules {
            self.respect_rules = Some(v);
        }
        if let Some(v) = patch.use_system_hosts {
            self.use_system_hosts = Some(v);
        }
        if let Some(v) = patch.use_hosts {
            self.use_hosts = Some(v);
        }
        if let Some(v) = patch.cache {
            self.cache = Some(v);
        }
        if let Some(v) = patch.edns_client_subnet {
            self.edns_client_subnet = Some(v);
        }
        if let Some(v) = patch.cache_algorithm {
            self.cache_algorithm = Some(v);
        }
        if let Some(v) = patch.max_ttl {
            self.max_ttl = Some(v);
        }
        if let Some(v) = patch.min_ttl {
            self.min_ttl = Some(v);
        }
        if let Some(v) = patch.search_domains {
            self.search_domains = Some(v);
        }
        if let Some(v) = patch.ecs_override_policy {
            self.ecs_override_policy = Some(v);
        }
        if let Some(v) = patch.bogus_nxdomain {
            self.bogus_nxdomain = Some(v);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.enable.is_none()
            && self.listen.is_none()
            && self.ipv6.is_none()
            && self.default_nameserver.is_none()
            && self.enhanced_mode.is_none()
            && self.fake_ip_range.is_none()
            && self.fake_ip_filter.is_none()
            && self.fake_ip_filter_mode.is_none()
            && self.store_fake_ip.is_none()
            && self.nameserver.is_none()
            && self.fallback.is_none()
            && self.proxy_server_nameserver.is_none()
            && self.direct_nameserver.is_none()
            && self.nameserver_policy.is_none()
            && self.fallback_filter.is_none()
            && self.prefer_h3.is_none()
            && self.respect_rules.is_none()
            && self.use_system_hosts.is_none()
            && self.use_hosts.is_none()
            && self.cache.is_none()
            && self.edns_client_subnet.is_none()
            && self.cache_algorithm.is_none()
            && self.max_ttl.is_none()
            && self.min_ttl.is_none()
            && self.search_domains.is_none()
            && self.ecs_override_policy.is_none()
            && self.bogus_nxdomain.is_none()
    }

    /// Retrieve nameservers belonging to a specific topology tier.
    pub fn get_tier_nameservers(&self, tier: NameserverTier) -> &[String] {
        match tier {
            NameserverTier::Bootstrap => self.default_nameserver.as_deref().unwrap_or(&[]),
            NameserverTier::Direct => self.direct_nameserver.as_deref().unwrap_or(&[]),
            NameserverTier::ProxyServer => self.proxy_server_nameserver.as_deref().unwrap_or(&[]),
            NameserverTier::Remote => self.nameserver.as_deref().unwrap_or(&[]),
            NameserverTier::Fallback => self.fallback.as_deref().unwrap_or(&[]),
        }
    }
}

/// Nameserver isolation tiers in high-assurance DNS topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NameserverTier {
    /// Tier 1: Bootstrap DNS for resolving DoH/DoT domains & proxy hostnames (must be pure IP).
    Bootstrap,
    /// Tier 2: Direct DNS for domestic and direct-routed requests.
    Direct,
    /// Tier 3: Dedicated DNS for resolving proxy node server endpoints to prevent recursion loops.
    ProxyServer,
    /// Tier 4: Primary remote DNS for proxy and domain routing.
    Remote,
    /// Tier 4 Fallback: Secondary anti-pollution fallback DNS for untrusted networks.
    Fallback,
}

/// Upstream protocol format recognized by the DNS engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DnsUpstreamProtocol {
    Udp,
    Tcp,
    DoH,
    DoH3,
    DoT,
    DoQ,
    DnsCrypt,
    Unknown,
}

/// A parsed DNS upstream configuration endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedUpstream {
    pub raw: String,
    pub protocol: DnsUpstreamProtocol,
    pub host: String,
    pub port: u16,
    pub path: Option<String>,
    pub outbound_tag: Option<String>,
    pub params: BTreeMap<String, String>,
}

/// Parse a raw DNS upstream string into structured metadata.
///
/// Supports:
/// - `8.8.8.8` / `8.8.8.8:53` -> UDP
/// - `udp://1.1.1.1:53#DIRECT` -> UDP with tag
/// - `tcp://8.8.8.8:53` -> TCP
/// - `tls://1.1.1.1:853#DNS` -> DoT (RFC 7858)
/// - `https://dns.google/dns-query#Proxy` -> DoH (RFC 8484)
/// - `https://cloudflare-dns.com/dns-query?h3=true#Proxy` -> DoH3
/// - `quic://dns.adguard.com:853#Proxy` -> DoQ (RFC 9250)
/// - `sdns://...` -> DNSCrypt
pub fn parse_upstream_uri(server: &str) -> Result<ParsedUpstream> {
    let trimmed = server.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("upstream server address cannot be empty"));
    }

    // Split outbound tag anchor `#...`
    let (body, outbound_tag) = match trimmed.split_once('#') {
        Some((b, tag)) => (b.trim(), Some(tag.trim().to_string())),
        None => (trimmed, None),
    };

    // DNSCrypt Stamp
    if body.starts_with("sdns://") {
        return Ok(ParsedUpstream {
            raw: trimmed.to_string(),
            protocol: DnsUpstreamProtocol::DnsCrypt,
            host: body.to_string(),
            port: 443,
            path: None,
            outbound_tag,
            params: BTreeMap::new(),
        });
    }

    let (scheme, rest) = if let Some((s, r)) = body.split_once("://") {
        (s.to_ascii_lowercase(), r)
    } else {
        ("udp".to_string(), body)
    };

    let (host_port_path, query_str) = match rest.split_once('?') {
        Some((hp, q)) => (hp, Some(q)),
        None => (rest, None),
    };

    let mut params = BTreeMap::new();
    if let Some(qs) = query_str {
        for pair in qs.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                params.insert(k.trim().to_string(), v.trim().to_string());
            } else if !pair.trim().is_empty() {
                params.insert(pair.trim().to_string(), "true".to_string());
            }
        }
    }

    let (host_port, path) = match host_port_path.split_once('/') {
        Some((hp, p)) => (hp, Some(format!("/{}", p))),
        None => (host_port_path, None),
    };

    // Parse host and port (handling IPv6 `[::1]:53` or `[::1]`)
    let (host, port) = parse_host_port(host_port, &scheme)?;

    let protocol = match scheme.as_str() {
        "udp" => DnsUpstreamProtocol::Udp,
        "tcp" => DnsUpstreamProtocol::Tcp,
        "tls" | "dot" => DnsUpstreamProtocol::DoT,
        "quic" | "doq" => DnsUpstreamProtocol::DoQ,
        "https" | "doh" => {
            if params.get("h3").map(|v| v == "true").unwrap_or(false) {
                DnsUpstreamProtocol::DoH3
            } else {
                DnsUpstreamProtocol::DoH
            }
        }
        _ => DnsUpstreamProtocol::Unknown,
    };

    Ok(ParsedUpstream {
        raw: trimmed.to_string(),
        protocol,
        host,
        port,
        path,
        outbound_tag,
        params,
    })
}

fn parse_host_port(hp: &str, scheme: &str) -> Result<(String, u16)> {
    if hp.starts_with('[') {
        // IPv6 bracket notation
        if let Some(closing) = hp.find(']') {
            let host = hp[1..closing].to_string();
            let remainder = &hp[closing + 1..];
            let port = if let Some(port_str) = remainder.strip_prefix(':') {
                port_str
                    .parse::<u16>()
                    .map_err(|_| anyhow!("invalid port in upstream: {}", remainder))?
            } else {
                default_port_for_scheme(scheme)
            };
            return Ok((host, port));
        } else {
            return Err(anyhow!("unclosed IPv6 bracket in upstream: {}", hp));
        }
    }

    if let Some((h, p_str)) = hp.rsplit_once(':') {
        // Check if `h` is an IPv6 without brackets (multiple colons)
        if h.contains(':') {
            // Raw IPv6 address without port
            let port = default_port_for_scheme(scheme);
            return Ok((hp.to_string(), port));
        }
        let port = p_str
            .parse::<u16>()
            .map_err(|_| anyhow!("invalid port in upstream: {}", p_str))?;
        Ok((h.to_string(), port))
    } else {
        let port = default_port_for_scheme(scheme);
        Ok((hp.to_string(), port))
    }
}

fn default_port_for_scheme(scheme: &str) -> u16 {
    match scheme {
        "tls" | "dot" | "quic" | "doq" => 853,
        "https" | "doh" => 443,
        _ => 53,
    }
}

/// Checks whether a server specification is a pure IP address (not a domain name).
/// Used for Tier-1 `default-nameserver` to guarantee no circular dependencies.
pub fn is_pure_ip_server(server: &str) -> bool {
    let parsed = match parse_upstream_uri(server) {
        Ok(p) => p,
        Err(_) => return false,
    };
    parsed.host.parse::<IpAddr>().is_ok()
}

/// Sanitizes and validates an EDNS Client Subnet CIDR string.
///
/// Converts host addresses to network prefix (e.g. `101.10.20.30/24` -> `101.10.20.0/24`).
pub fn sanitize_ecs_subnet(cidr: &str) -> Result<String> {
    let parts: Vec<&str> = cidr.trim().split('/').collect();
    if parts.len() != 2 {
        return Err(anyhow!(
            "invalid EDNS client subnet format (expected IP/prefix): {}",
            cidr
        ));
    }

    let ip_str = parts[0].trim();
    let prefix_len: u8 = parts[1]
        .trim()
        .parse()
        .map_err(|_| anyhow!("invalid subnet prefix length: {}", parts[1]))?;

    if let Ok(ipv4) = ip_str.parse::<Ipv4Addr>() {
        if prefix_len > 32 {
            return Err(anyhow!(
                "IPv4 subnet prefix cannot exceed /32: {}",
                prefix_len
            ));
        }
        let ip_num = u32::from(ipv4);
        let mask = if prefix_len == 0 {
            0
        } else {
            !((1u32 << (32 - prefix_len)) - 1)
        };
        let sanitized = Ipv4Addr::from(ip_num & mask);
        Ok(format!("{}/{}", sanitized, prefix_len))
    } else if let Ok(ipv6) = ip_str.parse::<Ipv6Addr>() {
        if prefix_len > 128 {
            return Err(anyhow!(
                "IPv6 subnet prefix cannot exceed /128: {}",
                prefix_len
            ));
        }
        let ip_num = u128::from(ipv6);
        let mask = if prefix_len == 0 {
            0
        } else {
            !((1u128 << (128 - prefix_len)) - 1)
        };
        let sanitized = Ipv6Addr::from(ip_num & mask);
        Ok(format!("{}/{}", sanitized, prefix_len))
    } else {
        Err(anyhow!(
            "invalid IP address in EDNS client subnet: {}",
            ip_str
        ))
    }
}

pub fn apply_dns_config_to_yaml(content: &str, config: &DnsConfig) -> Result<String> {
    validate_dns_config(config)?;
    let mut doc: Value = serde_yaml_ng::from_str(content).context("parse profile yaml")?;
    apply_dns_config(&mut doc, config)?;
    serde_yaml_ng::to_string(&doc).context("serialize profile yaml")
}

pub fn apply_dns_patch_to_yaml(content: &str, patch: DnsConfigPatch) -> Result<String> {
    let mut doc: Value = serde_yaml_ng::from_str(content).context("parse profile yaml")?;
    let mut config = extract_dns_config_from_doc(&doc)?;
    config.apply_patch(patch);
    validate_dns_config(&config)?;
    apply_dns_config(&mut doc, &config)?;
    serde_yaml_ng::to_string(&doc).context("serialize profile yaml")
}

pub fn extract_dns_config_from_doc(doc: &Value) -> Result<DnsConfig> {
    let value = doc
        .get("dns")
        .cloned()
        .unwrap_or(Value::Mapping(Mapping::new()));
    let config = serde_yaml_ng::from_value(value).context("decode dns config")?;
    Ok(config)
}

pub fn apply_dns_config_to_doc(doc: &mut Value, config: &DnsConfig) -> Result<()> {
    apply_dns_config(doc, config)
}

fn apply_dns_config(doc: &mut Value, config: &DnsConfig) -> Result<()> {
    let map = doc
        .as_mapping_mut()
        .ok_or_else(|| anyhow!("profile config is not a mapping"))?;
    if config.is_empty() {
        map.remove(Value::String("dns".to_string()));
        return Ok(());
    }
    let dns_value = serde_yaml_ng::to_value(config).context("encode dns config")?;
    map.insert(Value::String("dns".to_string()), dns_value);
    Ok(())
}

/// Comprehensive DNS topology auditor and validator.
pub fn validate_dns_config(config: &DnsConfig) -> Result<()> {
    if let Some(l) = config.listen.as_ref()
        && l.trim().is_empty()
    {
        return Err(anyhow!("listen cannot be empty"));
    }
    if let Some(f) = config.fake_ip_range.as_ref() {
        if f.trim().is_empty() {
            return Err(anyhow!("fake-ip-range cannot be empty"));
        }
        // Validate CIDR format
        if let Err(e) = sanitize_ecs_subnet(f) {
            return Err(anyhow!("invalid fake-ip-range CIDR: {}", e));
        }
    }
    if let Some(e) = config.edns_client_subnet.as_ref() {
        if e.trim().is_empty() {
            return Err(anyhow!("edns-client-subnet cannot be empty"));
        }
        sanitize_ecs_subnet(e).context("validate edns-client-subnet")?;
    }
    if let Some(mode) = config.enhanced_mode.as_ref() {
        let lower = mode.trim().to_ascii_lowercase();
        if lower != "fake-ip" && lower != "redir-host" {
            return Err(anyhow!("unsupported dns enhanced-mode: {}", mode));
        }
    }
    if let Some(mode) = config.fake_ip_filter_mode.as_ref() {
        let lower = mode.trim().to_ascii_lowercase();
        if lower != "whitelist" && lower != "blacklist" {
            return Err(anyhow!("unsupported fake-ip-filter-mode: {}", mode));
        }
    }
    if let Some(algo) = config.cache_algorithm.as_ref() {
        let lower = algo.trim().to_ascii_lowercase();
        if lower != "lru" && lower != "arc" {
            return Err(anyhow!("unsupported cache-algorithm: {}", algo));
        }
    }
    if let Some(policy) = config.ecs_override_policy.as_ref() {
        let lower = policy.trim().to_ascii_lowercase();
        if lower != "strip" && lower != "pass" && lower != "custom" {
            return Err(anyhow!("unsupported ecs-override-policy: {}", policy));
        }
    }

    // Validate Tier-1 Bootstrap Nameservers (MUST be pure IP)
    validate_bootstrap_server_list(config.default_nameserver.as_ref())?;

    // Validate other nameserver tiers
    validate_server_list(config.nameserver.as_ref(), "nameserver")?;
    validate_server_list(config.fallback.as_ref(), "fallback")?;
    validate_server_list(
        config.proxy_server_nameserver.as_ref(),
        "proxy-server-nameserver",
    )?;
    validate_server_list(config.direct_nameserver.as_ref(), "direct-nameserver")?;

    if let Some(policy) = config.nameserver_policy.as_ref() {
        for (k, v) in policy {
            if k.trim().is_empty() {
                return Err(anyhow!("nameserver-policy contains empty domain key"));
            }
            match v {
                serde_json::Value::String(s) => {
                    if s.trim().is_empty() {
                        return Err(anyhow!("nameserver-policy for '{}' has empty server", k));
                    }
                    parse_upstream_uri(s)
                        .map_err(|e| anyhow!("nameserver-policy for '{}' invalid URI: {}", k, e))?;
                }
                serde_json::Value::Array(arr) => {
                    if arr.is_empty() {
                        return Err(anyhow!(
                            "nameserver-policy for '{}' has empty server list",
                            k
                        ));
                    }
                    for item in arr {
                        if let Some(s) = item.as_str() {
                            if s.trim().is_empty() {
                                return Err(anyhow!(
                                    "nameserver-policy for '{}' contains empty server entry",
                                    k
                                ));
                            }
                            parse_upstream_uri(s).map_err(|e| {
                                anyhow!("nameserver-policy for '{}' invalid URI: {}", k, e)
                            })?;
                        } else {
                            return Err(anyhow!(
                                "nameserver-policy for '{}' contains non-string entry",
                                k
                            ));
                        }
                    }
                }
                _ => {
                    return Err(anyhow!(
                        "nameserver-policy for '{}' has invalid value type",
                        k
                    ));
                }
            }
        }
    }

    if let Some(ff) = config.fallback_filter.as_ref() {
        if let Some(ref code) = ff.geoip_code
            && code.trim().is_empty()
        {
            return Err(anyhow!("fallback-filter geoip-code cannot be empty"));
        }
        if let Some(ref ipcidr) = ff.ipcidr {
            for entry in ipcidr {
                if entry.trim().is_empty() {
                    return Err(anyhow!("fallback-filter ipcidr contains empty entry"));
                }
                sanitize_ecs_subnet(entry)
                    .map_err(|e| anyhow!("fallback-filter ipcidr entry invalid: {}", e))?;
            }
        }
        if let Some(ref domain) = ff.domain {
            for entry in domain {
                if entry.trim().is_empty() {
                    return Err(anyhow!("fallback-filter domain contains empty entry"));
                }
            }
        }
        if let Some(ref domain_suffix) = ff.domain_suffix {
            for entry in domain_suffix {
                if entry.trim().is_empty() {
                    return Err(anyhow!(
                        "fallback-filter domain-suffix contains empty entry"
                    ));
                }
            }
        }
        if let Some(ref geosite) = ff.geosite {
            for entry in geosite {
                if entry.trim().is_empty() {
                    return Err(anyhow!("fallback-filter geosite contains empty entry"));
                }
            }
        }
    }

    if let Some(filters) = config.fake_ip_filter.as_ref() {
        for entry in filters {
            if entry.trim().is_empty() {
                return Err(anyhow!("fake-ip-filter contains empty entry"));
            }
        }
    }

    if let Some(bogus) = config.bogus_nxdomain.as_ref() {
        for entry in bogus {
            if entry.trim().is_empty() {
                return Err(anyhow!("bogus-nxdomain contains empty entry"));
            }
        }
    }

    if let Some(min) = config.min_ttl
        && let Some(max) = config.max_ttl
        && min > max
    {
        return Err(anyhow!("min-ttl ({}) cannot exceed max-ttl ({})", min, max));
    }
    Ok(())
}

fn validate_server_list(list: Option<&Vec<String>>, name: &str) -> Result<()> {
    if let Some(servers) = list {
        for server in servers {
            if server.trim().is_empty() {
                return Err(anyhow!("{} contains empty server entry", name));
            }
            parse_upstream_uri(server).map_err(|e| anyhow!("{} invalid entry '{}': {}", name, server, e))?;
        }
    }
    Ok(())
}

fn validate_bootstrap_server_list(list: Option<&Vec<String>>) -> Result<()> {
    if let Some(servers) = list {
        for server in servers {
            if server.trim().is_empty() {
                return Err(anyhow!("default-nameserver contains empty server entry"));
            }
            if !is_pure_ip_server(server) {
                return Err(anyhow!(
                    "default-nameserver '{}' must be a pure IP address to prevent bootstrap loops",
                    server
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "dns_test.rs"]
mod tests;

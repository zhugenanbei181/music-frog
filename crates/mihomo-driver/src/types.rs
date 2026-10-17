use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Rule {
    #[serde(rename = "type")]
    pub rule_type: String,
    pub payload: String,
    pub proxy: String,
    /// Preserve the controller's signed wire value; Mihomo can return `-1`.
    pub size: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RuleList {
    pub rules: Vec<Rule>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Version {
    pub version: String,
    #[serde(default)]
    pub premium: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TrafficData {
    pub up: u64,
    pub down: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MemoryData {
    #[serde(rename = "inuse")]
    pub in_use: u64,
    #[serde(rename = "oslimit", alias = "os")]
    pub os_limit: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DnsConfig {
    pub enable: bool,
    pub nameserver: Vec<String>,
    pub fallback: Option<Vec<String>>,
    #[serde(rename = "enhanced-mode", default)]
    pub enhanced_mode: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TunConfig {
    #[serde(default)]
    pub enable: Option<bool>,
    #[serde(default)]
    pub stack: Option<String>,
    #[serde(rename = "auto-route", default)]
    pub auto_route: Option<bool>,
    #[serde(rename = "strict-route", default)]
    pub strict_route: Option<bool>,
    #[serde(default)]
    pub mtu: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SnifferConfig {
    pub enable: bool,
    pub sniff: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ConfigResponse {
    pub port: u16,
    #[serde(rename = "socks-port")]
    pub socks_port: u16,
    #[serde(rename = "redir-port")]
    pub redir_port: u16,
    #[serde(rename = "tproxy-port")]
    pub tproxy_port: u16,
    #[serde(rename = "mixed-port")]
    pub mixed_port: u16,
    pub mode: String,
    #[serde(rename = "log-level")]
    pub log_level: String,
    #[serde(rename = "allow-lan")]
    pub allow_lan: bool,
    #[serde(default)]
    pub ipv6: Option<bool>,
    #[serde(rename = "bind-address", default)]
    pub bind_address: Option<String>,
    #[serde(rename = "lan-allowed-ips", default)]
    pub lan_allowed_ips: Option<Vec<String>>,
    #[serde(rename = "lan-disallowed-ips", default)]
    pub lan_disallowed_ips: Option<Vec<String>>,
    #[serde(rename = "skip-auth-prefixes", default)]
    pub skip_auth_prefixes: Option<Vec<String>>,
    #[serde(default)]
    pub authentication: Option<Vec<String>>,
    pub tun: Option<TunConfig>,
    pub sniffer: Option<SnifferConfig>,
    pub dns: Option<DnsConfig>,
    pub script: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DelayTestResponse {
    pub delay: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ConnectionMetadata {
    #[serde(default)]
    pub network: String,
    #[serde(rename = "type")]
    #[serde(default)]
    pub connection_type: String,
    #[serde(rename = "sourceIP")]
    #[serde(default)]
    pub source_ip: String,
    #[serde(rename = "destinationIP")]
    #[serde(default)]
    pub destination_ip: String,
    #[serde(rename = "sourcePort")]
    #[serde(default)]
    pub source_port: String,
    #[serde(rename = "destinationPort")]
    #[serde(default)]
    pub destination_port: String,
    #[serde(default)]
    pub host: String,
    #[serde(rename = "dnsMode")]
    #[serde(default)]
    pub dns_mode: String,
    #[serde(rename = "processPath")]
    #[serde(default)]
    pub process_path: String,
    #[serde(rename = "specialProxy", default)]
    pub special_proxy: String,
    /// DUAL-13-05: the kernel's GEOIP rule-evaluation result for the
    /// destination IP (`null` = never queried, `[]` = queried with no record,
    /// otherwise the country codes the kernel resolved).
    #[serde(rename = "destinationGeoIP", default)]
    pub destination_geo_ip: Option<Vec<String>>,
    /// DUAL-13-05: the kernel's raw `destinationIPASN` value (e.g.
    /// `15169 Google LLC`, no `AS` prefix added by the kernel). Empty when no
    /// IP-ASN rule ran; whitespace-only when the kernel looked up the IP and
    /// found no ASN record.
    #[serde(rename = "destinationIPASN", default)]
    pub destination_ip_asn: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub id: String,
    pub metadata: ConnectionMetadata,
    #[serde(rename = "uploadTotal")]
    #[serde(default)]
    pub upload: u64,
    #[serde(rename = "downloadTotal")]
    #[serde(default)]
    pub download: u64,
    #[serde(default)]
    pub start: String,
    pub rule: String,
    #[serde(rename = "rulePayload")]
    #[serde(default)]
    pub rule_payload: String,
    #[serde(default)]
    pub chains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectionSnapshot {
    #[serde(rename = "downloadTotal")]
    #[serde(default)]
    pub download_total: u64,
    #[serde(rename = "uploadTotal")]
    #[serde(default)]
    pub upload_total: u64,
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub connections: Vec<Connection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectionsResponse {
    #[serde(rename = "downloadTotal", default)]
    pub download_total: u64,
    #[serde(rename = "uploadTotal", default)]
    pub upload_total: u64,
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub connections: Vec<Connection>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RuleProvider {
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub behavior: String,
    #[serde(rename = "vehicleType")]
    pub vehicle_type: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "ruleCount")]
    pub rule_count: u32,
    /// DUAL-11-06: mihomo publishes the rule payload only for `type: inline`
    /// providers; downloaded providers omit it, and some controller builds
    /// encode the absent value as `null`. The client must not invent payloads.
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub payload: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RuleProviderList {
    pub providers: HashMap<String, RuleProvider>,
}

fn deserialize_null_as_empty_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    let opt = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

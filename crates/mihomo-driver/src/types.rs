//! Mihomo controller wire types.
//!
//! Every type in this module mirrors the controller's JSON exactly. The
//! driver converts them into the neutral contract types; the wire values are
//! never interpreted here.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One routing rule reported by the controller.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Rule {
    /// The rule type, for example `Domain` or `GeoIP`.
    #[serde(rename = "type")]
    pub rule_type: String,
    /// The payload the rule matches.
    pub payload: String,
    /// The outbound the matching traffic is sent to.
    pub proxy: String,
    /// Preserve the controller's signed wire value; Mihomo can return `-1`.
    pub size: i64,
}

/// The list of routing rules reported by the controller.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RuleList {
    /// The reported rules, in controller order.
    pub rules: Vec<Rule>,
}

/// The version reported by the controller.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Version {
    /// The raw version string of the running core.
    pub version: String,
    /// Whether the running core is a premium build.
    #[serde(default)]
    pub premium: bool,
}

/// One traffic counter sample.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TrafficData {
    /// Uploaded bytes.
    pub up: u64,
    /// Downloaded bytes.
    pub down: u64,
}

/// The memory usage of the running core.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MemoryData {
    /// The bytes currently in use.
    #[serde(rename = "inuse")]
    pub in_use: u64,
    /// The operating system limit of the process.
    #[serde(rename = "oslimit", alias = "os")]
    pub os_limit: u64,
}

/// The DNS configuration of the running core.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DnsConfig {
    /// Whether the DNS handling is enabled.
    pub enable: bool,
    /// The configured name servers.
    pub nameserver: Vec<String>,
    /// The configured fallback name servers.
    pub fallback: Option<Vec<String>>,
    /// The configured enhanced mode, for example `fake-ip`.
    #[serde(rename = "enhanced-mode", default)]
    pub enhanced_mode: String,
}

/// The TUN configuration of the running core.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TunConfig {
    /// Whether the TUN device is enabled.
    #[serde(default)]
    pub enable: Option<bool>,
    /// The TUN stack the core uses.
    #[serde(default)]
    pub stack: Option<String>,
    /// Whether the core installs the automatic routes.
    #[serde(rename = "auto-route", default)]
    pub auto_route: Option<bool>,
    /// Whether the core enforces strict routing.
    #[serde(rename = "strict-route", default)]
    pub strict_route: Option<bool>,
    /// The MTU of the TUN device.
    #[serde(default)]
    pub mtu: Option<u32>,
}

/// The sniffer configuration of the running core.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SnifferConfig {
    /// Whether sniffing is enabled.
    pub enable: bool,
    /// The per-protocol sniff rules.
    pub sniff: HashMap<String, serde_json::Value>,
}

/// The effective configuration reported by the controller.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ConfigResponse {
    /// The HTTP proxy port.
    pub port: u16,
    /// The SOCKS proxy port.
    #[serde(rename = "socks-port")]
    pub socks_port: u16,
    /// The transparent redirect port.
    #[serde(rename = "redir-port")]
    pub redir_port: u16,
    /// The TProxy port.
    #[serde(rename = "tproxy-port")]
    pub tproxy_port: u16,
    /// The mixed HTTP/SOCKS port.
    #[serde(rename = "mixed-port")]
    pub mixed_port: u16,
    /// The routing mode, for example `rule`.
    pub mode: String,
    /// The log level of the running core.
    #[serde(rename = "log-level")]
    pub log_level: String,
    /// Whether the core accepts connections from the LAN.
    #[serde(rename = "allow-lan")]
    pub allow_lan: bool,
    /// Whether the core routes IPv6 traffic.
    #[serde(default)]
    pub ipv6: Option<bool>,
    /// The address the core binds its listeners to.
    #[serde(rename = "bind-address", default)]
    pub bind_address: Option<String>,
    /// The LAN peers allowed to use the proxy.
    #[serde(rename = "lan-allowed-ips", default)]
    pub lan_allowed_ips: Option<Vec<String>>,
    /// The LAN peers denied access to the proxy.
    #[serde(rename = "lan-disallowed-ips", default)]
    pub lan_disallowed_ips: Option<Vec<String>>,
    /// The URL prefixes that bypass authentication.
    #[serde(rename = "skip-auth-prefixes", default)]
    pub skip_auth_prefixes: Option<Vec<String>>,
    /// The accepted authentication tokens.
    #[serde(default)]
    pub authentication: Option<Vec<String>>,
    /// The TUN configuration, when the core reports one.
    pub tun: Option<TunConfig>,
    /// The sniffer configuration, when the core reports one.
    pub sniffer: Option<SnifferConfig>,
    /// The DNS configuration, when the core reports one.
    pub dns: Option<DnsConfig>,
    /// The raw script configuration, when the core reports one.
    pub script: Option<serde_json::Value>,
}

/// The delay measurement the controller reports for one outbound.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DelayTestResponse {
    /// The measured delay in milliseconds.
    pub delay: u32,
}

/// The metadata of one observed connection.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ConnectionMetadata {
    /// The network of the connection, for example `tcp`.
    #[serde(default)]
    pub network: String,
    /// The application protocol the controller detected.
    #[serde(rename = "type")]
    #[serde(default)]
    pub connection_type: String,
    /// The source address of the connection.
    #[serde(rename = "sourceIP")]
    #[serde(default)]
    pub source_ip: String,
    /// The destination address of the connection.
    #[serde(rename = "destinationIP")]
    #[serde(default)]
    pub destination_ip: String,
    /// The source port of the connection.
    #[serde(rename = "sourcePort")]
    #[serde(default)]
    pub source_port: String,
    /// The destination port of the connection.
    #[serde(rename = "destinationPort")]
    #[serde(default)]
    pub destination_port: String,
    /// The host name of the connection, when it was sniffed.
    #[serde(default)]
    pub host: String,
    /// The DNS mode the connection used.
    #[serde(rename = "dnsMode")]
    #[serde(default)]
    pub dns_mode: String,
    /// The process that opened the connection.
    #[serde(rename = "processPath")]
    #[serde(default)]
    pub process_path: String,
    /// The special proxy the connection was routed through.
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

/// One observed connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    /// The connection identity used by the close endpoint.
    pub id: String,
    /// The metadata of the connection.
    pub metadata: ConnectionMetadata,
    /// The uploaded bytes.
    #[serde(rename = "uploadTotal")]
    #[serde(default)]
    pub upload: u64,
    /// The downloaded bytes.
    #[serde(rename = "downloadTotal")]
    #[serde(default)]
    pub download: u64,
    /// The start timestamp reported by the controller.
    #[serde(default)]
    pub start: String,
    /// The rule the connection matched.
    pub rule: String,
    /// The payload of the matched rule.
    #[serde(rename = "rulePayload")]
    #[serde(default)]
    pub rule_payload: String,
    /// The outbound chain the connection used.
    #[serde(default)]
    pub chains: Vec<String>,
}

/// The connection counters reported by the controller.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectionSnapshot {
    /// The total downloaded bytes.
    #[serde(rename = "downloadTotal")]
    #[serde(default)]
    pub download_total: u64,
    /// The total uploaded bytes.
    #[serde(rename = "uploadTotal")]
    #[serde(default)]
    pub upload_total: u64,
    /// The observed connections.
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub connections: Vec<Connection>,
}

/// The connections endpoint response of the controller.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectionsResponse {
    /// The total downloaded bytes.
    #[serde(rename = "downloadTotal", default)]
    pub download_total: u64,
    /// The total uploaded bytes.
    #[serde(rename = "uploadTotal", default)]
    pub upload_total: u64,
    /// The observed connections.
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub connections: Vec<Connection>,
}

/// One rule provider reported by the controller.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RuleProvider {
    /// The provider name.
    pub name: String,
    /// The provider type, for example `inline`.
    #[serde(rename = "type")]
    pub provider_type: String,
    /// The provider behaviour, for example `domain`.
    pub behavior: String,
    /// The vehicle the provider fetches its rules with.
    #[serde(rename = "vehicleType")]
    pub vehicle_type: String,
    /// The timestamp of the last provider update.
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    /// The number of rules the provider holds.
    #[serde(rename = "ruleCount")]
    pub rule_count: u32,
    /// DUAL-11-06: mihomo publishes the rule payload only for `type: inline`
    /// providers; downloaded providers omit it, and some controller builds
    /// encode the absent value as `null`. The client must not invent payloads.
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub payload: Vec<String>,
}

/// The rule providers reported by the controller.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RuleProviderList {
    /// The providers, keyed by their name.
    pub providers: HashMap<String, RuleProvider>,
}

/// Deserializes a `null` list into an empty list.
///
/// Some controller builds encode an absent list as `null`; the driver treats
/// that as "no entries" instead of failing the whole response.
fn deserialize_null_as_empty_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    let opt = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

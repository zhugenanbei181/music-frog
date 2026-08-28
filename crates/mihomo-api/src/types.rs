use crate::proxy::Proxy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Rule {
    #[serde(rename = "type")]
    pub rule_type: String,
    pub payload: String,
    pub proxy: String,
    pub size: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RuleList {
    pub rules: Vec<Rule>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Version {
    pub version: String,
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
    pub enable: bool,
    pub stack: String,
    #[serde(rename = "auto-route")]
    pub auto_route: bool,
    #[serde(rename = "strict-route")]
    pub strict_route: bool,
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
    pub tun: Option<TunConfig>,
    pub sniffer: Option<SnifferConfig>,
    pub dns: Option<DnsConfig>,
    pub script: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ProxiesResponse {
    pub proxies: HashMap<String, Proxy>,
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
    #[serde(rename = "downloadTotal")]
    pub download_total: u64,
    #[serde(rename = "uploadTotal")]
    pub upload_total: u64,
    pub connections: Vec<Connection>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ProxyProvider {
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub vehicle_type: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ProxyProviderList {
    pub providers: HashMap<String, ProxyProvider>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RuleProvider {
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub behavior: String,
    pub vehicle_type: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub rule_count: u32,
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

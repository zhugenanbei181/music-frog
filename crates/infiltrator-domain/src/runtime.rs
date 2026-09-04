//! Runtime observations shared by all product surfaces.
//!
//! These are decoded controller facts, not HTTP DTOs. The Mihomo adapter owns
//! wire-shape conversion; UIs and application services consume these stable
//! values without depending on `reqwest` or the controller crate.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ConfigSnapshot {
    pub mode: String,
    pub tun: Option<TunSnapshot>,
    pub dns: Option<DnsSnapshot>,
    pub sniffer: Option<SnifferSnapshot>,
    pub script: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TunSnapshot {
    pub enable: bool,
    pub stack: String,
    pub auto_route: bool,
    pub strict_route: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct DnsSnapshot {
    pub nameserver: Vec<String>,
    pub fallback: Vec<String>,
    pub enhanced_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SnifferSnapshot {
    pub enable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TrafficData {
    pub up: u64,
    pub down: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct MemoryData {
    #[serde(rename = "inuse")]
    pub in_use: u64,
    #[serde(rename = "oslimit")]
    pub os_limit: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ConnectionMetadata {
    pub network: String,
    #[serde(rename = "type")]
    pub connection_type: String,
    #[serde(rename = "sourceIP")]
    pub source_ip: String,
    #[serde(rename = "destinationIP")]
    pub destination_ip: String,
    #[serde(rename = "sourcePort")]
    pub source_port: String,
    #[serde(rename = "destinationPort")]
    pub destination_port: String,
    pub host: String,
    #[serde(rename = "dnsMode")]
    pub dns_mode: String,
    #[serde(rename = "processPath")]
    pub process_path: String,
    #[serde(rename = "specialProxy")]
    pub special_proxy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Connection {
    pub id: String,
    pub metadata: ConnectionMetadata,
    #[serde(rename = "uploadTotal")]
    pub upload: u64,
    #[serde(rename = "downloadTotal")]
    pub download: u64,
    pub start: String,
    pub rule: String,
    #[serde(rename = "rulePayload")]
    pub rule_payload: String,
    pub chains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ConnectionSnapshot {
    #[serde(rename = "downloadTotal")]
    pub download_total: u64,
    #[serde(rename = "uploadTotal")]
    pub upload_total: u64,
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub connections: Vec<Connection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ConnectionsResponse {
    #[serde(rename = "downloadTotal")]
    pub download_total: u64,
    #[serde(rename = "uploadTotal")]
    pub upload_total: u64,
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub connections: Vec<Connection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ProxyProvider {
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    #[serde(rename = "vehicleType")]
    pub vehicle_type: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
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
}

fn deserialize_null_as_empty_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    let value = Option::deserialize(deserializer)?;
    Ok(value.unwrap_or_default())
}

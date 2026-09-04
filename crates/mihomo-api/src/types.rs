use infiltrator_domain::proxy::Proxy;
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
    pub enable: bool,
    #[serde(default)]
    pub stack: String,
    #[serde(rename = "auto-route", default)]
    pub auto_route: bool,
    #[serde(rename = "strict-route", default)]
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
    #[serde(rename = "downloadTotal", default)]
    pub download_total: u64,
    #[serde(rename = "uploadTotal", default)]
    pub upload_total: u64,
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
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

impl From<TrafficData> for infiltrator_domain::runtime::TrafficData {
    fn from(value: TrafficData) -> Self {
        Self {
            up: value.up,
            down: value.down,
        }
    }
}

impl From<MemoryData> for infiltrator_domain::runtime::MemoryData {
    fn from(value: MemoryData) -> Self {
        Self {
            in_use: value.in_use,
            os_limit: value.os_limit,
        }
    }
}

impl From<ConnectionMetadata> for infiltrator_domain::runtime::ConnectionMetadata {
    fn from(value: ConnectionMetadata) -> Self {
        Self {
            network: value.network,
            connection_type: value.connection_type,
            source_ip: value.source_ip,
            destination_ip: value.destination_ip,
            source_port: value.source_port,
            destination_port: value.destination_port,
            host: value.host,
            dns_mode: value.dns_mode,
            process_path: value.process_path,
            special_proxy: value.special_proxy,
        }
    }
}

impl From<Connection> for infiltrator_domain::runtime::Connection {
    fn from(value: Connection) -> Self {
        Self {
            id: value.id,
            metadata: value.metadata.into(),
            upload: value.upload,
            download: value.download,
            start: value.start,
            rule: value.rule,
            rule_payload: value.rule_payload,
            chains: value.chains,
        }
    }
}

impl From<ConnectionSnapshot> for infiltrator_domain::runtime::ConnectionSnapshot {
    fn from(value: ConnectionSnapshot) -> Self {
        Self {
            download_total: value.download_total,
            upload_total: value.upload_total,
            connections: value.connections.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ConnectionsResponse> for infiltrator_domain::runtime::ConnectionsResponse {
    fn from(value: ConnectionsResponse) -> Self {
        Self {
            download_total: value.download_total,
            upload_total: value.upload_total,
            connections: value.connections.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ConnectionsResponse> for infiltrator_domain::runtime::ConnectionSnapshot {
    fn from(value: ConnectionsResponse) -> Self {
        Self {
            download_total: value.download_total,
            upload_total: value.upload_total,
            connections: value.connections.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ProxyProvider> for infiltrator_domain::runtime::ProxyProvider {
    fn from(value: ProxyProvider) -> Self {
        Self {
            name: value.name,
            provider_type: value.provider_type,
            vehicle_type: value.vehicle_type,
            updated_at: value.updated_at,
        }
    }
}

impl From<RuleProvider> for infiltrator_domain::runtime::RuleProvider {
    fn from(value: RuleProvider) -> Self {
        Self {
            name: value.name,
            provider_type: value.provider_type,
            behavior: value.behavior,
            vehicle_type: value.vehicle_type,
            updated_at: value.updated_at,
            rule_count: value.rule_count,
        }
    }
}

impl From<ConfigResponse> for infiltrator_domain::runtime::ConfigSnapshot {
    fn from(value: ConfigResponse) -> Self {
        Self {
            mode: value.mode,
            tun: value.tun.map(|tun| infiltrator_domain::runtime::TunSnapshot {
                enable: tun.enable,
                stack: tun.stack,
                auto_route: tun.auto_route,
                strict_route: tun.strict_route,
            }),
            dns: value.dns.map(|dns| infiltrator_domain::runtime::DnsSnapshot {
                nameserver: dns.nameserver,
                fallback: dns.fallback.unwrap_or_default(),
                enhanced_mode: dns.enhanced_mode,
            }),
            sniffer: value
                .sniffer
                .map(|sniffer| infiltrator_domain::runtime::SnifferSnapshot {
                    enable: sniffer.enable,
                }),
            script: value.script,
        }
    }
}

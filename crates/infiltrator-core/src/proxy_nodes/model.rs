//! Typed node and field models for Meta proxy entries.
//!
//! Every strongly typed variant carries a discriminant `node_type` field
//! (serialized as the `type` key) plus a `#[serde(flatten)]` catch-all map,
//! which together keep YAML roundtrips lossless. See the parent module docs
//! for the full design rationale.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

/// Lossless proxy node as parsed from a profile's `proxies:` list.
///
/// Alias of [`ProxyNode`]; the raw name documents intent: this is what comes
/// back from [`crate::proxy_nodes::parse_profile_yaml`] before any consumer
/// inspects it, and it is guaranteed to roundtrip back to YAML without
/// dropping fields.
pub type RawNode = ProxyNode;

/// Universal node fields shared by every protocol.
///
/// Flattened into every typed variant; `name`, `server` and `port` are
/// required (a node missing them degrades to [`OtherNode`] instead of being
/// dropped), the rest is optional.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CommonFields {
    pub name: String,
    pub server: String,
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udp: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_cert_verify: Option<bool>,
}

/// Marker proving the `type` key equals `vless` (see [`VlessNode`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VlessType {
    #[serde(rename = "vless")]
    Vless,
}

/// Marker proving the `type` key equals `hysteria2` (see [`Hysteria2Node`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hysteria2Type {
    #[serde(rename = "hysteria2")]
    Hysteria2,
}

/// Marker proving the `type` key equals `tuic` (see [`TuicNode`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TuicType {
    #[serde(rename = "tuic")]
    Tuic,
}

/// Marker proving the `type` key equals `wireguard` (see [`WireGuardNode`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireGuardType {
    #[serde(rename = "wireguard")]
    WireGuard,
}

/// Marker proving the `type` key equals `ss` (see [`ShadowsocksNode`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShadowsocksType {
    #[serde(rename = "ss")]
    Shadowsocks,
}

/// Marker proving the `type` key equals `anytls` (see [`AnytlsNode`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnytlsType {
    #[serde(rename = "anytls")]
    Anytls,
}

/// Marker proving the `type` key equals `trojan` (see [`TrojanNode`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrojanType {
    #[serde(rename = "trojan")]
    Trojan,
}

/// Marker proving the `type` key equals `vmess` (see [`VmessNode`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VmessType {
    #[serde(rename = "vmess")]
    Vmess,
}

/// VLESS / AnyTLS `reality-opts` block. Unknown sub-keys are caught by [`RealityOpts::extra`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct RealityOpts {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spider_x: Option<String>,
    /// Flatten catch-all for sub-keys the model does not know yet; keeps the
    /// roundtrip lossless at every nesting level.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// AmneziaWG (AWG) obfuscation options block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct AmneziaOpts {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jc: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jmin: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jmax: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s1: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s2: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h1: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h2: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h3: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h4: Option<u32>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Stream multiplexing (`smux`) options block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct SmuxOpts {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_connections: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_streams: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_streams: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statistic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub only_tcp: Option<bool>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// XHTTP / Splithttp transport options block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct XhttpOpts {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<BTreeMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_headers: Option<BTreeMap<String, Value>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Port-hopping specification item: either a single port or a range `start-end`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortSpec {
    Single(u16),
    Range(u16, u16),
}

/// Port-hopping range representation for Hysteria 2.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PortHopping {
    pub specs: Vec<PortSpec>,
}

impl PortHopping {
    /// Parse a comma-separated list of ports and ranges, e.g. `"20000-30000,8443"`.
    pub fn parse(s: &str) -> Result<Self, String> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err("empty port hopping specification".to_string());
        }
        let mut specs = Vec::new();
        for chunk in trimmed.split(',') {
            let chunk = chunk.trim();
            if chunk.is_empty() {
                continue;
            }
            if let Some((start_str, end_str)) = chunk.split_once('-') {
                let start = start_str
                    .trim()
                    .parse::<u16>()
                    .map_err(|_| format!("invalid start port in range: {chunk}"))?;
                let end = end_str
                    .trim()
                    .parse::<u16>()
                    .map_err(|_| format!("invalid end port in range: {chunk}"))?;
                if start == 0 || end == 0 {
                    return Err(format!("port numbers must be positive: {chunk}"));
                }
                if start > end {
                    return Err(format!("start port must be <= end port: {start}-{end}"));
                }
                specs.push(PortSpec::Range(start, end));
            } else {
                let port = chunk
                    .parse::<u16>()
                    .map_err(|_| format!("invalid port number: {chunk}"))?;
                if port == 0 {
                    return Err("port number must be positive: 0".to_string());
                }
                specs.push(PortSpec::Single(port));
            }
        }
        if specs.is_empty() {
            return Err("no valid ports found".to_string());
        }
        Ok(Self { specs })
    }

    /// Check if the given port is covered by this hopping range.
    pub fn contains(&self, port: u16) -> bool {
        self.specs.iter().any(|spec| match spec {
            PortSpec::Single(p) => *p == port,
            PortSpec::Range(start, end) => port >= *start && port <= *end,
        })
    }

    /// Calculate total number of accessible ports.
    pub fn total_ports(&self) -> usize {
        self.specs
            .iter()
            .map(|spec| match spec {
                PortSpec::Single(_) => 1,
                PortSpec::Range(start, end) => (end - start + 1) as usize,
            })
            .sum()
    }

    /// Convert back to canonical comma-separated string representation.
    pub fn to_canonical_string(&self) -> String {
        self.specs
            .iter()
            .map(|spec| match spec {
                PortSpec::Single(p) => p.to_string(),
                PortSpec::Range(start, end) => format!("{start}-{end}"),
            })
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// Hysteria2 `up`/`down` bandwidth value: either a Meta-style string such as
/// `"100 Mbps"` or a bare number (`down: 200`). The shape is preserved on
/// serialization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Bandwidth {
    Text(String),
    U64(u64),
    F64(f64),
}

impl Bandwidth {
    /// Convert human-readable bandwidth text or number to approximate bits per second.
    pub fn to_bps(&self) -> Option<u64> {
        match self {
            Bandwidth::U64(v) => Some(*v),
            Bandwidth::F64(v) => Some(*v as u64),
            Bandwidth::Text(text) => {
                let trimmed = text.trim();
                let lower = trimmed.to_ascii_lowercase();
                if let Some(num_str) = lower.strip_suffix("gbps") {
                    num_str.trim().parse::<f64>().ok().map(|n| (n * 1_000_000_000.0) as u64)
                } else if let Some(num_str) = lower.strip_suffix("mbps") {
                    num_str.trim().parse::<f64>().ok().map(|n| (n * 1_000_000.0) as u64)
                } else if let Some(num_str) = lower.strip_suffix("kbps") {
                    num_str.trim().parse::<f64>().ok().map(|n| (n * 1_000.0) as u64)
                } else if let Some(num_str) = lower.strip_suffix("bps") {
                    num_str.trim().parse::<f64>().ok().map(|n| n as u64)
                } else if let Some(num_str) = lower.strip_suffix("mb/s") {
                    num_str.trim().parse::<f64>().ok().map(|n| (n * 8_000_000.0) as u64)
                } else if let Some(num_str) = lower.strip_suffix("kb/s") {
                    num_str.trim().parse::<f64>().ok().map(|n| (n * 8_000.0) as u64)
                } else {
                    trimmed.parse::<u64>().ok()
                }
            }
        }
    }
}

/// WireGuard `reserved` field, written by mihomo either as a byte list
/// (`reserved: [1, 2, 3]`) or as a base64 string (`reserved: AQID`).
///
/// The untagged enum tries the list form first; whichever shape matched is
/// re-emitted verbatim on serialization ("保形").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Reserved {
    Array(Vec<u8>),
    Base64(String),
}

/// VLESS node (`type: vless`). See the module docs for the lossless design.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct VlessNode {
    #[serde(rename = "type")]
    pub node_type: VlessType,
    #[serde(flatten)]
    pub common: CommonFields,
    /// VLESS user UUID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    /// e.g. `xtls-rprx-vision`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reality_opts: Option<RealityOpts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servername: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packet_encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xhttp_opts: Option<XhttpOpts>,
    /// `grpc-opts` — intentionally loose (`Value`) so nested vendor options
    /// never block parsing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grpc_opts: Option<Value>,
    /// `ws-opts` — same loose modeling as [`VlessNode::grpc_opts`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ws_opts: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smux: Option<SmuxOpts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dialer_proxy: Option<String>,
    /// Flatten catch-all: every key the model does not know lands here and is
    /// re-emitted on serialization. This is what makes the roundtrip lossless.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Hysteria2 node (`type: hysteria2`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Hysteria2Node {
    #[serde(rename = "type")]
    pub node_type: Hysteria2Type,
    #[serde(flatten)]
    pub common: CommonFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<String>,
    /// e.g. `salamander`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obfs: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obfs_password: Option<String>,
    /// Port-hopping interval in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hop_interval: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub up: Option<Bandwidth>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub down: Option<Bandwidth>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servername: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwnd: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recv_window_conn: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recv_window: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fast_open: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dialer_proxy: Option<String>,
    /// Flatten catch-all for unknown keys (see [`VlessNode::extra`]).
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// TUIC node (`type: tuic`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TuicNode {
    #[serde(rename = "type")]
    pub node_type: TuicType,
    #[serde(flatten)]
    pub common: CommonFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heartbeat_interval: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_timeout: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_udp_relay_packet_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_open_streams: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fast_open: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hop_interval: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_window: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receive_window: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_sni: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reduce_rtt: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udp_over_stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u8>,
    /// e.g. `bbr` / `cubic` / `new-reno`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub congestion_controller: Option<String>,
    /// e.g. `native` / `quic`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udp_relay_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servername: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dialer_proxy: Option<String>,
    /// Flatten catch-all for unknown keys (see [`VlessNode::extra`]).
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// WireGuard node (`type: wireguard`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct WireGuardNode {
    #[serde(rename = "type")]
    pub node_type: WireGuardType,
    #[serde(flatten)]
    pub common: CommonFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_shared_key: Option<String>,
    /// List form `[1, 2, 3]` or base64 string; the matched shape is kept on
    /// serialization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reserved: Option<Reserved>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mtu: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_dns_resolve: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workers: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistent_keepalive: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_ips: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amnezia_opts: Option<AmneziaOpts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dialer_proxy: Option<String>,
    /// Flatten catch-all for unknown keys (see [`VlessNode::extra`]).
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Shadowsocks node (`type: ss`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ShadowsocksNode {
    #[serde(rename = "type")]
    pub node_type: ShadowsocksType,
    #[serde(flatten)]
    pub common: CommonFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cipher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_opts: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udp_over_tcp: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uot: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uot_version: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smux: Option<SmuxOpts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dialer_proxy: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// AnyTLS node (`type: anytls`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AnytlsNode {
    #[serde(rename = "type")]
    pub node_type: AnytlsType,
    #[serde(flatten)]
    pub common: CommonFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_timeout: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_range: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servername: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reality_opts: Option<RealityOpts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ws_opts: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grpc_opts: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dialer_proxy: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Trojan node (`type: trojan`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TrojanNode {
    #[serde(rename = "type")]
    pub node_type: TrojanType,
    #[serde(flatten)]
    pub common: CommonFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servername: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ws_opts: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grpc_opts: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smux: Option<SmuxOpts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dialer_proxy: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// VMess node (`type: vmess`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct VmessNode {
    #[serde(rename = "type")]
    pub node_type: VmessType,
    #[serde(flatten)]
    pub common: CommonFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alter_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cipher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servername: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packet_encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ws_opts: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grpc_opts: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h2_opts: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_opts: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smux: Option<SmuxOpts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dialer_proxy: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Fallback node for unknown `type` values or for known types whose fields
/// failed to type-check.
///
/// `type_name` keeps the original `type` string and `fields` keeps every
/// other key verbatim (including `name`/`server`/`port`), so the node can
/// always be written back to YAML unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtherNode {
    #[serde(rename = "type")]
    pub type_name: String,
    /// Every key except `type`, exactly as parsed.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// A parsed `proxies:` entry. See the module docs for the lossless design.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProxyNode {
    Vless(VlessNode),
    Hysteria2(Hysteria2Node),
    Tuic(TuicNode),
    WireGuard(WireGuardNode),
    Shadowsocks(ShadowsocksNode),
    Anytls(AnytlsNode),
    Trojan(TrojanNode),
    Vmess(VmessNode),
    /// Lossless fallback; tried last.
    Other(OtherNode),
}

impl ProxyNode {
    /// The Meta `type` string of this node.
    pub fn type_name(&self) -> &str {
        match self {
            ProxyNode::Vless(_) => "vless",
            ProxyNode::Hysteria2(_) => "hysteria2",
            ProxyNode::Tuic(_) => "tuic",
            ProxyNode::WireGuard(_) => "wireguard",
            ProxyNode::Shadowsocks(_) => "ss",
            ProxyNode::Anytls(_) => "anytls",
            ProxyNode::Trojan(_) => "trojan",
            ProxyNode::Vmess(_) => "vmess",
            ProxyNode::Other(other) => other.type_name.as_str(),
        }
    }

    /// Typed common fields; `None` only for [`ProxyNode::Other`].
    pub fn common(&self) -> Option<&CommonFields> {
        match self {
            ProxyNode::Vless(node) => Some(&node.common),
            ProxyNode::Hysteria2(node) => Some(&node.common),
            ProxyNode::Tuic(node) => Some(&node.common),
            ProxyNode::WireGuard(node) => Some(&node.common),
            ProxyNode::Shadowsocks(node) => Some(&node.common),
            ProxyNode::Anytls(node) => Some(&node.common),
            ProxyNode::Trojan(node) => Some(&node.common),
            ProxyNode::Vmess(node) => Some(&node.common),
            ProxyNode::Other(_) => None,
        }
    }

    /// Node display name; falls back to the raw map for [`ProxyNode::Other`].
    pub fn name(&self) -> &str {
        match self {
            ProxyNode::Vless(node) => node.common.name.as_str(),
            ProxyNode::Hysteria2(node) => node.common.name.as_str(),
            ProxyNode::Tuic(node) => node.common.name.as_str(),
            ProxyNode::WireGuard(node) => node.common.name.as_str(),
            ProxyNode::Shadowsocks(node) => node.common.name.as_str(),
            ProxyNode::Anytls(node) => node.common.name.as_str(),
            ProxyNode::Trojan(node) => node.common.name.as_str(),
            ProxyNode::Vmess(node) => node.common.name.as_str(),
            ProxyNode::Other(other) => other
                .fields
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        }
    }

    /// Node server address; falls back to the raw map for [`ProxyNode::Other`].
    pub fn server(&self) -> Option<&str> {
        match self {
            ProxyNode::Vless(node) => Some(node.common.server.as_str()),
            ProxyNode::Hysteria2(node) => Some(node.common.server.as_str()),
            ProxyNode::Tuic(node) => Some(node.common.server.as_str()),
            ProxyNode::WireGuard(node) => Some(node.common.server.as_str()),
            ProxyNode::Shadowsocks(node) => Some(node.common.server.as_str()),
            ProxyNode::Anytls(node) => Some(node.common.server.as_str()),
            ProxyNode::Trojan(node) => Some(node.common.server.as_str()),
            ProxyNode::Vmess(node) => Some(node.common.server.as_str()),
            ProxyNode::Other(other) => other.fields.get("server").and_then(Value::as_str),
        }
    }

    /// Unknown keys captured by the flatten catch-all. For
    /// [`ProxyNode::Other`] this is the whole raw field map (minus `type`).
    pub fn extra(&self) -> &BTreeMap<String, Value> {
        match self {
            ProxyNode::Vless(node) => &node.extra,
            ProxyNode::Hysteria2(node) => &node.extra,
            ProxyNode::Tuic(node) => &node.extra,
            ProxyNode::WireGuard(node) => &node.extra,
            ProxyNode::Shadowsocks(node) => &node.extra,
            ProxyNode::Anytls(node) => &node.extra,
            ProxyNode::Trojan(node) => &node.extra,
            ProxyNode::Vmess(node) => &node.extra,
            ProxyNode::Other(other) => &other.fields,
        }
    }

    /// `true` when the node matched one of the strongly typed protocols.
    pub fn is_typed(&self) -> bool {
        !matches!(self, ProxyNode::Other(_))
    }
}

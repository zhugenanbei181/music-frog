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

/// VLESS `reality-opts` block. Unknown sub-keys are caught by [`RealityOpts::extra`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RealityOpts {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_id: Option<String>,
    /// Flatten catch-all for sub-keys the model does not know yet; keeps the
    /// roundtrip lossless at every nesting level.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
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
    /// e.g. `xtls-rprx-vision`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reality_opts: Option<RealityOpts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    /// `grpc-opts` — intentionally loose (`Value`) so nested vendor options
    /// never block parsing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grpc_opts: Option<Value>,
    /// `ws-opts` — same loose modeling as [`VlessNode::grpc_opts`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ws_opts: Option<Value>,
    /// Flatten catch-all: every key the model does not know (e.g. `uuid`,
    /// `servername`, or a future Meta field) lands here and is re-emitted on
    /// serialization. This is what makes the roundtrip lossless.
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
    pub alpn: Option<Vec<String>>,
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
    /// e.g. `bbr` / `cubic` / `new-reno`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub congestion_controller: Option<String>,
    /// e.g. `native` / `quic`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udp_relay_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,
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
    /// Flatten catch-all for unknown keys, e.g. `amnezia-wg-config`, `peers`
    /// (see [`VlessNode::extra`]).
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Fallback node for unknown `type` values (`ss`, `vmess`, `trojan`, future
/// Meta protocols, ...) or for known types whose fields failed to type-check.
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
    /// Lossless fallback; tried last.
    Other(OtherNode),
}

impl ProxyNode {
    /// The Meta `type` string of this node (`vless`, `hysteria2`, `tuic`,
    /// `wireguard`, or whatever an [`OtherNode`] captured).
    pub fn type_name(&self) -> &str {
        match self {
            ProxyNode::Vless(_) => "vless",
            ProxyNode::Hysteria2(_) => "hysteria2",
            ProxyNode::Tuic(_) => "tuic",
            ProxyNode::WireGuard(_) => "wireguard",
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
            ProxyNode::Other(other) => &other.fields,
        }
    }

    /// `true` when the node matched one of the strongly typed protocols.
    pub fn is_typed(&self) -> bool {
        !matches!(self, ProxyNode::Other(_))
    }
}

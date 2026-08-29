//! Typed models for Meta-specific proxy node fields ([缺口04], CORE-003).
//!
//! mihomo (Meta) profiles carry a `proxies:` list whose entries mix a handful
//! of universal keys (`name`, `type`, `server`, `port`, `udp`, ...) with
//! protocol-specific fields (VLESS REALITY options, Hysteria2 obfs and
//! bandwidth caps, TUIC congestion control, WireGuard keys and reserved
//! bytes, ...). This module gives those fields strong Rust types while
//! guaranteeing **zero data loss** on a YAML roundtrip, so a parsed profile
//! can always be written back unchanged.
//!
//! # Zero-loss mechanism
//!
//! [`ProxyNode`] is a `#[serde(untagged)]` enum tried in declaration order:
//!
//! 1. [`VlessNode`], [`Hysteria2Node`], [`TuicNode`] and [`WireGuardNode`]
//!    strongly type the known fields. Each of them carries a discriminant
//!    field `node_type` (serialized as the `type` key) plus
//!    `#[serde(flatten)] extra: BTreeMap<String, serde_yaml_ng::Value>`.
//!    The flatten catch-all is *the* key mechanism for losslessness: serde
//!    routes every key the model does not know about into `extra`, and
//!    serialization emits `extra` back into the mapping. Unknown fields
//!    (e.g. a future `fake-field: 123`) therefore survive the
//!    `YAML -> typed -> YAML` roundtrip untouched.
//! 2. If every typed variant fails to decode — an unknown `type` value such
//!    as `ss`/`vmess`/`trojan`, or a known field holding a value the model
//!    rejects (e.g. `port: "abc"`) — the node degrades to [`OtherNode`],
//!    which keeps the raw `type` string plus **every** other key verbatim in
//!    a single map. Degrading never drops data; run [`validate`] on the
//!    parsed nodes to surface the underlying problems instead.
//! 3. Only entries that are not mappings, or that lack a string `type` key,
//!    are rejected outright (mihomo rejects them too).
//!
//! Note on representation: `#[serde(tag = "type")]` (internally tagged enum)
//! would consume the `type` key, which makes a lossless fallback variant for
//! unknown `type` values impossible — an untagged variant inside a tagged
//! enum is serialized without its tag, so the `type` key would be lost on
//! the way back to YAML. Modeling the tag as an ordinary discriminating
//! field per variant keeps the exact same wire shape (`type: vless`, ...) as
//! mihomo expects while remaining fully reversible.
//!
//! Guarantees (all covered by tests in this module):
//! * `parse_profile_yaml(text)` followed by `nodes_to_profile_yaml(&nodes)`
//!   reproduces the `proxies:` section with **semantic equivalence** at the
//!   `serde_yaml_ng::Value` level (key order may differ; values do not).
//! * `typed -> YAML -> typed` roundtrip is a fixed point, and repeated
//!   serialization is byte-stable.
//! * [`Reserved`] keeps both wire shapes — the `[1, 2, 3]` list form and the
//!   base64 string form — exactly as written.

use std::collections::BTreeMap;
use std::net::IpAddr;

use anyhow::{Context, anyhow};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};

/// Lossless proxy node as parsed from a profile's `proxies:` list.
///
/// Alias of [`ProxyNode`]; the raw name documents intent: this is what comes
/// back from [`parse_profile_yaml`] before any consumer inspects it, and it
/// is guaranteed to roundtrip back to YAML without dropping fields.
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

/// Parse a profile YAML document and return its `proxies:` list.
///
/// Profiles without a `proxies:` key (or with an empty/null one) yield an
/// empty vec. Nodes the typed models cannot represent degrade to
/// [`ProxyNode::Other`] instead of failing; only a non-mapping document, a
/// non-list `proxies:` key, or an entry without a string `type` is an error.
pub fn parse_profile_yaml(text: &str) -> anyhow::Result<Vec<RawNode>> {
    let doc: Value = serde_yaml_ng::from_str(text).context("parse profile yaml")?;
    extract_nodes_from_doc(&doc)
}

/// Extract nodes from an already parsed profile document.
pub fn extract_nodes_from_doc(doc: &Value) -> anyhow::Result<Vec<RawNode>> {
    if !doc.is_mapping() {
        return Err(anyhow!("profile yaml must be a top-level mapping"));
    }
    match doc.get("proxies") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Sequence(entries)) => {
            let mut nodes = Vec::with_capacity(entries.len());
            for (index, entry) in entries.iter().enumerate() {
                let node: RawNode = serde_yaml_ng::from_value(entry.clone())
                    .with_context(|| format!("failed to decode proxies[{index}]"))?;
                nodes.push(node);
            }
            Ok(nodes)
        }
        Some(_) => Err(anyhow!("`proxies` must be a list of node mappings")),
    }
}

/// Serialize nodes into a minimal profile document containing only the
/// `proxies:` section. The output re-parses to exactly `nodes`.
pub fn nodes_to_profile_yaml(nodes: &[RawNode]) -> anyhow::Result<String> {
    let mut doc = Mapping::new();
    let proxies = serde_yaml_ng::to_value(nodes).context("encode proxies nodes")?;
    doc.insert(Value::String("proxies".to_string()), proxies);
    serde_yaml_ng::to_string(&Value::Mapping(doc)).context("serialize proxies yaml")
}

/// Replace (or insert) the `proxies:` section of a profile document with
/// `nodes`, leaving every other section untouched. Useful to write parsed
/// and edited nodes back without losing the rest of the profile.
pub fn replace_proxies_in_profile(text: &str, nodes: &[RawNode]) -> anyhow::Result<String> {
    let mut doc: Value = serde_yaml_ng::from_str(text).context("parse profile yaml")?;
    if !doc.is_mapping() {
        return Err(anyhow!("profile yaml must be a top-level mapping"));
    }
    let proxies = serde_yaml_ng::to_value(nodes).context("encode proxies nodes")?;
    if let Some(map) = doc.as_mapping_mut() {
        map.insert(Value::String("proxies".to_string()), proxies);
    }
    serde_yaml_ng::to_string(&doc).context("serialize profile yaml")
}

const TUIC_CONGESTION_CONTROLLERS: [&str; 3] = ["bbr", "cubic", "new-reno"];
const TUIC_UDP_RELAY_MODES: [&str; 2] = ["native", "quic"];

/// Lightweight validation: returns one human-readable message per detected
/// problem (missing required fields, empty values, unknown enum-like
/// values). An empty vec means no obvious problem; this is advisory only and
/// never mutates the node.
pub fn validate(node: &RawNode) -> Vec<String> {
    let mut issues = Vec::new();
    match node {
        ProxyNode::Vless(node) => {
            validate_common(&node.common, &mut issues);
            match node.extra.get("uuid") {
                Some(Value::String(uuid)) if !uuid.trim().is_empty() => {}
                _ => issues.push("vless: uuid is required".to_string()),
            }
            if let Some(flow) = node.flow.as_deref()
                && flow != "xtls-rprx-vision"
            {
                issues.push("vless: flow must be xtls-rprx-vision when present".to_string());
            }
        }
        ProxyNode::Hysteria2(node) => {
            validate_common(&node.common, &mut issues);
            if node.password.as_deref().is_none_or(|p| p.trim().is_empty()) {
                issues.push("hysteria2: password is required".to_string());
            }
            if node.obfs.as_deref().map(str::trim) == Some("salamander")
                && node
                    .obfs_password
                    .as_deref()
                    .is_none_or(|p| p.trim().is_empty())
            {
                issues.push(
                    "hysteria2: obfs-password is required when obfs is salamander".to_string(),
                );
            }
            if node.hop_interval == Some(0) {
                issues.push("hysteria2: hop-interval must be positive".to_string());
            }
        }
        ProxyNode::Tuic(node) => {
            validate_common(&node.common, &mut issues);
            if node.uuid.as_deref().is_none_or(|u| u.trim().is_empty()) {
                issues.push("tuic: uuid is required".to_string());
            }
            if node.password.as_deref().is_none_or(|p| p.trim().is_empty()) {
                issues.push("tuic: password is required".to_string());
            }
            if let Some(cc) = node.congestion_controller.as_deref()
                && !TUIC_CONGESTION_CONTROLLERS.contains(&cc.trim())
            {
                issues.push(format!(
                    "tuic: unknown congestion-controller {cc:?} (expected one of {TUIC_CONGESTION_CONTROLLERS:?})"
                ));
            }
            if let Some(mode) = node.udp_relay_mode.as_deref()
                && !TUIC_UDP_RELAY_MODES.contains(&mode.trim())
            {
                issues.push(format!(
                    "tuic: unknown udp-relay-mode {mode:?} (expected one of {TUIC_UDP_RELAY_MODES:?})"
                ));
            }
        }
        ProxyNode::WireGuard(node) => {
            validate_common(&node.common, &mut issues);
            if node
                .private_key
                .as_deref()
                .is_none_or(|k| k.trim().is_empty())
            {
                issues.push("wireguard: private-key is required".to_string());
            }
            if node
                .public_key
                .as_deref()
                .is_none_or(|k| k.trim().is_empty())
            {
                issues.push("wireguard: public-key is required".to_string());
            }
            if node.ip.is_none() && node.ipv6.is_none() {
                issues.push("wireguard: ip or ipv6 is required".to_string());
            }
            for (label, value) in [("ip", node.ip.as_deref()), ("ipv6", node.ipv6.as_deref())] {
                if let Some(ip) = value
                    && !ip.trim().is_empty()
                    && ip.trim().parse::<IpAddr>().is_err()
                {
                    issues.push(format!("wireguard: {label} is not a valid IP address"));
                }
            }
            match node.reserved.as_ref() {
                Some(Reserved::Array(items)) if items.is_empty() => {
                    issues.push("wireguard: reserved must not be empty".to_string());
                }
                Some(Reserved::Base64(text)) if text.trim().is_empty() => {
                    issues.push("wireguard: reserved must not be empty".to_string());
                }
                _ => {}
            }
            if node.mtu == Some(0) {
                issues.push("wireguard: mtu must be positive".to_string());
            }
        }
        ProxyNode::Other(other) => {
            if other.type_name.trim().is_empty() {
                issues.push("type must not be empty".to_string());
            }
            for key in ["name", "server", "port"] {
                if !other.fields.contains_key(key) {
                    issues.push(format!(
                        "{}: {key} is required for untyped node",
                        other.type_name
                    ));
                }
            }
            if let Some(port) = other.fields.get("port")
                && !port.is_number()
            {
                issues.push(format!("{}: port must be a number", other.type_name));
            }
        }
    }
    issues
}

fn validate_common(common: &CommonFields, issues: &mut Vec<String>) {
    if common.name.trim().is_empty() {
        issues.push("name must not be empty".to_string());
    }
    if common.server.trim().is_empty() {
        issues.push("server must not be empty".to_string());
    }
    if common.port == 0 {
        issues.push("port must be positive".to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VLESS_YAML: &str = r#"
proxies:
  - name: vless-reality-vision
    type: vless
    server: 203.0.113.10
    port: 443
    uuid: b831381d-6324-4d53-ad4f-8cda48b30811
    udp: true
    tls: true
    skip-cert-verify: false
    flow: xtls-rprx-vision
    servername: www.microsoft.com
    client-fingerprint: chrome
    network: tcp
    reality-opts:
      public-key: SbVKOEMjK0sIlbwg4akyBg5mL5KZwwB-ed4eEE7YnRc
      short-id: "6ba85179e30d4fc2"
    grpc-opts:
      grpc-service-name: grpc-svc
    ws-opts:
      path: /ws
      headers:
        Host: www.microsoft.com
    fake-field: 123
"#;

    const HYSTERIA2_YAML: &str = r#"
proxies:
  - name: hy2-full
    type: hysteria2
    server: 203.0.113.20
    port: 36712
    password: hy2-pass
    obfs: salamander
    obfs-password: obfs-pass
    hop-interval: 30
    up: "100 Mbps"
    down: 200
    alpn:
      - h3
    fake-field: hello
"#;

    const TUIC_YAML: &str = r#"
proxies:
  - name: tuic-full
    type: tuic
    server: 203.0.113.30
    port: 443
    uuid: 5c1eee1f-1f0b-4e11-9a2e-f1d3aa09ab22
    password: tuic-pass
    congestion-controller: bbr
    udp-relay-mode: native
    alpn:
      - h3
    reduce-rtt: true
    fake-field: 123
"#;

    const WIREGUARD_YAML: &str = r#"
proxies:
  - name: wg-full
    type: wireguard
    server: 203.0.113.40
    port: 51820
    udp: true
    private-key: eCtXsJZ27+4PbhDkHnB923tkUn2Gj59wZw5wFA75MnU=
    public-key: Cr8hWlKvtDt7nrvf+f0brNQQzabAqrjfBvas9pmowjo=
    pre-shared-key: 31aIhAPwktDGpH4JDhA8GNvjFXEf/a6+UaQRyOAiyfM=
    reserved: [1, 2, 3]
    mtu: 1420
    ip: 172.16.0.2
    ipv6: fd01:5ca1:ab1e:80fa:ab85:6eea:213f:f4a5
    remote-dns-resolve: true
    dns:
      - 1.1.1.1
      - 8.8.8.8
    fake-field: 123
"#;

    const WIREGUARD_RESERVED_BASE64_YAML: &str = r#"
proxies:
  - name: wg-base64
    type: wireguard
    server: 203.0.113.41
    port: 51820
    private-key: eCtXsJZ27+4PbhDkHnB923tkUn2Gj59wZw5wFA75MnU=
    public-key: Cr8hWlKvtDt7nrvf+f0brNQQzabAqrjfBvas9pmowjo=
    reserved: AQID
    ip: 172.16.0.3
    fake-field: keep-me
"#;

    /// Five protocols in one profile: the four typed ones (each carrying an
    /// unknown `fake-field`) plus a legacy `ss` node the model does not know.
    const PROFILE_YAML: &str = r#"
mixed-port: 7890
mode: rule
log-level: info

dns:
  enable: true
  enhanced-mode: fake-ip
  fake-ip-range: 198.18.0.1/16

proxies:
  - name: vless-reality-vision
    type: vless
    server: 203.0.113.10
    port: 443
    uuid: b831381d-6324-4d53-ad4f-8cda48b30811
    flow: xtls-rprx-vision
    reality-opts:
      public-key: SbVKOEMjK0sIlbwg4akyBg5mL5KZwwB-ed4eEE7YnRc
      short-id: "6ba85179e30d4fc2"
    fake-field: 123
  - name: hy2-full
    type: hysteria2
    server: 203.0.113.20
    port: 36712
    password: hy2-pass
    obfs: salamander
    obfs-password: obfs-pass
    up: "100 Mbps"
    down: 200
    fake-field: hello
  - name: tuic-full
    type: tuic
    server: 203.0.113.30
    port: 443
    uuid: 5c1eee1f-1f0b-4e11-9a2e-f1d3aa09ab22
    password: tuic-pass
    congestion-controller: bbr
    fake-field: 123
  - name: wg-full
    type: wireguard
    server: 203.0.113.40
    port: 51820
    private-key: eCtXsJZ27+4PbhDkHnB923tkUn2Gj59wZw5wFA75MnU=
    public-key: Cr8hWlKvtDt7nrvf+f0brNQQzabAqrjfBvas9pmowjo=
    reserved: [1, 2, 3]
    ip: 172.16.0.2
    fake-field: 123
  - name: legacy-ss
    type: ss
    server: 203.0.113.50
    port: 8388
    cipher: aes-256-gcm
    password: ss-pass
    plugin: v2ray-plugin
    fake-field: 7

rules:
  - DOMAIN-SUFFIX,example.com,DIRECT
  - MATCH,PROXY
"#;

    fn parse_single(text: &str) -> RawNode {
        let nodes = parse_profile_yaml(text).expect("parse profile yaml");
        assert_eq!(nodes.len(), 1, "expected exactly one node");
        nodes.into_iter().next().expect("node")
    }

    /// typed -> YAML -> typed must be a fixed point.
    fn assert_roundtrip_fixed_point(nodes: &[RawNode]) {
        let yaml = nodes_to_profile_yaml(nodes).expect("serialize nodes");
        let reparsed = parse_profile_yaml(&yaml).expect("re-parse serialized nodes");
        assert_eq!(nodes, &reparsed, "typed -> YAML -> typed must be lossless");
    }

    /// The `proxies:` section must be semantically equivalent (same
    /// `serde_yaml_ng::Value`) before and after the roundtrip.
    fn assert_proxies_semantic_equivalence(input: &str) {
        let nodes = parse_profile_yaml(input).expect("parse profile");
        let output = nodes_to_profile_yaml(&nodes).expect("serialize nodes");
        assert_eq!(
            proxies_value(input),
            proxies_value(&output),
            "proxies section changed across the YAML roundtrip\n--- re-serialized: ---\n{output}"
        );
    }

    fn proxies_value(text: &str) -> Value {
        let doc: Value = serde_yaml_ng::from_str(text).expect("yaml doc");
        doc.get("proxies").cloned().expect("proxies section")
    }

    #[test]
    fn test_vless_full_roundtrip() {
        let node = parse_single(VLESS_YAML);
        let ProxyNode::Vless(vless) = &node else {
            panic!("vless node degraded to Other: {node:?}");
        };
        assert_eq!(node.type_name(), "vless");
        assert_eq!(vless.common.name, "vless-reality-vision");
        assert_eq!(vless.common.server, "203.0.113.10");
        assert_eq!(vless.common.port, 443);
        assert_eq!(vless.common.udp, Some(true));
        assert_eq!(vless.common.tls, Some(true));
        assert_eq!(vless.common.skip_cert_verify, Some(false));
        assert_eq!(vless.flow.as_deref(), Some("xtls-rprx-vision"));
        assert_eq!(vless.client_fingerprint.as_deref(), Some("chrome"));
        assert_eq!(vless.network.as_deref(), Some("tcp"));
        let reality = vless.reality_opts.as_ref().expect("reality-opts");
        assert_eq!(
            reality.public_key.as_deref(),
            Some("SbVKOEMjK0sIlbwg4akyBg5mL5KZwwB-ed4eEE7YnRc")
        );
        assert_eq!(reality.short_id.as_deref(), Some("6ba85179e30d4fc2"));
        let grpc = vless.grpc_opts.as_ref().expect("grpc-opts");
        assert_eq!(
            grpc.get("grpc-service-name"),
            Some(&Value::String("grpc-svc".to_string()))
        );
        let ws = vless.ws_opts.as_ref().expect("ws-opts");
        assert_eq!(ws.get("path"), Some(&Value::String("/ws".to_string())));

        // Unknown keys must be captured by the flatten extra map.
        assert!(vless.extra.contains_key("uuid"), "uuid kept in extra");
        assert!(
            vless.extra.contains_key("servername"),
            "servername kept in extra"
        );
        assert!(
            matches!(vless.extra.get("fake-field"), Some(Value::Number(_))),
            "fake-field kept in extra"
        );

        assert_proxies_semantic_equivalence(VLESS_YAML);
        assert_roundtrip_fixed_point(std::slice::from_ref(&node));

        let yaml = nodes_to_profile_yaml(std::slice::from_ref(&node)).expect("serialize");
        assert!(
            yaml.contains("fake-field: 123"),
            "unknown field must be re-emitted into YAML: {yaml}"
        );
        assert!(yaml.contains("flow: xtls-rprx-vision"));
    }

    #[test]
    fn test_hysteria2_full_roundtrip() {
        let node = parse_single(HYSTERIA2_YAML);
        let ProxyNode::Hysteria2(hy2) = &node else {
            panic!("hysteria2 node degraded to Other: {node:?}");
        };
        assert_eq!(node.type_name(), "hysteria2");
        assert_eq!(hy2.common.port, 36712);
        assert_eq!(hy2.password.as_deref(), Some("hy2-pass"));
        assert_eq!(hy2.obfs.as_deref(), Some("salamander"));
        assert_eq!(hy2.obfs_password.as_deref(), Some("obfs-pass"));
        assert_eq!(hy2.hop_interval, Some(30));
        assert_eq!(hy2.up, Some(Bandwidth::Text("100 Mbps".to_string())));
        assert_eq!(hy2.down, Some(Bandwidth::U64(200)));
        assert_eq!(hy2.alpn, Some(vec!["h3".to_string()]));
        assert_eq!(
            hy2.extra.get("fake-field"),
            Some(&Value::String("hello".to_string()))
        );

        assert_proxies_semantic_equivalence(HYSTERIA2_YAML);
        assert_roundtrip_fixed_point(std::slice::from_ref(&node));
        assert!(validate(&node).is_empty());
    }

    #[test]
    fn test_tuic_full_roundtrip() {
        let node = parse_single(TUIC_YAML);
        let ProxyNode::Tuic(tuic) = &node else {
            panic!("tuic node degraded to Other: {node:?}");
        };
        assert_eq!(node.type_name(), "tuic");
        assert_eq!(
            tuic.uuid.as_deref(),
            Some("5c1eee1f-1f0b-4e11-9a2e-f1d3aa09ab22")
        );
        assert_eq!(tuic.password.as_deref(), Some("tuic-pass"));
        assert_eq!(tuic.congestion_controller.as_deref(), Some("bbr"));
        assert_eq!(tuic.udp_relay_mode.as_deref(), Some("native"));
        assert_eq!(tuic.alpn, Some(vec!["h3".to_string()]));
        assert!(
            tuic.extra.contains_key("reduce-rtt"),
            "reduce-rtt kept in extra"
        );
        assert_eq!(
            tuic.extra.get("fake-field"),
            Some(&Value::Number(123.into()))
        );

        assert_proxies_semantic_equivalence(TUIC_YAML);
        assert_roundtrip_fixed_point(std::slice::from_ref(&node));
        assert!(validate(&node).is_empty());
    }

    #[test]
    fn test_wireguard_full_roundtrip() {
        let node = parse_single(WIREGUARD_YAML);
        let ProxyNode::WireGuard(wg) = &node else {
            panic!("wireguard node degraded to Other: {node:?}");
        };
        assert_eq!(node.type_name(), "wireguard");
        assert_eq!(
            wg.private_key.as_deref(),
            Some("eCtXsJZ27+4PbhDkHnB923tkUn2Gj59wZw5wFA75MnU=")
        );
        assert_eq!(
            wg.public_key.as_deref(),
            Some("Cr8hWlKvtDt7nrvf+f0brNQQzabAqrjfBvas9pmowjo=")
        );
        assert_eq!(
            wg.pre_shared_key.as_deref(),
            Some("31aIhAPwktDGpH4JDhA8GNvjFXEf/a6+UaQRyOAiyfM=")
        );
        // List form must be modeled as Reserved::Array and preserved.
        assert_eq!(wg.reserved, Some(Reserved::Array(vec![1, 2, 3])));
        assert_eq!(wg.mtu, Some(1420));
        assert_eq!(wg.ip.as_deref(), Some("172.16.0.2"));
        assert_eq!(
            wg.ipv6.as_deref(),
            Some("fd01:5ca1:ab1e:80fa:ab85:6eea:213f:f4a5")
        );
        assert_eq!(wg.remote_dns_resolve, Some(true));
        assert_eq!(
            wg.dns,
            Some(vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()])
        );
        assert_eq!(wg.extra.get("fake-field"), Some(&Value::Number(123.into())));

        assert_proxies_semantic_equivalence(WIREGUARD_YAML);
        assert_roundtrip_fixed_point(std::slice::from_ref(&node));
        assert!(validate(&node).is_empty());
    }

    #[test]
    fn test_wireguard_reserved_base64_roundtrip() {
        let node = parse_single(WIREGUARD_RESERVED_BASE64_YAML);
        let ProxyNode::WireGuard(wg) = &node else {
            panic!("wireguard node degraded to Other: {node:?}");
        };
        // Base64 string form must stay a string (保形), not be decoded.
        assert_eq!(wg.reserved, Some(Reserved::Base64("AQID".to_string())));

        assert_proxies_semantic_equivalence(WIREGUARD_RESERVED_BASE64_YAML);
        let yaml = nodes_to_profile_yaml(std::slice::from_ref(&node)).expect("serialize");
        assert!(
            yaml.contains("reserved: AQID"),
            "base64 reserved shape must survive serialization: {yaml}"
        );
        assert!(yaml.contains("fake-field: keep-me"));
        assert_roundtrip_fixed_point(std::slice::from_ref(&node));
    }

    #[test]
    fn test_malformed_values_degrade_to_other_losslessly() {
        let text = r#"
proxies:
  - name: wg-bad-reserved
    type: wireguard
    server: 10.0.0.1
    port: 51820
    private-key: k
    public-key: k2
    reserved: [300, 2, 3]
  - name: hy2-bad-port
    type: hysteria2
    server: 10.0.0.2
    port: not-a-port
    password: p
"#;
        let nodes = parse_profile_yaml(text).expect("parse");
        assert_eq!(nodes.len(), 2);

        // reserved [300, ...] does not fit u8 -> the typed variant fails and
        // the node degrades to Other instead of dropping data.
        let ProxyNode::Other(wg) = &nodes[0] else {
            panic!("expected lossless Other fallback, got {:?}", nodes[0]);
        };
        assert_eq!(wg.type_name, "wireguard");
        assert_eq!(
            wg.fields.get("reserved"),
            Some(&serde_yaml_ng::from_str::<Value>("[300, 2, 3]").expect("reserved value"))
        );
        assert!(wg.fields.contains_key("private-key"));

        let ProxyNode::Other(hy2) = &nodes[1] else {
            panic!("expected lossless Other fallback, got {:?}", nodes[1]);
        };
        assert_eq!(hy2.type_name, "hysteria2");
        assert_eq!(
            hy2.fields.get("port"),
            Some(&Value::String("not-a-port".to_string()))
        );

        assert_proxies_semantic_equivalence(text);
        assert_roundtrip_fixed_point(&nodes);
    }

    #[test]
    fn test_unknown_type_falls_back_to_other() {
        let text = r#"
proxies:
  - name: legacy-ss
    type: ss
    server: 203.0.113.50
    port: 8388
    cipher: aes-256-gcm
    password: ss-pass
    plugin: v2ray-plugin
    fake-field: 7
"#;
        let node = parse_single(text);
        let ProxyNode::Other(other) = &node else {
            panic!("unknown type must degrade to Other, got {node:?}");
        };
        assert_eq!(other.type_name, "ss");
        for key in [
            "name",
            "server",
            "port",
            "cipher",
            "password",
            "plugin",
            "fake-field",
        ] {
            assert!(
                other.fields.contains_key(key),
                "{key} must be kept verbatim"
            );
        }
        assert_eq!(
            other.fields.get("fake-field"),
            Some(&Value::Number(7.into()))
        );
        assert_eq!(node.name(), "legacy-ss");
        assert!(!node.is_typed());

        assert_proxies_semantic_equivalence(text);
        assert_roundtrip_fixed_point(std::slice::from_ref(&node));

        let issues = validate(&node);
        assert!(
            issues.is_empty(),
            "ss node has all common fields: {issues:?}"
        );
    }

    #[test]
    fn test_future_unknown_type_falls_back_to_other() {
        let text = r#"
proxies:
  - name: future-node
    type: quantum-tunnel-v9
    server: 203.0.113.99
    port: 9000
    secret-handshake: open-sesame
"#;
        let node = parse_single(text);
        let ProxyNode::Other(other) = &node else {
            panic!("future protocol must degrade to Other, got {node:?}");
        };
        assert_eq!(other.type_name, "quantum-tunnel-v9");
        assert_eq!(
            other.fields.get("secret-handshake"),
            Some(&Value::String("open-sesame".to_string()))
        );
        assert_proxies_semantic_equivalence(text);
        assert_roundtrip_fixed_point(std::slice::from_ref(&node));
    }

    #[test]
    fn test_full_profile_roundtrip_all_protocols() {
        let nodes = parse_profile_yaml(PROFILE_YAML).expect("parse profile");
        assert_eq!(nodes.len(), 5);
        let type_names: Vec<&str> = nodes.iter().map(ProxyNode::type_name).collect();
        assert_eq!(
            type_names,
            ["vless", "hysteria2", "tuic", "wireguard", "ss"]
        );
        // Exactly one node (the legacy ss) falls back to Other; the rest are
        // strongly typed.
        assert_eq!(nodes.iter().filter(|n| n.is_typed()).count(), 4);

        // typed -> YAML -> typed stability for the whole list.
        assert_roundtrip_fixed_point(&nodes);
        assert_proxies_semantic_equivalence(PROFILE_YAML);

        // Writing nodes back into the original profile must not touch the
        // other sections.
        let updated = replace_proxies_in_profile(PROFILE_YAML, &nodes).expect("write back");
        let doc: Value = serde_yaml_ng::from_str(&updated).expect("updated doc");
        assert_eq!(doc.get("mixed-port").and_then(Value::as_i64), Some(7890));
        let dns = doc.get("dns").expect("dns section kept");
        assert_eq!(
            dns.get("fake-ip-range"),
            Some(&Value::String("198.18.0.1/16".to_string()))
        );
        let rules = doc
            .get("rules")
            .and_then(Value::as_sequence)
            .expect("rules section kept");
        assert_eq!(rules.len(), 2);
        assert_eq!(doc.get("proxies"), Some(&proxies_value(PROFILE_YAML)));

        // A minimal profile built from nodes parses back to the same nodes.
        let minimal = nodes_to_profile_yaml(&nodes).expect("serialize");
        assert!(parse_profile_yaml(&minimal).expect("reparse minimal") == nodes);
    }

    #[test]
    fn test_unknown_field_survives_repeated_roundtrips() {
        let nodes1 = parse_profile_yaml(TUIC_YAML).expect("parse");
        let yaml1 = nodes_to_profile_yaml(&nodes1).expect("serialize 1");
        assert!(yaml1.contains("fake-field: 123"));

        let nodes2 = parse_profile_yaml(&yaml1).expect("re-parse 1");
        let yaml2 = nodes_to_profile_yaml(&nodes2).expect("serialize 2");
        assert_eq!(yaml1, yaml2, "serialization must be a fixed point");
        assert_eq!(nodes1, nodes2);

        let extra = nodes2[0].extra();
        assert!(
            extra.contains_key("fake-field"),
            "unknown field must still be captured after roundtrips"
        );
        assert!(extra.contains_key("reduce-rtt"));
    }

    #[test]
    fn test_parse_rejects_malformed_profiles() {
        // Top-level sequence is not a profile document.
        let err = parse_profile_yaml("- a\n- b\n").expect_err("must reject");
        assert!(err.to_string().contains("mapping"), "{err}");

        // `proxies` that is not a list.
        let err = parse_profile_yaml("proxies: oops\n").expect_err("must reject");
        assert!(err.to_string().contains("proxies"), "{err}");

        // An entry without a string `type` key.
        let err = parse_profile_yaml("proxies:\n  - name: n0\n    server: 1.2.3.4\n    port: 1\n")
            .expect_err("must reject");
        assert!(err.to_string().contains("proxies[0]"), "{err}");

        // A scalar entry.
        let err = parse_profile_yaml("proxies:\n  - just-a-string\n").expect_err("must reject");
        assert!(err.to_string().contains("proxies[0]"), "{err}");
    }

    #[test]
    fn test_profile_without_proxies_is_empty() {
        assert!(
            parse_profile_yaml("port: 7890\n")
                .expect("parse")
                .is_empty()
        );
        // Explicitly null proxies is treated as empty, not an error.
        assert!(parse_profile_yaml("proxies:\n").expect("parse").is_empty());
    }

    #[test]
    fn test_validate_flags_missing_fields() {
        let text = r#"
proxies:
  - name: vless-broken
    type: vless
    server: 10.4.4.4
    port: 443
  - name: hy2-broken
    type: hysteria2
    server: 10.2.2.2
    port: 443
    obfs: salamander
  - name: tuic-broken
    type: tuic
    server: 10.1.1.1
    port: 443
    congestion-controller: reno
    udp-relay-mode: tcp
  - name: wg-broken
    type: wireguard
    server: 10.3.3.3
    port: 51820
    private-key: k
    public-key: k2
    ip: not-an-ip
  - name: mystery
    type: quantum-tunnel
    port: 7000
"#;
        let nodes = parse_profile_yaml(text).expect("parse");
        assert_eq!(nodes.len(), 5);

        let issues = validate(&nodes[0]);
        assert!(
            issues.iter().any(|m| m.contains("uuid")),
            "vless without uuid must be reported: {issues:?}"
        );

        let issues = validate(&nodes[1]);
        assert!(
            issues.iter().any(|m| m.contains("password"))
                && issues.iter().any(|m| m.contains("obfs-password")),
            "hysteria2 salamander without obfs-password must be reported: {issues:?}"
        );

        let issues = validate(&nodes[2]);
        assert!(
            issues.iter().any(|m| m.contains("uuid"))
                && issues.iter().any(|m| m.contains("password"))
                && issues.iter().any(|m| m.contains("congestion-controller"))
                && issues.iter().any(|m| m.contains("udp-relay-mode")),
            "tuic problems must be reported: {issues:?}"
        );

        let issues = validate(&nodes[3]);
        // `ip` is present but malformed, so only the parse error fires; the
        // "ip or ipv6 is required" hint is reserved for nodes missing both.
        assert!(
            !issues.iter().any(|m| m.contains("ip or ipv6"))
                && issues.iter().any(|m| m.contains("not a valid IP")),
            "wireguard address problems must be reported: {issues:?}"
        );

        // A wireguard node without any address reports the missing-address hint.
        let no_addr = r#"
proxies:
  - name: wg-no-address
    type: wireguard
    server: 10.3.3.3
    port: 51820
    private-key: k
    public-key: k2
"#;
        let no_addr_nodes = parse_profile_yaml(no_addr).expect("parse");
        let issues = validate(&no_addr_nodes[0]);
        assert!(
            issues.iter().any(|m| m.contains("ip or ipv6")),
            "wireguard without addresses must be reported: {issues:?}"
        );

        let issues = validate(&nodes[4]);
        assert!(
            issues.iter().any(|m| m.contains("server")),
            "untyped node without server must be reported: {issues:?}"
        );
    }

    #[test]
    fn test_validate_clean_for_full_nodes() {
        for text in [
            VLESS_YAML,
            HYSTERIA2_YAML,
            TUIC_YAML,
            WIREGUARD_YAML,
            WIREGUARD_RESERVED_BASE64_YAML,
        ] {
            let node = parse_single(text);
            let issues = validate(&node);
            assert!(
                issues.is_empty(),
                "fully populated node must validate clean: {issues:?}"
            );
        }
    }

    #[test]
    fn test_typed_common_accessors() {
        let nodes = parse_profile_yaml(PROFILE_YAML).expect("parse");
        assert_eq!(nodes[0].name(), "vless-reality-vision");
        assert_eq!(nodes[0].server(), Some("203.0.113.10"));
        assert_eq!(nodes[0].common().map(|c| c.port), Some(443));
        assert_eq!(nodes[4].name(), "legacy-ss");
        assert_eq!(nodes[4].server(), Some("203.0.113.50"));
        assert!(nodes[4].common().is_none());
    }
}

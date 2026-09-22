//! DUAL-05 typed protocol parameter blocks, part 2 (05-04…05-08).
//!
//! Split from [`crate::protocol_params`] to respect the business-file line
//! budget; the aggregate [`crate::protocol_params::ProtocolParams`] re-uses
//! these blocks and keeps the validation/report logic in one place.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::protocol_fidelity::{ProtocolFamily, ProtocolIssue, ShadowsocksCipher};
use crate::protocol_params::{KNOWN_SIP003_PLUGINS, KNOWN_TRANSPORT_NETWORKS, XHTTP_MODES};

pub(crate) fn push(issues: &mut Vec<ProtocolIssue>, field: &str, message: impl Into<String>) {
    issues.push(ProtocolIssue::new(field, message));
}

pub(crate) fn note(notes: &mut Vec<String>, message: impl Into<String>) {
    notes.push(message.into());
}

pub(crate) fn is_base64(value: &str) -> bool {
    !value.is_empty()
        && value.len().is_multiple_of(4)
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '-' | '_'))
}

pub(crate) fn is_positive_number_or_bandwidth(value: &str) -> bool {
    let first = value.trim().chars().next();
    matches!(first, Some(c) if c.is_ascii_digit() || c == '.')
}

/// DUAL-05-13: a SHA-256 certificate fingerprint (`fingerprint:`), accepting
/// the openssl spelling (`AA:BB:...`) and an optional `sha256:` prefix.
pub(crate) fn is_valid_sha256_fingerprint(value: &str) -> bool {
    let trimmed = value.trim();
    let trimmed = trimmed
        .strip_prefix("sha256:")
        .or_else(|| trimmed.strip_prefix("SHA256:"))
        .unwrap_or(trimmed);
    let compact: String = trimmed
        .chars()
        .filter(|c| !matches!(c, ':' | ' ' | '\t'))
        .collect();
    compact.len() == 64 && compact.chars().all(|c| c.is_ascii_hexdigit())
}

/// AmneziaWG obfuscation block (`amnezia-wg-option`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AmneziaWgParams {
    pub jc: Option<u8>,
    pub jmin: Option<u16>,
    pub jmax: Option<u16>,
    pub s1: Option<u16>,
    pub s2: Option<u16>,
    pub h1: Option<u32>,
    pub h2: Option<u32>,
    pub h3: Option<u32>,
    pub h4: Option<u32>,
}

impl AmneziaWgParams {
    pub fn is_present(&self) -> bool {
        self != &Self::default()
    }

    pub fn validate(&self, issues: &mut Vec<ProtocolIssue>) {
        if let Some(jc) = self.jc
            && jc > 128
        {
            push(issues, "wireguard.jc", "AmneziaWG `jc` must not exceed 128");
        }
        if let (Some(jmin), Some(jmax)) = (self.jmin, self.jmax)
            && jmin > jmax
        {
            push(issues, "wireguard.jmin", "jmin must not exceed jmax");
        }
        let headers: Vec<u32> = [self.h1, self.h2, self.h3, self.h4]
            .into_iter()
            .flatten()
            .collect();
        let mut sorted = headers.clone();
        sorted.sort_unstable();
        sorted.dedup();
        if sorted.len() != headers.len() {
            push(
                issues,
                "wireguard.h1",
                "AmneziaWG magic headers h1..h4 must differ from each other",
            );
        }
    }

    pub fn chips(&self) -> Vec<String> {
        let mut chips = Vec::new();
        if let Some(jc) = self.jc {
            chips.push(format!("awg:jc={jc}"));
        }
        if let Some(jmin) = self.jmin {
            chips.push(format!("awg:jmin={jmin}"));
        }
        if let Some(jmax) = self.jmax {
            chips.push(format!("awg:jmax={jmax}"));
        }
        for (name, value) in [("s1", self.s1), ("s2", self.s2)] {
            if let Some(value) = value {
                chips.push(format!("awg:{name}={value}"));
            }
        }
        for (name, value) in [
            ("h1", self.h1),
            ("h2", self.h2),
            ("h3", self.h3),
            ("h4", self.h4),
        ] {
            if let Some(value) = value {
                chips.push(format!("awg:{name}={value}"));
            }
        }
        chips
    }
}

/// DUAL-05-04: WireGuard / AmneziaWG client parameters.
///
/// `reserved` keeps its authored shape: mihomo accepts a byte list and a
/// base64 string and re-emits whichever it parsed, so the draft stores the
/// exact text plus the shape flag instead of normalising one into the other.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WireGuardParams {
    pub private_key: String,
    pub public_key: String,
    /// `pre-shared-key`.
    pub pre_shared_key: String,
    /// `reserved` as authored (comma list `1,2,3` or base64).
    pub reserved: String,
    pub reserved_is_base64: bool,
    pub ip: String,
    pub ipv6: String,
    /// `0` means the core default.
    pub mtu: u32,
    pub dns: Vec<String>,
    pub workers: u32,
    pub persistent_keepalive: u32,
    pub allowed_ips: Vec<String>,
    pub remote_dns_resolve: bool,
    pub amnezia: AmneziaWgParams,
}

impl WireGuardParams {
    pub fn is_present(&self) -> bool {
        self != &Self::default()
    }

    /// The three base64 keys WireGuard expects to be 32 bytes.
    fn validate_key(&self, field: &str, value: &str, issues: &mut Vec<ProtocolIssue>) {
        let value = value.trim();
        if value.is_empty() {
            return;
        }
        if !is_base64(value) {
            push(
                issues,
                field,
                "WireGuard keys must be standard base64 (43-44 chars)",
            );
            return;
        }
        let padding = value.chars().filter(|c| *c == '=').count();
        let decoded = (value.len() * 3 / 4).saturating_sub(padding);
        if decoded != 32 {
            push(issues, field, "WireGuard keys must decode to 32 bytes");
        }
    }

    pub fn validate(&self, issues: &mut Vec<ProtocolIssue>) {
        self.validate_key("wireguard.private-key", &self.private_key, issues);
        self.validate_key("wireguard.public-key", &self.public_key, issues);
        self.validate_key("wireguard.pre-shared-key", &self.pre_shared_key, issues);
        if self.ip.trim().is_empty() && self.ipv6.trim().is_empty() {
            push(
                issues,
                "wireguard.ip",
                "WireGuard requires an interface address (`ip` or `ipv6`)",
            );
        }
        let reserved = self.reserved.trim();
        if !reserved.is_empty() && !self.reserved_is_base64 {
            let bytes: Vec<u16> = reserved
                .split(',')
                .filter_map(|part| part.trim().parse::<u16>().ok())
                .collect();
            if bytes.len() != 3 || bytes.iter().any(|byte| *byte > 255) {
                push(
                    issues,
                    "wireguard.reserved",
                    "reserved must be three bytes (`1,2,3`) or a base64 string",
                );
            }
        }
        if self.reserved_is_base64 && !reserved.is_empty() && !is_base64(reserved) {
            push(
                issues,
                "wireguard.reserved",
                "reserved is marked base64 but is not valid base64",
            );
        }
        self.amnezia.validate(issues);
    }

    pub fn chips(&self) -> Vec<String> {
        let mut chips = Vec::new();
        if !self.private_key.trim().is_empty() {
            chips.push("wg:key".to_string());
        }
        if !self.public_key.trim().is_empty() {
            chips.push("wg:peer-key".to_string());
        }
        if !self.pre_shared_key.trim().is_empty() {
            chips.push("wg:psk".to_string());
        }
        if !self.reserved.trim().is_empty() {
            chips.push(format!(
                "wg:reserved({})",
                if self.reserved_is_base64 {
                    "base64"
                } else {
                    "list"
                }
            ));
        }
        if !self.ip.trim().is_empty() {
            chips.push(format!("ip:{}", self.ip.trim()));
        }
        if self.mtu > 0 {
            chips.push(format!("mtu:{}", self.mtu));
        }
        if self.persistent_keepalive > 0 {
            chips.push(format!("keepalive:{}s", self.persistent_keepalive));
        }
        if self.remote_dns_resolve {
            chips.push("remote-dns".to_string());
        }
        chips.extend(self.amnezia.chips());
        chips
    }
}

/// DUAL-05-05: WebSocket transport options (`ws-opts`), including 0-RTT early data.
///
/// mihomo's `WSOptions.Headers` is an exact `map[string]string`, so the draft
/// carries the whole map instead of a single `host` slot; that keeps every
/// custom header lossless while `host()`/`set_host()` expose the common case.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WsOptsParams {
    pub path: String,
    pub headers: BTreeMap<String, String>,
    /// `max-early-data`: bytes of 0-RTT payload the client sends with the upgrade.
    pub max_early_data: u64,
    pub early_data_header_name: String,
    pub v2ray_http_upgrade: bool,
    pub v2ray_http_upgrade_fast_open: bool,
}

impl WsOptsParams {
    pub fn is_present(&self) -> bool {
        self != &Self::default()
    }

    pub fn host(&self) -> String {
        self.headers.get("Host").cloned().unwrap_or_default()
    }

    pub fn set_host(&mut self, host: impl Into<String>) {
        let host = host.into();
        if host.trim().is_empty() {
            self.headers.remove("Host");
        } else {
            self.headers
                .insert("Host".to_string(), host.trim().to_string());
        }
    }

    pub fn uses_early_data(&self) -> bool {
        self.max_early_data > 0
    }

    pub fn chips(&self) -> Vec<String> {
        let mut chips = Vec::new();
        if !self.path.trim().is_empty() {
            chips.push(format!("path:{}", self.path.trim()));
        }
        let host = self.host();
        if !host.is_empty() {
            chips.push(format!("host:{host}"));
        }
        if self.max_early_data > 0 {
            chips.push(format!("ws-0rtt:{}", self.max_early_data));
        }
        if self.v2ray_http_upgrade {
            chips.push("http-upgrade".to_string());
        }
        chips
    }
}

/// gRPC transport options (`grpc-opts`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GrpcOptsParams {
    pub service_name: String,
}

impl GrpcOptsParams {
    pub fn is_present(&self) -> bool {
        !self.service_name.trim().is_empty()
    }

    pub fn chips(&self) -> Vec<String> {
        if self.service_name.trim().is_empty() {
            Vec::new()
        } else {
            vec![format!("grpc:{}", self.service_name.trim())]
        }
    }
}

/// HTTP/2 transport options (`h2-opts`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct H2OptsParams {
    /// `host:` is a list in mihomo's `HTTP2Options`.
    pub hosts: Vec<String>,
    pub path: String,
}

impl H2OptsParams {
    pub fn is_present(&self) -> bool {
        self != &Self::default()
    }

    pub fn chips(&self) -> Vec<String> {
        let mut chips = Vec::new();
        if !self.hosts.is_empty() {
            chips.push(format!("h2-host:{}", self.hosts.join(",")));
        }
        if !self.path.trim().is_empty() {
            chips.push(format!("h2-path:{}", self.path.trim()));
        }
        chips
    }
}

/// HTTP transport options (`http-opts`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HttpOptsParams {
    pub method: String,
    /// `path:` is a list in mihomo's `HTTPOptions`.
    pub paths: Vec<String>,
    pub headers: BTreeMap<String, Vec<String>>,
}

impl HttpOptsParams {
    pub fn is_present(&self) -> bool {
        self != &Self::default()
    }

    pub fn chips(&self) -> Vec<String> {
        let mut chips = Vec::new();
        if !self.method.trim().is_empty() {
            chips.push(format!("http:{}", self.method.trim()));
        }
        if !self.paths.is_empty() {
            chips.push(format!("http-path:{}", self.paths.join(",")));
        }
        chips
    }
}

/// XHTTP / splithttp transport options (`xhttp-opts`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct XhttpOptsParams {
    pub mode: String,
    pub path: String,
    pub host: String,
}

impl XhttpOptsParams {
    pub fn is_present(&self) -> bool {
        self != &Self::default()
    }

    pub fn mode_known(&self) -> bool {
        let mode = self.mode.trim();
        mode.is_empty() || XHTTP_MODES.contains(&mode)
    }

    pub fn chips(&self) -> Vec<String> {
        let mut chips = Vec::new();
        if self.is_present() {
            chips.push("xhttp".to_string());
        }
        if !self.mode.trim().is_empty() {
            chips.push(format!("xhttp-mode:{}", self.mode.trim()));
        }
        if !self.path.trim().is_empty() {
            chips.push(format!("xhttp-path:{}", self.path.trim()));
        }
        chips
    }
}

/// DUAL-05-05: transport selection plus every typed transport sub-block.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TransportParams {
    /// `network:` — `tcp` / `ws` / `grpc` / `h2` / `http` / `xhttp`.
    pub network: String,
    /// `packet-encoding` — `packetaddr` / `packet` / `xudp`.
    pub packet_encoding: String,
    pub ws: WsOptsParams,
    pub grpc: GrpcOptsParams,
    pub h2: H2OptsParams,
    pub http: HttpOptsParams,
    pub xhttp: XhttpOptsParams,
}

impl TransportParams {
    pub fn is_present(&self) -> bool {
        self != &Self::default()
    }

    /// Families whose node schema carries `network`/`*-opts` in v1.19.18.
    pub const fn supported_by(family: ProtocolFamily) -> bool {
        matches!(
            family,
            ProtocolFamily::Vless | ProtocolFamily::Trojan | ProtocolFamily::Vmess
        )
    }

    pub fn network_known(&self) -> bool {
        let network = self.network.trim();
        network.is_empty() || KNOWN_TRANSPORT_NETWORKS.contains(&network)
    }

    pub fn validate(&self, issues: &mut Vec<ProtocolIssue>) {
        if self.ws.uses_early_data() && self.ws.path.trim().is_empty() {
            push(
                issues,
                "transport.ws.max-early-data",
                "WebSocket early data needs a websocket path",
            );
        }
        if self.xhttp.is_present() && !self.xhttp.mode_known() {
            push(
                issues,
                "transport.xhttp.mode",
                "xhttp mode must be auto / packet-up / stream-up / stream-one",
            );
        }
        if self.network.trim() == "xhttp" && !self.xhttp.is_present() {
            push(
                issues,
                "transport.xhttp",
                "network `xhttp` needs an xhttp-opts block with at least a path or mode",
            );
        }
    }

    pub fn chips(&self) -> Vec<String> {
        let mut chips = Vec::new();
        let network = self.network.trim();
        if !network.is_empty() {
            chips.push(format!("net:{network}"));
        }
        if !self.packet_encoding.trim().is_empty() {
            chips.push(format!("packet:{}", self.packet_encoding.trim()));
        }
        chips.extend(self.ws.chips());
        chips.extend(self.grpc.chips());
        chips.extend(self.h2.chips());
        chips.extend(self.http.chips());
        chips.extend(self.xhttp.chips());
        chips
    }

    pub fn notes(&self, notes: &mut Vec<String>) {
        let network = self.network.trim();
        if network == "quic" {
            note(
                notes,
                "the pinned mihomo v1.19.18 has no `quic` transport network; this node falls back to plain TCP while the key is preserved",
            );
        } else if !network.is_empty() && !self.network_known() {
            note(
                notes,
                format!(
                    "network `{network}` is not a v1.19.18 transport; the core falls back to TCP"
                ),
            );
        }
        if self.xhttp.is_present() {
            note(
                notes,
                "xhttp-opts is typed and preserved, but the pinned mihomo v1.19.18 has no xhttp transport",
            );
        }
    }
}

/// One SIP003 plugin option value; mihomo's `plugin-opts` is a weakly typed map.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PluginOptValue {
    Bool(bool),
    Number(i64),
    Text(String),
}

impl PluginOptValue {
    pub fn as_text(&self) -> String {
        match self {
            Self::Bool(value) => value.to_string(),
            Self::Number(value) => value.to_string(),
            Self::Text(value) => value.clone(),
        }
    }
}

/// DUAL-05-06: the SIP003 `plugin` + `plugin-opts` pair.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sip003Plugin {
    /// `plugin:` — obfs / v2ray-plugin / gost-plugin / shadow-tls / restls-plugin / kcptun.
    pub name: String,
    pub opts: BTreeMap<String, PluginOptValue>,
}

impl Sip003Plugin {
    pub fn is_present(&self) -> bool {
        !self.name.trim().is_empty() || !self.opts.is_empty()
    }

    pub fn known(&self) -> bool {
        KNOWN_SIP003_PLUGINS.contains(&self.name.trim())
    }

    pub fn opt(&self, key: &str) -> Option<String> {
        self.opts.get(key).map(PluginOptValue::as_text)
    }

    pub fn validate(&self, issues: &mut Vec<ProtocolIssue>) {
        let name = self.name.trim();
        if name.is_empty() {
            if !self.opts.is_empty() {
                push(
                    issues,
                    "plugin.name",
                    "plugin-opts is set without a SIP003 plugin name",
                );
            }
            return;
        }
        let require = |key: &str, field: &str, issues: &mut Vec<ProtocolIssue>| {
            let missing = self
                .opts
                .get(key)
                .map(PluginOptValue::as_text)
                .is_none_or(|value| value.trim().is_empty());
            if missing {
                push(
                    issues,
                    field,
                    format!("plugin `{name}` requires the `{key}` option"),
                );
            }
        };
        match name {
            "obfs" => {
                let mode = self.opt("mode").unwrap_or_default();
                if mode != "tls" && mode != "http" {
                    push(issues, "plugin.mode", "obfs mode must be `tls` or `http`");
                }
            }
            "v2ray-plugin" => require("mode", "plugin.mode", issues),
            "shadow-tls" => {
                require("host", "plugin.host", issues);
                require("password", "plugin.password", issues);
            }
            "restls-plugin" => {
                require("host", "plugin.host", issues);
                require("password", "plugin.password", issues);
            }
            "kcptun" => require("key", "plugin.key", issues),
            _ => {}
        }
    }

    pub fn chips(&self) -> Vec<String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Vec::new();
        }
        let mut chips = vec![format!("plugin:{name}")];
        for (key, value) in &self.opts {
            chips.push(format!("plugin-opt:{key}={}", value.as_text()));
        }
        chips
    }

    pub fn notes(&self, notes: &mut Vec<String>) {
        let name = self.name.trim();
        if !name.is_empty() && !self.known() {
            note(
                notes,
                format!(
                    "SIP003 plugin `{name}` is not implemented by the pinned mihomo v1.19.18; the key is preserved but the core will not use it"
                ),
            );
        }
    }
}

/// DUAL-05-07: native SSH proxy parameters (`adapter/outbound/ssh.go`).
///
/// The password lives in [`ProtocolDraft::password`]; this block carries the
/// remaining typed slots.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SshParams {
    pub username: String,
    pub private_key: String,
    pub passphrase: String,
    pub host_key_algorithms: Vec<String>,
}

impl SshParams {
    pub fn is_present(&self) -> bool {
        self != &Self::default()
    }

    pub fn validate(&self, password: &str, issues: &mut Vec<ProtocolIssue>) {
        if self.username.trim().is_empty() {
            push(issues, "ssh.username", "SSH requires a username");
        }
        if self.private_key.trim().is_empty() && password.trim().is_empty() {
            push(
                issues,
                "ssh.auth",
                "SSH requires either a password or a private key",
            );
        }
        if !self.passphrase.trim().is_empty() && self.private_key.trim().is_empty() {
            push(
                issues,
                "ssh.passphrase",
                "a private-key passphrase needs a private key",
            );
        }
        if self
            .host_key_algorithms
            .iter()
            .any(|entry| entry.trim().is_empty())
        {
            push(
                issues,
                "ssh.host-key-algorithms",
                "host-key algorithms must not contain empty entries",
            );
        }
    }

    pub fn chips(&self) -> Vec<String> {
        let mut chips = Vec::new();
        if !self.username.trim().is_empty() {
            chips.push(format!("ssh:{}", self.username.trim()));
        }
        if !self.private_key.trim().is_empty() {
            chips.push("ssh:key".to_string());
        }
        if !self.host_key_algorithms.is_empty() {
            chips.push(format!("ssh-algs:{}", self.host_key_algorithms.len()));
        }
        chips
    }
}

/// DUAL-05-08: AnyTLS session reuse parameters (`adapter/outbound/anytls.go`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnyTlsParams {
    /// `idle-session-timeout` in milliseconds; the core default is 30000.
    pub idle_session_timeout: u64,
    /// `idle-session-check-interval` in milliseconds.
    pub idle_session_check_interval: u64,
    /// `min-idle-session`: how many idle sessions to keep alive.
    pub min_idle_session: u64,
}

impl AnyTlsParams {
    pub fn is_present(&self) -> bool {
        self != &Self::default()
    }

    pub fn chips(&self) -> Vec<String> {
        let mut chips = Vec::new();
        if self.idle_session_timeout > 0 {
            chips.push(format!("idle:{}ms", self.idle_session_timeout));
        }
        if self.idle_session_check_interval > 0 {
            chips.push(format!("idle-check:{}ms", self.idle_session_check_interval));
        }
        if self.min_idle_session > 0 {
            chips.push(format!("min-idle:{}", self.min_idle_session));
        }
        chips
    }
}

/// DUAL-05-08: trojan-go's Shadowsocks multiplexing, which mihomo exposes as
/// `trojan.ss-opts` (`adapter/outbound/trojan.go`). The pinned core has no
/// separate `trojan-go` node type; this block is how the feature is expressed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrojanSsParams {
    pub enabled: bool,
    pub method: String,
    pub password: String,
}

impl TrojanSsParams {
    pub fn is_present(&self) -> bool {
        self != &Self::default()
    }

    pub fn validate(&self, issues: &mut Vec<ProtocolIssue>) {
        if !self.enabled {
            if self != &Self::default() {
                push(
                    issues,
                    "trojan.ss-opts",
                    "ss-opts is configured while ss-opts.enabled is off; the core ignores it",
                );
            }
            return;
        }
        if ShadowsocksCipher::parse(&self.method).is_none() {
            push(
                issues,
                "trojan.ss-opts.method",
                "ss-opts.method must be a mihomo cipher name",
            );
        }
        if self.password.trim().is_empty() {
            push(
                issues,
                "trojan.ss-opts.password",
                "ss-opts requires a password",
            );
        }
    }

    pub fn chips(&self) -> Vec<String> {
        if !self.enabled {
            return Vec::new();
        }
        let mut chips = vec!["trojan-go:ss".to_string()];
        if !self.method.trim().is_empty() {
            chips.push(format!("ss-opts:{}", self.method.trim()));
        }
        chips
    }
}

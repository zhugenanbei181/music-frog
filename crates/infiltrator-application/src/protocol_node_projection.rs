//! DUAL-05 draft ⇄ profile-node projection.
//!
//! One shared mapping between the typed [`ProtocolDraft`] the application and
//! both surfaces edit and the flat [`ProxyNodeItem`] the domain codec reads and
//! writes. This module owns:
//!
//! * the *scalar* draft-owned key list (`DRAFT_OWNED_KEYS`) so the profile
//!   splice replaces exactly what the draft owns and preserves the rest;
//! * the *map-valued* draft-owned keys (`DRAFT_NESTED_KEYS`) whose unknown
//!   sub-keys the splice merges instead of dropping;
//! * the typed-extra key list (`TYPED_EXTRA_KEYS`) that the draft now models,
//!   so unknown-field audits stay honest;
//! * typed parameter blocks (05-03…05-12) reading and writing the mihomo
//!   v1.19.18 wire keys below.
//!
//! Keys that the repo's flat DTO previously spelled differently from mihomo
//! (`amnezia-opts` vs `amnezia-wg-option`, `preshared-key` vs
//! `pre-shared-key`, `idle-timeout` vs `idle-session-timeout`) now serialize
//! the mihomo spelling with the old spelling accepted as a serde alias.

use infiltrator_contract::protocol_fidelity::{ProtocolDraft, ProtocolFamily};
use infiltrator_contract::protocol_params::EchParams;
use infiltrator_contract::protocol_params_ext::TransportParams;
use infiltrator_domain::profile_converter::ProxyNodeItem;

/// Scalar keys every draft owns regardless of family.
pub const DRAFT_OWNED_KEYS: [&str; 15] = [
    "name",
    "type",
    "server",
    "port",
    "password",
    "uuid",
    "cipher",
    "flow",
    "servername",
    "sni",
    "tls",
    "skip-cert-verify",
    "alpn",
    "client-fingerprint",
    "smux",
];

/// Every key the draft owns for one family: the base scalars plus the family's
/// typed block keys. A key owned here is replaced (or removed) by the draft;
/// everything else an existing node carries stays verbatim.
pub fn owned_keys(family: ProtocolFamily) -> Vec<&'static str> {
    let mut keys: Vec<&'static str> = DRAFT_OWNED_KEYS.to_vec();
    if EchParams::supported_by(family) {
        keys.push("ech-opts");
    }
    match family {
        ProtocolFamily::Tuic => keys.extend([
            "congestion-controller",
            "udp-relay-mode",
            "reduce-rtt",
            "heartbeat-interval",
            "request-timeout",
            "disable-sni",
            "recv-window-conn",
            "recv-window",
        ]),
        ProtocolFamily::Hysteria2 => keys.extend([
            "ports",
            "hop-interval",
            "obfs",
            "obfs-password",
            "up",
            "down",
            "cwnd",
            "udp-mtu",
        ]),
        ProtocolFamily::WireGuard => keys.extend([
            "private-key",
            "public-key",
            "pre-shared-key",
            "reserved",
            "ip",
            "ipv6",
            "mtu",
            "remote-dns-resolve",
            "workers",
            "persistent-keepalive",
            "allowed-ips",
            "amnezia-wg-option",
            "dns",
        ]),
        ProtocolFamily::Shadowsocks => keys.extend(["plugin", "plugin-opts"]),
        ProtocolFamily::Ssh => keys.extend([
            "username",
            "private-key",
            "private-key-passphrase",
            "host-key-algorithms",
        ]),
        ProtocolFamily::Anytls => keys.extend([
            "idle-session-timeout",
            "idle-session-check-interval",
            "min-idle-session",
        ]),
        ProtocolFamily::Trojan => keys.push("ss-opts"),
        _ => {}
    }
    if TransportParams::supported_by(family) {
        keys.extend([
            "network",
            "packet-encoding",
            "ws-opts",
            "grpc-opts",
            "h2-opts",
            "http-opts",
            "xhttp-opts",
        ]);
    }
    keys
}

/// Map-valued draft-owned keys the splice merges key-by-key.
pub const DRAFT_NESTED_KEYS: [&str; 10] = [
    "reality-opts",
    "smux",
    "ech-opts",
    "ss-opts",
    "plugin-opts",
    "ws-opts",
    "grpc-opts",
    "h2-opts",
    "http-opts",
    "xhttp-opts",
];

/// Keys the draft now types, so an unknown-field audit must not call them unknown.
pub const TYPED_EXTRA_KEYS: [&str; 8] = [
    "ech-opts",
    "ss-opts",
    "udp-mtu",
    "dns",
    "xhttp-opts",
    "idle-session-timeout",
    "idle-session-check-interval",
    "min-idle-session",
];

/// `true` when the key is modelled by a typed block *for this family* (audits
/// skip it; a foreign family's key stays an honestly preserved unknown).
pub fn is_typed_extra_key_for(key: &str, family: ProtocolFamily) -> bool {
    if key == "plugin-opts" {
        return family == ProtocolFamily::Shadowsocks;
    }
    if !TYPED_EXTRA_KEYS.contains(&key) {
        return false;
    }
    match key {
        "ech-opts" => EchParams::supported_by(family),
        "ss-opts" => family == ProtocolFamily::Trojan,
        "udp-mtu" => family == ProtocolFamily::Hysteria2,
        "dns" => family == ProtocolFamily::WireGuard,
        "xhttp-opts" => TransportParams::supported_by(family),
        "idle-session-timeout" | "idle-session-check-interval" | "min-idle-session" => {
            family == ProtocolFamily::Anytls
        }
        _ => false,
    }
}

/// `true` when the key is modelled by some typed block (any family).
pub fn is_typed_extra_key(key: &str) -> bool {
    TYPED_EXTRA_KEYS.contains(&key) || key == "plugin-opts"
}
/// DUAL-05: project a parsed profile node into the shared draft.
/// `spider-x` lives on the flat field when a URI parse set it and inside
/// `reality-opts` when a profile node carried it; read both shapes.
fn effective_spider_x(item: &ProxyNodeItem) -> String {
    if let Some(spider_x) = item.spider_x.as_deref()
        && !spider_x.trim().is_empty()
    {
        return spider_x.to_string();
    }
    item.reality_opts
        .as_ref()
        .and_then(|value| value.get("spider-x"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string()
}

pub fn draft_from_node(item: &ProxyNodeItem) -> ProtocolDraft {
    let family =
        infiltrator_contract::protocol_fidelity::ProtocolFamily::from_type_str(&item.node_type);
    let mut draft = ProtocolDraft::new(item.node_type.clone());
    draft.name = item.name.clone();
    draft.server = item.server.clone();
    draft.port = item.port;
    draft.password = item
        .password
        .clone()
        .or_else(|| item.auth.clone())
        .unwrap_or_default();
    draft.uuid = item.uuid.clone().unwrap_or_default();
    draft.sni = item
        .get_effective_sni()
        .map(str::to_string)
        .unwrap_or_default();
    draft.cipher = item.cipher.clone().unwrap_or_default();
    draft.flow = item.flow.clone().unwrap_or_default();
    // `public_key` is shared by REALITY and WireGuard in the flat DTO; only the
    // families whose schema carries a REALITY block may read it as such.
    draft.reality = if family.supports_reality() {
        infiltrator_contract::protocol_fidelity::RealityParams {
            public_key: item
                .get_effective_public_key()
                .map(str::to_string)
                .unwrap_or_default(),
            short_id: item
                .get_effective_short_id()
                .map(str::to_string)
                .unwrap_or_default(),
            spider_x: effective_spider_x(item),
            fingerprint: item.client_fingerprint.clone().unwrap_or_default(),
        }
    } else {
        infiltrator_contract::protocol_fidelity::RealityParams::default()
    };
    draft.smux = smux_from_json(item.smux.as_ref());
    draft.tls = item.tls;
    draft.skip_cert_verify = item.skip_cert_verify.unwrap_or(false);
    draft.alpn = item.alpn.clone().unwrap_or_default();
    draft.params = crate::protocol_node_params::params_from_node(item, family);
    let mut preserved: Vec<String> = item
        .extra
        .keys()
        .filter(|key| !is_typed_extra_key_for(key, family))
        .cloned()
        .collect();
    preserved.sort();
    draft.preserved_fields = preserved;
    draft
}

fn smux_from_json(
    value: Option<&serde_json::Value>,
) -> infiltrator_contract::protocol_fidelity::SmuxParams {
    use infiltrator_contract::protocol_fidelity::SmuxParams;
    let Some(object) = value.and_then(serde_json::Value::as_object) else {
        return SmuxParams::default();
    };
    let text = |key: &str| {
        object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    };
    let number = |key: &str| {
        object
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .map(|value| value as u32)
    };
    let flag = |key: &str| {
        object
            .get(key)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    };
    let mut params = SmuxParams::default();
    if let Some(protocol) = text("protocol") {
        params.protocol = protocol;
    }
    params.enabled = flag("enabled");
    if let Some(value) = number("max-connections") {
        params.max_connections = value;
    }
    if let Some(value) = number("min-streams") {
        params.min_streams = value;
    }
    if let Some(value) = number("max-streams") {
        params.max_streams = value;
    }
    params.padding = flag("padding");
    params.statistic = flag("statistic");
    params.only_tcp = flag("only-tcp");
    params
}

pub(crate) fn smux_to_json(
    params: &infiltrator_contract::protocol_fidelity::SmuxParams,
) -> serde_json::Value {
    serde_json::json!({
        "enabled": params.enabled,
        "protocol": params.protocol.trim(),
        "max-connections": params.max_connections,
        "min-streams": params.min_streams,
        "max-streams": params.max_streams,
        "padding": params.padding,
        "statistic": params.statistic,
        "only-tcp": params.only_tcp,
    })
}

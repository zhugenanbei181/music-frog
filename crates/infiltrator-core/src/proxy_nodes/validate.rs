//! Lightweight validation rules for parsed proxy nodes.
//!
//! One human-readable message per detected problem (missing required fields,
//! empty values, unknown enum-like values), advisory only — validation never
//! mutates the node and never rejects it.

use std::net::IpAddr;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_yaml_ng::Value;

use super::model::{
    CommonFields, PortHopping, ProxyNode, RawNode, Reserved,
};

const TUIC_CONGESTION_CONTROLLERS: [&str; 3] = ["bbr", "cubic", "new-reno"];
const TUIC_UDP_RELAY_MODES: [&str; 2] = ["native", "quic"];
const VLESS_ALLOWED_FLOWS: [&str; 2] = ["xtls-rprx-vision", "xtls-rprx-vision-udp443"];
const VMESS_ALLOWED_CIPHERS: [&str; 5] = [
    "auto",
    "aes-128-gcm",
    "chacha20-poly1305",
    "none",
    "zero",
];
const XHTTP_ALLOWED_MODES: [&str; 4] = ["auto", "stream-up", "stream-down", "packet-up"];

/// Lightweight validation: returns one human-readable message per detected
/// problem (missing required fields, empty values, unknown enum-like
/// values). An empty vec means no obvious problem; this is advisory only and
/// never mutates the node.
pub fn validate(node: &RawNode) -> Vec<String> {
    let mut issues = Vec::new();
    match node {
        ProxyNode::Vless(node) => {
            validate_common(&node.common, &mut issues);
            let has_uuid = node
                .uuid
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .is_some()
                || match node.extra.get("uuid") {
                    Some(Value::String(uuid)) if !uuid.trim().is_empty() => true,
                    _ => false,
                };
            if !has_uuid {
                issues.push("vless: uuid is required".to_string());
            }
            if let Some(flow) = node.flow.as_deref()
                && !VLESS_ALLOWED_FLOWS.contains(&flow.trim())
            {
                issues.push(format!(
                    "vless: unknown flow {flow:?} (expected one of {VLESS_ALLOWED_FLOWS:?})"
                ));
            }
            if let Some(ref reality) = node.reality_opts {
                if let Some(ref pk) = reality.public_key {
                    let trimmed_pk = pk.trim();
                    if trimmed_pk.is_empty() {
                        issues.push("vless: reality public-key must not be empty".to_string());
                    } else if trimmed_pk.len() < 43 || trimmed_pk.len() > 44 {
                        issues.push(
                            "vless: reality public-key should be a 32-byte Base64 string (43-44 chars)"
                                .to_string(),
                        );
                    }
                } else {
                    issues.push("vless: reality-opts requires public-key".to_string());
                }
                if let Some(ref sid) = reality.short_id {
                    let trimmed_sid = sid.trim();
                    if trimmed_sid.len() > 16 || trimmed_sid.len() % 2 != 0 {
                        issues.push(
                            "vless: reality short-id must be an even-length hex string <= 16 chars"
                                .to_string(),
                        );
                    } else if !trimmed_sid.chars().all(|c| c.is_ascii_hexdigit()) {
                        issues.push("vless: reality short-id contains non-hex characters".to_string());
                    }
                }
            }
            if let Some(ref pe) = node.packet_encoding
                && pe != "packetaddr"
                && pe != "xudp"
            {
                issues.push(format!(
                    "vless: unknown packet-encoding {pe:?} (expected 'packetaddr' or 'xudp')"
                ));
            }
            if let Some(ref xhttp) = node.xhttp_opts {
                if let Some(ref mode) = xhttp.mode
                    && !XHTTP_ALLOWED_MODES.contains(&mode.trim())
                {
                    issues.push(format!(
                        "vless: unknown xhttp mode {mode:?} (expected one of {XHTTP_ALLOWED_MODES:?})"
                    ));
                }
                if let Some(ref path) = xhttp.path
                    && !path.starts_with('/')
                {
                    issues.push("vless: xhttp path must start with '/'".to_string());
                }
            }
        }
        ProxyNode::Hysteria2(node) => {
            validate_common(&node.common, &mut issues);
            let has_pw = node
                .password
                .as_deref()
                .or(node.auth.as_deref())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .is_some();
            if !has_pw {
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
            if let Some(ref ports_str) = node.ports
                && let Err(err) = PortHopping::parse(ports_str)
            {
                issues.push(format!("hysteria2: invalid port hopping range: {err}"));
            }
            if node.cwnd == Some(0) {
                issues.push("hysteria2: cwnd must be positive".to_string());
            }
            if node.recv_window_conn == Some(0) {
                issues.push("hysteria2: recv-window-conn must be positive".to_string());
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
            if node.heartbeat_interval == Some(0) {
                issues.push("tuic: heartbeat-interval must be positive".to_string());
            }
            if node.request_timeout == Some(0) {
                issues.push("tuic: request-timeout must be positive".to_string());
            }
        }
        ProxyNode::WireGuard(node) => {
            validate_common(&node.common, &mut issues);
            if let Some(ref priv_key) = node.private_key {
                let trimmed = priv_key.trim();
                if trimmed.is_empty() {
                    issues.push("wireguard: private-key is required".to_string());
                } else if let Ok(bytes) = decode_base64_tolerant(trimmed) {
                    if bytes.len() != 32 {
                        issues.push("wireguard: private-key must decode to 32 bytes".to_string());
                    }
                } else {
                    issues.push("wireguard: private-key is not valid base64".to_string());
                }
            } else {
                issues.push("wireguard: private-key is required".to_string());
            }

            if let Some(ref pub_key) = node.public_key {
                let trimmed = pub_key.trim();
                if trimmed.is_empty() {
                    issues.push("wireguard: public-key is required".to_string());
                } else if let Ok(bytes) = decode_base64_tolerant(trimmed) {
                    if bytes.len() != 32 {
                        issues.push("wireguard: public-key must decode to 32 bytes".to_string());
                    }
                } else {
                    issues.push("wireguard: public-key is not valid base64".to_string());
                }
            } else {
                issues.push("wireguard: public-key is required".to_string());
            }

            if node.ip.is_none() && node.ipv6.is_none() {
                issues.push("wireguard: ip or ipv6 is required".to_string());
            }
            for (label, value) in [("ip", node.ip.as_deref()), ("ipv6", node.ipv6.as_deref())] {
                if let Some(ip) = value
                    && !ip.trim().is_empty()
                {
                    let clean_ip = ip.trim().split('/').next().unwrap_or_default();
                    if clean_ip.parse::<IpAddr>().is_err() {
                        issues.push(format!("wireguard: {label} is not a valid IP address"));
                    }
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
            if let Some(ref awg) = node.amnezia_opts {
                if let Some(jc) = awg.jc
                    && jc > 128
                {
                    issues.push("wireguard amnezia-opts: jc must not exceed 128".to_string());
                }
                if let (Some(jmin), Some(jmax)) = (awg.jmin, awg.jmax)
                    && jmin > jmax
                {
                    issues.push(
                        "wireguard amnezia-opts: jmin must be less than or equal to jmax"
                            .to_string(),
                    );
                }
            }
        }
        ProxyNode::Shadowsocks(node) => {
            validate_common(&node.common, &mut issues);
            if node.cipher.as_deref().is_none_or(|c| c.trim().is_empty()) {
                issues.push("ss: cipher is required".to_string());
            }
            if node.password.as_deref().is_none_or(|p| p.trim().is_empty()) {
                issues.push("ss: password is required".to_string());
            }
            if let (Some(cipher), Some(password)) =
                (node.cipher.as_deref(), node.password.as_deref())
            {
                let cipher_clean = cipher.trim();
                let password_clean = password.trim();
                if cipher_clean.starts_with("2022-blake3-") {
                    validate_ss_2022(cipher_clean, password_clean, &mut issues);
                }
            }
            if let Some(uot_v) = node.uot_version
                && uot_v != 1
                && uot_v != 2
            {
                issues.push(format!("ss: unknown uot-version {uot_v} (expected 1 or 2)"));
            }
        }
        ProxyNode::Anytls(node) => {
            validate_common(&node.common, &mut issues);
            let has_auth = node
                .password
                .as_deref()
                .or(node.uuid.as_deref())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .is_some();
            if !has_auth {
                issues.push("anytls: password or uuid is required".to_string());
            }
            if let Some(ref pr) = node.padding_range
                && let Some((start_s, end_s)) = pr.split_once('-')
                && let (Ok(s), Ok(e)) = (start_s.trim().parse::<u32>(), end_s.trim().parse::<u32>())
                && s > e
            {
                issues.push(
                    "anytls: padding-range start must be less than or equal to end".to_string(),
                );
            }
        }
        ProxyNode::Trojan(node) => {
            validate_common(&node.common, &mut issues);
            if node.password.as_deref().is_none_or(|p| p.trim().is_empty()) {
                issues.push("trojan: password is required".to_string());
            }
        }
        ProxyNode::Vmess(node) => {
            validate_common(&node.common, &mut issues);
            if node.uuid.as_deref().is_none_or(|u| u.trim().is_empty()) {
                issues.push("vmess: uuid is required".to_string());
            }
            if let Some(cipher) = node.cipher.as_deref()
                && !VMESS_ALLOWED_CIPHERS.contains(&cipher.trim())
            {
                issues.push(format!(
                    "vmess: unknown cipher {cipher:?} (expected one of {VMESS_ALLOWED_CIPHERS:?})"
                ));
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

fn validate_ss_2022(cipher: &str, password: &str, issues: &mut Vec<String>) {
    let expected_len = match cipher {
        "2022-blake3-aes-128-gcm" => 16,
        "2022-blake3-aes-256-gcm"
        | "2022-blake3-chacha20-poly1305"
        | "2022-blake3-chacha8-poly1305" => 32,
        _ => return,
    };

    if password.contains(':') {
        for part in password.split(':') {
            let trimmed = part.trim();
            if let Ok(bytes) = decode_base64_tolerant(trimmed) {
                if bytes.len() != expected_len {
                    issues.push(format!(
                        "ss 2022: each key component for {cipher} must decode to {expected_len} bytes"
                    ));
                    break;
                }
            } else {
                issues.push(format!(
                    "ss 2022: key component for {cipher} is not valid base64"
                ));
                break;
            }
        }
    } else if let Ok(bytes) = decode_base64_tolerant(password) {
        if bytes.len() != expected_len {
            issues.push(format!(
                "ss 2022: password for {cipher} must decode to {expected_len} bytes (got {})",
                bytes.len()
            ));
        }
    } else {
        issues.push(format!(
            "ss 2022: password for {cipher} is not valid base64"
        ));
    }
}

fn decode_base64_tolerant(input: &str) -> Result<Vec<u8>, ()> {
    let clean: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    STANDARD
        .decode(&clean)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(&clean))
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(&clean))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(&clean))
        .map_err(|_| ())
}

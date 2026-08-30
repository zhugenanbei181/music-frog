//! Lightweight validation rules for parsed proxy nodes.
//!
//! One human-readable message per detected problem (missing required fields,
//! empty values, unknown enum-like values), advisory only — validation never
//! mutates the node and never rejects it.

use std::net::IpAddr;

use serde_yaml_ng::Value;

use super::model::{CommonFields, ProxyNode, RawNode, Reserved};

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

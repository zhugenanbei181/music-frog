//! User-owned local proxy configuration, node storage, and typed operation contracts (FLPARITY-037).
//!
//! Enables adding observed proxy nodes into a user-owned local proxy collection without
//! altering remote subscription profiles or leaking sensitive secrets into telemetry/UI logs.

use serde::{Deserialize, Serialize};

/// Supported proxy protocol families for user-owned local proxy configurations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LocalProxyProtocol {
    #[default]
    Shadowsocks,
    Vmess,
    Vless,
    Trojan,
    Hysteria2,
    Tuic,
    Wireguard,
    Http,
    Socks5,
}

impl LocalProxyProtocol {
    /// Canonical wire protocol identifier.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Shadowsocks => "ss",
            Self::Vmess => "vmess",
            Self::Vless => "vless",
            Self::Trojan => "trojan",
            Self::Hysteria2 => "hysteria2",
            Self::Tuic => "tuic",
            Self::Wireguard => "wireguard",
            Self::Http => "http",
            Self::Socks5 => "socks5",
        }
    }

    /// Parse protocol from string with case-insensitivity.
    pub fn parse_protocol(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "ss" | "shadowsocks" => Some(Self::Shadowsocks),
            "vmess" => Some(Self::Vmess),
            "vless" => Some(Self::Vless),
            "trojan" => Some(Self::Trojan),
            "hy2" | "hysteria2" => Some(Self::Hysteria2),
            "tuic" => Some(Self::Tuic),
            "wg" | "wireguard" => Some(Self::Wireguard),
            "http" => Some(Self::Http),
            "socks" | "socks5" => Some(Self::Socks5),
            _ => None,
        }
    }

    /// Display badge label.
    pub const fn badge(self) -> &'static str {
        match self {
            Self::Shadowsocks => "SS",
            Self::Vmess => "VMESS",
            Self::Vless => "VLESS",
            Self::Trojan => "TROJAN",
            Self::Hysteria2 => "HY2",
            Self::Tuic => "TUIC",
            Self::Wireguard => "WG",
            Self::Http => "HTTP",
            Self::Socks5 => "SOCKS5",
        }
    }
}

/// Fully-qualified user-owned local proxy node (persisted securely).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalProxyNode {
    /// Stable node identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Protocol family.
    pub protocol: LocalProxyProtocol,
    /// Server address (hostname or IP).
    pub server: String,
    /// Server port.
    pub port: u16,
    /// Secret credential (password, UUID, PSK).
    pub secret: Option<String>,
    /// Cipher / encryption algorithm.
    pub cipher: Option<String>,
    /// Transport network (e.g. "tcp", "ws", "grpc").
    pub network: Option<String>,
    /// Whether TLS / encryption wrapper is enabled.
    pub tls: bool,
    /// Server Name Indication.
    pub sni: Option<String>,
    /// Skip TLS certificate verification.
    pub skip_cert_verify: bool,
    /// Originating subscription profile ID, if copied from a subscription.
    pub source_profile_id: Option<String>,
    /// Timestamp when this node was added (epoch milliseconds).
    pub added_at_epoch_ms: u64,
}

impl LocalProxyNode {
    /// Create a summary with sensitive secrets redacted.
    pub fn to_summary(&self) -> LocalProxyNodeSummary {
        LocalProxyNodeSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            protocol: self.protocol,
            server: self.server.clone(),
            port: self.port,
            has_secret: self.secret.is_some(),
            cipher: self.cipher.clone(),
            network: self.network.clone(),
            tls: self.tls,
            sni: self.sni.clone(),
            source_profile_id: self.source_profile_id.clone(),
            added_at_epoch_ms: self.added_at_epoch_ms,
        }
    }
}

/// Redacted summary of a local proxy node for safe UI rendering and telemetry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalProxyNodeSummary {
    pub id: String,
    pub name: String,
    pub protocol: LocalProxyProtocol,
    pub server: String,
    pub port: u16,
    pub has_secret: bool,
    pub cipher: Option<String>,
    pub network: Option<String>,
    pub tls: bool,
    pub sni: Option<String>,
    pub source_profile_id: Option<String>,
    pub added_at_epoch_ms: u64,
}

/// Collection of user-owned local proxy nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocalProxyCollection {
    pub nodes: Vec<LocalProxyNode>,
    pub revision: u64,
    pub last_updated_epoch_ms: u64,
}

/// Typed intent command to add a proxy node to the local collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddLocalProxyCommand {
    pub node: LocalProxyNode,
    pub expected_revision: Option<u64>,
}

/// Typed intent command to remove a proxy node by ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoveLocalProxyCommand {
    pub node_id: String,
    pub expected_revision: Option<u64>,
}

/// Typed receipt for the add-local-proxy operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AddLocalProxyReceipt {
    Added {
        id: String,
        name: String,
        revision: u64,
    },
    DuplicateNode {
        existing_id: String,
        name: String,
    },
    InvalidNode {
        reason: String,
    },
    StaleRevision {
        expected: u64,
        actual: u64,
    },
}

/// Typed receipt for the remove-local-proxy operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RemoveLocalProxyReceipt {
    Removed { id: String, revision: u64 },
    NotFound { id: String },
    StaleRevision { expected: u64, actual: u64 },
}

/// Surface snapshot representing the user-owned local proxy collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocalProxySnapshot {
    pub nodes: Vec<LocalProxyNodeSummary>,
    pub total_count: usize,
    pub revision: u64,
    pub last_updated_epoch_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_proxy_node_summary_redacts_secret() {
        let node = LocalProxyNode {
            id: "node-1".into(),
            name: "HK Fast".into(),
            protocol: LocalProxyProtocol::Shadowsocks,
            server: "1.2.3.4".into(),
            port: 8388,
            secret: Some("super-secret-password".into()),
            cipher: Some("chacha20-ietf-poly1305".into()),
            network: Some("tcp".into()),
            tls: false,
            sni: None,
            skip_cert_verify: false,
            source_profile_id: Some("sub-profile-1".into()),
            added_at_epoch_ms: 1000,
        };

        let summary = node.to_summary();
        assert_eq!(summary.id, "node-1");
        assert_eq!(summary.name, "HK Fast");
        assert!(summary.has_secret);

        let json = serde_json::to_string(&summary).unwrap();
        assert!(!json.contains("super-secret-password"));
        assert!(json.contains("has_secret\":true"));
    }

    #[test]
    fn local_proxy_protocol_parsing_and_badges() {
        assert_eq!(
            LocalProxyProtocol::parse_protocol("ss"),
            Some(LocalProxyProtocol::Shadowsocks)
        );
        assert_eq!(
            LocalProxyProtocol::parse_protocol("VMess"),
            Some(LocalProxyProtocol::Vmess)
        );
        assert_eq!(
            LocalProxyProtocol::parse_protocol("hy2"),
            Some(LocalProxyProtocol::Hysteria2)
        );
        assert_eq!(LocalProxyProtocol::parse_protocol("unknown"), None);

        assert_eq!(LocalProxyProtocol::Hysteria2.badge(), "HY2");
        assert_eq!(LocalProxyProtocol::Shadowsocks.as_str(), "ss");
    }
}

//! Shared live contract for Mihomo LAN proxy sharing.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Mihomo's wildcard bind address for all local interfaces.
pub const DEFAULT_BIND_ADDRESS: &str = "*";

/// A confirmed live Allow-LAN configuration. Failures stay in the enclosing
/// application result instead of being represented as guessed UI state.
///
/// This is the one LAN sharing readback of the product. The snapshots that
/// carry it publish it whole or not at all, so a consumer can tell
/// "sharing is off" (`Some` with `enabled == false`) from "sharing was never
/// observed" (`None`). The `revision` is the configuration revision the
/// observation was taken at, so a write can be fenced against the
/// observation it was derived from.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanSharingSnapshot {
    /// Whether the proxy shares its port with the local network.
    pub enabled: bool,
    /// The port the shared proxy listens on.
    pub mixed_port: u16,
    /// The address the shared proxy binds to.
    pub bind_address: String,
    /// The configuration revision this observation was taken at.
    pub revision: u64,
}

impl LanSharingSnapshot {
    /// The confirmed configuration the owner reported.
    pub fn new(
        revision: u64,
        enabled: bool,
        mixed_port: u16,
        bind_address: impl Into<String>,
    ) -> Self {
        Self {
            enabled,
            mixed_port,
            bind_address: bind_address.into(),
            revision,
        }
    }

    /// Whether the shared proxy is confirmed off.
    ///
    /// A missing readback is not "off": the caller has to answer that from
    /// the absence of the observation itself.
    pub fn is_disabled(&self) -> bool {
        !self.enabled
    }
}

/// Credentials are accepted only as an in-memory command input. Debug output
/// is deliberately redacted and serde never serializes the password.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanCredentials {
    pub username: String,
    #[serde(skip_serializing, default)]
    pub password: String,
}

impl fmt::Debug for LanCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LanCredentials")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

/// Live ACL/authentication summary. It intentionally contains no password or
/// raw `username:password` material from Mihomo's configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanSecuritySnapshot {
    pub allowed_ips: Vec<String>,
    pub disallowed_ips: Vec<String>,
    pub skip_auth_prefixes: Vec<String>,
    pub authentication_enabled: bool,
    pub authentication_user_count: usize,
    pub authentication_username: Option<String>,
    pub revision: u64,
}

impl Default for LanSecuritySnapshot {
    fn default() -> Self {
        Self {
            allowed_ips: vec!["192.168.0.0/16".to_owned(), "10.0.0.0/8".to_owned()],
            disallowed_ips: Vec::new(),
            skip_auth_prefixes: vec!["127.0.0.0/8".to_owned(), "::1/128".to_owned()],
            authentication_enabled: false,
            authentication_user_count: 0,
            authentication_username: None,
            revision: 0,
        }
    }
}

impl LanSecuritySnapshot {
    pub fn new(
        revision: u64,
        allowed_ips: Vec<String>,
        disallowed_ips: Vec<String>,
        skip_auth_prefixes: Vec<String>,
        authentication_enabled: bool,
        authentication_user_count: usize,
        authentication_username: Option<String>,
    ) -> Self {
        Self {
            allowed_ips,
            disallowed_ips,
            skip_auth_prefixes,
            authentication_enabled,
            authentication_user_count,
            authentication_username,
            revision,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lan_sharing_readback_tells_off_from_unobserved() {
        let off = LanSharingSnapshot::new(7, false, 7890, "*");
        assert!(off.is_disabled());
        assert_eq!(off.revision, 7);

        // The snapshot that carries the readback publishes it whole or not at
        // all: an absent readback is "never observed", never "off".
        let observed: Option<LanSharingSnapshot> = Some(off.clone());
        let unobserved: Option<LanSharingSnapshot> = None;
        assert!(observed.is_some_and(|sharing| sharing.is_disabled()));
        assert!(unobserved.is_none());
    }

    #[test]
    fn the_lan_sharing_readback_round_trips_through_the_wire() {
        let sharing = LanSharingSnapshot::new(11, true, 8080, "192.168.1.10");
        let encoded = serde_json::to_string(&sharing).expect("serialize the LAN sharing readback");
        let decoded: LanSharingSnapshot =
            serde_json::from_str(&encoded).expect("decode the LAN sharing readback");
        assert_eq!(decoded, sharing);
    }
}

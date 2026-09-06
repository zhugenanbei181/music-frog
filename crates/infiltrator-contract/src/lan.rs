//! Shared live contract for Mihomo LAN proxy sharing.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Mihomo's wildcard bind address for all local interfaces.
pub const DEFAULT_BIND_ADDRESS: &str = "*";

/// A confirmed live Allow-LAN configuration. Failures stay in the enclosing
/// application result instead of being represented as guessed UI state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanSharingSnapshot {
    pub enabled: bool,
    pub mixed_port: u16,
    pub bind_address: String,
    pub revision: u64,
}

impl LanSharingSnapshot {
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

//! Runtime-neutral configuration snapshot metadata and hash rules.

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// Metadata of one stored snapshot; content is read on demand by an adapter.
///
/// `path` is an opaque storage identity carried back to the selected host. No
/// filesystem operation is performed by this domain module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotMeta {
    pub profile: String,
    /// RFC 3339 timestamp captured at write time, also part of the name.
    pub timestamp: DateTime<Utc>,
    /// Lowercase hex SHA-256 of the snapshot content.
    pub sha256: String,
    pub path: PathBuf,
}

/// Hex SHA-256 of `content`.
pub fn content_hash(content: &[u8]) -> String {
    let digest = Sha256::digest(content);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// Parse `<unix_millis>-<8 hex>.yaml` into its parts. Numeric timestamps
/// keep the name stable and sortable without locale/format ambiguity.
pub fn parse_snapshot_name(name: &str) -> Option<(DateTime<Utc>, String)> {
    let stem = name.strip_suffix(".yaml")?;
    let (stamp, hash) = stem.split_once('-')?;
    let millis = stamp.parse::<i64>().ok()?;
    if hash.len() != 8 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let timestamp = DateTime::from_timestamp_millis(millis)?;
    Some((timestamp, hash.to_ascii_lowercase()))
}


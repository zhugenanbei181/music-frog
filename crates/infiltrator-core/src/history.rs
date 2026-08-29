//! Config snapshot history ([缺口13], CORE-004).
//!
//! Every successful [`crate::apply::apply_current_profile`] stores the newly
//! live content as a timestamped, SHA-256-tagged snapshot under
//! `<config_dir>/snapshots/<profile>/`. Rolling back is itself an apply:
//! callers read a snapshot and feed its content through the same
//! transaction, so restores inherit validation, atomic write, readiness and
//! rollback — never a raw file overwrite.

use chrono::{DateTime, Utc};
use mihomo_api::error::{MihomoError, Result};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// How many snapshots are retained per profile after pruning.
pub const DEFAULT_KEEP: usize = 20;

/// Metadata of one stored snapshot; content is read on demand.
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

/// Directory holding `profile`'s snapshots: `<config_dir>/snapshots/<profile>`.
pub fn snapshot_dir(config_dir: &Path, profile: &str) -> PathBuf {
    config_dir.join("snapshots").join(profile)
}

/// Persist `content` as the newest snapshot of `profile`.
///
/// The file name embeds the timestamp and the first 8 hex characters of the
/// content hash, so identical content never produces duplicate entries and
/// ordering never depends on file mtimes.
pub async fn save_snapshot(
    config_dir: &Path,
    profile: &str,
    content: &str,
) -> Result<SnapshotMeta> {
    let dir = snapshot_dir(config_dir, profile);
    tokio::fs::create_dir_all(&dir).await?;

    let timestamp = Utc::now();
    let sha256 = content_hash(content.as_bytes());
    let file_name = format!("{}-{}.yaml", timestamp.timestamp_millis(), &sha256[..8]);
    let path = dir.join(&file_name);
    tokio::fs::write(&path, content).await?;

    Ok(SnapshotMeta {
        profile: profile.to_string(),
        timestamp,
        sha256,
        path,
    })
}

/// All snapshots of `profile`, newest first.
pub async fn list_snapshots(config_dir: &Path, profile: &str) -> Result<Vec<SnapshotMeta>> {
    let dir = snapshot_dir(config_dir, profile);
    let mut out = Vec::new();
    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(entries) => entries,
        // No snapshots yet is not an error — the list is just empty.
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(err) => return Err(MihomoError::from(err)),
    };
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().to_string();
        match parse_snapshot_name(&name) {
            Some((timestamp, sha256)) => out.push(SnapshotMeta {
                profile: profile.to_string(),
                timestamp,
                sha256,
                path: entry.path(),
            }),
            // Foreign files never break listing; they are simply ignored.
            None => continue,
        }
    }
    out.sort_by_key(|meta| std::cmp::Reverse(meta.timestamp));
    Ok(out)
}

/// Read the content of a snapshot previously listed by
/// [`list_snapshots`].
pub async fn read_snapshot(path: &Path) -> Result<String> {
    tokio::fs::read_to_string(path)
        .await
        .map_err(MihomoError::from)
}

/// Keep only the `keep` newest snapshots of `profile`; older ones are
/// deleted. `keep == 0` clears the profile's history.
pub async fn prune_snapshots(config_dir: &Path, profile: &str, keep: usize) -> Result<usize> {
    let snapshots = list_snapshots(config_dir, profile).await?;
    let mut removed = 0;
    for meta in snapshots.into_iter().skip(keep) {
        if tokio::fs::remove_file(&meta.path).await.is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

/// Parse `<unix_millis>-<8 hex>.yaml` into its parts. Numeric timestamps
/// keep the name stable and sortable without locale/format ambiguity.
fn parse_snapshot_name(name: &str) -> Option<(DateTime<Utc>, String)> {
    let stem = name.strip_suffix(".yaml")?;
    let (stamp, hash) = stem.split_once('-')?;
    let millis = stamp.parse::<i64>().ok()?;
    if hash.len() != 8 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let timestamp = DateTime::from_timestamp_millis(millis)?;
    Some((timestamp, hash.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_dir() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        (dir, path)
    }

    #[tokio::test]
    async fn save_then_list_returns_newest_first() {
        let (_dir, config) = config_dir();
        let first = save_snapshot(&config, "main", "port: 1").await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let second = save_snapshot(&config, "main", "port: 2").await.unwrap();

        let list = list_snapshots(&config, "main").await.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].path, second.path);
        assert_eq!(list[1].path, first.path);
        assert!(list[0].timestamp >= list[1].timestamp);
    }

    #[tokio::test]
    async fn snapshot_name_embeds_hash_prefix() {
        let (_dir, config) = config_dir();
        let meta = save_snapshot(&config, "main", "port: 42").await.unwrap();
        assert_eq!(meta.sha256.len(), 64);
        let name = meta.path.file_name().unwrap().to_string_lossy();
        assert!(name.contains(&meta.sha256[..8]), "file name {name} must embed the hash prefix");
        // hash8 comes from the file name and matches full content hash
        assert_eq!(meta.sha256, content_hash(b"port: 42"));
    }

    #[tokio::test]
    async fn read_snapshot_returns_original_content() {
        let (_dir, config) = config_dir();
        let meta = save_snapshot(&config, "main", "port: 7890\nsecret: s3cret\n")
            .await
            .unwrap();
        assert_eq!(read_snapshot(&meta.path).await.unwrap(), "port: 7890\nsecret: s3cret\n");
    }

    #[tokio::test]
    async fn prune_keeps_newest_only() {
        let (_dir, config) = config_dir();
        for port in 1..=5 {
            save_snapshot(&config, "main", &format!("port: {port}")).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        let removed = prune_snapshots(&config, "main", 2).await.unwrap();
        assert_eq!(removed, 3);
        let list = list_snapshots(&config, "main").await.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(read_snapshot(&list[0].path).await.unwrap(), "port: 5");
    }

    #[tokio::test]
    async fn list_missing_profile_is_empty_not_error() {
        let (_dir, config) = config_dir();
        assert!(list_snapshots(&config, "ghost").await.unwrap().is_empty());
    }

    #[test]
    fn parse_snapshot_name_rejects_foreign_files() {
        assert!(parse_snapshot_name("1735489200123-abcd1234.yaml").is_some());
        assert!(parse_snapshot_name("notes.txt").is_none());
        assert!(parse_snapshot_name("1735489200123-nothex.yaml").is_none());
        assert!(parse_snapshot_name("garbage.yaml").is_none());
    }
}

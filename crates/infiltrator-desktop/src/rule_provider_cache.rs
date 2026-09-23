//! Desktop rule-provider cache port (DUAL-11-06/07).
//!
//! The desktop core is started with `-d <config dir>`, so mihomo's home
//! directory is exactly the directory that holds the profiles. mihomo caches
//! every downloaded (`type: http`) rule provider as
//! `<home>/rules/<md5(url)>` (`constant.Path.GetPathByHash("rules", url)`),
//! and an explicit `path:` resolves relative to the same home directory.
//!
//! Only that `rules` directory is ever purged: profile documents, snapshots,
//! GeoIP databases and the kernel state database live elsewhere and stay
//! untouched.

use async_trait::async_trait;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::provider_cache::{
    ProviderCachePurge, ProviderContentOrigin, ProviderFileFingerprint, RuleProviderCacheSnapshot,
};
use infiltrator_domain::rules::provider_store::{
    MAX_PROVIDER_FILE_BYTES, PROVIDER_CACHE_DIR_NAME, ProviderSourceKind, RuleProviderDeclaration,
    provider_source_candidates,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::rule_provider_cache::{
    ProviderCacheEntry, ProviderFileFact, RuleProviderCachePort,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

/// Digest memo entry: `(size, modified_unix_nanos, sha256)` per file path.
///
/// The modification stamp is kept at **nanosecond** precision on purpose: a
/// second-granularity stamp made an in-place rewrite of equal length within
/// the same second look unchanged, so the memo served a stale digest.
type DigestMemo = HashMap<PathBuf, (u64, i64, String)>;

/// File-system adapter over the kernel's local provider files.
#[derive(Clone, Debug)]
pub struct DesktopRuleProviderCache {
    home: PathBuf,
    /// Digest memo keyed by file path: `(size, modified_unix_nanos, sha256)`.
    /// Only used when the file's filesystem exposes a modification time.
    digests: Arc<Mutex<DigestMemo>>,
}

impl DesktopRuleProviderCache {
    pub fn new(home: PathBuf) -> Self {
        Self {
            home,
            digests: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// The kernel's provider cache directory, whether or not it exists yet.
    pub fn cache_dir(&self) -> PathBuf {
        self.home.join(PROVIDER_CACHE_DIR_NAME)
    }

    /// SHA-256 of the file, recomputed only when its length or nanosecond
    /// modification stamp no longer match what this host last hashed.
    async fn digest_for(
        &self,
        path: &Path,
        size_bytes: u64,
        modified_unix_nanos: Option<i64>,
    ) -> Result<String, PortError> {
        if let Some(modified) = modified_unix_nanos
            && let Some((cached_size, cached_modified, digest)) = self
                .digests
                .lock()
                .expect("provider digest memo lock")
                .get(path)
                .cloned()
            && cached_size == size_bytes
            && cached_modified == modified
        {
            return Ok(digest);
        }
        let digest = {
            let bytes = tokio::fs::read(path).await.map_err(|error| {
                PortError::Io(format!(
                    "cannot read provider file {}: {error}",
                    path.display()
                ))
            })?;
            infiltrator_domain::snapshots::content_hash(&bytes)
        };
        if let Some(modified) = modified_unix_nanos {
            self.digests
                .lock()
                .expect("provider digest memo lock")
                .insert(path.to_path_buf(), (size_bytes, modified, digest.clone()));
        }
        Ok(digest)
    }

    async fn scan_cache(&self) -> Result<Vec<(PathBuf, u64)>, PortError> {
        let dir = self.cache_dir();
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(PortError::Io(format!(
                    "cannot list rule-provider cache {}: {error}",
                    dir.display()
                )));
            }
        };
        let mut files = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|error| PortError::Io(error.to_string()))?
        {
            // Only regular files: never recurse into or delete directories.
            let Ok(metadata) = entry.metadata().await else {
                continue;
            };
            if !metadata.is_file() {
                continue;
            }
            files.push((entry.path(), metadata.len()));
        }
        files.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(files)
    }
}

#[async_trait]
impl RuleProviderCachePort for DesktopRuleProviderCache {
    async fn read_provider(
        &self,
        declaration: &RuleProviderDeclaration,
    ) -> Result<Option<ProviderCacheEntry>, PortError> {
        for candidate in provider_source_candidates(declaration, &self.home) {
            let metadata = match tokio::fs::metadata(&candidate.path).await {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(PortError::Io(format!(
                        "cannot stat provider file {}: {error}",
                        candidate.path.display()
                    )));
                }
            };
            if !metadata.is_file() {
                continue;
            }
            if metadata.len() > MAX_PROVIDER_FILE_BYTES {
                return Err(PortError::Failed(format!(
                    "provider file {} is {} bytes, above the {} byte safety cap",
                    candidate.path.display(),
                    metadata.len(),
                    MAX_PROVIDER_FILE_BYTES
                )));
            }
            let bytes = tokio::fs::read(&candidate.path).await.map_err(|error| {
                PortError::Io(format!(
                    "cannot read provider file {}: {error}",
                    candidate.path.display()
                ))
            })?;
            let origin = match candidate.kind {
                ProviderSourceKind::DeclaredFile => ProviderContentOrigin::DeclaredFile,
                ProviderSourceKind::KernelCacheFile => ProviderContentOrigin::KernelCacheFile,
            };
            return Ok(Some(ProviderCacheEntry {
                origin,
                path: Some(candidate.path),
                bytes,
            }));
        }
        Ok(None)
    }

    async fn fingerprint(
        &self,
        declaration: &RuleProviderDeclaration,
    ) -> Result<Option<ProviderFileFact>, PortError> {
        // Only a kernel-downloaded provider has a cache file to fingerprint;
        // a `type: file` provider is the user's own static input.
        if !declaration.is_remote() {
            return Ok(None);
        }
        let Some(candidate) = provider_source_candidates(declaration, &self.home)
            .into_iter()
            .next()
        else {
            return Ok(None);
        };
        let path = candidate.path;
        let metadata = match tokio::fs::metadata(&path).await {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(PortError::Io(format!(
                    "cannot stat provider file {}: {error}",
                    path.display()
                )));
            }
        };
        if !metadata.is_file() {
            return Ok(None);
        }
        if metadata.len() > MAX_PROVIDER_FILE_BYTES {
            return Err(PortError::Failed(format!(
                "provider file {} is {} bytes, above the {} byte safety cap",
                path.display(),
                metadata.len(),
                MAX_PROVIDER_FILE_BYTES
            )));
        }
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok());
        let modified_unix_secs = modified.map(|duration| duration.as_secs() as i64);
        // The memo keys on nanoseconds so an equal-length in-place rewrite
        // inside the same second can never reuse a stale digest.
        let modified_unix_nanos =
            modified.and_then(|duration| i64::try_from(duration.as_nanos()).ok());
        let size_bytes = metadata.len();
        let sha256 = self
            .digest_for(&path, size_bytes, modified_unix_nanos)
            .await?;
        Ok(Some(ProviderFileFact {
            path,
            fingerprint: ProviderFileFingerprint {
                size_bytes,
                sha256,
                modified_unix_secs,
            },
        }))
    }

    async fn purge(&self) -> Result<ProviderCachePurge, PortError> {
        let directory = self.cache_dir();
        let mut files_removed = 0usize;
        let mut bytes_freed = 0u64;
        for (path, len) in self.scan_cache().await? {
            match tokio::fs::remove_file(&path).await {
                Ok(()) => {
                    files_removed += 1;
                    bytes_freed += len;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(PortError::PermissionDenied(format!(
                        "cannot remove cached provider {}: {error}",
                        path.display()
                    )));
                }
            }
        }
        Ok(ProviderCachePurge {
            directory: Some(directory.display().to_string()),
            files_removed,
            bytes_freed,
        })
    }

    async fn snapshot(&self) -> Result<RuleProviderCacheSnapshot, PortError> {
        let directory = self.cache_dir();
        let files = self.scan_cache().await?;
        let total_bytes = files.iter().map(|(_, len)| *len).sum();
        Ok(RuleProviderCacheSnapshot::ready(
            directory.display().to_string(),
            files.len(),
            total_bytes,
        ))
    }
}

/// Build the port for a kernel home directory, when one is known.
pub fn port_for_home(home: Option<&Path>) -> Option<DesktopRuleProviderCache> {
    home.map(|path| DesktopRuleProviderCache::new(path.to_path_buf()))
}

/// Typed failure for hosts that never resolved a kernel home directory.
pub fn missing_home_error() -> PortError {
    PortError::unsupported(
        Capability::Profiles,
        "the kernel home directory is unknown, so no rule-provider cache location exists",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn declaration(value: serde_json::Value) -> RuleProviderDeclaration {
        RuleProviderDeclaration::from_value("ads", &value)
    }

    #[tokio::test]
    async fn purge_removes_only_cached_provider_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path();
        let rules = home.join("rules");
        tokio::fs::create_dir_all(&rules).await.unwrap();
        tokio::fs::write(rules.join("aaa"), b"one").await.unwrap();
        tokio::fs::write(rules.join("bbb"), b"two-two")
            .await
            .unwrap();
        tokio::fs::create_dir_all(rules.join("nested"))
            .await
            .unwrap();
        tokio::fs::write(rules.join("nested").join("keep"), b"nested")
            .await
            .unwrap();
        // A sibling profile must survive a purge untouched.
        tokio::fs::write(home.join("default.yaml"), b"rules: []")
            .await
            .unwrap();

        let port = DesktopRuleProviderCache::new(home.to_path_buf());
        let snapshot = port.snapshot().await.expect("snapshot");
        assert_eq!(snapshot.file_count, 2);
        assert_eq!(snapshot.total_bytes, 10);

        let purge = port.purge().await.expect("purge");
        assert_eq!(purge.files_removed, 2);
        assert_eq!(purge.bytes_freed, 10);
        assert_eq!(purge.directory.as_deref(), Some(rules.to_str().unwrap()));
        assert!(!rules.join("aaa").exists());
        assert!(rules.join("nested").join("keep").exists());
        assert!(home.join("default.yaml").exists());

        let after = port.snapshot().await.expect("snapshot after purge");
        assert_eq!(after.file_count, 0);
        assert_eq!(after.total_bytes, 0);

        // A second purge is an honest no-op, not a fabricated success.
        let empty = port.purge().await.expect("second purge");
        assert_eq!(empty.files_removed, 0);
        assert!(empty.is_noop());
    }

    #[tokio::test]
    async fn missing_cache_directory_is_empty_not_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let port = DesktopRuleProviderCache::new(dir.path().to_path_buf());
        let snapshot = port.snapshot().await.expect("snapshot");
        assert_eq!(snapshot.file_count, 0);
        assert!(snapshot.is_available());
        assert!(port.purge().await.expect("purge").is_noop());
    }

    #[tokio::test]
    async fn reads_declared_files_and_the_url_hash_cache() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path();
        let rules = home.join("rules");
        tokio::fs::create_dir_all(&rules).await.unwrap();
        let url = "https://example.com/ads.txt";
        let cached =
            rules.join(infiltrator_domain::rules::provider_store::provider_cache_file_name(url));
        tokio::fs::write(&cached, b"cached.cn\n").await.unwrap();
        tokio::fs::write(home.join("local.yaml"), b"payload:\n  - DOMAIN,local.com\n")
            .await
            .unwrap();

        let port = DesktopRuleProviderCache::new(home.to_path_buf());
        let remote = declaration(json!({
            "type": "http",
            "behavior": "domain",
            "format": "text",
            "url": url
        }));
        let entry = port
            .read_provider(&remote)
            .await
            .expect("read")
            .expect("cache hit");
        assert_eq!(entry.origin, ProviderContentOrigin::KernelCacheFile);
        assert_eq!(entry.bytes, b"cached.cn\n");

        let local = declaration(json!({
            "type": "file",
            "behavior": "classical",
            "format": "yaml",
            "path": "local.yaml"
        }));
        let entry = port
            .read_provider(&local)
            .await
            .expect("read")
            .expect("declared file");
        assert_eq!(entry.origin, ProviderContentOrigin::DeclaredFile);

        let absent = declaration(json!({
            "type": "file",
            "behavior": "classical",
            "format": "yaml",
            "path": "absent.yaml"
        }));
        assert!(port.read_provider(&absent).await.expect("read").is_none());
    }

    #[tokio::test]
    async fn fingerprint_reports_real_size_digest_and_mtime_for_the_cache_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path();
        let rules = home.join("rules");
        tokio::fs::create_dir_all(&rules).await.unwrap();
        let url = "https://example.com/ads.txt";
        let cached =
            rules.join(infiltrator_domain::rules::provider_store::provider_cache_file_name(url));
        tokio::fs::write(&cached, b"cached.cn\n").await.unwrap();

        let port = DesktopRuleProviderCache::new(home.to_path_buf());
        let remote = declaration(json!({
            "type": "http",
            "behavior": "domain",
            "format": "text",
            "url": url
        }));
        let fact = port
            .fingerprint(&remote)
            .await
            .expect("fingerprint")
            .expect("cache file");
        assert_eq!(fact.path, cached);
        assert_eq!(fact.fingerprint.size_bytes, 10);
        // The digest is the real SHA-256 of the bytes on disk.
        assert_eq!(
            fact.fingerprint.sha256,
            "62caf262e0e4d9bf9098026aebc2f841f6a31e9d866309f1c4ea7ecaf4a58e0e"
        );
        assert!(
            fact.fingerprint.modified_unix_secs.is_some(),
            "a real file has a modification time"
        );

        // Rewriting the cache file changes the observed digest.
        tokio::fs::write(&cached, b"payload-a\n").await.unwrap();
        let rewritten = port
            .fingerprint(&remote)
            .await
            .expect("fingerprint")
            .expect("cache file");
        assert_eq!(
            rewritten.fingerprint.sha256,
            "fe2e4485e53d99f52cc60b0fdd517a47d2b30adad1ed103a32f09a01ae1b0546"
        );
        assert_ne!(rewritten.fingerprint.sha256, fact.fingerprint.sha256);
    }

    #[tokio::test]
    async fn fingerprint_answers_none_for_static_or_absent_providers() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path();
        let port = DesktopRuleProviderCache::new(home.to_path_buf());
        let local = declaration(json!({
            "type": "file",
            "behavior": "classical",
            "format": "yaml",
            "path": "local.yaml"
        }));
        // A static declared file is not a downloaded cache: nothing to observe.
        assert!(port.fingerprint(&local).await.expect("static").is_none());
        let remote = declaration(json!({
            "type": "http",
            "behavior": "domain",
            "format": "text",
            "url": "https://example.com/absent.txt"
        }));
        // No cache file yet is a normal answer, not a fabricated fingerprint.
        assert!(port.fingerprint(&remote).await.expect("absent").is_none());
    }

    #[tokio::test]
    async fn digest_memo_only_reuses_a_digest_for_identical_size_and_mtime() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("provider");
        tokio::fs::write(&path, b"payload-a\n").await.unwrap();
        let port = DesktopRuleProviderCache::new(dir.path().to_path_buf());

        let first = port
            .digest_for(&path, 10, Some(1_700_000_000_000_000_000))
            .await
            .unwrap();
        assert_eq!(
            first,
            "fe2e4485e53d99f52cc60b0fdd517a47d2b30adad1ed103a32f09a01ae1b0546"
        );
        // Same size + stamp: the memoised digest is reused.
        let repeated = port
            .digest_for(&path, 10, Some(1_700_000_000_000_000_000))
            .await
            .unwrap();
        assert_eq!(repeated, first);
        // A changed cheap fact forces a real re-read of the new bytes.
        tokio::fs::write(&path, b"payload-b\n").await.unwrap();
        let changed = port
            .digest_for(&path, 10, Some(1_700_000_000_000_000_001))
            .await
            .unwrap();
        assert_eq!(
            changed,
            "3cc83b8abe45f1bfb461f3cd276f09c57433e9532979d08510d1542f27094e1b"
        );
        assert_ne!(changed, first);
        // Without a readable mtime the host hashes every time instead of
        // guessing that an unchanged file is unchanged.
        tokio::fs::write(&path, b"payload-a\n").await.unwrap();
        let without_mtime = port.digest_for(&path, 10, None).await.unwrap();
        assert_eq!(without_mtime, first);
    }
}

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
    ProviderCachePurge, ProviderContentOrigin, RuleProviderCacheSnapshot,
};
use infiltrator_domain::rules::provider_store::{
    MAX_PROVIDER_FILE_BYTES, PROVIDER_CACHE_DIR_NAME, ProviderSourceKind, RuleProviderDeclaration,
    provider_source_candidates,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::rule_provider_cache::{ProviderCacheEntry, RuleProviderCachePort};
use std::path::{Path, PathBuf};

/// File-system adapter over the kernel's local provider files.
#[derive(Clone, Debug)]
pub struct DesktopRuleProviderCache {
    home: PathBuf,
}

impl DesktopRuleProviderCache {
    pub fn new(home: PathBuf) -> Self {
        Self { home }
    }

    /// The kernel's provider cache directory, whether or not it exists yet.
    pub fn cache_dir(&self) -> PathBuf {
        self.home.join(PROVIDER_CACHE_DIR_NAME)
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
}

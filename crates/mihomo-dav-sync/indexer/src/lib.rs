use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use tokio::fs;
use std::time::SystemTime;

pub struct LocalEntry {
    pub path: PathBuf,
    pub relative_path: String,
    pub hash: String,
    pub last_modified: DateTime<Utc>,
    pub size: u64,
}

pub struct Indexer;

impl Indexer {
    /// Recursively scan a directory and calculate file hashes
    pub async fn scan(root: &Path) -> Result<Vec<LocalEntry>> {
        let mut entries = Vec::new();
        Self::scan_recursive(root, root, &mut entries).await?;
        Ok(entries)
    }

    async fn scan_recursive(
        root: &Path,
        current: &Path,
        entries: &mut Vec<LocalEntry>,
    ) -> Result<()> {
        let mut reader = fs::read_dir(current).await?;

        while let Some(entry) = reader.next_entry().await? {
            let path = entry.path();
            let metadata = entry.metadata().await?;

            if metadata.is_dir() {
                Box::pin(Self::scan_recursive(root, &path, entries)).await?;
            } else if metadata.is_file() {
                // 只同步 YAML 配置文件和特定的 settings
                let extension = path.extension().and_then(|s| s.to_str());
                if extension == Some("yaml") || extension == Some("yml") || extension == Some("toml") {
                    let relative_path = path
                        .strip_prefix(root)?
                        .to_string_lossy()
                        .replace('\\', "/");

                    let content = fs::read(&path).await?;
                    let hash = format!("{:x}", md5::compute(&content));
                    
                    let last_modified: DateTime<Utc> = metadata.modified() 
                        .unwrap_or_else(|_| SystemTime::now())
                        .into();

                    entries.push(LocalEntry {
                        path,
                        relative_path,
                        hash,
                        last_modified,
                        size: metadata.len(),
                    });
                }
            }
        }
        Ok(())
    }
}
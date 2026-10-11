use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteEntry {
    pub path: String,
    pub etag: String,
    pub last_modified: DateTime<Utc>,
    pub is_dir: bool,
    pub size: u64,
}

#[async_trait]
pub trait DavClient: Send + Sync {
    /// Recursively list directory contents (Depth: 1)
    async fn list(&self, path: &str) -> Result<Vec<RemoteEntry>>;

    /// Download file content
    async fn get(&self, path: &str) -> Result<Vec<u8>>;

    /// Upload file content with atomicity and If-Match support
    async fn put(&self, path: &str, data: &[u8], if_match: Option<&str>) -> Result<String>;

    /// Create a file only when the destination does not exist. Adapters which
    /// cannot provide WebDAV `If-None-Match: *` semantics must fail closed.
    async fn put_if_absent(&self, _path: &str, _data: &[u8]) -> Result<String> {
        Err(anyhow::anyhow!(
            "conditional WebDAV create is not supported by this adapter"
        ))
    }

    /// Delete a file or directory
    async fn delete(&self, path: &str) -> Result<()>;

    /// Delete a file only if its current strong ETag still matches the
    /// observed `RemoteEntry::etag` value (quotes removed). Adapters without
    /// conditional delete must fail closed.
    async fn delete_if_match(&self, _path: &str, _etag: &str) -> Result<()> {
        Err(anyhow::anyhow!(
            "conditional WebDAV delete is not supported by this adapter"
        ))
    }

    /// Move/Rename a file
    async fn move_item(&self, from: &str, to: &str) -> Result<()>;

    /// Create directory
    async fn mkdir(&self, path: &str) -> Result<()>;
}

pub mod client;
pub mod xml_parser;

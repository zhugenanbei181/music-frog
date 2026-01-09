use anyhow::Result;
use tokio::fs;
use chrono::Utc;
use tracing::{info, warn};

use dav_client::DavClient;
use state_store::{StateStore, SyncStateRow};
use crate::SyncAction;

pub struct SyncExecutor<'a> {
    dav: &'a dyn DavClient,
    store: &'a StateStore,
}

impl<'a> SyncExecutor<'a> {
    pub fn new(dav: &'a dyn DavClient, store: &'a StateStore) -> Self {
        Self { dav, store }
    }

    pub async fn execute(&self, action: SyncAction) -> Result<()> {
        match action {
            SyncAction::Upload { local, remote_path, last_etag } => {
                info!("Uploading: {:?}", local);
                let content = fs::read(&local).await?;
                let hash = format!("{:x}", md5::compute(&content));
                
                let new_etag = self.dav.put(&remote_path, &content, last_etag.as_deref()).await?;
                
                self.store.upsert_state(SyncStateRow {
                    path: remote_path,
                    last_etag: new_etag,
                    last_hash: hash,
                    last_sync_at: Utc::now(),
                    is_tombstone: 0,
                }).await?;
            }
            
            SyncAction::Download { remote_path, local, remote_etag } => {
                info!("Downloading: {}", remote_path);
                let content = self.dav.get(&remote_path).await?;
                let hash = format!("{:x}", md5::compute(&content));
                
                let tmp_path = local.with_extension("sync-tmp");
                if let Some(parent) = local.parent() {
                    fs::create_dir_all(parent).await?;
                }
                fs::write(&tmp_path, &content).await?;
                fs::rename(&tmp_path, &local).await?;

                self.store.upsert_state(SyncStateRow {
                    path: remote_path,
                    last_etag: remote_etag,
                    last_hash: hash,
                    last_sync_at: Utc::now(),
                    is_tombstone: 0,
                }).await?;
            }

            SyncAction::Conflict { local, remote_path } => {
                warn!("Conflict detected for: {}", remote_path);
                let content = self.dav.get(&remote_path).await?;
                let bak_path = local.with_extension(format!("remote-bak-{}", Utc::now().format("%Y%m%d%H%M%S")));
                fs::write(&bak_path, &content).await?;
                info!("Saved remote version to: {:?}", bak_path);
            }

            SyncAction::DeleteRemote { remote_path, .. } => {
                info!("Deleting remote: {}", remote_path);
                self.dav.delete(&remote_path).await?;
                self.store.delete_state(&remote_path).await?;
            }

            SyncAction::DeleteLocal { local, remote_path } => {
                info!("Deleting local: {:?}", local);
                if local.exists() {
                    fs::remove_file(&local).await?;
                }
                self.store.delete_state(&remote_path).await?;
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use crate::SyncAction;
    use crate::executor::SyncExecutor;
    use anyhow::Result;
    use async_trait::async_trait;
    use dav_client::{DavClient, RemoteEntry};
    use state_store::{StateStore, SyncStateRow};
    #[cfg(test)]
    use std::fs::read_dir;
    #[cfg(test)]
    use std::fs::read_to_string;
    #[cfg(test)]
    use std::fs::write;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    struct MockDav {
        put_called: Arc<Mutex<bool>>,
        get_content: Vec<u8>,
    }

    #[async_trait]
    impl DavClient for MockDav {
        async fn list(&self, _path: &str) -> Result<Vec<RemoteEntry>> {
            Ok(vec![])
        }
        async fn get(&self, _path: &str) -> Result<Vec<u8>> {
            Ok(self.get_content.clone())
        }
        async fn put(
            &self,
            _path: &str,
            _content: &[u8],
            _last_etag: Option<&str>,
        ) -> Result<String> {
            *self.put_called.lock().unwrap() = true;
            Ok("new_etag".to_string())
        }
        async fn delete(&self, _path: &str) -> Result<()> {
            Ok(())
        }
        async fn move_item(&self, _from: &str, _to: &str) -> Result<()> {
            Ok(())
        }
        async fn mkdir(&self, _path: &str) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_execute_upload_updates_store() {
        let temp = tempdir().unwrap();
        let local_file = temp.path().join("test.yaml");
        write(&local_file, "content").unwrap();

        // Use :memory: instead of new_in_memory which is internal to state-store tests
        let store = StateStore::new(":memory:").await.unwrap();
        let put_called = Arc::new(Mutex::new(false));
        let dav = MockDav {
            put_called: put_called.clone(),
            get_content: vec![],
        };
        let executor = SyncExecutor::new(&dav, &store);

        let action = SyncAction::Upload {
            local: local_file,
            remote_path: "remote.yaml".to_string(),
            last_etag: None,
        };

        executor.execute(action).await.unwrap();

        assert!(*put_called.lock().unwrap());
        let state = store.get_state("remote.yaml").await.unwrap().unwrap();
        assert_eq!(state.last_etag, "new_etag");
    }

    #[tokio::test]
    async fn test_execute_download_atomic_rename() {
        let temp = tempdir().unwrap();
        let local_file = temp.path().join("config.yaml");

        let store = StateStore::new(":memory:").await.unwrap();
        let dav = MockDav {
            put_called: Arc::new(Mutex::new(false)),
            get_content: b"remote_content".to_vec(),
        };
        let executor = SyncExecutor::new(&dav, &store);

        let action = SyncAction::Download {
            remote_path: "remote.yaml".to_string(),
            local: local_file.clone(),
            remote_etag: "etag_123".to_string(),
        };

        executor.execute(action).await.unwrap();

        // 验证文件内容
        let content = read_to_string(&local_file).unwrap();
        assert_eq!(content, "remote_content");

        // 验证临时文件已清理
        assert!(!local_file.with_extension("sync-tmp").exists());

        // 验证数据库
        let state = store.get_state("remote.yaml").await.unwrap().unwrap();
        assert_eq!(state.last_etag, "etag_123");
    }

    #[tokio::test]
    async fn test_execute_conflict_creates_backup() {
        let temp = tempdir().unwrap();
        let local_file = temp.path().join("my_config.yaml");
        write(&local_file, "local_data").unwrap();

        let store = StateStore::new(":memory:").await.unwrap();
        let dav = MockDav {
            put_called: Arc::new(Mutex::new(false)),
            get_content: b"remote_data".to_vec(),
        };
        let executor = SyncExecutor::new(&dav, &store);

        let action = SyncAction::Conflict {
            local: local_file.clone(),
            remote_path: "remote.yaml".to_string(),
        };

        executor.execute(action).await.unwrap();

        // 本地原始文件应保持不变
        assert_eq!(read_to_string(&local_file).unwrap(), "local_data");

        // 检查是否存在备份文件
        let mut found_bak = false;
        for entry in read_dir(temp.path()).unwrap() {
            let path = entry.unwrap().path();
            if path.to_string_lossy().contains("remote-bak-") {
                assert_eq!(read_to_string(path).unwrap(), "remote_data");
                found_bak = true;
            }
        }
        assert!(found_bak, "Backup file for conflict should be created");
    }

    #[tokio::test]
    async fn test_execute_delete_remote_updates_store() {
        let store = StateStore::new(":memory:").await.unwrap();
        store
            .upsert_state(SyncStateRow {
                path: "gone.yaml".into(),
                last_etag: "e1".into(),
                last_hash: "h1".into(),
                last_sync_at: chrono::Utc::now(),
                is_tombstone: 0,
            })
            .await
            .unwrap();

        let dav = MockDav {
            put_called: Arc::new(Mutex::new(false)),
            get_content: vec![],
        };
        let executor = SyncExecutor::new(&dav, &store);

        executor
            .execute(SyncAction::DeleteRemote {
                remote_path: "gone.yaml".into(),
                last_etag: "e1".into(),
            })
            .await
            .unwrap();

        assert!(store.get_state("gone.yaml").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_execute_mkdir_logic() {
        // Since execute matches on SyncAction, and mkdir is not a SyncAction variant,
        // it's internal to Planner or other components.
        // Let's test the md5 computation integrity in Upload.
        let temp = tempdir().unwrap();
        let local = temp.path().join("md5.txt");
        write(&local, "abc").unwrap(); // md5 of abc is 900150983cd24fb0d6963f7d28e17f72

        let store = StateStore::new(":memory:").await.unwrap();
        let dav = MockDav {
            put_called: Arc::new(Mutex::new(false)),
            get_content: vec![],
        };
        let executor = SyncExecutor::new(&dav, &store);

        executor
            .execute(SyncAction::Upload {
                local,
                remote_path: "md5.txt".into(),
                last_etag: None,
            })
            .await
            .unwrap();

        let state = store.get_state("md5.txt").await.unwrap().unwrap();
        assert_eq!(state.last_hash, "900150983cd24fb0d6963f7d28e17f72");
    }
}

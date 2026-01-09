use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::collections::HashSet;

pub mod executor;

use dav_client::{DavClient, RemoteEntry};
use state_store::{StateStore, SyncStateRow};
use indexer::{Indexer, LocalEntry};

#[derive(Debug, Clone, PartialEq)]
pub enum SyncAction {
    Upload { local: PathBuf, remote_path: String, last_etag: Option<String> },
    Download { remote_path: String, local: PathBuf, remote_etag: String },
    Conflict { local: PathBuf, remote_path: String },
    DeleteRemote { remote_path: String, last_etag: String },
    DeleteLocal { local: PathBuf, remote_path: String },
}

pub struct SyncPlanner<'a> {
    local_root: PathBuf,
    remote_base: String,
    dav: &'a dyn DavClient,
    store: &'a StateStore,
}

impl<'a> SyncPlanner<'a> {
    pub fn new(local_root: PathBuf, remote_base: String, dav: &'a dyn DavClient, store: &'a StateStore) -> Self {
        Self { local_root, remote_base, dav, store }
    }

    pub async fn build_plan(&self) -> Result<Vec<SyncAction>> {
        let locals = Indexer::scan(&self.local_root).await?;
        let remotes = self.dav.list(&self.remote_base).await?;
        let states = self.store.get_all_states().await?;

        let local_map: HashMap<String, LocalEntry> = locals.into_iter()
            .map(|e| (e.relative_path.clone(), e))
            .collect();

        let remote_map: HashMap<String, RemoteEntry> = remotes.into_iter()
            .filter(|e| !e.is_dir)
            .map(|e| (self.normalize_remote_path(&e.path), e))
            .collect();

        let state_map: HashMap<String, SyncStateRow> = states.into_iter()
            .map(|s| (s.path.clone(), s))
            .collect();

        let mut actions = Vec::new();
        let all_paths: HashSet<String> = local_map.keys().cloned()
            .chain(remote_map.keys().cloned())
            .chain(state_map.keys().cloned())
            .collect();

        for path in all_paths {
            let l = local_map.get(&path);
            let r = remote_map.get(&path);
            let s = state_map.get(&path);

            match (l, r, s) {
                // 1. 本地新增
                (Some(l_val), None, None) => {
                    actions.push(SyncAction::Upload {
                        local: l_val.path.clone(),
                        remote_path: path.clone(),
                        last_etag: None,
                    });
                }
                // 2. 远端新增
                (None, Some(r_val), None) => {
                    actions.push(SyncAction::Download {
                        remote_path: path.clone(),
                        local: self.local_root.join(&path),
                        remote_etag: r_val.etag.clone(),
                    });
                }
                // 3. 正常状态检查
                (Some(l_val), Some(r_val), Some(s_val)) => {
                    let l_changed = l_val.hash != s_val.last_hash;
                    let r_changed = r_val.etag != s_val.last_etag;

                    if l_changed && r_changed {
                        // 双向修改，检查内容是否一致（哈希 vs ETag 碰撞检查）
                        // 如果服务器不支持 Hash ETag，这里默认冲突
                        actions.push(SyncAction::Conflict {
                            local: l_val.path.clone(),
                            remote_path: path.clone(),
                        });
                    } else if l_changed {
                        actions.push(SyncAction::Upload {
                            local: l_val.path.clone(),
                            remote_path: path.clone(),
                            last_etag: Some(r_val.etag.clone()),
                        });
                    } else if r_changed {
                        actions.push(SyncAction::Download {
                            remote_path: path.clone(),
                            local: l_val.path.clone(),
                            remote_etag: r_val.etag.clone(),
                        });
                    }
                }
                // 4. 本地删除
                (None, Some(r_val), Some(s_val)) => {
                    if r_val.etag == s_val.last_etag {
                        actions.push(SyncAction::DeleteRemote {
                            remote_path: path.clone(),
                            last_etag: r_val.etag.clone(),
                        });
                    } else {
                        // 远端改了，恢复下载
                        actions.push(SyncAction::Download {
                            remote_path: path.clone(),
                            local: self.local_root.join(&path),
                            remote_etag: r_val.etag.clone(),
                        });
                    }
                }
                // 5. 远端删除
                (Some(l_val), None, Some(s_val)) => {
                    if l_val.hash == s_val.last_hash {
                        actions.push(SyncAction::DeleteLocal {
                            local: l_val.path.clone(),
                            remote_path: path.clone(),
                        });
                    } else {
                        // 本地改了，重新上传
                        actions.push(SyncAction::Upload {
                            local: l_val.path.clone(),
                            remote_path: path.clone(),
                            last_etag: None,
                        });
                    }
                }
                _ => {}
            }
        }

        Ok(actions)
    }

    fn normalize_remote_path(&self, full_path: &str) -> String {
        full_path.trim_start_matches(&self.remote_base)
            .trim_start_matches('/')
            .to_string()
    }
}

/// Utility module for building sync plans from in-memory data (for testing)
#[cfg(test)]
pub mod test_utils {
    use super::*;
    use chrono::Utc;

    /// Build sync actions directly from provided data without I/O
    pub fn build_plan_from_data(
        local_root: PathBuf,
        locals: Vec<LocalEntry>,
        remotes: Vec<RemoteEntry>,
        states: Vec<SyncStateRow>,
    ) -> Vec<SyncAction> {
        let local_map: HashMap<String, LocalEntry> = locals.into_iter()
            .map(|e| (e.relative_path.clone(), e))
            .collect();

        let remote_map: HashMap<String, RemoteEntry> = remotes.into_iter()
            .filter(|e| !e.is_dir)
            .map(|e| (e.path.trim_start_matches('/').to_string(), e))
            .collect();

        let state_map: HashMap<String, SyncStateRow> = states.into_iter()
            .map(|s| (s.path.clone(), s))
            .collect();

        let mut actions = Vec::new();
        let all_paths: HashSet<String> = local_map.keys().cloned()
            .chain(remote_map.keys().cloned())
            .chain(state_map.keys().cloned())
            .collect();

        for path in all_paths {
            let l = local_map.get(&path);
            let r = remote_map.get(&path);
            let s = state_map.get(&path);

            match (l, r, s) {
                (Some(l_val), None, None) => {
                    actions.push(SyncAction::Upload {
                        local: l_val.path.clone(),
                        remote_path: path.clone(),
                        last_etag: None,
                    });
                }
                (None, Some(r_val), None) => {
                    actions.push(SyncAction::Download {
                        remote_path: path.clone(),
                        local: local_root.join(&path),
                        remote_etag: r_val.etag.clone(),
                    });
                }
                (Some(l_val), Some(r_val), Some(s_val)) => {
                    let l_changed = l_val.hash != s_val.last_hash;
                    let r_changed = r_val.etag != s_val.last_etag;

                    if l_changed && r_changed {
                        actions.push(SyncAction::Conflict {
                            local: l_val.path.clone(),
                            remote_path: path.clone(),
                        });
                    } else if l_changed {
                        actions.push(SyncAction::Upload {
                            local: l_val.path.clone(),
                            remote_path: path.clone(),
                            last_etag: Some(r_val.etag.clone()),
                        });
                    } else if r_changed {
                        actions.push(SyncAction::Download {
                            remote_path: path.clone(),
                            local: l_val.path.clone(),
                            remote_etag: r_val.etag.clone(),
                        });
                    }
                }
                (None, Some(r_val), Some(s_val)) => {
                    if r_val.etag == s_val.last_etag {
                        actions.push(SyncAction::DeleteRemote {
                            remote_path: path.clone(),
                            last_etag: r_val.etag.clone(),
                        });
                    } else {
                        actions.push(SyncAction::Download {
                            remote_path: path.clone(),
                            local: local_root.join(&path),
                            remote_etag: r_val.etag.clone(),
                        });
                    }
                }
                (Some(l_val), None, Some(s_val)) => {
                    if l_val.hash == s_val.last_hash {
                        actions.push(SyncAction::DeleteLocal {
                            local: l_val.path.clone(),
                            remote_path: path.clone(),
                        });
                    } else {
                        actions.push(SyncAction::Upload {
                            local: l_val.path.clone(),
                            remote_path: path.clone(),
                            last_etag: None,
                        });
                    }
                }
                _ => {}
            }
        }

        actions
    }

    pub fn make_local_entry(path: &str, hash: &str) -> LocalEntry {
        LocalEntry {
            path: PathBuf::from(path),
            relative_path: path.to_string(),
            hash: hash.to_string(),
            last_modified: Utc::now(),
            size: 100,
        }
    }

    pub fn make_remote_entry(path: &str, etag: &str) -> RemoteEntry {
        RemoteEntry {
            path: path.to_string(),
            etag: etag.to_string(),
            last_modified: Utc::now(),
            is_dir: false,
            size: 100,
        }
    }

    pub fn make_state_row(path: &str, etag: &str, hash: &str) -> SyncStateRow {
        SyncStateRow {
            path: path.to_string(),
            last_etag: etag.to_string(),
            last_hash: hash.to_string(),
            last_sync_at: Utc::now(),
            is_tombstone: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::test_utils::*;

    #[test]
    fn test_local_new_file_should_upload() {
        let local_root = PathBuf::from("/configs");
        let locals = vec![make_local_entry("new.yaml", "hash123")];
        let remotes = vec![];
        let states = vec![];

        let actions = build_plan_from_data(local_root, locals, remotes, states);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::Upload { remote_path, last_etag, .. } => {
                assert_eq!(remote_path, "new.yaml");
                assert!(last_etag.is_none());
            }
            _ => panic!("Expected Upload action"),
        }
    }

    #[test]
    fn test_remote_new_file_should_download() {
        let local_root = PathBuf::from("/configs");
        let locals = vec![];
        let remotes = vec![make_remote_entry("new.yaml", "etag123")];
        let states = vec![];

        let actions = build_plan_from_data(local_root, locals, remotes, states);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::Download { remote_path, remote_etag, .. } => {
                assert_eq!(remote_path, "new.yaml");
                assert_eq!(remote_etag, "etag123");
            }
            _ => panic!("Expected Download action"),
        }
    }

    #[test]
    fn test_local_modified_should_upload() {
        let local_root = PathBuf::from("/configs");
        let locals = vec![make_local_entry("config.yaml", "new_hash")];
        let remotes = vec![make_remote_entry("config.yaml", "old_etag")];
        let states = vec![make_state_row("config.yaml", "old_etag", "old_hash")];

        let actions = build_plan_from_data(local_root, locals, remotes, states);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::Upload { remote_path, last_etag, .. } => {
                assert_eq!(remote_path, "config.yaml");
                assert_eq!(last_etag.as_deref(), Some("old_etag"));
            }
            _ => panic!("Expected Upload action"),
        }
    }

    #[test]
    fn test_remote_modified_should_download() {
        let local_root = PathBuf::from("/configs");
        let locals = vec![make_local_entry("config.yaml", "old_hash")];
        let remotes = vec![make_remote_entry("config.yaml", "new_etag")];
        let states = vec![make_state_row("config.yaml", "old_etag", "old_hash")];

        let actions = build_plan_from_data(local_root, locals, remotes, states);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::Download { remote_path, remote_etag, .. } => {
                assert_eq!(remote_path, "config.yaml");
                assert_eq!(remote_etag, "new_etag");
            }
            _ => panic!("Expected Download action"),
        }
    }

    #[test]
    fn test_both_modified_should_conflict() {
        let local_root = PathBuf::from("/configs");
        let locals = vec![make_local_entry("config.yaml", "new_local_hash")];
        let remotes = vec![make_remote_entry("config.yaml", "new_remote_etag")];
        let states = vec![make_state_row("config.yaml", "old_etag", "old_hash")];

        let actions = build_plan_from_data(local_root, locals, remotes, states);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::Conflict { remote_path, .. } => {
                assert_eq!(remote_path, "config.yaml");
            }
            _ => panic!("Expected Conflict action"),
        }
    }

    #[test]
    fn test_local_deleted_remote_unchanged_should_delete_remote() {
        let local_root = PathBuf::from("/configs");
        let locals = vec![];
        let remotes = vec![make_remote_entry("deleted.yaml", "same_etag")];
        let states = vec![make_state_row("deleted.yaml", "same_etag", "old_hash")];

        let actions = build_plan_from_data(local_root, locals, remotes, states);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::DeleteRemote { remote_path, last_etag } => {
                assert_eq!(remote_path, "deleted.yaml");
                assert_eq!(last_etag, "same_etag");
            }
            _ => panic!("Expected DeleteRemote action"),
        }
    }

    #[test]
    fn test_remote_deleted_local_unchanged_should_delete_local() {
        let local_root = PathBuf::from("/configs");
        let locals = vec![make_local_entry("deleted.yaml", "same_hash")];
        let remotes = vec![];
        let states = vec![make_state_row("deleted.yaml", "old_etag", "same_hash")];

        let actions = build_plan_from_data(local_root, locals, remotes, states);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::DeleteLocal { remote_path, .. } => {
                assert_eq!(remote_path, "deleted.yaml");
            }
            _ => panic!("Expected DeleteLocal action"),
        }
    }

    #[test]
    fn test_no_changes_no_actions() {
        let local_root = PathBuf::from("/configs");
        let locals = vec![make_local_entry("config.yaml", "same_hash")];
        let remotes = vec![make_remote_entry("config.yaml", "same_etag")];
        let states = vec![make_state_row("config.yaml", "same_etag", "same_hash")];

        let actions = build_plan_from_data(local_root, locals, remotes, states);

        assert!(actions.is_empty());
    }

    #[test]
    fn test_local_deleted_but_remote_changed_should_download() {
        let local_root = PathBuf::from("/configs");
        let locals = vec![];
        let remotes = vec![make_remote_entry("file.yaml", "new_etag")];
        let states = vec![make_state_row("file.yaml", "old_etag", "old_hash")];

        let actions = build_plan_from_data(local_root, locals, remotes, states);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::Download { remote_path, remote_etag, .. } => {
                assert_eq!(remote_path, "file.yaml");
                assert_eq!(remote_etag, "new_etag");
            }
            _ => panic!("Expected Download action (remote changed while local deleted)"),
        }
    }

    #[test]
    fn test_remote_deleted_but_local_changed_should_upload() {
        let local_root = PathBuf::from("/configs");
        let locals = vec![make_local_entry("file.yaml", "new_hash")];
        let remotes = vec![];
        let states = vec![make_state_row("file.yaml", "old_etag", "old_hash")];

        let actions = build_plan_from_data(local_root, locals, remotes, states);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::Upload { remote_path, last_etag, .. } => {
                assert_eq!(remote_path, "file.yaml");
                assert!(last_etag.is_none());
            }
            _ => panic!("Expected Upload action (local changed while remote deleted)"),
        }
    }
}

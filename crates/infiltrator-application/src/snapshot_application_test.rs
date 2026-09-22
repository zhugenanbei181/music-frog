//! DUAL-09-08/09: snapshot diff and restore behavior over fake ports.
//!
//! These tests prove the shared application computes the diff from real
//! snapshot bytes, publishes it with its storage identity, and clears the
//! cache after a restore — the facts both surfaces render.
//!
//! `await_holding_lock` is allowed: the process-wide caches must stay stable
//! between the act and the assert, and these tests use a current-thread
//! runtime, so holding the guard across an await cannot deadlock.
#![allow(clippy::await_holding_lock)]

use super::*;
use async_trait::async_trait;
use infiltrator_domain::profiles::ProfileMetadata;
use infiltrator_ports::error::PortError;
use std::collections::BTreeMap;
use std::sync::Mutex;

#[derive(Default)]
struct FakeProfileStore {
    current: Mutex<String>,
    profiles: Mutex<BTreeMap<String, (String, ProfileMetadata)>>,
}

impl FakeProfileStore {
    fn with_profile(name: &str, content: &str) -> Self {
        let mut profiles = BTreeMap::new();
        profiles.insert(
            name.to_string(),
            (content.to_string(), ProfileMetadata::default()),
        );
        Self {
            current: Mutex::new(name.to_string()),
            profiles: Mutex::new(profiles),
        }
    }
}

#[async_trait]
impl ProfileStore for FakeProfileStore {
    fn config_dir(&self) -> std::path::PathBuf {
        std::path::PathBuf::from("/fake/configs")
    }

    async fn list_profiles(
        &self,
    ) -> Result<Vec<infiltrator_domain::profiles::ProfileInfo>, PortError> {
        Ok(self
            .profiles
            .lock()
            .expect("profiles lock")
            .iter()
            .map(
                |(name, (_content, metadata))| infiltrator_domain::profiles::ProfileInfo {
                    name: name.clone(),
                    active: true,
                    path: format!("/fake/configs/{name}.yaml"),
                    subscription_url: metadata.subscription_url.clone(),
                    ..Default::default()
                },
            )
            .collect())
    }

    async fn get_current(&self) -> Result<String, PortError> {
        Ok(self.current.lock().expect("current lock").clone())
    }

    async fn set_current(&self, profile: &str) -> Result<(), PortError> {
        *self.current.lock().expect("current lock") = profile.to_string();
        Ok(())
    }

    async fn load(&self, profile: &str) -> Result<String, PortError> {
        self.profiles
            .lock()
            .expect("profiles lock")
            .get(profile)
            .map(|(content, _)| content.clone())
            .ok_or_else(|| PortError::NotFound(profile.to_string()))
    }

    async fn save(&self, profile: &str, content: &str) -> Result<(), PortError> {
        self.profiles
            .lock()
            .expect("profiles lock")
            .entry(profile.to_string())
            .or_insert_with(|| (String::new(), ProfileMetadata::default()))
            .0 = content.to_string();
        Ok(())
    }

    async fn delete_profile(&self, profile: &str) -> Result<(), PortError> {
        self.profiles
            .lock()
            .expect("profiles lock")
            .remove(profile)
            .map(|_| ())
            .ok_or_else(|| PortError::NotFound(profile.to_string()))
    }

    async fn get_profile_metadata(&self, profile: &str) -> Result<ProfileMetadata, PortError> {
        self.profiles
            .lock()
            .expect("profiles lock")
            .get(profile)
            .map(|(_, metadata)| metadata.clone())
            .ok_or_else(|| PortError::NotFound(profile.to_string()))
    }

    async fn update_profile_metadata(
        &self,
        profile: &str,
        metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        self.profiles
            .lock()
            .expect("profiles lock")
            .get_mut(profile)
            .map(|entry| entry.1 = metadata.clone())
            .ok_or_else(|| PortError::NotFound(profile.to_string()))
    }

    async fn delete_subscription_credential(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn load_options(
        &self,
        _profile: &str,
    ) -> Result<infiltrator_domain::profile_options::ProfileOptions, PortError> {
        Ok(Default::default())
    }

    async fn save_options(
        &self,
        _profile: &str,
        _options: &infiltrator_domain::profile_options::ProfileOptions,
    ) -> Result<(), PortError> {
        Ok(())
    }

    async fn delete_options(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn clear_backup(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn restore_backup(&self, _profile: &str) -> Result<bool, PortError> {
        Ok(false)
    }
}

#[derive(Default)]
struct FakeSnapshotStore {
    snapshots: Mutex<BTreeMap<String, Vec<(SnapshotMeta, String)>>>,
}

impl FakeSnapshotStore {
    fn with_snapshot(profile: &str, millis: i64, content: &str) -> Self {
        Self::with_snapshots(profile, [(millis, content.to_string())])
    }

    fn with_snapshots(profile: &str, snapshots: impl IntoIterator<Item = (i64, String)>) -> Self {
        let store = Self::default();
        {
            let mut guard = store.snapshots.lock().expect("snapshots lock");
            let entry = guard.entry(profile.to_string()).or_default();
            for (millis, content) in snapshots {
                let timestamp =
                    chrono::DateTime::from_timestamp_millis(millis).expect("valid timestamp");
                let path = std::path::PathBuf::from(format!(
                    "/fake/configs/{profile}-history/{millis}-deadbeef.yaml"
                ));
                entry.push((
                    SnapshotMeta {
                        profile: profile.to_string(),
                        timestamp,
                        sha256: infiltrator_domain::snapshots::content_hash(content.as_bytes()),
                        path,
                    },
                    content.to_string(),
                ));
            }
        }
        store
    }
}

#[async_trait]
impl SnapshotStore for FakeSnapshotStore {
    async fn save(&self, profile: &str, content: &str) -> Result<SnapshotMeta, PortError> {
        let millis = 1_750_000_000_000;
        let timestamp = chrono::DateTime::from_timestamp_millis(millis).expect("timestamp");
        let path = std::path::PathBuf::from(format!(
            "/fake/configs/{profile}-history/{millis}-cafebabe.yaml"
        ));
        let meta = SnapshotMeta {
            profile: profile.to_string(),
            timestamp,
            sha256: infiltrator_domain::snapshots::content_hash(content.as_bytes()),
            path,
        };
        self.snapshots
            .lock()
            .expect("snapshots lock")
            .entry(profile.to_string())
            .or_default()
            .push((meta.clone(), content.to_string()));
        Ok(meta)
    }

    async fn list(&self, profile: &str) -> Result<Vec<SnapshotMeta>, PortError> {
        Ok(self
            .snapshots
            .lock()
            .expect("snapshots lock")
            .get(profile)
            .map(|items| items.iter().map(|(meta, _)| meta.clone()).collect())
            .unwrap_or_default())
    }

    async fn read(&self, profile: &str, path: &Path) -> Result<String, PortError> {
        self.snapshots
            .lock()
            .expect("snapshots lock")
            .get(profile)
            .and_then(|items| {
                items
                    .iter()
                    .find(|(meta, _)| meta.path == path)
                    .map(|(_, content)| content.clone())
            })
            .ok_or_else(|| PortError::NotFound(path.display().to_string()))
    }

    async fn delete(&self, profile: &str, path: &Path) -> Result<(), PortError> {
        let mut guard = self.snapshots.lock().expect("snapshots lock");
        let items = guard
            .get_mut(profile)
            .ok_or_else(|| PortError::NotFound(profile.to_string()))?;
        let before = items.len();
        items.retain(|(meta, _)| meta.path != path);
        if items.len() == before {
            return Err(PortError::NotFound(path.display().to_string()));
        }
        Ok(())
    }
}

/// The diff/history caches are process-wide; tests that assert on them take
/// this lock so a parallel test cannot publish between the act and the assert.
fn cache_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn application(profile_content: &str, snapshot_content: &str) -> SnapshotApplication {
    SnapshotApplication::new(
        Arc::new(FakeProfileStore::with_profile("main", profile_content)),
        Arc::new(FakeSnapshotStore::with_snapshot(
            "main",
            1_750_000_000_000,
            snapshot_content,
        )),
    )
}

#[tokio::test]
async fn diff_newest_publishes_the_real_diff_and_its_snapshot_path() {
    let _cache = cache_lock();
    let app = application(
        "port: 7890\nmode: global\n# 用户注释\n",
        "port: 7890\nmode: rule\n# 用户注释\n",
    );
    let diff = app
        .diff_newest("main")
        .await
        .expect("diff")
        .expect("newest snapshot exists");

    assert_eq!(diff.stats.modifications, 1, "mode changed");
    assert_eq!(diff.target_id, "main");
    assert!(diff.fidelity_preserved, "the shared comment line survives");
    assert!(
        diff.source_path
            .as_deref()
            .is_some_and(|path| path.ends_with(".yaml")),
        "the diff carries the storage identity needed for rollback"
    );
    assert!(
        diff.unified_lines
            .iter()
            .any(|line| line.content.contains("mode: global")),
        "unified rows are the real current content"
    );
    assert_eq!(
        last_snapshot_diff().and_then(|cached| cached.source_path),
        diff.source_path,
        "the process-wide cache exposes the same diff to Bevy"
    );
}

#[tokio::test]
async fn diff_newest_is_none_without_history_and_clears_the_cache() {
    let _cache = cache_lock();
    let app = SnapshotApplication::new(
        Arc::new(FakeProfileStore::with_profile("main", "mode: rule\n")),
        Arc::new(FakeSnapshotStore::default()),
    );
    assert!(app.diff_newest("main").await.expect("diff").is_none());
    assert!(last_snapshot_diff().is_none());
}

#[tokio::test]
async fn restore_clears_the_cached_diff() {
    let _cache = cache_lock();
    let app = application("port: 7890\nmode: global\n", "port: 7890\nmode: rule\n");
    let diff = app.diff_newest("main").await.expect("diff").expect("diff");
    let path = diff.source_path.expect("path");
    assert!(last_snapshot_diff().is_some());

    app.restore(
        None::<Arc<dyn infiltrator_ports::runtime_gateway::ManagedRuntime>>,
        "main",
        Path::new(&path),
    )
    .await
    .expect("restore");
    assert!(
        last_snapshot_diff().is_none(),
        "a restore makes the cached diff stale by definition"
    );
}

#[tokio::test]
async fn creating_a_snapshot_clears_the_stale_diff_cache() {
    let _cache = cache_lock();
    let app = application("port: 7890\nmode: rule\n", "port: 7890\nmode: rule\n");
    publish_snapshot_diff(infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot::demo_fixture());
    assert!(last_snapshot_diff().is_some());
    app.create("main").await.expect("create snapshot");
    assert!(last_snapshot_diff().is_none());
}

#[tokio::test]
async fn history_reports_duplicates_and_the_shared_prune_view() {
    let _cache = cache_lock();
    let app = SnapshotApplication::new(
        Arc::new(FakeProfileStore::with_profile("main", "port: 2\n")),
        Arc::new(FakeSnapshotStore::with_snapshots(
            "main",
            [
                (1_750_000_000_000, "port: 1\n".to_string()),
                (1_750_000_000_100, "port: 1\n".to_string()),
                (1_750_000_000_200, "port: 2\n".to_string()),
            ],
        )),
    );

    let history = app.history("main", 20).await.expect("history");
    assert_eq!(history.entries.len(), 3);
    assert!(history.entries[0].is_newest);
    assert!(
        history.entries[0].timestamp_millis > history.entries[1].timestamp_millis,
        "entries are newest first"
    );
    assert_eq!(history.duplicate_entries, 1, "one older duplicate copy");
    assert_eq!(history.pending_prune, 1, "the older duplicate is prunable");
    assert_eq!(history.keep_limit, 20);
    assert_eq!(history.entries[1].short_hash().len(), 8);
    assert!(history.summary_zh().contains("待修剪 1 份"));
    assert!(
        last_snapshot_history().is_some_and(|cached| cached.profile == "main"),
        "the shared history is published for the Bevy projection"
    );
}

#[tokio::test]
async fn prune_executes_the_shared_policy_and_publishes_the_report() {
    let _cache = cache_lock();
    let store = Arc::new(FakeSnapshotStore::with_snapshots(
        "main",
        [
            (1_750_000_000_000, "port: 1\n".to_string()),
            (1_750_000_000_100, "port: 1\n".to_string()),
            (1_750_000_000_200, "port: 2\n".to_string()),
            (1_750_000_000_300, "port: 3\n".to_string()),
        ],
    ));
    let app = SnapshotApplication::new(
        Arc::new(FakeProfileStore::with_profile("main", "port: 3\n")),
        store.clone(),
    );

    let report = app
        .prune("main", 20, SnapshotPruneSource::Manual)
        .await
        .expect("prune");
    assert_eq!(report.removed, 1, "only the older duplicate is removed");
    assert_eq!(report.keep_limit, 20);
    assert_eq!(report.source, SnapshotPruneSource::Manual);
    assert_eq!(store.list("main").await.expect("list").len(), 3);

    // The keep limit is the second half of the shared policy.
    let limited = app
        .prune("main", 1, SnapshotPruneSource::Apply)
        .await
        .expect("prune to one");
    assert_eq!(limited.removed, 2);
    assert_eq!(store.list("main").await.expect("list").len(), 1);

    let history = last_snapshot_history().expect("history published");
    assert_eq!(history.entries.len(), 1);
    assert_eq!(history.pending_prune, 0);
    assert_eq!(history.last_prune.map(|report| report.removed), Some(2));
}

#[tokio::test]
async fn prune_clamps_the_requested_retention_to_the_supported_range() {
    let app = application("port: 7890\n", "port: 7890\n");
    let report = app
        .prune("main", 0, SnapshotPruneSource::Manual)
        .await
        .expect("prune");
    assert_eq!(
        report.keep_limit, 1,
        "0 is clamped to the documented minimum"
    );
}

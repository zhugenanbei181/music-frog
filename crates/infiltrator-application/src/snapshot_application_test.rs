//! DUAL-09-08/09: snapshot diff and restore behavior over fake ports.
//!
//! These tests prove the shared application computes the diff from real
//! snapshot bytes, publishes it with its storage identity, and clears the
//! cache after a restore — the facts both surfaces render.

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
    let app = SnapshotApplication::new(
        Arc::new(FakeProfileStore::with_profile("main", "mode: rule\n")),
        Arc::new(FakeSnapshotStore::default()),
    );
    assert!(app.diff_newest("main").await.expect("diff").is_none());
    assert!(last_snapshot_diff().is_none());
}

#[tokio::test]
async fn restore_clears_the_cached_diff() {
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
    let app = application("port: 7890\nmode: rule\n", "port: 7890\nmode: rule\n");
    publish_snapshot_diff(infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot::demo_fixture());
    assert!(last_snapshot_diff().is_some());
    app.create("main").await.expect("create snapshot");
    assert!(last_snapshot_diff().is_none());
}

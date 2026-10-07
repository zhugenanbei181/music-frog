//! DUAL-09-08/09: snapshot diff and restore behavior over fake ports.
//!
//! These tests prove the shared application computes the diff from real
//! snapshot bytes, publishes it with its storage identity, and clears the
//! cache after a restore — the facts both surfaces render.
//!
use super::*;
use crate::snapshot_presentation;
use async_trait::async_trait;
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_contract::profile_source::ProfileSourceIdentity;
use infiltrator_contract::snapshot_restore::SnapshotRestoreTarget;
use infiltrator_domain::profile_options::ProfileOptions;
use infiltrator_domain::profile_source::identify_profile_source;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_domain::snapshots::content_hash;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use infiltrator_ports::runtime_gateway::ManagedRuntime;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Default)]
struct FakeProfileStore {
    denied: AtomicBool,
    writes: AtomicUsize,
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
            ..Default::default()
        }
    }
}

#[async_trait]
impl ProfileStore for FakeProfileStore {
    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/fake/configs")
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        Ok(self
            .profiles
            .lock()
            .expect("profiles lock")
            .iter()
            .map(|(name, (_content, metadata))| ProfileInfo {
                name: name.clone(),
                active: true,
                path: format!("/fake/configs/{name}.yaml"),
                subscription_url: metadata.subscription_url.clone(),
                ..Default::default()
            })
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
        self.writes.fetch_add(1, Ordering::SeqCst);
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

    async fn load_options(&self, _profile: &str) -> Result<ProfileOptions, PortError> {
        Ok(Default::default())
    }

    async fn save_options(
        &self,
        _profile: &str,
        _options: &ProfileOptions,
    ) -> Result<(), PortError> {
        Ok(())
    }

    async fn delete_options(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn load_workspace(&self, profile: &str) -> Result<ProfileWorkspace, PortError> {
        let profiles = self.profiles.lock().unwrap();
        let content = &profiles
            .get(profile)
            .ok_or_else(|| PortError::NotFound(profile.into()))?
            .0;
        Ok(ProfileWorkspace {
            write_protection: ProfileWriteProtection::Editable,
            source: identify_profile_source(profile.into(), content, None),
            content: content.clone(),
            options: Default::default(),
            options_document: None,
        })
    }
    async fn compare_and_save_workspace(
        &self,
        expected: &ProfileSourceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        if self.denied.load(Ordering::SeqCst) {
            return Err(PortError::PermissionDenied(
                "fixture denies the commit".into(),
            ));
        }
        let mut profiles = self.profiles.lock().unwrap();
        let entry = profiles
            .get_mut(&expected.profile)
            .ok_or_else(|| PortError::NotFound(expected.profile.clone()))?;
        if identify_profile_source(expected.profile.clone(), &entry.0, None) != *expected {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "source changed",
                false,
            )));
        }
        self.writes.fetch_add(1, Ordering::SeqCst);
        entry.0 = update.content.clone();
        Ok(ProfileWorkspace {
            write_protection: ProfileWriteProtection::Editable,
            source: identify_profile_source(expected.profile.clone(), &update.content, None),
            content: update.content.clone(),
            options: update.options.clone(),
            options_document: None,
        })
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
                let path = PathBuf::from(format!(
                    "/fake/configs/{profile}-history/{millis}-deadbeef.yaml"
                ));
                entry.push((
                    SnapshotMeta {
                        profile: profile.to_string(),
                        timestamp,
                        sha256: content_hash(content.as_bytes()),
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
        let path = PathBuf::from(format!(
            "/fake/configs/{profile}-history/{millis}-cafebabe.yaml"
        ));
        let meta = SnapshotMeta {
            profile: profile.to_string(),
            timestamp,
            sha256: content_hash(content.as_bytes()),
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
        app.diff_observation(
            "main",
            &app.profiles
                .load_profile_detail("main")
                .await
                .unwrap()
                .content
        )
        .and_then(|cached| cached.source_path),
        diff.source_path,
        "the instance exposes its source-bound diff to its reader"
    );
}

#[tokio::test]
async fn diff_newest_is_none_without_history_and_clears_the_cache() {
    let app = SnapshotApplication::new(
        Arc::new(FakeProfileStore::with_profile("main", "mode: rule\n")),
        Arc::new(FakeSnapshotStore::default()),
    );
    assert!(app.diff_newest("main").await.expect("diff").is_none());
    assert!(
        app.diff_observation(
            "main",
            &app.profiles
                .load_profile_detail("main")
                .await
                .unwrap()
                .content
        )
        .is_none()
    );
}

#[tokio::test]
async fn restore_clears_the_cached_diff() {
    let app = application("port: 7890\nmode: global\n", "port: 7890\nmode: rule\n");
    let diff = app.diff_newest("main").await.expect("diff").expect("diff");
    let path = diff.source_path.expect("path");
    assert!(
        app.diff_observation(
            "main",
            &app.profiles
                .load_profile_detail("main")
                .await
                .unwrap()
                .content
        )
        .is_some()
    );

    let review = app
        .prepare_restore(&SnapshotRestoreTarget {
            profile: "main".into(),
            snapshot_id: path,
        })
        .await
        .expect("prepare");
    app.confirm_restore(None::<Arc<dyn ManagedRuntime>>, &review.identity)
        .await
        .expect("restore");
    assert!(
        app.diff_observation(
            "main",
            &app.profiles
                .load_profile_detail("main")
                .await
                .unwrap()
                .content
        )
        .is_none(),
        "a restore makes the cached diff stale by definition"
    );
}

#[tokio::test]
async fn creating_a_snapshot_clears_the_stale_diff_cache() {
    let app = application("port: 7890\nmode: rule\n", "port: 7890\nmode: rule\n");
    app.diff_newest("main")
        .await
        .expect("diff")
        .expect("stored snapshot");
    assert!(
        app.diff_observation(
            "main",
            &app.profiles
                .load_profile_detail("main")
                .await
                .unwrap()
                .content
        )
        .is_some()
    );
    app.create("main").await.expect("create snapshot");
    assert!(
        app.diff_observation(
            "main",
            &app.profiles
                .load_profile_detail("main")
                .await
                .unwrap()
                .content
        )
        .is_none()
    );
}

#[tokio::test]
async fn history_reports_duplicates_and_the_shared_prune_view() {
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
    assert!(
        snapshot_presentation::history_summary(Some(&history), history.keep_limit, "zh-CN")
            .contains("待修剪 1 份")
    );
    assert!(
        app.history_observation("main")
            .is_some_and(|cached| cached.profile == "main"),
        "the shared history is published for the Bevy projection"
    );
}

#[tokio::test]
async fn prune_executes_the_shared_policy_and_publishes_the_report() {
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

    let history = app.history_observation("main").expect("history published");
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

#[tokio::test]
async fn product_instances_and_document_changes_cannot_borrow_snapshot_observations() {
    let first = application("mode: global\n", "mode: rule\n");
    let other = application("mode: global\n", "mode: rule\n");
    let peer_reader = first.clone();
    let diff = first.diff_newest("main").await.unwrap().unwrap();
    let history = first.history("main", 5).await.unwrap();
    assert_eq!(
        peer_reader.diff_observation("main", "mode: global\n"),
        Some(diff)
    );
    assert_eq!(peer_reader.history_observation("main"), Some(history));
    assert!(other.diff_observation("main", "mode: global\n").is_none());
    assert!(other.history_observation("main").is_none());
    assert!(
        peer_reader
            .diff_observation("other", "mode: global\n")
            .is_none()
    );
    assert!(peer_reader.history_observation("other").is_none());
    assert!(
        peer_reader
            .diff_observation("main", "mode: global\r\n")
            .is_none(),
        "even a line-ending change invalidates the compared document identity"
    );
    assert!(
        peer_reader
            .diff_observation("main", "mode: rule\n")
            .is_none()
    );
}

#[tokio::test]
async fn snapshot_command_receipts_are_real_scoped_results_from_the_reader_instance() {
    use crate::command_application::CommandApplication;
    use infiltrator_contract::command::CommandIntent;
    use infiltrator_contract::command_output::CommandOutput;
    let snapshots = application("mode: global\n", "mode: rule\n");
    let reader = snapshots.clone();
    let commands = CommandApplication::default()
        .with_profile(snapshots.profiles.clone())
        .with_snapshots(snapshots);
    let intent = CommandIntent::LoadSnapshotHistory {
        profile: Some("main".into()),
        keep: 5,
    };
    let output = commands.execute_output(intent.clone()).await.unwrap();
    output.validate_for(&intent).unwrap();
    assert!(CommandOutput::Unit.validate_for(&intent).is_err());
    let history = output.into_snapshot_history().unwrap();
    assert_eq!(reader.history_observation("main"), Some(history));
    let intent = CommandIntent::LoadSnapshotDiff {
        profile: Some("main".into()),
        snapshot_id: None,
    };
    let output = commands.execute_output(intent.clone()).await.unwrap();
    output.validate_for(&intent).unwrap();
    let diff = output.into_snapshot_diff().unwrap().unwrap();
    assert_eq!(
        reader.diff_observation("main", "mode: global\n"),
        Some(diff)
    );
    let wrong_scope = CommandIntent::LoadSnapshotHistory {
        profile: Some("other".into()),
        keep: 5,
    };
    assert!(
        commands.execute_output(wrong_scope).await.is_err(),
        "missing profile storage must fail, never reuse another history"
    );
}

#[tokio::test]
async fn another_profile_backup_and_history_do_not_retire_the_open_comparison() {
    let profiles = Arc::new(FakeProfileStore::with_profile("main", "mode: global\n"));
    profiles.profiles.lock().unwrap().insert(
        "other".into(),
        ("mode: rule\n".into(), ProfileMetadata::default()),
    );
    let snapshots = Arc::new(FakeSnapshotStore::with_snapshot(
        "main",
        1_750_000_000_000,
        "mode: rule\n",
    ));
    let app = SnapshotApplication::new(profiles, snapshots);
    let diff = app.diff_newest("main").await.unwrap().unwrap();
    let history = app.history("main", 5).await.unwrap();
    let created = app.create("other").await.unwrap();
    assert_eq!(created.profile, "other");
    assert_eq!(app.diff_observation("main", "mode: global\n"), Some(diff));
    assert_eq!(app.history_observation("main"), Some(history));
    let other = app
        .history_observation("other")
        .expect("other profile has its own actual history");
    assert_eq!(other.entries.len(), 1);
    assert_eq!(other.entries[0].id, created.path.to_string_lossy());
}

#[path = "snapshot_restore_tests.rs"]
mod restoration;

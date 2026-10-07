//! DUAL-08-13: real-store coverage for the aggregation template sidecar.
//!
//! Mounted from `profile_store.rs` so the file stays a business module while
//! the round-trip runs against the concrete `ConfigManager`.

use crate::manager::ConfigManager;
use infiltrator_contract::aggregator::{AggregationDraft, AggregationTemplate};
use infiltrator_domain::profile_options::{FilterSpec, ProfileOptions, options_path};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::fs::{create_dir_all, read, read_dir, read_to_string, remove_dir, write};

struct TestStore;

#[async_trait::async_trait]
impl SecureStore for TestStore {
    async fn get(&self, _namespace: &str, _key: &str) -> Result<Option<String>, PortError> {
        Ok(None)
    }

    async fn set(&self, _namespace: &str, _key: &str, _value: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn delete(&self, _namespace: &str, _key: &str) -> Result<(), PortError> {
        Ok(())
    }
}

fn template() -> AggregationTemplate {
    AggregationTemplate {
        name: "我的模板".to_owned(),
        draft: AggregationDraft {
            source_profiles: vec!["A".to_owned(), "B".to_owned()],
            target_name: "Aggregated-Test".to_owned(),
            availability_precheck: true,
            ..Default::default()
        },
        updated_at: "2026-09-22T12:00:00+00:00".to_owned(),
    }
}

#[tokio::test]
async fn aggregation_templates_persist_beside_the_profile_options() {
    let temp_dir = TempDir::new().unwrap();
    let manager =
        ConfigManager::with_home_and_store(temp_dir.path().to_path_buf(), TestStore).unwrap();
    let path = manager
        .config_dir()
        .join("options")
        .join(".aggregation-templates.yaml");

    // A fresh store answers with an empty library, not an error.
    assert!(
        manager
            .load_aggregation_templates()
            .await
            .unwrap()
            .is_empty()
    );
    assert!(!path.exists());

    manager
        .save_aggregation_templates(&[template()])
        .await
        .expect("save templates");
    assert!(path.exists(), "the sidecar is written under options/");
    assert_eq!(
        manager.load_aggregation_templates().await.unwrap(),
        vec![template()]
    );

    // Saving an empty library removes the sidecar entirely.
    manager.save_aggregation_templates(&[]).await.unwrap();
    assert!(!path.exists());
    assert!(
        manager
            .load_aggregation_templates()
            .await
            .unwrap()
            .is_empty()
    );

    // A malformed sidecar is a typed storage error, never a fake empty library.
    create_dir_all(path.parent().unwrap()).await.unwrap();
    write(&path, "not: [a, template").await.unwrap();
    let failure = manager.load_aggregation_templates().await.unwrap_err();
    assert!(matches!(failure, PortError::Io(_)));
}

fn options(keyword: String) -> ProfileOptions {
    ProfileOptions {
        filter: Some(FilterSpec {
            include_keywords: vec![keyword],
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[tokio::test]
async fn unreadable_sidecars_and_failed_removal_do_not_become_empty_success() {
    let temp_dir = TempDir::new().unwrap();
    let manager =
        ConfigManager::with_home_and_store(temp_dir.path().to_path_buf(), TestStore).unwrap();
    let option_path = options_path(manager.config_dir(), "main");
    let template_path = manager
        .config_dir()
        .join("options/.aggregation-templates.yaml");
    assert_eq!(
        manager.load_options("main").await.unwrap(),
        Default::default()
    );
    for path in [&option_path, &template_path] {
        create_dir_all(path).await.unwrap();
        write(path.join("sentinel"), "keep").await.unwrap();
    }
    assert!(matches!(
        manager.load_options("main").await,
        Err(PortError::Io(_))
    ));
    assert!(matches!(
        manager.load_aggregation_templates().await,
        Err(PortError::Io(_))
    ));
    assert!(
        manager
            .save_options("main", &Default::default())
            .await
            .is_err()
    );
    assert!(manager.delete_options("main").await.is_err());
    assert!(manager.save_aggregation_templates(&[]).await.is_err());
    assert!(
        manager
            .save_options("main", &options("new".into()))
            .await
            .is_err()
    );
    assert!(
        manager
            .save_aggregation_templates(&[template()])
            .await
            .is_err()
    );
    for path in [&option_path, &template_path] {
        assert_eq!(read_to_string(path.join("sentinel")).await.unwrap(), "keep");
    }
    let mut entries = read_dir(option_path.parent().unwrap()).await.unwrap();
    let mut count = 0;
    while let Some(entry) = entries.next_entry().await.unwrap() {
        assert!(
            entry.file_type().await.unwrap().is_dir(),
            "no abandoned write temporary"
        );
        count += 1;
    }
    assert_eq!(count, 2);
}

#[tokio::test]
async fn invalid_utf8_and_malformed_options_remain_storage_failures() {
    let temp_dir = TempDir::new().unwrap();
    let manager =
        ConfigManager::with_home_and_store(temp_dir.path().to_path_buf(), TestStore).unwrap();
    let path = options_path(manager.config_dir(), "main");
    create_dir_all(path.parent().unwrap()).await.unwrap();
    for bytes in [&b"\xff"[..], &b"filter: [unfinished"[..]] {
        write(&path, bytes).await.unwrap();
        assert!(matches!(
            manager.load_options("main").await,
            Err(PortError::Io(_))
        ));
        assert_eq!(read(&path).await.unwrap(), bytes);
    }
    manager.delete_options("main").await.unwrap();
    manager.delete_options("main").await.unwrap();
    assert_eq!(
        manager.load_options("main").await.unwrap(),
        Default::default()
    );
}

#[tokio::test]
async fn concurrent_writers_publish_complete_sidecars_and_own_their_temporaries() {
    let temp_dir = TempDir::new().unwrap();
    let manager = Arc::new(
        ConfigManager::with_home_and_store(temp_dir.path().to_path_buf(), TestStore).unwrap(),
    );
    let mut writers = Vec::new();
    let candidates: Vec<_> = (0..16)
        .map(|index| options(format!("writer-{index}-{}", "x".repeat(16384))))
        .collect();
    for expected in &candidates {
        let expected = expected.clone();
        let manager = manager.clone();
        writers.push(tokio::spawn(async move {
            manager.save_options("main", &expected).await.unwrap();
        }));
    }
    for writer in writers {
        writer.await.unwrap();
    }
    let stored = manager.load_options("main").await.unwrap();
    assert!(
        candidates.contains(&stored),
        "the winner is one complete submitted document"
    );
    let directory = manager.config_dir().join("options");
    let mut entries = read_dir(&directory).await.unwrap();
    let mut names = Vec::new();
    while let Some(entry) = entries.next_entry().await.unwrap() {
        names.push(entry.file_name());
    }
    assert_eq!(names, vec!["main.yaml"]);
    manager.delete_options("main").await.unwrap();
    remove_dir(directory).await.unwrap();
}

#[tokio::test]
async fn option_paths_reject_traversal_before_read_write_or_delete() {
    let temp_dir = TempDir::new().unwrap();
    let manager =
        ConfigManager::with_home_and_store(temp_dir.path().to_path_buf(), TestStore).unwrap();
    let protected = manager.config_dir().join("protected.yaml");
    create_dir_all(manager.config_dir()).await.unwrap();
    write(&protected, "preserve").await.unwrap();
    for name in ["../protected", "folder/name", "", " main"] {
        assert!(manager.load_options(name).await.is_err());
        assert!(
            manager
                .save_options(name, &options("new".into()))
                .await
                .is_err()
        );
        assert!(manager.delete_options(name).await.is_err());
    }
    assert_eq!(read_to_string(&protected).await.unwrap(), "preserve");
    assert!(!manager.config_dir().join("options").exists());
}

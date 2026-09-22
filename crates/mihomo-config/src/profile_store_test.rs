//! DUAL-08-13: real-store coverage for the aggregation template sidecar.
//!
//! Mounted from `profile_store.rs` so the file stays a business module while
//! the round-trip runs against the concrete `ConfigManager`.

use crate::manager::ConfigManager;
use infiltrator_contract::aggregator::{AggregationDraft, AggregationTemplate};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use tempfile::TempDir;

struct TestStore;

#[async_trait::async_trait]
impl SecureStore for TestStore {
    async fn get(
        &self,
        _namespace: &str,
        _key: &str,
    ) -> Result<Option<String>, infiltrator_ports::error::PortError> {
        Ok(None)
    }

    async fn set(
        &self,
        _namespace: &str,
        _key: &str,
        _value: &str,
    ) -> Result<(), infiltrator_ports::error::PortError> {
        Ok(())
    }

    async fn delete(
        &self,
        _namespace: &str,
        _key: &str,
    ) -> Result<(), infiltrator_ports::error::PortError> {
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
    tokio::fs::create_dir_all(path.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&path, "not: [a, template").await.unwrap();
    let failure = manager.load_aggregation_templates().await.unwrap_err();
    assert!(matches!(
        failure,
        infiltrator_ports::error::PortError::Io(_)
    ));
}

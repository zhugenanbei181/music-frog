//! DUAL-08: headless tests for the shared aggregation application.

use super::*;
use async_trait::async_trait;
use infiltrator_domain::profile_options::ProfileOptions;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct FakeStore {
    saved: Mutex<BTreeMap<String, String>>,
}

impl FakeStore {
    fn with(entries: &[(&str, &str)]) -> Arc<Self> {
        let store = Self::default();
        for (name, content) in entries {
            store
                .saved
                .lock()
                .expect("saved")
                .insert((*name).to_string(), (*content).to_string());
        }
        Arc::new(store)
    }
}

#[async_trait]
impl ProfileStore for FakeStore {
    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/fake/configs")
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        Ok(self
            .saved
            .lock()
            .expect("saved")
            .keys()
            .map(|name| ProfileInfo {
                name: name.clone(),
                path: format!("/fake/configs/{name}.yaml"),
                ..ProfileInfo::default()
            })
            .collect())
    }

    async fn get_current(&self) -> Result<String, PortError> {
        Ok(String::new())
    }

    async fn set_current(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn load(&self, profile: &str) -> Result<String, PortError> {
        self.saved
            .lock()
            .expect("saved")
            .get(profile)
            .cloned()
            .ok_or_else(|| PortError::NotFound(profile.to_string()))
    }

    async fn save(&self, profile: &str, content: &str) -> Result<(), PortError> {
        self.saved
            .lock()
            .expect("saved")
            .insert(profile.to_string(), content.to_string());
        Ok(())
    }

    async fn delete_profile(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn get_profile_metadata(&self, profile: &str) -> Result<ProfileMetadata, PortError> {
        Ok(ProfileMetadata {
            subscription_url: Some(format!("https://example.invalid/{profile}")),
            ..ProfileMetadata::default()
        })
    }

    async fn update_profile_metadata(
        &self,
        _profile: &str,
        _metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        Ok(())
    }

    async fn delete_subscription_credential(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn load_options(&self, _profile: &str) -> Result<ProfileOptions, PortError> {
        Ok(ProfileOptions::default())
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

    async fn clear_backup(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn restore_backup(&self, _profile: &str) -> Result<bool, PortError> {
        Ok(false)
    }
}

const SOURCE_A: &str = r#"
proxies:
  - name: "🇭🇰 香港 01"
    type: ss
    server: 1.1.1.1
    port: 443
    password: pass
  - name: "🇯🇵 Tokyo 01"
    type: ss
    server: 2.2.2.2
    port: 443
    password: pass
"#;

const SOURCE_B: &str = r#"
proxies:
  - name: "🇭🇰 香港 01"
    type: ss
    server: 1.1.1.1
    port: 443
    password: pass
  - name: "🇺🇸 US West"
    type: trojan
    server: 3.3.3.3
    port: 443
    password: pass
"#;

fn draft() -> AggregationDraft {
    AggregationDraft {
        source_profiles: vec!["A".to_owned(), "B".to_owned()],
        target_name: "Aggregated-Test".to_owned(),
        deduplicate: true,
        deduplicate_names: true,
        geo_cluster: true,
        generate_groups: true,
        remove_emojis: true,
    }
}

fn application(entries: &[(&str, &str)]) -> ProfileAggregationApplication {
    ProfileAggregationApplication::new(ProfileApplication::new(FakeStore::with(entries)))
}

#[tokio::test]
async fn preview_reports_real_dedup_clusters_and_master_cascade() {
    let app = application(&[("A", SOURCE_A), ("B", SOURCE_B)]);
    let report = app.preview(&draft()).await.expect("preview");

    assert_eq!(report.source_count, 2);
    assert!(report.missing_sources.is_empty());
    assert_eq!(report.input_nodes, 4);
    assert_eq!(report.total_nodes, 3);
    assert_eq!(report.duplicates_removed, 1);

    let isos: Vec<&str> = report.regions.iter().map(|r| r.iso.as_str()).collect();
    assert_eq!(isos, vec!["HK", "JP", "US"]);
    assert_eq!(report.regions[0].group_name, "香港自动测速");

    let master = report.master_group().expect("master selector");
    assert!(master.members.contains(&"香港自动测速".to_owned()));
    assert!(master.members.contains(&"♻️ 自动选择".to_owned()));
    // The master lists every regional group before the concrete nodes.
    let region_pos = master
        .members
        .iter()
        .position(|m| m == "香港自动测速")
        .unwrap();
    assert!(region_pos < master.members.len() - report.total_nodes);

    assert!(report.yaml.contains("proxy-groups:"));
    assert!(report.yaml_preview(3).lines().count() <= 4);
}

#[tokio::test]
async fn preview_publishes_one_process_wide_report() {
    clear_aggregation_report();
    assert!(last_aggregation_report().is_none());

    let app = application(&[("A", SOURCE_A), ("B", SOURCE_B)]);
    let report = app.preview(&draft()).await.expect("preview");
    let published = last_aggregation_report().expect("published report");
    assert_eq!(published.total_nodes, report.total_nodes);
    clear_aggregation_report();
}

#[tokio::test]
async fn preview_reports_unreadable_sources_without_fabricating() {
    let app = application(&[("A", SOURCE_A)]);
    let report = app.preview(&draft()).await.expect("preview");
    assert_eq!(report.missing_sources, vec!["B".to_owned()]);
    assert_eq!(report.source_count, 1);
}

#[tokio::test]
async fn preview_rejects_an_empty_selection() {
    let app = application(&[("A", SOURCE_A)]);
    let empty = AggregationDraft::default();
    let failure = app.preview(&empty).await.expect_err("empty draft");
    assert_eq!(failure.code, ErrorCode::InvalidInput);
}

#[tokio::test]
async fn preview_rejects_when_no_source_can_be_read() {
    let app = application(&[]);
    let failure = app.preview(&draft()).await.expect_err("no sources");
    assert_eq!(failure.code, ErrorCode::Configuration);
}

#[tokio::test]
async fn create_profile_saves_a_new_independent_profile() {
    clear_aggregation_report();
    let app = application(&[("A", SOURCE_A), ("B", SOURCE_B)]);
    let report = app.create_profile(&draft()).await.expect("create");

    let profiles = app.profile.list_profiles().await.expect("list");
    let names: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"A"));
    assert!(names.contains(&"B"));
    assert!(names.contains(&"Aggregated-Test"));
    assert!(report.yaml.contains("香港自动测速"));
    clear_aggregation_report();
}

#[tokio::test]
async fn create_profile_refuses_to_overwrite_an_existing_profile() {
    let app = application(&[("A", SOURCE_A), ("Aggregated-Test", SOURCE_B)]);
    let failure = app
        .create_profile(&draft())
        .await
        .expect_err("existing target");
    assert_eq!(failure.code, ErrorCode::InvalidInput);
}

#[test]
fn aggregation_options_follow_the_draft_switches() {
    let off = aggregation_options(&AggregationDraft::default());
    assert_eq!(off.content_dedup, ContentDedupStrategy::Disabled);
    assert_eq!(off.name_dedup, DeduplicationStrategy::Disabled);
    assert!(!off.generate_proxy_groups);
    assert!(!off.normalize_country_code);

    let on = aggregation_options(&draft());
    assert_eq!(on.content_dedup, ContentDedupStrategy::KeepFirst);
    assert_eq!(on.name_dedup, DeduplicationStrategy::AppendIndex);
    assert!(on.generate_proxy_groups);
    assert!(on.normalize_country_code);
    assert_eq!(on.sort_by, NodeSortOrder::CountryCode);
}
/// DUAL-08-15: the whole pipeline — multi-source merge, fingerprint dedup,
/// geo clustering, group synthesis and persistence — in one headless run.
#[tokio::test]
async fn aggregation_pipeline_regression_matrix() {
    clear_aggregation_report();
    let app = application(&[("A", SOURCE_A), ("B", SOURCE_B)]);

    // Merge + dedup + cluster + topology preview.
    let preview = app.preview(&draft()).await.expect("preview");
    assert_eq!(preview.input_nodes, 4);
    assert_eq!(preview.total_nodes, 3);
    assert_eq!(preview.duplicates_removed, 1);
    assert_eq!(preview.regions.len(), 3);
    assert_eq!(preview.groups.len(), 3 + 4);
    assert!(preview.master_group().is_some());
    assert!(preview.yaml.contains("MATCH,🚀 节点选择"));

    // The preview also seeds the shared projection cache.
    assert_eq!(
        last_aggregation_report().map(|report| report.total_nodes),
        Some(3)
    );

    // Persist without touching the sources.
    let created = app.create_profile(&draft()).await.expect("create");
    assert_eq!(created.total_nodes, 3);
    let stored = app
        .profile
        .load_profile_detail("Aggregated-Test")
        .await
        .expect("stored profile");
    assert!(stored.content.contains("proxy-groups:"));
    assert!(stored.content.contains("香港自动测速"));
    clear_aggregation_report();
}

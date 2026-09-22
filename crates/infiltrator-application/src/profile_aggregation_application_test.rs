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
    current: Mutex<String>,
    templates: Mutex<Vec<infiltrator_contract::aggregator::AggregationTemplate>>,
    /// Whether this store exposes an aggregation-template sidecar (DUAL-08-13).
    sidecar_supported: bool,
}

impl FakeStore {
    fn with(entries: &[(&str, &str)]) -> Arc<Self> {
        let store = Self {
            sidecar_supported: true,
            ..Self::default()
        };
        for (name, content) in entries {
            store
                .saved
                .lock()
                .expect("saved")
                .insert((*name).to_string(), (*content).to_string());
        }
        Arc::new(store)
    }

    /// A store without the template sidecar (typed unsupported host capability).
    fn without_sidecar(entries: &[(&str, &str)]) -> Arc<Self> {
        let store = Self {
            sidecar_supported: false,
            ..Self::default()
        };
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
        Ok(self.current.lock().expect("current").clone())
    }

    async fn set_current(&self, _profile: &str) -> Result<(), PortError> {
        *self.current.lock().expect("current") = _profile.to_string();
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

    async fn load_aggregation_templates(
        &self,
    ) -> Result<Vec<infiltrator_contract::aggregator::AggregationTemplate>, PortError> {
        if !self.sidecar_supported {
            return Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Profiles,
                "fake store keeps no aggregation-template sidecar",
            ));
        }
        Ok(self.templates.lock().expect("templates").clone())
    }

    async fn save_aggregation_templates(
        &self,
        templates: &[infiltrator_contract::aggregator::AggregationTemplate],
    ) -> Result<(), PortError> {
        if !self.sidecar_supported {
            return Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Profiles,
                "fake store keeps no aggregation-template sidecar",
            ));
        }
        *self.templates.lock().expect("templates") = templates.to_vec();
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
    cipher: aes-128-gcm
    password: pass
  - name: "🇯🇵 Tokyo 01"
    type: ss
    server: 2.2.2.2
    port: 443
    cipher: aes-128-gcm
    password: pass
"#;

const SOURCE_B: &str = r#"
proxies:
  - name: "🇭🇰 香港 01"
    type: ss
    server: 1.1.1.1
    port: 443
    cipher: aes-128-gcm
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
        rename_rules: Vec::new(),
        custom_groups: Vec::new(),
        availability_precheck: false,
        activate_after_create: false,
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
    let outcome = app.create_profile(&draft()).await.expect("create");

    let profiles = app.profile.list_profiles().await.expect("list");
    let names: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"A"));
    assert!(names.contains(&"B"));
    assert!(names.contains(&"Aggregated-Test"));
    assert!(outcome.report.yaml.contains("香港自动测速"));
    assert_eq!(outcome.profile_name, "Aggregated-Test");
    // No activation was requested, so the surface must not claim one.
    assert!(!outcome.activated);
    assert!(!outcome.core_reloaded);
    // DUAL-08-13: creating also remembers the draft under the profile name.
    assert_eq!(outcome.template_name.as_deref(), Some("Aggregated-Test"));
    let templates = app.list_templates().await.expect("templates");
    assert_eq!(templates.len(), 1);
    assert_eq!(
        templates[0].draft.source_profiles,
        vec!["A".to_owned(), "B".to_owned()]
    );
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

/// DUAL-08-09/08-08: the precheck and rename switches reach the domain plan
/// and the report publishes their real counters.
#[tokio::test]
async fn preview_applies_precheck_and_rename_switches() {
    let messy = r#"
proxies:
  - name: "HK 01-Promo"
    type: trojan
    server: 1.1.1.1
    port: 443
    password: pass
  - name: "Broken Key"
    type: vmess
    server: 2.2.2.2
    port: 443
"#;
    let app = application(&[("Messy", messy)]);
    let mut request = draft();
    request.source_profiles = vec!["Messy".to_owned()];
    request.target_name = "Cleaned".to_owned();
    request.availability_precheck = true;
    request.rename_rules = vec![infiltrator_contract::aggregator::AggregationRenameRule {
        pattern: "-Promo$".to_owned(),
        replacement: String::new(),
    }];

    let report = app.preview(&request).await.expect("preview");
    assert_eq!(report.input_nodes, 2);
    assert_eq!(report.total_nodes, 1);
    assert_eq!(report.invalid_nodes_removed, 1);
    assert_eq!(
        report.invalid_node_samples,
        vec!["Broken Key: vmess: uuid is required".to_owned()]
    );
    assert_eq!(report.rule_renamed_nodes, 1);
    assert!(report.yaml.contains("HK 01"));
    assert!(!report.yaml.contains("-Promo"));
    assert!(!report.yaml.contains("Broken Key"));
}

/// DUAL-08-10: custom groups authored in the draft appear in the shared report
/// and the rendered YAML.
#[tokio::test]
async fn preview_synthesizes_custom_groups_from_the_draft() {
    let app = application(&[("A", SOURCE_A), ("B", SOURCE_B)]);
    let mut request = draft();
    request.custom_groups = vec![
        infiltrator_contract::aggregator::AggregationCustomGroup {
            name: "流媒体专用".to_owned(),
            group_type: "select".to_owned(),
            member_keywords: vec!["west".to_owned()],
        },
        infiltrator_contract::aggregator::AggregationCustomGroup {
            name: "游戏专用".to_owned(),
            group_type: "url-test".to_owned(),
            member_keywords: Vec::new(),
        },
    ];
    let report = app.preview(&request).await.expect("preview");
    let custom: Vec<&GeneratedGroupSnapshot> = report
        .groups
        .iter()
        .filter(|group| group.is_custom)
        .collect();
    assert_eq!(custom.len(), 2);
    assert_eq!(custom[0].members.len(), 1);
    assert_eq!(custom[1].members.len(), report.total_nodes);
    assert!(report.yaml.contains("流媒体专用"));
    assert!(report.yaml.contains("游戏专用"));
}

/// DUAL-08-12: the activation switch reuses the shared activation path and is
/// reported honestly — a host without a managed-runtime seam still switches
/// the active profile but reports `core_reloaded = false`.
#[tokio::test]
async fn create_profile_activates_through_the_shared_profile_path() {
    let store: Arc<dyn ProfileStore> = FakeStore::with(&[("A", SOURCE_A), ("B", SOURCE_B)]);
    let app = ProfileAggregationApplication::new(ProfileApplication::new(Arc::clone(&store)));
    let mut request = draft();
    request.activate_after_create = true;

    let outcome = app.create_profile(&request).await.expect("create");
    assert!(outcome.activated);
    assert!(!outcome.core_reloaded);
    assert_eq!(
        store.get_current().await.expect("current"),
        "Aggregated-Test"
    );
}

/// DUAL-08-13: a store without a template sidecar answers typed unsupported;
/// profile creation still succeeds and reports that nothing was remembered.
#[tokio::test]
async fn create_profile_reports_missing_template_sidecar_without_failing() {
    let store = FakeStore::without_sidecar(&[("A", SOURCE_A)]);
    let app = ProfileAggregationApplication::new(ProfileApplication::new(store));
    let outcome = app.create_profile(&draft()).await.expect("create");
    assert!(outcome.template_name.is_none());
    assert!(outcome.report.total_nodes > 0);

    let failure = app
        .save_template("Named", &draft())
        .await
        .expect_err("typed unsupported");
    assert_eq!(failure.code, ErrorCode::Unsupported);
}

/// DUAL-08-13: templates round-trip through the store and reject invalid input.
#[tokio::test]
async fn templates_upsert_list_and_delete() {
    let app = application(&[("A", SOURCE_A)]);
    let mut request = draft();
    request.source_profiles = vec!["A".to_owned()];
    request.target_name = "Template-Target".to_owned();

    let saved = app.save_template("我的模板", &request).await.expect("save");
    assert_eq!(saved.name, "我的模板");
    assert!(!saved.updated_at.is_empty());

    // Upsert replaces instead of duplicating.
    let mut edited = request.clone();
    edited.remove_emojis = false;
    app.save_template("我的模板", &edited)
        .await
        .expect("upsert");
    let templates = app.list_templates().await.expect("list");
    assert_eq!(templates.len(), 1);
    assert!(!templates[0].draft.remove_emojis);

    assert!(app.delete_template("我的模板").await.expect("delete"));
    assert!(!app.delete_template("我的模板").await.expect("idempotent"));
    assert!(app.list_templates().await.expect("list").is_empty());

    let empty_name = app
        .save_template("   ", &request)
        .await
        .expect_err("empty name");
    assert_eq!(empty_name.code, ErrorCode::InvalidInput);
    let empty_draft = app
        .save_template("Named", &AggregationDraft::default())
        .await
        .expect_err("no sources");
    assert_eq!(empty_draft.code, ErrorCode::InvalidInput);
}

/// DUAL-08-07: re-aggregation re-reads the (updated) sources, overwrites the
/// generated profile and leaves both sources untouched.
#[tokio::test]
async fn reaggregate_refreshes_a_generated_profile_from_live_sources() {
    let store: Arc<dyn ProfileStore> = FakeStore::with(&[("A", SOURCE_A), ("B", SOURCE_B)]);
    let app = ProfileAggregationApplication::new(ProfileApplication::new(Arc::clone(&store)));
    let created = app.create_profile(&draft()).await.expect("create");
    assert_eq!(created.report.total_nodes, 3);

    // The source subscription is updated: one more node arrives upstream.
    store
        .save(
            "A",
            r#"
proxies:
  - name: "🇭🇰 香港 01"
    type: ss
    server: 1.1.1.1
    port: 443
    cipher: aes-128-gcm
    password: pass
  - name: "🇯🇵 Tokyo 01"
    type: ss
    server: 2.2.2.2
    port: 443
    cipher: aes-128-gcm
    password: pass
  - name: "🇸🇬 Singapore 01"
    type: ss
    server: 4.4.4.4
    port: 443
    cipher: aes-128-gcm
    password: pass
"#,
        )
        .await
        .expect("update source");

    let refreshed = app
        .reaggregate::<dyn ManagedRuntime>(None, "Aggregated-Test")
        .await
        .expect("reaggregate");
    assert_eq!(refreshed.report.total_nodes, 4);
    assert_eq!(refreshed.profile_name, "Aggregated-Test");

    let stored = app
        .profile
        .load_profile_detail("Aggregated-Test")
        .await
        .expect("stored profile");
    assert!(stored.content.contains("Singapore"));
    // Sources keep their own raw content — only the aggregated profile moved.
    let source = app
        .profile
        .load_profile_detail("A")
        .await
        .expect("source profile");
    assert!(source.content.contains("Singapore"));

    let unknown = app
        .reaggregate::<dyn ManagedRuntime>(None, "nope")
        .await
        .expect_err("unknown template");
    assert_eq!(unknown.code, ErrorCode::Configuration);
}

#[test]
fn aggregation_options_follow_the_draft_switches() {
    let off = aggregation_options(&AggregationDraft::default());
    assert_eq!(off.content_dedup, ContentDedupStrategy::Disabled);
    assert_eq!(off.name_dedup, DeduplicationStrategy::Disabled);
    assert!(!off.generate_proxy_groups);
    assert!(!off.normalize_country_code);
    assert!(!off.availability_precheck);
    assert!(off.rename_rules.is_empty());
    assert!(off.custom_groups.is_empty());

    let mut request = draft();
    request.availability_precheck = true;
    request.rename_rules = vec![infiltrator_contract::aggregator::AggregationRenameRule {
        pattern: "x".to_owned(),
        replacement: "y".to_owned(),
    }];
    request.custom_groups = vec![infiltrator_contract::aggregator::AggregationCustomGroup {
        name: "自定义".to_owned(),
        group_type: "select".to_owned(),
        member_keywords: Vec::new(),
    }];
    let on = aggregation_options(&request);
    assert_eq!(on.content_dedup, ContentDedupStrategy::KeepFirst);
    assert_eq!(on.name_dedup, DeduplicationStrategy::AppendIndex);
    assert!(on.generate_proxy_groups);
    assert!(on.normalize_country_code);
    assert_eq!(on.sort_by, NodeSortOrder::CountryCode);
    assert!(on.availability_precheck);
    assert_eq!(on.rename_rules.len(), 1);
    assert_eq!(on.custom_groups.len(), 1);
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
    assert_eq!(created.report.total_nodes, 3);
    assert_eq!(created.profile_name, "Aggregated-Test");
    let stored = app
        .profile
        .load_profile_detail("Aggregated-Test")
        .await
        .expect("stored profile");
    assert!(stored.content.contains("proxy-groups:"));
    assert!(stored.content.contains("香港自动测速"));
    clear_aggregation_report();
}

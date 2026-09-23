//! Headless tests for the shared rule-edit command path (DUAL-11-09/10/11/12)
//! and the DUAL-07-09/14 subscription settings commands.
//!
//! A minimal in-memory `ProfileStore` proves the application applies the same
//! list arithmetic as `infiltrator_domain::rules::edit` and persists the
//! result, without a real profile directory.

use super::*;
use crate::configuration_application::ConfigurationApplication;
use async_trait::async_trait;
use infiltrator_contract::rule_edit::{RuleDraft, RuleMoveDirection};
use infiltrator_contract::subscription_import::SubscriptionScheduleDraft;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default)]
struct FakeStore {
    profile: Mutex<String>,
    content: Mutex<String>,
    metadata: Mutex<ProfileMetadata>,
}

impl FakeStore {
    fn with_profile(content: &str) -> Self {
        Self {
            profile: Mutex::new("main".to_owned()),
            content: Mutex::new(content.to_owned()),
            metadata: Mutex::new(ProfileMetadata::default()),
        }
    }

    fn content(&self) -> String {
        self.content.lock().expect("content lock").clone()
    }

    fn metadata(&self) -> ProfileMetadata {
        self.metadata.lock().expect("metadata lock").clone()
    }
}

#[async_trait]
impl ProfileStore for FakeStore {
    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/fake/configs")
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        let name = self.profile.lock().expect("profile lock").clone();
        Ok(vec![ProfileInfo {
            name,
            active: true,
            path: "/fake/configs/main.yaml".to_owned(),
            ..ProfileInfo::default()
        }])
    }

    async fn get_current(&self) -> Result<String, PortError> {
        Ok(self.profile.lock().expect("profile lock").clone())
    }

    async fn set_current(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn load(&self, _profile: &str) -> Result<String, PortError> {
        Ok(self.content())
    }

    async fn save(&self, _profile: &str, content: &str) -> Result<(), PortError> {
        *self.content.lock().expect("content lock") = content.to_owned();
        Ok(())
    }

    async fn delete_profile(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn get_profile_metadata(&self, _profile: &str) -> Result<ProfileMetadata, PortError> {
        Ok(self.metadata())
    }

    async fn update_profile_metadata(
        &self,
        _profile: &str,
        metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        *self.metadata.lock().expect("metadata lock") = metadata.clone();
        Ok(())
    }

    async fn delete_subscription_credential(&self, _profile: &str) -> Result<(), PortError> {
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

    async fn load_options(
        &self,
        _profile: &str,
    ) -> Result<infiltrator_domain::profile_options::ProfileOptions, PortError> {
        Ok(infiltrator_domain::profile_options::ProfileOptions::default())
    }

    async fn save_options(
        &self,
        _profile: &str,
        _options: &infiltrator_domain::profile_options::ProfileOptions,
    ) -> Result<(), PortError> {
        Ok(())
    }
}

const THREE_RULES: &str =
    "rules:\n  - DOMAIN,a.com,DIRECT\n  - DOMAIN,b.com,PROXY\n  - MATCH,DIRECT\n";

/// Minimal runtime gateway that records the Geo database upgrade trigger. All
/// other operations are the honest no-ops of a gateway that is not the subject
/// of these tests.
#[derive(Default)]
struct RecordingGeoGateway {
    geo_upgrades: std::sync::atomic::AtomicUsize,
}

impl RecordingGeoGateway {
    fn geo_upgrades(&self) -> usize {
        self.geo_upgrades.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait]
impl infiltrator_ports::runtime_gateway::RuntimeGateway for RecordingGeoGateway {
    async fn get_config(&self) -> Result<infiltrator_domain::runtime::ConfigSnapshot, PortError> {
        Ok(infiltrator_domain::runtime::ConfigSnapshot::default())
    }

    async fn patch_config(&self, _updates: serde_json::Value) -> Result<(), PortError> {
        Ok(())
    }

    async fn set_proxy_mode(
        &self,
        _mode: infiltrator_contract::command::ProxyMode,
    ) -> Result<(), PortError> {
        Ok(())
    }

    async fn get_proxies(
        &self,
    ) -> Result<std::collections::HashMap<String, infiltrator_domain::proxy::Proxy>, PortError>
    {
        Ok(std::collections::HashMap::new())
    }

    async fn switch_proxy(&self, _group: &str, _proxy: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn test_delay(
        &self,
        _proxy: &str,
        _url: &str,
        _timeout_ms: u32,
    ) -> Result<u32, PortError> {
        Ok(1)
    }

    async fn get_proxy_providers(
        &self,
    ) -> Result<Vec<infiltrator_domain::runtime::ProxyProvider>, PortError> {
        Ok(Vec::new())
    }

    async fn get_rule_providers(
        &self,
    ) -> Result<Vec<infiltrator_domain::runtime::RuleProvider>, PortError> {
        Ok(Vec::new())
    }

    async fn update_proxy_provider(&self, _name: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn update_rule_provider(&self, _name: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn flush_fakeip_cache(&self) -> Result<(), PortError> {
        Ok(())
    }

    async fn upgrade_geo(&self) -> Result<(), PortError> {
        self.geo_upgrades
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    async fn get_connections(
        &self,
    ) -> Result<infiltrator_domain::runtime::ConnectionSnapshot, PortError> {
        Ok(infiltrator_domain::runtime::ConnectionSnapshot::default())
    }

    async fn get_memory(&self) -> Result<infiltrator_domain::runtime::MemoryData, PortError> {
        Ok(infiltrator_domain::runtime::MemoryData::default())
    }

    async fn close_connection(&self, _id: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn close_all_connections(&self) -> Result<(), PortError> {
        Ok(())
    }

    async fn stream_logs(
        &self,
        _level: Option<String>,
    ) -> Result<infiltrator_ports::runtime_gateway::RuntimeStream<String>, PortError> {
        Err(PortError::Failed("not implemented".into()))
    }

    async fn stream_traffic(
        &self,
    ) -> Result<
        infiltrator_ports::runtime_gateway::RuntimeStream<infiltrator_domain::runtime::TrafficData>,
        PortError,
    > {
        Err(PortError::Failed("not implemented".into()))
    }

    async fn stream_connections(
        &self,
    ) -> Result<
        infiltrator_ports::runtime_gateway::RuntimeStream<
            infiltrator_domain::runtime::ConnectionSnapshot,
        >,
        PortError,
    > {
        Err(PortError::Failed("not implemented".into()))
    }
}

fn application(store: &Arc<FakeStore>) -> CommandApplication {
    CommandApplication::new().with_profile(ProfileApplication::new(
        Arc::clone(store) as Arc<dyn ProfileStore>
    ))
}

#[tokio::test]
async fn toggle_and_move_apply_shared_reductions_and_persist() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let application = application(&store);

    application
        .execute(CommandIntent::ToggleRuleEnabled { index: 1 })
        .await
        .expect("toggle");

    let rules = infiltrator_domain::rules::load_rules_from_yaml(&store.content()).expect("parse");
    assert!(!rules[1].enabled);
    assert!(store.content().contains("# DOMAIN,b.com,PROXY"));

    application
        .execute(CommandIntent::MoveRule {
            index: 0,
            direction: RuleMoveDirection::Down,
        })
        .await
        .expect("move");

    let rules = infiltrator_domain::rules::load_rules_from_yaml(&store.content()).expect("parse");
    assert_eq!(rules[0].rule, "DOMAIN,b.com,PROXY");
    assert_eq!(rules[1].rule, "DOMAIN,a.com,DIRECT");
}

#[tokio::test]
async fn toggle_out_of_range_is_a_noop() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let application = application(&store);
    application
        .execute(CommandIntent::ToggleRuleEnabled { index: 9 })
        .await
        .expect("noop");
    assert_eq!(store.content(), THREE_RULES);
}

#[tokio::test]
async fn add_custom_rule_prepends_and_rejects_invalid_logical_form() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let application = application(&store);

    application
        .execute(CommandIntent::AddCustomRule {
            rule_type: "DOMAIN-SUFFIX".to_owned(),
            payload: "github.com".to_owned(),
            target: "PROXY".to_owned(),
        })
        .await
        .expect("add");

    let rules = infiltrator_domain::rules::load_rules_from_yaml(&store.content()).expect("parse");
    assert_eq!(rules[0].rule, "DOMAIN-SUFFIX,github.com,PROXY");

    let failure = application
        .execute(CommandIntent::AddCustomRule {
            rule_type: "AND".to_owned(),
            payload: "(DOMAIN,a.com".to_owned(),
            target: "PROXY".to_owned(),
        })
        .await
        .expect_err("unclosed logical rule must fail");
    assert_eq!(failure.code, ErrorCode::InvalidInput);

    // The shared builder also rejects an empty payload before touching disk.
    assert!(
        infiltrator_domain::rules::edit::build_custom_rule(&RuleDraft {
            rule_type: "DOMAIN".to_owned(),
            payload: "  ".to_owned(),
            target: "PROXY".to_owned(),
        })
        .is_err()
    );
}

#[tokio::test]
async fn game_presets_prepend_the_shared_default_list() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let application = application(&store);

    application
        .execute(CommandIntent::ApplyGameRoutingPresets {
            target: "Game-Proxy".to_owned(),
        })
        .await
        .expect("presets");

    let rules = infiltrator_domain::rules::load_rules_from_yaml(&store.content()).expect("parse");
    let expected = infiltrator_domain::rules::game_routing_presets("Game-Proxy");
    assert_eq!(rules[0].rule, expected[0].rule);
    assert_eq!(
        rules[expected.len() - 1].rule,
        expected[expected.len() - 1].rule
    );
    assert_eq!(rules[expected.len()].rule, "DOMAIN,a.com,DIRECT");
}

/// DUAL-07-09: enabling auto-reload on a host without a managed-runtime reload
/// seam is a typed unsupported failure; disabling is always allowed because it
/// cannot silently no-op.
#[tokio::test]
async fn auto_reload_enable_requires_a_managed_runtime_seam() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let application = application(&store);

    let failure = application
        .execute(CommandIntent::SetSubscriptionAutoReload {
            profile_id: "main".to_owned(),
            enabled: true,
        })
        .await
        .expect_err("no reload seam on this host");
    assert_eq!(failure.code, ErrorCode::Unsupported);
    assert!(!store.metadata().auto_reload_core, "nothing was persisted");

    application
        .execute(CommandIntent::SetSubscriptionAutoReload {
            profile_id: "main".to_owned(),
            enabled: false,
        })
        .await
        .expect("disabling never needs the seam");
    assert!(!store.metadata().auto_reload_core);
}

/// DUAL-07-14: the schedule command validates and persists through the same
/// shared application both surfaces use.
#[tokio::test]
async fn subscription_schedule_command_persists_and_rejects_invalid_cron() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let application = application(&store);

    application
        .execute(CommandIntent::UpdateSubscriptionSchedule {
            profile_id: "main".to_owned(),
            draft: SubscriptionScheduleDraft {
                url: "https://sub.example.com/token".to_owned(),
                auto_update_enabled: true,
                update_interval_hours: "6".to_owned(),
                cron_expression: Some("0 */6 * * *".to_owned()),
            },
        })
        .await
        .expect("valid schedule persists");
    let metadata = store.metadata();
    assert_eq!(
        metadata.subscription_url.as_deref(),
        Some("https://sub.example.com/token")
    );
    assert_eq!(metadata.update_interval_hours, Some(6));
    assert_eq!(metadata.cron_expression.as_deref(), Some("0 */6 * * *"));

    let failure = application
        .execute(CommandIntent::UpdateSubscriptionSchedule {
            profile_id: "main".to_owned(),
            draft: SubscriptionScheduleDraft {
                url: "https://sub.example.com/token".to_owned(),
                auto_update_enabled: true,
                update_interval_hours: "6".to_owned(),
                cron_expression: Some("definitely not a cron".to_owned()),
            },
        })
        .await
        .expect_err("malformed cron is rejected");
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(
        store.metadata().cron_expression.as_deref(),
        Some("0 */6 * * *"),
        "a rejected draft never rewrites the stored schedule"
    );
}

// ---- DUAL-09-01 / LEFT-05 L1: comment-preserving rule writes ---------------

#[tokio::test]
async fn rule_commands_keep_handwritten_comments_end_to_end() {
    let source = "\
# 手写头注释
rules:
  # 规则块说明
  - MATCH,DIRECT   # 兜底规则
";
    let store = Arc::new(FakeStore::with_profile(source));
    let application = application(&store);

    application
        .execute(CommandIntent::AddCustomRule {
            rule_type: "DOMAIN-SUFFIX".to_owned(),
            payload: "google.com".to_owned(),
            target: "PROXY".to_owned(),
        })
        .await
        .expect("add");
    let saved = store.content();
    assert!(saved.contains("# 手写头注释"), "top comment kept: {saved}");
    assert!(
        saved.contains("# 规则块说明"),
        "block comment kept: {saved}"
    );
    assert!(saved.contains("# 兜底规则"), "inline comment kept: {saved}");
    assert_eq!(
        infiltrator_domain::rules::load_rules_from_yaml(&saved)
            .expect("parse")
            .len(),
        2
    );

    application
        .execute(CommandIntent::ToggleRuleEnabled { index: 0 })
        .await
        .expect("toggle");
    let saved = store.content();
    let rules = infiltrator_domain::rules::load_rules_from_yaml(&saved).expect("parse");
    assert!(!rules[0].enabled, "the new rule is now disabled");
    assert!(
        saved.contains("# 手写头注释"),
        "toggle keeps comments: {saved}"
    );
    assert!(saved.contains("# 规则块说明"));
}

// ---- DUAL-09-03/06/07/14: document + history commands ----------------------

#[tokio::test]
async fn load_and_save_profile_document_round_trips_through_the_shared_guard() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let application = application(&store);

    application
        .execute(CommandIntent::LoadProfileDocument { profile: None })
        .await
        .expect("load");
    let document = infiltrator_contract::profile_document::last_profile_document()
        .expect("the shared document is published for the surfaces");
    assert_eq!(document.profile, "main");
    assert_eq!(document.content, THREE_RULES);
    assert!(
        document.is_clean(),
        "a valid stored document has no diagnostic"
    );
    assert!(!document.write_protection.is_protected());
    assert_eq!(document.line_count, THREE_RULES.lines().count());

    // A typed syntax error is rejected by the shared preflight and never
    // reaches the apply transaction.
    let error = application
        .execute(CommandIntent::SaveProfileDocument {
            profile: "main".to_owned(),
            content: "rules: [\n".to_owned(),
            allow_protected: false,
        })
        .await
        .expect_err("invalid yaml is rejected");
    assert!(
        error.message.contains("YAML 语法错误"),
        "unexpected failure: {}",
        error.message
    );
    assert_eq!(store.content(), THREE_RULES, "the store is untouched");

    // A valid buffer commits through `save_edited_profile_content` and the
    // stored document is re-read for the surfaces.
    let saved = "rules:\n  - MATCH,DIRECT\n";
    application
        .execute(CommandIntent::SaveProfileDocument {
            profile: "main".to_owned(),
            content: saved.to_owned(),
            allow_protected: false,
        })
        .await
        .expect("save");
    assert_eq!(store.content(), saved);
    assert_eq!(
        infiltrator_contract::profile_document::last_profile_document()
            .expect("published")
            .content,
        saved
    );
}

// ---- DUAL-09-04: shared snippet insertion ----------------------------------

#[test]
fn insert_snippet_keeps_the_document_parseable_and_refuses_a_broken_splice() {
    let document = "proxies:\n  - name: keep\n    type: ss\nrules:\n  - MATCH,DIRECT\n";

    // A node item appended to the proxies list keeps the document valid, and
    // every untouched byte is preserved.
    let insertion = crate::profile_document_application::insert_snippet(document, "ss", 3, 12)
        .expect("a node item may join the list");
    assert!(
        insertion.is_clean(),
        "the splice keeps the document parseable"
    );
    assert!(
        insertion
            .content
            .starts_with("proxies:\n  - name: keep\n    type: ss"),
        "the original bytes stay in place: {}",
        insertion.content
    );
    assert!(
        insertion.content.contains("- name: SS-Node"),
        "the catalogue body is spliced verbatim"
    );

    // Splitting a scalar to make room for a block sequence is not valid YAML:
    // the shared preflight refuses it instead of writing a broken profile.
    let refusal =
        crate::profile_document_application::insert_snippet("mode: rule\n", "select", 1, 6)
            .expect_err("a sequence cannot continue a scalar");
    assert!(
        refusal.message.contains("片段") && refusal.message.contains("无法解析"),
        "the typed failure carries the shared diagnostic: {}",
        refusal.message
    );

    // An unknown id is a programming error, never a silently dropped insert.
    let unknown = crate::profile_document_application::insert_snippet(document, "nope", 1, 0)
        .expect_err("unknown snippet ids are rejected");
    assert!(unknown.message.contains("nope"));
}

#[tokio::test]
async fn snapshot_history_intents_require_the_shared_application() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let application = application(&store);

    let error = application
        .execute(CommandIntent::LoadSnapshotHistory)
        .await
        .expect_err("a host without the snapshot port must say so");
    assert!(error.message.contains("snapshot application"), "{error:?}");

    let error = application
        .execute(CommandIntent::PruneSnapshots { keep: Some(5) })
        .await
        .expect_err("a host without the snapshot port must say so");
    assert!(error.message.contains("snapshot application"), "{error:?}");
}

/// DUAL-11-06: the unpack command reads a provider's real declared payload and
/// prepends the mapped rules; nothing is fabricated and an unknown provider is
/// a typed failure that leaves the profile untouched.
#[tokio::test]
async fn unpack_rule_provider_imports_real_payload_and_rejects_unknown() {
    let profile = "rule-providers:\n  ads:\n    type: inline\n    behavior: domain\n    payload:\n      - ads.com\n      - tracker.net\nrules:\n  - MATCH,DIRECT\n";
    let store = Arc::new(FakeStore::with_profile(profile));
    let application = application(&store).with_configuration(ConfigurationApplication::new(
        Arc::clone(&store) as Arc<dyn ProfileStore>,
    ));

    application
        .execute(CommandIntent::UnpackRuleProvider {
            provider_name: "ads".to_owned(),
        })
        .await
        .expect("unpack");

    let rules = infiltrator_domain::rules::load_rules_from_yaml(&store.content()).expect("parse");
    assert_eq!(rules.len(), 3);
    assert_eq!(rules[0].rule, "DOMAIN-SUFFIX,ads.com,PROXY");
    assert_eq!(rules[1].rule, "DOMAIN-SUFFIX,tracker.net,PROXY");
    assert_eq!(rules[2].rule, "MATCH,DIRECT");

    let failure = application
        .execute(CommandIntent::UnpackRuleProvider {
            provider_name: "absent".to_owned(),
        })
        .await
        .expect_err("unknown provider");
    assert!(
        failure.message.contains("absent"),
        "the failure names the provider: {failure:?}"
    );
    let failure = application
        .execute(CommandIntent::UnpackRuleProvider {
            provider_name: "  ".to_owned(),
        })
        .await
        .expect_err("empty provider name");
    assert_eq!(failure.code, ErrorCode::InvalidInput);
}

/// DUAL-11-14: the rules-workspace JSON partition submits one document and the
/// shared configuration use-case validates and persists it; a malformed or
/// empty document is a typed input error that leaves the profile untouched.
#[tokio::test]
async fn apply_rules_json_document_validates_and_persists_each_section() {
    use infiltrator_contract::rules_workspace::RulesJsonSection;

    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let configured = application(&store).with_configuration(ConfigurationApplication::new(
        Arc::clone(&store) as Arc<dyn ProfileStore>,
    ));

    // Rule providers: a valid document is written into the profile section.
    configured
        .execute(CommandIntent::ApplyRulesJsonDocument {
            section: RulesJsonSection::RuleProviders,
            json: r#"{"ads":{"type":"inline","behavior":"domain","payload":["ads.com"]}}"#
                .to_owned(),
        })
        .await
        .expect("rule providers document");
    let providers = configured
        .configuration()
        .expect("configuration")
        .load_rule_providers()
        .await
        .expect("reload providers");
    assert_eq!(providers.len(), 1);
    assert!(providers.contains_key("ads"));

    // Proxy providers and sniffer go through their own shared use-cases.
    configured
        .execute(CommandIntent::ApplyRulesJsonDocument {
            section: RulesJsonSection::ProxyProviders,
            json: r#"{"sub":{"type":"http","url":"https://example.com/proxies.yaml","interval":3600}}"#
                .to_owned(),
        })
        .await
        .expect("proxy providers document");
    configured
        .execute(CommandIntent::ApplyRulesJsonDocument {
            section: RulesJsonSection::Sniffer,
            json: r#"{"enable":true}"#.to_owned(),
        })
        .await
        .expect("sniffer document");
    let sniffer = configured
        .configuration()
        .expect("configuration")
        .load_sniffer_config()
        .await
        .expect("reload sniffer");
    assert_eq!(
        sniffer.get("enable").and_then(|value| value.as_bool()),
        Some(true)
    );

    // The rule list the same profile carries is untouched by the JSON writes.
    let rules = infiltrator_domain::rules::load_rules_from_yaml(&store.content()).expect("parse");
    assert_eq!(rules.len(), 3);

    // A malformed document never reaches the profile.
    let before = store.content();
    let failure = configured
        .execute(CommandIntent::ApplyRulesJsonDocument {
            section: RulesJsonSection::RuleProviders,
            json: "{not json".to_owned(),
        })
        .await
        .expect_err("malformed document");
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(store.content(), before, "the profile is untouched");

    // An empty document is refused before any write.
    let failure = configured
        .execute(CommandIntent::ApplyRulesJsonDocument {
            section: RulesJsonSection::Sniffer,
            json: "   ".to_owned(),
        })
        .await
        .expect_err("empty document");
    assert_eq!(failure.code, ErrorCode::InvalidInput);

    // A host without the configuration application is a typed unsupported.
    let hostless = application(&store);
    let failure = hostless
        .execute(CommandIntent::ApplyRulesJsonDocument {
            section: RulesJsonSection::Sniffer,
            json: "{}".to_owned(),
        })
        .await
        .expect_err("no configuration application");
    assert!(
        failure.message.contains("configuration application"),
        "{failure:?}"
    );
}

/// DUAL-11-14: the Geo database upgrade is the shared runtime-gateway call;
/// without a gateway the intent is a typed unsupported, never a fake success.
#[tokio::test]
async fn upgrade_geo_databases_requires_the_runtime_gateway() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let hostless = application(&store);
    let failure = hostless
        .execute(CommandIntent::UpgradeGeoDatabases)
        .await
        .expect_err("no runtime gateway");
    assert!(failure.message.contains("runtime gateway"), "{failure:?}");

    let gateway = Arc::new(RecordingGeoGateway::default());
    let with_gateway = application(&store).with_runtime(gateway.clone());
    with_gateway
        .execute(CommandIntent::UpgradeGeoDatabases)
        .await
        .expect("geo upgrade");
    assert_eq!(gateway.geo_upgrades(), 1);
}

/// DUAL-11-07: the purge command only reports what the injected host port
/// really removed, and a host without a cache location is a typed unsupported.
#[tokio::test]
async fn purge_rule_provider_cache_requires_and_reports_the_host_location() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let hostless = application(&store);
    let failure = hostless
        .execute(CommandIntent::PurgeRuleProviderCache)
        .await
        .expect_err("no cache location");
    assert_eq!(failure.code, ErrorCode::Unsupported);

    let cache = Arc::new(RecordingProviderCache::default());
    let application = application(&store).with_rule_provider_cache(cache.clone());
    application
        .execute(CommandIntent::PurgeRuleProviderCache)
        .await
        .expect("purge");
    assert_eq!(cache.purges(), 1);
}

/// A provider whose declaration has no local file and no inline payload is an
/// honest unsupported failure naming the sources that were tried.
#[tokio::test]
async fn unpack_without_any_source_is_unsupported() {
    let profile = "rule-providers:\n  cn:\n    type: http\n    behavior: domain\n    format: text\n    url: https://example.com/cn.txt\nrules:\n  - MATCH,DIRECT\n";
    let store = Arc::new(FakeStore::with_profile(profile));
    let application = application(&store)
        .with_configuration(ConfigurationApplication::new(
            Arc::clone(&store) as Arc<dyn ProfileStore>
        ))
        .with_rule_provider_cache(Arc::new(RecordingProviderCache::default()));

    let failure = application
        .execute(CommandIntent::UnpackRuleProvider {
            provider_name: "cn".to_owned(),
        })
        .await
        .expect_err("no readable source");
    assert_eq!(failure.code, ErrorCode::Unsupported);
    assert!(
        failure.message.contains("controller payload"),
        "{failure:?}"
    );
    assert_eq!(store.content(), profile, "nothing was persisted");
}

#[derive(Default)]
struct RecordingProviderCache {
    purges: Mutex<usize>,
}

impl RecordingProviderCache {
    fn purges(&self) -> usize {
        *self.purges.lock().expect("purge lock")
    }
}

#[async_trait]
impl infiltrator_ports::rule_provider_cache::RuleProviderCachePort for RecordingProviderCache {
    async fn read_provider(
        &self,
        _declaration: &infiltrator_domain::rules::provider_store::RuleProviderDeclaration,
    ) -> Result<Option<infiltrator_ports::rule_provider_cache::ProviderCacheEntry>, PortError> {
        Ok(None)
    }

    async fn purge(
        &self,
    ) -> Result<infiltrator_contract::provider_cache::ProviderCachePurge, PortError> {
        *self.purges.lock().expect("purge lock") += 1;
        Ok(infiltrator_contract::provider_cache::ProviderCachePurge {
            directory: Some("/fake/configs/rules".to_owned()),
            files_removed: 2,
            bytes_freed: 64,
        })
    }

    async fn fingerprint(
        &self,
        _declaration: &infiltrator_domain::rules::provider_store::RuleProviderDeclaration,
    ) -> Result<Option<infiltrator_ports::rule_provider_cache::ProviderFileFact>, PortError> {
        Ok(None)
    }

    async fn snapshot(
        &self,
    ) -> Result<infiltrator_contract::provider_cache::RuleProviderCacheSnapshot, PortError> {
        Ok(
            infiltrator_contract::provider_cache::RuleProviderCacheSnapshot::ready(
                "/fake/configs/rules",
                0,
                0,
            ),
        )
    }
}

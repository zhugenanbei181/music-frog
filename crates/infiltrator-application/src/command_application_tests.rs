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

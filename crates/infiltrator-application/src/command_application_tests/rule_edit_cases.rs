//! Behavior cases for rule, subscription, profile, and provider commands.

use super::*;
use crate::core_capability_test_support::capability_application;
use crate::geo_database_application::GeoDatabaseApplication;
use crate::rule_provider_runtime_application::RuleProviderRuntimeApplication;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::geo_database::GeoDatabaseTriggerReceipt;
use infiltrator_contract::profile_document_identity::ProfileDocumentIdentity;
use infiltrator_contract::rule_provider_runtime::{
    RuleProviderFact, RuleProviderId, RuleProviderInventorySnapshot, RuleProviderPayloadSnapshot,
    RuleProviderRefreshReceipt, RuleProviderRevision,
};
use infiltrator_contract::rule_provider_snapshot::RuleProviderControllerReference;
use infiltrator_ports::geo_database::GeoDatabasePort;
use infiltrator_ports::rule_provider_runtime::RuleProviderPort;
use lurker_core_contract::identity::CoreScope;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

#[derive(Default)]
struct RecordingGeoPort(AtomicUsize);

#[async_trait::async_trait]
impl GeoDatabasePort for RecordingGeoPort {
    async fn trigger(&self, scope: CoreScope) -> Result<GeoDatabaseTriggerReceipt, PortError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(GeoDatabaseTriggerReceipt::accepted(scope))
    }
}

#[derive(Default)]
struct RecordingRuleProviderPort(Mutex<Option<(CoreScope, RuleProviderRevision, RuleProviderId)>>);

#[async_trait::async_trait]
impl RuleProviderPort for RecordingRuleProviderPort {
    async fn inventory(
        &self,
        _scope: CoreScope,
    ) -> Result<RuleProviderInventorySnapshot, PortError> {
        Err(PortError::unsupported(
            Capability::RoutingIntent,
            "single-provider command must not query inventory",
        ))
    }

    async fn refresh(
        &self,
        scope: CoreScope,
        revision: RuleProviderRevision,
        id: RuleProviderId,
    ) -> Result<RuleProviderRefreshReceipt, PortError> {
        *self.0.lock().expect("provider command recording lock") =
            Some((scope.clone(), revision, id.clone()));
        let fact = RuleProviderFact {
            id: id.clone(),
            provider_type: "http".to_owned(),
            behavior: "domain".to_owned(),
            vehicle_type: "HTTP".to_owned(),
            updated_at: "accepted".to_owned(),
            rule_count: 1,
        };
        let inventory = RuleProviderInventorySnapshot::new(
            scope.clone(),
            revision.next().expect("test revision advances"),
            vec![fact],
        )
        .expect("test inventory is valid");
        RuleProviderRefreshReceipt::accepted(scope, revision, inventory, id)
            .map_err(|error| PortError::Failed(error.to_string()))
    }

    async fn payload(
        &self,
        _scope: CoreScope,
        _revision: RuleProviderRevision,
        _id: RuleProviderId,
    ) -> Result<RuleProviderPayloadSnapshot, PortError> {
        Err(PortError::unsupported(
            Capability::RoutingIntent,
            "single-provider command does not read payload",
        ))
    }
}

#[tokio::test]
async fn toggle_and_move_apply_shared_reductions_and_persist() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let application = application(&store);

    application
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::ToggleRuleEnabled { index: 1 },
        )
        .await
        .expect("toggle");

    let rules = load_rules_from_yaml(&store.content()).expect("parse");
    assert!(!rules[1].enabled);
    assert!(store.content().contains("# DOMAIN,b.com,PROXY"));

    application
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::MoveRule {
                index: 0,
                direction: RuleMoveDirection::Down,
            },
        )
        .await
        .expect("move");

    let rules = load_rules_from_yaml(&store.content()).expect("parse");
    assert_eq!(rules[0].rule, "DOMAIN,b.com,PROXY");
    assert_eq!(rules[1].rule, "DOMAIN,a.com,DIRECT");
}

#[tokio::test]
async fn toggle_out_of_range_is_a_noop() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let application = application(&store);
    application
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::ToggleRuleEnabled { index: 9 },
        )
        .await
        .expect("noop");
    assert_eq!(store.content(), THREE_RULES);
}

#[tokio::test]
async fn add_custom_rule_prepends_and_rejects_invalid_logical_form() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let application = application(&store);

    application
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::AddCustomRule {
                rule_type: "DOMAIN-SUFFIX".to_owned(),
                payload: "github.com".to_owned(),
                target: "PROXY".to_owned(),
            },
        )
        .await
        .expect("add");

    let rules = load_rules_from_yaml(&store.content()).expect("parse");
    assert_eq!(rules[0].rule, "DOMAIN-SUFFIX,github.com,PROXY");

    let failure = application
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::AddCustomRule {
                rule_type: "AND".to_owned(),
                payload: "(DOMAIN,a.com".to_owned(),
                target: "PROXY".to_owned(),
            },
        )
        .await
        .expect_err("unclosed logical rule must fail");
    assert_eq!(failure.code, ErrorCode::InvalidInput);

    // The shared builder also rejects an empty payload before touching disk.
    assert!(
        build_custom_rule(&RuleDraft {
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
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::ApplyGameRoutingPresets {
                target: "Game-Proxy".to_owned(),
            },
        )
        .await
        .expect("presets");

    let rules = load_rules_from_yaml(&store.content()).expect("parse");
    let expected = game_routing_presets("Game-Proxy");
    assert_eq!(rules[0].rule, expected[0].rule);
    assert_eq!(
        rules[expected.len() - 1].rule,
        expected[expected.len() - 1].rule
    );
    assert_eq!(rules[expected.len()].rule, "DOMAIN,a.com,DIRECT");
}

/// DUAL-07-09: enabling auto-reload on a host without a Core-owner profile
/// seam is a typed unsupported failure; disabling is always allowed because it
/// cannot silently no-op.
#[tokio::test]
async fn auto_reload_enable_requires_a_profile_reference_applier_seam() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let application = application(&store);

    let failure = application
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::SetSubscriptionAutoReload {
                profile_id: "main".to_owned(),
                enabled: true,
            },
        )
        .await
        .expect_err("no reload seam on this host");
    assert_eq!(failure.code, ErrorCode::Unsupported);
    assert!(!store.metadata().auto_reload_core, "nothing was persisted");

    application
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::SetSubscriptionAutoReload {
                profile_id: "main".to_owned(),
                enabled: false,
            },
        )
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
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::UpdateSubscriptionSchedule {
                profile_id: "main".to_owned(),
                draft: SubscriptionScheduleDraft {
                    url: "https://sub.example.com/token".to_owned(),
                    auto_update_enabled: true,
                    update_interval_hours: "6".to_owned(),
                    cron_expression: Some("0 */6 * * *".to_owned()),
                },
            },
        )
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
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::UpdateSubscriptionSchedule {
                profile_id: "main".to_owned(),
                draft: SubscriptionScheduleDraft {
                    url: "https://sub.example.com/token".to_owned(),
                    auto_update_enabled: true,
                    update_interval_hours: "6".to_owned(),
                    cron_expression: Some("definitely not a cron".to_owned()),
                },
            },
        )
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
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::AddCustomRule {
                rule_type: "DOMAIN-SUFFIX".to_owned(),
                payload: "google.com".to_owned(),
                target: "PROXY".to_owned(),
            },
        )
        .await
        .expect("add");
    let saved = store.content();
    assert!(saved.contains("# 手写头注释"), "top comment kept: {saved}");
    assert!(
        saved.contains("# 规则块说明"),
        "block comment kept: {saved}"
    );
    assert!(saved.contains("# 兜底规则"), "inline comment kept: {saved}");
    assert_eq!(load_rules_from_yaml(&saved).expect("parse").len(), 2);

    application
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::ToggleRuleEnabled { index: 0 },
        )
        .await
        .expect("toggle");
    let saved = store.content();
    let rules = load_rules_from_yaml(&saved).expect("parse");
    assert!(!rules[0].enabled, "the new rule is now disabled");
    assert!(
        saved.contains("# 手写头注释"),
        "toggle keeps comments: {saved}"
    );
    assert!(saved.contains("# 规则块说明"));
}

// ---- DUAL-09-03/06/07/14: document + history commands ----------------------

#[tokio::test]
async fn snapshot_history_intents_require_the_shared_application() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let application = application(&store);

    let error = application
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::LoadSnapshotHistory {
                profile: None,
                keep: 20,
            },
        )
        .await
        .expect_err("a host without the snapshot port must say so");
    assert!(error.message.contains("snapshot application"), "{error:?}");

    let error = application
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::PruneSnapshots {
                profile: None,
                keep: Some(5),
            },
        )
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
    let application = application(&store).with_configuration(
        ConfigurationApplication::test_configuration_application(
            Arc::clone(&store) as Arc<dyn ProfileStore>
        ),
    );

    application
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::UnpackRuleProvider {
                provider_name: "ads".to_owned(),
            },
        )
        .await
        .expect("unpack");

    let rules = load_rules_from_yaml(&store.content()).expect("parse");
    assert_eq!(rules.len(), 3);
    assert_eq!(rules[0].rule, "DOMAIN-SUFFIX,ads.com,PROXY");
    assert_eq!(rules[1].rule, "DOMAIN-SUFFIX,tracker.net,PROXY");
    assert_eq!(rules[2].rule, "MATCH,DIRECT");

    let failure = application
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::UnpackRuleProvider {
                provider_name: "absent".to_owned(),
            },
        )
        .await
        .expect_err("unknown provider");
    assert!(
        failure.message.contains("absent"),
        "the failure names the provider: {failure:?}"
    );
    let failure = application
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::UnpackRuleProvider {
                provider_name: "  ".to_owned(),
            },
        )
        .await
        .expect_err("empty provider name");
    assert_eq!(failure.code, ErrorCode::InvalidInput);
}

/// Profile section updates are typed and revision-bound at command ingress.
#[tokio::test]
async fn apply_profile_configuration_updates_each_typed_section() {
    use infiltrator_contract::profile_configuration::{
        ProfileConfigurationRequest, ProfileConfigurationUpdate, SnifferConfiguration,
    };

    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let configured = application(&store).with_configuration(
        ConfigurationApplication::test_configuration_application(
            Arc::clone(&store) as Arc<dyn ProfileStore>
        ),
    );
    let scope = || ValidatedCoreCommandScope::from_core_application(Some(core_scope()));

    // Rule providers use a typed map and are scoped to the observed document.
    let identity = active_document_identity(&store).await;
    configured
        .handle_scoped(scope(), CommandIntent::ApplyProfileConfiguration {
            request: Box::new(ProfileConfigurationRequest {
                identity,
                update: ProfileConfigurationUpdate::RuleProviders(serde_json::from_value(
                    serde_json::json!({"ads":{"type":"inline","behavior":"domain","payload":["ads.com"]}}),
                ).unwrap()),
            }),
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

    // Proxy providers and sniffer keep their own typed codec operations.
    let identity = active_document_identity(&store).await;
    configured
        .handle_scoped(scope(), CommandIntent::ApplyProfileConfiguration {
            request: Box::new(ProfileConfigurationRequest {
                identity,
                update: ProfileConfigurationUpdate::ProxyProviders(serde_json::from_value(
                    serde_json::json!({"sub":{"type":"http","url":"https://example.com/proxies.yaml","interval":3600}}),
                ).unwrap()),
            }),
        })
        .await
        .expect("proxy providers document");
    configured
        .handle_scoped(
            scope(),
            CommandIntent::ApplyProfileConfiguration {
                request: Box::new(ProfileConfigurationRequest {
                    identity: active_document_identity(&store).await,
                    update: ProfileConfigurationUpdate::Sniffer(SnifferConfiguration {
                        enable: Some(true),
                        ..Default::default()
                    }),
                }),
            },
        )
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
    let rules = configured
        .configuration()
        .expect("configuration")
        .load_rules()
        .await
        .expect("read rules");
    assert_eq!(rules.len(), 3);

    // A host without the configuration application is a typed unsupported.
    let profiles = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>)
        .with_document_codec(Arc::new(TestProfileDocumentCodec::mihomo()));
    let hostless = with_test_capabilities(CommandApplication::new().with_profile(profiles));
    let failure = hostless
        .handle_scoped(
            scope(),
            CommandIntent::ApplyProfileConfiguration {
                request: Box::new(ProfileConfigurationRequest {
                    identity: active_document_identity(&store).await,
                    update: ProfileConfigurationUpdate::Sniffer(SnifferConfiguration::default()),
                }),
            },
        )
        .await
        .expect_err("no configuration application");
    assert!(
        failure.message.contains("configuration application"),
        "{failure:?}"
    );
}

async fn active_document_identity(store: &Arc<FakeStore>) -> ProfileDocumentIdentity {
    ProfileApplication::new(Arc::clone(store) as Arc<dyn ProfileStore>)
        .load_active_workspace()
        .await
        .expect("active profile workspace")
        .document
        .identity
}

/// Geo trigger dispatch uses the owner-scoped port; without it the intent is
/// a typed unsupported, never a fake success.
#[tokio::test]
async fn upgrade_geo_databases_requires_the_scoped_port() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let hostless = application(&store);
    let failure = hostless
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::UpgradeGeoDatabases,
        )
        .await
        .expect_err("no scoped Geo database application");
    assert!(
        failure.message.contains("Geo database application"),
        "{failure:?}"
    );

    let port = Arc::new(RecordingGeoPort::default());
    let with_port = application(&store).with_geo_database(GeoDatabaseApplication::new(
        port.clone(),
        capability_application(),
    ));
    with_port
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::UpgradeGeoDatabases,
        )
        .await
        .expect("geo upgrade");
    assert_eq!(port.0.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn refresh_rule_provider_dispatches_snapshot_reference_to_scoped_port() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let scope = core_scope();
    let reference = RuleProviderControllerReference {
        id: RuleProviderId::new(scope.identity.engine.clone(), "ads").unwrap(),
        inventory_revision: RuleProviderRevision::new(7).unwrap(),
    };
    let port = Arc::new(RecordingRuleProviderPort::default());
    let application = application(&store).with_rule_provider_runtime(
        RuleProviderRuntimeApplication::new(port.clone(), capability_application()),
    );

    application
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(scope.clone())),
            CommandIntent::RefreshRuleProvider { reference },
        )
        .await
        .expect("typed provider refresh");

    assert_eq!(
        *port.0.lock().expect("provider command recording lock"),
        Some((
            scope,
            RuleProviderRevision::new(7).unwrap(),
            RuleProviderId::new(core_scope().identity.engine, "ads").unwrap(),
        )),
    );
}

/// DUAL-11-07: the purge command only reports what the injected host port
/// really removed, and a host without a cache location is a typed unsupported.
#[tokio::test]
async fn purge_rule_provider_cache_requires_and_reports_the_host_location() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let hostless = application(&store);
    let failure = hostless
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::PurgeRuleProviderCache,
        )
        .await
        .expect_err("no cache location");
    assert_eq!(failure.code, ErrorCode::Unsupported);

    let cache = Arc::new(RecordingProviderCache::default());
    let application = application(&store).with_rule_provider_cache(cache.clone());
    application
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::PurgeRuleProviderCache,
        )
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
        .with_configuration(ConfigurationApplication::test_configuration_application(
            Arc::clone(&store) as Arc<dyn ProfileStore>,
        ))
        .with_rule_provider_cache(Arc::new(RecordingProviderCache::default()));

    let failure = application
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::UnpackRuleProvider {
                provider_name: "cn".to_owned(),
            },
        )
        .await
        .expect_err("no readable source");
    assert_eq!(failure.code, ErrorCode::Unsupported);
    assert!(
        failure.message.contains("controller payload"),
        "{failure:?}"
    );
    assert_eq!(store.content(), profile, "nothing was persisted");
}

/// DUAL-11-05: the active profile's top-level `etag-support` declaration is
/// read honestly: an explicit `true`/`false` classifies as enabled/disabled,
/// and an absent key stays `NotDeclared` instead of inventing the kernel
/// default. The per-request `304` outcome is never inferred.
#[tokio::test]
async fn load_etag_support_reads_the_top_level_declaration_honestly() {
    use infiltrator_contract::provider_cache::KernelEtagSupportState;

    let enabled_store = Arc::new(FakeStore::with_profile(
        "etag-support: true\nrules:\n  - MATCH,DIRECT\n",
    ));
    let enabled = ConfigurationApplication::test_configuration_application(Arc::clone(
        &enabled_store,
    )
        as Arc<dyn ProfileStore>)
    .load_etag_support()
    .await
    .expect("read enabled");
    assert_eq!(enabled.state, KernelEtagSupportState::Enabled);
    assert_eq!(enabled.declared, Some(true));

    let disabled_store = Arc::new(FakeStore::with_profile("etag-support: false\n"));
    let disabled = ConfigurationApplication::test_configuration_application(Arc::clone(
        &disabled_store,
    )
        as Arc<dyn ProfileStore>)
    .load_etag_support()
    .await
    .expect("read disabled");
    assert_eq!(disabled.state, KernelEtagSupportState::Disabled);
    assert_eq!(disabled.declared, Some(false));

    // No `etag-support` key at all: the state is NotDeclared and no raw value
    // is invented from the kernel's own default.
    let absent_store = Arc::new(FakeStore::with_profile("rules:\n  - MATCH,DIRECT\n"));
    let absent = ConfigurationApplication::test_configuration_application(
        Arc::clone(&absent_store) as Arc<dyn ProfileStore>,
    )
    .load_etag_support()
    .await
    .expect("read absent");
    assert_eq!(absent.state, KernelEtagSupportState::NotDeclared);
    assert_eq!(absent.declared, None);
    assert_eq!(absent.state.as_str(), "not-declared");
}

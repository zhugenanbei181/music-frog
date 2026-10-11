use super::{
    CommandCapabilityRoute, CoreTerminalScopePolicy, classify_command, terminal_scope_policy,
    validate_terminal_scope,
};
use crate::command_application::CommandApplication;
use crate::command_application::handler::{CommandHandler, ValidatedCoreCommandScope};
use crate::core_capability_application::{
    CoreCapabilityApplication, CoreCapabilityRequirement, CoreCommandEffect,
};
use crate::profile_application::ProfileApplication;
use async_trait::async_trait;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::command::{CommandIntent, CommandResult, RequestId};
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, AdapterRevision, ConnectionOperation, CoreCapabilityEntry,
    CoreCapabilityMatrix, CoreCapabilityScope, CoreGarbageCollectionOperation,
    CoreLifecycleOperation, CoreOperation, DnsOperation, EngineBinaryTarget,
    EngineCapabilityDecision, FakeIpCacheOperation, GeoOperation, HostCapabilityDecision,
    OutboundOperation, ProfileOperation, ProviderOperation, ProxyControlOperation, RulesOperation,
    SubscriptionOperation, TunOperation, VersionOperation,
};
use infiltrator_contract::core_identity::{
    CoreCommandOutcome, CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion,
    EngineId,
};
use infiltrator_contract::dns::DnsSettingsPatch;
use infiltrator_contract::dns_cache::DnsCacheOperationId;
use infiltrator_contract::dns_query::{DnsQueryOperationId, DnsQueryRequest, DnsRecordType};
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::routing_intent::RoutingIntent;
use infiltrator_contract::session::SessionToken;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::core_capability_provider::{
    CoreCapabilityProvider, CoreCapabilityProviderError,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct ExplicitlyUnsupportedOperation(CoreOperation);

impl CoreCapabilityProvider for ExplicitlyUnsupportedOperation {
    fn matrix_for(
        &self,
        engine_id: &EngineId,
        core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        let binary_target = EngineBinaryTarget::current();
        let host = binary_target.host_kind().ok_or_else(|| {
            CoreCapabilityProviderError::MatrixUnavailable {
                reason: "test target has no registered host".to_owned(),
            }
        })?;
        let scope = CoreCapabilityScope::new_for_target(
            engine_id.clone(),
            core_version.cloned(),
            host,
            binary_target,
            AdapterRevision::new(1),
        );
        CoreCapabilityMatrix::new(
            scope,
            vec![CoreCapabilityEntry::new(
                self.0,
                EngineCapabilityDecision::Unsupported {
                    reason: "native profile transactions are not composed".to_owned(),
                },
                HostCapabilityDecision::Supported,
                AdapterCapabilityDecision::Supported,
            )],
        )
        .map_err(|error| CoreCapabilityProviderError::MatrixUnavailable {
            reason: error.to_string(),
        })
    }
}

#[derive(Default)]
struct CountingProfileStore {
    calls: AtomicUsize,
}

impl CountingProfileStore {
    fn record(&self) {
        self.calls.fetch_add(1, Ordering::SeqCst);
    }
}

#[async_trait]
impl ProfileStore for CountingProfileStore {
    fn engine_id(&self) -> EngineId {
        EngineId::new("mihomo").expect("test engine ID")
    }

    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/fake/profile-store")
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        self.record();
        Ok(vec![ProfileInfo {
            name: "main".to_owned(),
            ..ProfileInfo::default()
        }])
    }

    async fn get_current(&self) -> Result<String, PortError> {
        self.record();
        Ok("main".to_owned())
    }

    async fn set_current(&self, _profile: &str) -> Result<(), PortError> {
        self.record();
        Ok(())
    }

    async fn load(&self, _profile: &str) -> Result<String, PortError> {
        self.record();
        Ok("profile".to_owned())
    }

    async fn save(&self, _profile: &str, _content: &str) -> Result<(), PortError> {
        self.record();
        Ok(())
    }

    async fn delete_profile(&self, _profile: &str) -> Result<(), PortError> {
        self.record();
        Ok(())
    }

    async fn get_profile_metadata(&self, _profile: &str) -> Result<ProfileMetadata, PortError> {
        self.record();
        Ok(ProfileMetadata::default())
    }

    async fn update_profile_metadata(
        &self,
        _profile: &str,
        _metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        self.record();
        Ok(())
    }

    async fn delete_subscription_credential(&self, _profile: &str) -> Result<(), PortError> {
        self.record();
        Ok(())
    }

    async fn clear_backup(&self, _profile: &str) -> Result<(), PortError> {
        self.record();
        Ok(())
    }

    async fn restore_backup(&self, _profile: &str) -> Result<bool, PortError> {
        self.record();
        Ok(false)
    }
}

fn lifecycle(operation: CoreLifecycleOperation) -> CommandCapabilityRoute {
    CommandCapabilityRoute::LifecycleOwned(CoreCapabilityRequirement::new(
        CoreOperation::Lifecycle(operation),
        CoreCommandEffect::Write,
    ))
}

#[test]
fn lifecycle_command_routes_preserve_engine_operation_identity() {
    assert_eq!(
        classify_command(&CommandIntent::StartCore),
        lifecycle(CoreLifecycleOperation::Start)
    );
    assert_eq!(
        classify_command(&CommandIntent::StopCore),
        lifecycle(CoreLifecycleOperation::Stop)
    );
    assert_eq!(
        classify_command(&CommandIntent::RestartCore),
        lifecycle(CoreLifecycleOperation::Restart)
    );
    assert_eq!(
        classify_command(&CommandIntent::ResumeKernel),
        lifecycle(CoreLifecycleOperation::Start)
    );
    assert_eq!(
        classify_command(&CommandIntent::PauseKernel),
        lifecycle(CoreLifecycleOperation::Stop)
    );
    assert_eq!(
        classify_command(&CommandIntent::RestartKernel),
        lifecycle(CoreLifecycleOperation::Restart)
    );
}

#[test]
fn proxy_lifecycle_uses_core_start_and_typed_host_children() {
    assert_eq!(
        classify_command(&CommandIntent::StartProxy),
        lifecycle(CoreLifecycleOperation::Start)
    );
    assert_eq!(
        classify_command(&CommandIntent::StopProxy),
        CommandCapabilityRoute::LifecycleChild
    );
    assert_eq!(
        terminal_scope_policy(&CommandIntent::StartProxy),
        CoreTerminalScopePolicy::SameInstanceMayAdvance
    );
    assert_eq!(
        terminal_scope_policy(&CommandIntent::StopProxy),
        CoreTerminalScopePolicy::Stable
    );
    let intent = CommandIntent::SetRoutingIntent {
        intent: RoutingIntent::RouteAllThroughProxy,
    };
    assert_eq!(
        classify_command(&intent),
        CommandCapabilityRoute::CoreBackend(CoreCapabilityRequirement::new(
            CoreOperation::ProxyControl(ProxyControlOperation::SetRoutingIntent),
            CoreCommandEffect::Write,
        ))
    );
    assert_eq!(
        terminal_scope_policy(&intent),
        CoreTerminalScopePolicy::Stable
    );
    let observation = CommandIntent::GetRoutingIntent;
    assert_eq!(
        classify_command(&observation),
        CommandCapabilityRoute::CoreBackend(CoreCapabilityRequirement::new(
            CoreOperation::ProxyControl(ProxyControlOperation::GetRoutingIntent),
            CoreCommandEffect::Read,
        ))
    );
    assert_eq!(
        terminal_scope_policy(&observation),
        CoreTerminalScopePolicy::Stable
    );
}

fn scope(engine: &str, instance: &str, generation: u64, session: u128) -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::new(engine).expect("engine ID"),
            CoreInstanceId::new(instance).expect("core instance ID"),
            Some(CoreVersion::new("1.20.0").expect("core version")),
        ),
        generation: CoreGeneration::new(generation),
        session_token: Some(SessionToken::new(session)),
    }
}

fn completed(scope: CoreScope) -> CoreCommandOutcome {
    CoreCommandOutcome {
        scope: Some(scope),
        result: CommandResult::Completed {
            request_id: RequestId::new(9),
        },
    }
}

#[test]
fn active_profile_apply_allows_same_instance_reload_and_rejects_identity_drift() {
    let intent = CommandIntent::SwitchProfile {
        profile_id: "travel".to_owned(),
    };
    let request_scope = scope("mihomo", "desktop-main", 4, 40);
    assert_eq!(
        terminal_scope_policy(&intent),
        CoreTerminalScopePolicy::SameInstanceMayAdvance
    );

    let reloaded_scope = scope("mihomo", "desktop-main", 5, 50);
    assert_eq!(
        validate_terminal_scope(&request_scope, &intent, completed(reloaded_scope.clone())),
        completed(reloaded_scope)
    );

    for divergent_scope in [
        scope("sing-box", "desktop-main", 5, 50),
        scope("mihomo", "desktop-secondary", 5, 50),
        scope("mihomo", "desktop-main", 4, 41),
        scope("mihomo", "desktop-main", 3, 30),
    ] {
        assert!(matches!(
            validate_terminal_scope(&request_scope, &intent, completed(divergent_scope)).result,
            CommandResult::Rejected { .. }
        ));
    }
}

#[test]
fn active_profile_configuration_save_allows_same_instance_reload() {
    let intent = CommandIntent::ApplyDnsSettings {
        patch: DnsSettingsPatch::default(),
    };
    let request_scope = scope("mihomo", "desktop-main", 4, 40);
    let reloaded_scope = scope("mihomo", "desktop-main", 5, 50);

    assert_eq!(
        terminal_scope_policy(&intent),
        CoreTerminalScopePolicy::SameInstanceMayAdvance
    );
    assert_eq!(
        validate_terminal_scope(&request_scope, &intent, completed(reloaded_scope.clone())),
        completed(reloaded_scope)
    );
}

#[test]
fn stop_core_allows_only_the_reducer_session_clear_transition() {
    let intent = CommandIntent::StopCore;
    let request_scope = scope("mihomo", "desktop-main", 4, 40);
    assert_eq!(
        terminal_scope_policy(&intent),
        CoreTerminalScopePolicy::SameInstanceMayStop
    );

    let mut stopped_scope = request_scope.clone();
    stopped_scope.session_token = None;
    assert_eq!(
        validate_terminal_scope(&request_scope, &intent, completed(stopped_scope.clone())),
        completed(stopped_scope)
    );

    let mut changed_session = request_scope.clone();
    changed_session.session_token = Some(SessionToken::new(41));
    let mut changed_engine = scope("sing-box", "desktop-main", 4, 40);
    changed_engine.session_token = None;
    let mut changed_instance = scope("mihomo", "desktop-secondary", 4, 40);
    changed_instance.session_token = None;
    let mut changed_generation = scope("mihomo", "desktop-main", 5, 50);
    changed_generation.session_token = None;

    for divergent_scope in [
        changed_session,
        changed_engine,
        changed_instance,
        changed_generation,
    ] {
        assert!(matches!(
            validate_terminal_scope(&request_scope, &intent, completed(divergent_scope)).result,
            CommandResult::Rejected { .. }
        ));
    }
}

#[test]
fn ordinary_runtime_command_requires_a_stable_terminal_scope() {
    let intent = CommandIntent::ClearLogs;
    let request_scope = scope("mihomo", "desktop-main", 4, 40);
    assert_eq!(
        terminal_scope_policy(&intent),
        CoreTerminalScopePolicy::Stable
    );
    let advanced_scope = scope("mihomo", "desktop-main", 5, 50);
    assert!(matches!(
        validate_terminal_scope(&request_scope, &intent, completed(advanced_scope)).result,
        CommandResult::Rejected { .. }
    ));
}

#[test]
fn profile_document_and_options_loads_are_local_commands_in_the_scoped_owner_ingress() {
    let document = CommandIntent::LoadProfileDocument {
        profile: Some("main".to_owned()),
    };
    let options = CommandIntent::LoadProfileOptions {
        profile: Some("main".to_owned()),
    };
    for intent in [&document, &options] {
        assert_eq!(classify_command(intent), CommandCapabilityRoute::LocalOnly);
    }
}

#[tokio::test]
async fn every_core_command_family_fails_closed_without_scope_and_provider() {
    let application = CommandApplication::default();
    let cases = [
        (
            CommandIntent::SwitchProfile {
                profile_id: "primary".to_owned(),
            },
            CoreOperation::Profile(ProfileOperation::Apply),
            CoreCommandEffect::Write,
        ),
        (
            CommandIntent::QueryDns {
                operation: DnsQueryOperationId(1),
                request: DnsQueryRequest {
                    name: "example.com".to_owned(),
                    record_type: DnsRecordType::A,
                },
            },
            CoreOperation::Dns(DnsOperation::Query),
            CoreCommandEffect::Read,
        ),
        (
            CommandIntent::ToggleRuleEnabled { index: 0 },
            CoreOperation::Rules(RulesOperation::Apply),
            CoreCommandEffect::Write,
        ),
        (
            CommandIntent::UpdateAllSubscriptions,
            CoreOperation::Subscription(SubscriptionOperation::Refresh),
            CoreCommandEffect::Write,
        ),
        (
            CommandIntent::RefreshRuleProviders,
            CoreOperation::Provider(ProviderOperation::UpdateRule),
            CoreCommandEffect::Write,
        ),
        (
            CommandIntent::ReadOutboundInventory,
            CoreOperation::Outbound(OutboundOperation::List),
            CoreCommandEffect::Read,
        ),
        (
            CommandIntent::CloseAllConnections,
            CoreOperation::Connection(ConnectionOperation::Close),
            CoreCommandEffect::Write,
        ),
        (
            CommandIntent::UpgradeGeoDatabases,
            CoreOperation::Geo(GeoOperation::UpdateDatabases),
            CoreCommandEffect::Write,
        ),
        (
            CommandIntent::ClearDnsCache {
                operation: DnsCacheOperationId(2),
            },
            CoreOperation::FakeIpCache(FakeIpCacheOperation::Flush),
            CoreCommandEffect::Write,
        ),
        (
            CommandIntent::ToggleTun { enabled: true },
            CoreOperation::Tun(TunOperation::Configure),
            CoreCommandEffect::Write,
        ),
        (
            CommandIntent::SetRoutingIntent {
                intent: RoutingIntent::FollowRules,
            },
            CoreOperation::ProxyControl(ProxyControlOperation::SetRoutingIntent),
            CoreCommandEffect::Write,
        ),
        (
            CommandIntent::PurgeRuleProviderCache,
            CoreOperation::CoreGarbageCollection(CoreGarbageCollectionOperation::Trigger),
            CoreCommandEffect::Write,
        ),
        (
            CommandIntent::CheckUpdates,
            CoreOperation::Version(VersionOperation::Inspect),
            CoreCommandEffect::Read,
        ),
    ];

    for (intent, operation, effect) in cases {
        assert_eq!(
            classify_command(&intent),
            CommandCapabilityRoute::CoreBackend(CoreCapabilityRequirement::new(operation, effect)),
            "{intent:?} must be bound to its typed engine operation"
        );
        let failure = application
            .execute_output(intent)
            .await
            .expect_err("an unscoped command without a provider must be denied");
        assert_eq!(failure.code, ErrorCode::NotReady);
        assert!(failure.message.contains("capability gate unavailable"));
    }

    let failure = application
        .execute_output(CommandIntent::ClearLogs)
        .await
        .expect_err("local log-buffer clearing requires the host Logs capability");
    assert_eq!(
        classify_command(&CommandIntent::ClearLogs),
        CommandCapabilityRoute::HostOnly(Capability::Logs)
    );
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert!(
        failure
            .message
            .contains("host-only command has no composed host capability")
    );
}

#[tokio::test]
async fn unsupported_profile_validate_and_apply_are_rejected_before_store_dispatch() {
    let cases = [
        (
            CommandIntent::ScanDialerChains,
            CoreOperation::Profile(ProfileOperation::Validate),
            CoreCommandEffect::Read,
        ),
        (
            CommandIntent::SwitchProfile {
                profile_id: "main".to_owned(),
            },
            CoreOperation::Profile(ProfileOperation::Apply),
            CoreCommandEffect::Write,
        ),
    ];

    for (intent, operation, effect) in cases {
        assert_eq!(
            classify_command(&intent),
            CommandCapabilityRoute::CoreBackend(CoreCapabilityRequirement::new(operation, effect))
        );
        let store = Arc::new(CountingProfileStore::default());
        let application = CommandApplication::new()
            .with_profile(ProfileApplication::new(store.clone()))
            .with_core_capabilities(CoreCapabilityApplication::new(Arc::new(
                ExplicitlyUnsupportedOperation(operation),
            )));
        let failure = application
            .handle_scoped(
                ValidatedCoreCommandScope::from_core_application(Some(scope(
                    "mihomo",
                    "command-gate",
                    1,
                    1,
                ))),
                intent,
            )
            .await
            .expect_err("unsupported native profile operations are denied at command admission");

        assert_eq!(failure.code, ErrorCode::Unsupported);
        assert!(failure.message.contains("core capability denied"));
        assert_eq!(
            store.calls.load(Ordering::SeqCst),
            0,
            "denied profile operations must stop before the ProfileStore"
        );
    }
}

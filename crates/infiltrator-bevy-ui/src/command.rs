//! Strongly typed UI Command Bus and EntityObserver/Trigger Infrastructure.
//!
//! Charter law (docs/bevy-ui/BEVY_UI_FRONTEND.md):
//! All UI user interactions (button clicks, switches, mode selections,
//! reconnects, clears) dispatch through typed commands into a centralized
//! command sink handle. No direct blocking calls in UI systems.

use crate::command_events::CommandExecutedEvent;
use crate::command_execution::{ApplicationCommandSink, drain_command_results};
use bevy::app::{App, Plugin, Update};
use bevy::ecs::resource::Resource;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_contract::aggregator::AggregationDraft;
use infiltrator_contract::command::{CommandIntent, CoreLogLevel, ProxyMode, RequestId};
use infiltrator_contract::dns::DnsSettingsPatch;
use infiltrator_contract::dns_cache::DnsCacheOperationId;
use infiltrator_contract::dns_query::{DnsQueryOperationId, DnsQueryRequest};
use infiltrator_contract::lan::LanCredentials;
use infiltrator_contract::language::LanguagePreference;
use infiltrator_contract::log_export::LogExportIdentity;
use infiltrator_contract::overview_layout::OverviewCardKind;
use infiltrator_contract::profile_source::ProfileSourceIdentity;
use infiltrator_contract::protocol_fidelity::ProtocolDraft;
use infiltrator_contract::protocol_trust::TlsTrustParams;
use infiltrator_contract::proxies::ProxySortOrder;
use infiltrator_contract::proxy_probe_options::ProxyProbeOptions;
use infiltrator_contract::rule_document::RuleListCommit;
use infiltrator_contract::rule_source::RuleSourceIdentity;
use infiltrator_contract::rule_trace_run::{RuleTraceOperationId, RuleTraceRequest};
use infiltrator_contract::rule_tracer::TracerRuleOverride;
use infiltrator_contract::rules_workspace::RulesJsonSection;
use infiltrator_contract::script_export_review::{ScriptExportDraft, ScriptExportIdentity};
use infiltrator_contract::script_run::{ScriptOperationId, ScriptRunRequest};
use infiltrator_contract::subscription_import::{
    SubscriptionFilterDraft, SubscriptionImportChannel, SubscriptionScheduleDraft,
};
use infiltrator_contract::tun::TunStack;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// All user action commands emitted from Bevy UI pages and controls.
#[derive(Clone, Debug, PartialEq, Eq)]
#[rustfmt::skip]
pub enum UiCommand {
    /// Start the shared core lifecycle.
    StartCore,
    /// Stop the shared core lifecycle.
    StopCore,
    /// Restart the shared core lifecycle.
    RestartCore,
    /// Prepare the host-owned privileged service mode for TUN routing.
    PrepareServiceMode,
    /// Repair the host-owned mixed/controller port bindings safely.
    RepairPortConflicts,
    /// Change Mihomo's live core log verbosity.
    SetCoreLogLevel(CoreLogLevel),
    /// Change Mihomo's live TUN protocol stack.
    SetTunStack(TunStack),
    /// Toggle Mihomo automatic route installation.
    SetTunAutoRoute(bool),
    /// Toggle Mihomo strict route enforcement.
    SetTunStrictRoute(bool),
    /// Probe the physical link and negotiate the virtual TUN MTU.
    ProbeTunMtu,
    RefreshPublicIpProbe,
    ReorderOverviewCards { order: Vec<OverviewCardKind> },
    MoveOverviewCardUp(OverviewCardKind),
    MoveOverviewCardDown(OverviewCardKind),
    ResetOverviewCardOrder,
    /// Switch core proxy mode (Rule / Global / Direct).
    SetProxyMode(ProxyMode),
    /// Select a specific proxy node in a policy group.
    SelectProxyNode { group: String, node: String },
    /// Run latency benchmark across all proxy groups.
    TestAllProxyGroups,
    /// DUAL-06-03: run the all-groups benchmark with a custom target URL typed
    /// in the Overview field.
    TestAllProxyGroupsWithUrl { url: String },
    /// DUAL-06-01: set the shared speedtest engine's concurrency bound.
    SetSpeedtestConcurrency { limit: usize },
    /// Cancel the active shared speedtest batch.
    CancelSpeedtest,
    /// Run latency benchmark for a specific proxy group.
    TestProxyGroup { group: String },
    TestProxyNode { node: String },
    /// Toggle expand/fold of a proxy group card.
    ToggleProxyGroupExpand { group: String },
    /// Change the proxy node sorting order.
    SetProxySortOrder(ProxySortOrder),
    SetProxySearchQuery { query: String },
    SetProxyProbeOptions { options: ProxyProbeOptions },
    /// Toggle Filter Alive (只看可用) mode.
    ToggleFilterAlive(bool),
    /// Toggle favorite status of a proxy node.
    ToggleFavoriteProxy(String),
    /// Toggle compact list vs grid view for proxy nodes.
    SetProxyCompactView(bool),
    /// Reorder proxy groups by custom order.
    ReorderProxyGroups { group_names: Vec<String> },
    /// Reset proxy group custom ordering to default.
    ResetProxyGroupOrder,
    /// Activate a subscription configuration profile.
    ActivateProfile { id: String },
    /// Trigger an immediate remote update for a profile.
    UpdateProfile { id: String },
    /// DUAL-07-11: refresh every subscription profile now, ignoring schedules.
    UpdateAllSubscriptions,
    /// DUAL-07-13: restore a profile's transient pre-save backup copy.
    RestoreSubscriptionBackup { id: String },
    /// DUAL-07-09: persist the post-update core-reload preference. A host
    /// without a managed-runtime reload seam rejects `enabled = true` with a
    /// typed unsupported failure.
    SetSubscriptionAutoReload { profile_id: String, enabled: bool },
    /// DUAL-07-14: persist the subscription URL / auto-update / interval / cron
    /// through the shared schedule application.
    UpdateSubscriptionSchedule {
        profile_id: String,
        draft: SubscriptionScheduleDraft,
    },
    /// Persist a profile's subscription fetch options.
    SaveSubscriptionFetchSettings {
        profile_id: String,
        user_agent: Option<String>,
        insecure_skip_verify: bool,
    },
    /// DUAL-07-08: apply a profile's node-keyword filter through the shared
    /// pipeline.
    SaveSubscriptionFilter {
        source: ProfileSourceIdentity,
        filter: SubscriptionFilterDraft,
    },
    /// DUAL-07-01: import a profile through the shared multi-channel path.
    ImportSubscription {
        profile_id: String,
        channel: SubscriptionImportChannel,
        source: String,
    },
    /// Delete a subscription profile.
    DeleteProfile { id: String },
    /// DUAL-08-01/08-11: persist the aggregation draft and recompute the
    /// shared preview from the real source contents.
    PreviewProfileAggregation {
        draft: AggregationDraft,
    },
    /// DUAL-08-06: materialise the aggregation draft into a new profile.
    CreateAggregatedProfile {
        draft: AggregationDraft,
    },
    /// DUAL-08-13: upsert the draft as a reusable aggregation template.
    SaveAggregationTemplate {
        name: String,
        draft: AggregationDraft,
    },
    /// DUAL-08-13: delete a saved aggregation template.
    DeleteAggregationTemplate {
        name: String,
    },
    /// DUAL-08-07: refresh the profile a saved template produced.
    ReAggregateProfile {
        template_name: String,
    },
    /// DUAL-05-14: decode a share link through the shared protocol codec and
    /// publish the typed draft (cipher family / REALITY / smux) for both
    /// surfaces. Never writes a profile.
    ImportCustomNodeUri { uri: String },
    PrepareCustomNodeDraft { draft: Box<ProtocolDraft> },
    /// DUAL-05-14: commit the shared draft into the active profile with the
    /// lossless section splice.
    SaveCustomNodeDraft {
        draft: Box<ProtocolDraft>,
    },
    /// DUAL-05-09/13: edit one whitelisted field of the published shared draft
    /// (the dialer hop or a certificate-trust carrier). Unknown field names are
    /// refused by the shared application.
    UpdateCustomNodeDraftField { field: String, value: String },
    /// DUAL-05-09/10: analyse the active profile's dialer/relay graph and
    /// publish the resolved chains + loop findings.
    ScanCustomNodeDialer,
    /// DUAL-05-13: resolve a certificate-trust request against the host reader
    /// and publish the typed outcome.
    VerifyCustomNodeCa {
        trust: Box<TlsTrustParams>,
    },
    /// Trigger a remote update for all rule providers.
    RefreshRuleProviders,
    /// Reset every accumulated rule hit counter.
    ClearRuleHitCounters { expected_source: RuleSourceIdentity },
    /// DUAL-11-06: unpack one declared rule provider's real rules into the
    /// active profile's custom rule list.
    UnpackRuleProvider(String),
    /// DUAL-11-07: delete the kernel's cached rule-provider files.
    PurgeRuleProviderCache,
    /// DUAL-11-09: invert the enabled flag of one rule in the active profile.
    ToggleRuleEnabled(usize),
    CommitRuleList { request: RuleListCommit },
    /// DUAL-11-10: move one rule a single step up in the active profile.
    MoveRuleUp(usize),
    /// DUAL-11-10: move one rule a single step down in the active profile.
    MoveRuleDown(usize),
    /// DUAL-11-11: insert a wizard-built custom rule at the top of the list.
    AddCustomRule {
        rule_type: String,
        payload: String,
        target: String,
    },
    /// DUAL-11-12: prepend the built-in game-routing presets for a target.
    ApplyGameRoutingPresets { target: String },
    /// DUAL-11-14: trigger the kernel's GeoIP/GeoSite database upgrade.
    UpgradeGeoDatabases,
    /// DUAL-11-14: replace one rules-workspace JSON document.
    ApplyRulesJsonDocument { section: RulesJsonSection, json: String },
    /// DUAL-12-10: set the simulated sandbox source IP the tracer replays.

    /// Re-run the shared rule tracer for a target query.
    SimulateRuleTrace { operation: RuleTraceOperationId, request: RuleTraceRequest },
    /// DUAL-12-08: rewrite the traced rule's outbound and apply the config.
    ApplyTracerRuleOverride { request: TracerRuleOverride },
    /// Terminate a single active connection by ID.
    CloseConnection { id: String },
    /// Terminate all active connections.
    CloseAllConnections,
    /// Clear the in-memory log buffer.
    ClearLogs,
    RunScriptSandbox { request: ScriptRunRequest },
    ClearScriptSandbox { operation: ScriptOperationId },
    PrepareScriptExport { operation: u64, draft: ScriptExportDraft },
    SaveScriptExport { operation: u64, identity: ScriptExportIdentity },
    CancelScriptExport { operation: u64, identity: ScriptExportIdentity },
    PrepareLogExport { operation: u64 },
    SaveLogExport { operation: u64, identity: LogExportIdentity },
    CancelLogExport { operation: u64, identity: LogExportIdentity },
    /// Filter logs by severity level string.
    SetLogLevelFilter { level: Option<String> },
    /// Flush the DNS cache and Fake-IP table.
    ClearDnsCache { operation: DnsCacheOperationId },
    /// Test DNS server latency.
    QueryDns { operation: DnsQueryOperationId, request: DnsQueryRequest },
    TestDnsLatency,
    /// DUAL-14-08: run the shared DNS leak cross-source probe.
    TestDnsLeak,
    /// DUAL-14-09 (re-scoped): probe this host/process's UDP egress mapping
    /// through the configured STUN server.
    RunStunProbe,
    /// DUAL-14-01/02/03: apply the shared DNS workbench patch.
    ApplyDnsSettings {
        patch: DnsSettingsPatch,
    },
    /// Toggle the host-owned TUN/VPN capability.
    ToggleTun { enabled: bool },
    /// Ask the Android host to obtain VpnService consent and start foreground mode.
    StartVpn,
    /// Stop the Android VpnService and tun2proxy worker.
    StopVpn,
    /// Run the host-injected privileged network transaction and rollback test.
    RunPrivilegedNetworkRegression,
    /// Toggle the host-owned system proxy capability.
    SetSystemProxy { enabled: bool },
    /// Apply the live Mihomo LAN listener settings.
    SetLanSharing {
        enabled: bool,
        mixed_port: u16,
        bind_address: String,
    },
    /// Apply LAN CIDR ACLs and the in-memory Basic Authentication input.
    SetLanSecurity {
        allowed_ips: Vec<String>,
        disallowed_ips: Vec<String>,
        skip_auth_prefixes: Vec<String>,
        authentication_enabled: bool,
        credentials: Option<LanCredentials>,
    },
    /// Toggle Mihomo's top-level IPv6 routing policy.
    SetIpv6Routing { enabled: bool },
    /// Refresh Windows AppContainer loopback state.
    ScanUwpApps,
    /// Toggle one Windows AppContainer loopback exemption.
    SetUwpAppExemption { sid: String, exempt: bool },
    /// Apply or clear all Windows AppContainer loopback exemptions.
    SetAllUwpExemptions { exempt: bool },
    /// Generate and apply the shared PAC script/service request.
    ApplyPac {
        enabled: bool,
        bypass_domains: Vec<String>,
        bypass_lan: bool,
        minify: bool,
    },
    /// Refresh physical-link/default-gateway facts through the shared app.
    RefreshNetworkRoaming,
    /// Force a safe TUN route-anchor repair through the shared app.
    RepairNetworkRoutes,
    /// Run full system doctor diagnostics.
    RunDoctorDiagnostics,
    BootstrapDoctor,
    /// Repair a specific doctor issue by check ID.
    RepairDoctorIssue { check_id: String },
    /// Repair all detected doctor issues.
    RepairAllDoctorIssues,
    /// Toggle split tunneling rule for an application.
    ToggleAppRouting { app_id: String, enabled: bool },
    /// Set app routing split tunneling mode.
    SetAppRoutingMode { mode: String },
    /// Toggle include system apps in split tunneling.
    ToggleIncludeSystemApps { include: bool },
    /// Set specific app routing action rule.
    SetAppRule { app_id: String, rule: String },
    /// Immediate WebDAV sync action.
    SyncNow,
    /// Create backup snapshot.
    CreateBackupSnapshot,
    /// Resolve conflict by keeping local.
    ResolveConflictKeepLocal,
    /// Resolve conflict by taking remote.
    ResolveConflictTakeRemote,
    /// Restore a specific snapshot.
    SnapshotRestore { intent: CommandIntent },
    /// DUAL-09-08: compute the snapshot-vs-current diff through the shared
    /// snapshot application. `snapshot_id = None` diffs the newest snapshot.
    LoadSnapshotDiff { snapshot_id: Option<String> },
    /// DUAL-09-06/07: refresh the shared snapshot history (entries + prune view).
    LoadSnapshotHistory,
    /// DUAL-09-07: run the shared dedupe+LRU prune now.
    PruneSnapshots { keep: Option<usize> },
    /// DUAL-09-03/14: load the stored profile document for the editor card.
    LoadProfileDocument { profile: Option<String> },
    /// DUAL-09-14: commit the editor buffer through the shared guarded write.
    SaveProfileDocument {
        source: ProfileSourceIdentity,
        content: String,
        allow_protected: bool,
    },
    /// DUAL-09-14: load a profile's Mixin/filter sidecar for the editor panes.
    LoadProfileOptions { profile: Option<String> },
    /// DUAL-09-14: commit an edited Mixin overlay through the shared use-case.
    SaveMixinOverlay { source: ProfileSourceIdentity, mixin_yaml: String },
    /// Select the last installed, locally recorded core version.
    RollbackCore,
    /// Update a core or UI setting.
    SetLanguage { preference: LanguagePreference },
    UpdateSetting { key: String, value: String },
    /// Check for a new core release.
    CheckUpdates,
}

mod intent;

/// Abstract sink consuming typed UI commands.
pub trait UiCommandSink: Send + Sync {
    /// Submit a command for background processing.
    fn submit(&self, command: UiCommand);
    /// A host without exact terminal feedback refuses tracked editing operations.
    fn submit_tracked(&self, _command: UiCommand) -> Option<RequestId> {
        None
    }
    /// Terminal feedback; recording-only test sinks have no background executor.
    fn drain_results(&self) -> Vec<CommandExecutedEvent> {
        Vec::new()
    }
}

/// ECS resource handle wrapping a thread-safe UI command sink.
#[derive(Resource, Clone)]
pub struct CommandSinkHandle(pub Arc<dyn UiCommandSink>);

impl CommandSinkHandle {
    /// Submit a command through the sink.
    pub fn submit(&self, command: UiCommand) {
        self.0.submit(command);
    }
    pub fn submit_tracked(&self, command: UiCommand) -> Option<RequestId> {
        self.0.submit_tracked(command)
    }
}

/// Demo/in-memory command sink recording submitted commands for testing and mock runs.
#[derive(Clone, Debug, Default)]
pub struct DemoCommandSink {
    next_request: Arc<AtomicU64>,
    history: Arc<Mutex<Vec<UiCommand>>>,
}

impl DemoCommandSink {
    /// Create an active demo command sink.
    pub fn accepting() -> Self {
        Self {
            history: Arc::new(Mutex::new(Vec::new())),
            next_request: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Read all commands submitted so far.
    pub fn submitted(&self) -> Vec<UiCommand> {
        self.history.lock().expect("sink poisoned").clone()
    }

    /// Clear recorded command history.
    pub fn clear(&self) {
        self.history.lock().expect("sink poisoned").clear();
    }
}

impl UiCommandSink for DemoCommandSink {
    fn submit_tracked(&self, command: UiCommand) -> Option<RequestId> {
        let id = RequestId(self.next_request.fetch_add(1, Ordering::Relaxed) + 1);
        self.submit(command);
        Some(id)
    }
    fn submit(&self, command: UiCommand) {
        self.history.lock().expect("sink poisoned").push(command);
    }
}

/// Plugin installing the command sink handle into the Bevy App.
pub struct CommandPumpPlugin {
    sink: Arc<dyn UiCommandSink>,
}

impl CommandPumpPlugin {
    /// Create plugin with the given command sink implementation.
    pub fn new(sink: Arc<dyn UiCommandSink>) -> Self {
        Self { sink }
    }

    /// Create a production command pump backed by the shared application
    /// service. The application owns execution and runtime details; Bevy
    /// only emits the UI-local command vocabulary.
    pub fn for_application(application: Arc<CoreApplication>) -> Self {
        Self::new(Arc::new(ApplicationCommandSink::new(application)))
    }
}

impl Default for CommandPumpPlugin {
    fn default() -> Self {
        Self {
            sink: Arc::new(DemoCommandSink::accepting()),
        }
    }
}

impl Plugin for CommandPumpPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CommandSinkHandle(Arc::clone(&self.sink)));
        app.add_systems(Update, drain_command_results);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_command_sink_records_and_clears() {
        let sink = DemoCommandSink::accepting();
        assert!(sink.submitted().is_empty());

        sink.submit(UiCommand::ClearLogs);
        sink.submit(UiCommand::CloseAllConnections);

        let items = sink.submitted();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], UiCommand::ClearLogs);
        assert_eq!(items[1], UiCommand::CloseAllConnections);

        sink.clear();
        assert!(sink.submitted().is_empty());
    }

    #[test]
    fn business_commands_map_to_the_shared_contract() {
        assert_eq!(
            UiCommand::SetProxyMode(ProxyMode::Global).to_intent(),
            Some(CommandIntent::SetProxyMode {
                mode: ProxyMode::Global,
            })
        );
        assert_eq!(
            UiCommand::ToggleProxyGroupExpand {
                group: "auto".to_string(),
            }
            .to_intent(),
            Some(CommandIntent::ToggleProxyGroupExpand {
                group: "auto".to_string(),
            })
        );
        assert_eq!(
            UiCommand::SetProxySortOrder(ProxySortOrder::LatencyDesc).to_intent(),
            Some(CommandIntent::SetProxySortOrder {
                order: ProxySortOrder::LatencyDesc,
            })
        );
        assert_eq!(
            UiCommand::ToggleFilterAlive(true).to_intent(),
            Some(CommandIntent::ToggleFilterAlive { enabled: true })
        );
        assert_eq!(
            UiCommand::ToggleFavoriteProxy("HK-01".into()).to_intent(),
            Some(CommandIntent::ToggleFavoriteProxy {
                proxy: "HK-01".into()
            })
        );
        assert_eq!(
            UiCommand::RollbackCore.to_intent(),
            Some(CommandIntent::RollbackCore)
        );
        assert_eq!(
            UiCommand::SetCoreLogLevel(CoreLogLevel::Debug).to_intent(),
            Some(CommandIntent::SetCoreLogLevel {
                level: CoreLogLevel::Debug,
            })
        );
        assert_eq!(
            UiCommand::SetTunStack(TunStack::Mixed).to_intent(),
            Some(CommandIntent::SetTunStack {
                stack: TunStack::Mixed,
            })
        );
        assert_eq!(
            UiCommand::ProbeTunMtu.to_intent(),
            Some(CommandIntent::ProbeTunMtu)
        );
        assert_eq!(
            UiCommand::RefreshPublicIpProbe.to_intent(),
            Some(CommandIntent::RefreshPublicIpProbe)
        );
        assert_eq!(
            UiCommand::ResetOverviewCardOrder.to_intent(),
            Some(CommandIntent::ResetOverviewCardOrder)
        );
        assert_eq!(
            UiCommand::SetTunAutoRoute(false).to_intent(),
            Some(CommandIntent::SetTunAutoRoute { enabled: false })
        );
        assert_eq!(
            UiCommand::SetTunStrictRoute(true).to_intent(),
            Some(CommandIntent::SetTunStrictRoute { enabled: true })
        );
        assert_eq!(
            UiCommand::SetSystemProxy { enabled: true }.to_intent(),
            Some(CommandIntent::SetSystemProxy { enabled: true })
        );
        assert_eq!(
            UiCommand::SetLanSharing {
                enabled: true,
                mixed_port: 8080,
                bind_address: "192.168.1.10".to_owned(),
            }
            .to_intent(),
            Some(CommandIntent::SetLanSharing {
                enabled: true,
                mixed_port: 8080,
                bind_address: "192.168.1.10".to_owned(),
            })
        );
        assert_eq!(
            UiCommand::SetLanSecurity {
                allowed_ips: vec!["192.168.1.0/24".to_owned()],
                disallowed_ips: vec!["192.168.1.10/32".to_owned()],
                skip_auth_prefixes: vec!["127.0.0.0/8".to_owned()],
                authentication_enabled: true,
                credentials: Some(LanCredentials {
                    username: "lan-user".to_owned(),
                    password: "secret-value".to_owned(),
                }),
            }
            .to_intent(),
            Some(CommandIntent::SetLanSecurity {
                allowed_ips: vec!["192.168.1.0/24".to_owned()],
                disallowed_ips: vec!["192.168.1.10/32".to_owned()],
                skip_auth_prefixes: vec!["127.0.0.0/8".to_owned()],
                authentication_enabled: true,
                credentials: Some(LanCredentials {
                    username: "lan-user".to_owned(),
                    password: "secret-value".to_owned(),
                }),
            })
        );
        assert_eq!(
            UiCommand::SetIpv6Routing { enabled: false }.to_intent(),
            Some(CommandIntent::SetIpv6Routing { enabled: false })
        );
        assert_eq!(
            UiCommand::ScanUwpApps.to_intent(),
            Some(CommandIntent::ScanUwpApps)
        );
        assert_eq!(
            UiCommand::SetAllUwpExemptions { exempt: true }.to_intent(),
            Some(CommandIntent::SetAllUwpExemptions { exempt: true })
        );
        assert_eq!(
            UiCommand::ApplyPac {
                enabled: true,
                bypass_domains: vec!["example.com".to_owned()],
                bypass_lan: true,
                minify: false,
            }
            .to_intent(),
            Some(CommandIntent::ApplyPac {
                enabled: true,
                bypass_domains: vec!["example.com".to_owned()],
                bypass_lan: true,
                minify: false,
            })
        );
    }

    #[test]
    fn speedtest_commands_carry_concurrency_and_custom_url() {
        // DUAL-06-01: the runtime concurrency bound maps to the shared intent.
        assert_eq!(
            UiCommand::SetSpeedtestConcurrency { limit: 12 }.to_intent(),
            Some(CommandIntent::SetSpeedtestConcurrency { limit: 12 })
        );
        // DUAL-06-03: a typed target URL rides into the shared delay intent.
        assert_eq!(
            UiCommand::TestAllProxyGroupsWithUrl {
                url: "https://cp.cloudflare.com/generate_204".to_owned(),
            }
            .to_intent(),
            Some(CommandIntent::TestDelay {
                group: None,
                url: Some("https://cp.cloudflare.com/generate_204".to_owned()),
                timeout_ms: None,
            })
        );
        // The plain all-groups command keeps the engine's default target.
        assert_eq!(
            UiCommand::TestAllProxyGroups.to_intent(),
            Some(CommandIntent::TestDelay {
                group: None,
                url: None,
                timeout_ms: None,
            })
        );
    }

    #[test]
    fn lifecycle_commands_share_the_core_application_intents() {
        assert_eq!(
            UiCommand::StartCore.to_intent(),
            Some(CommandIntent::StartCore)
        );
        assert_eq!(
            UiCommand::StopCore.to_intent(),
            Some(CommandIntent::StopCore)
        );
        assert_eq!(
            UiCommand::RestartCore.to_intent(),
            Some(CommandIntent::RestartCore)
        );
    }
}

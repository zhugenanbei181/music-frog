//! Strongly typed UI Command Bus and EntityObserver/Trigger Infrastructure.
//!
//! Charter law (crates/infiltrator-bevy-ui/docs/BEVY_UI_FRONTEND.md):
//! All UI user interactions (button clicks, switches, routing-intent selections,
//! reconnects, clears) dispatch through typed commands into a centralized
//! command sink handle. No direct blocking calls in UI systems.

use crate::command_events::CommandExecutedEvent;
use crate::command_execution::drain_command_results;
use bevy::app::{App, Plugin, Update};
use bevy::ecs::resource::Resource;
use infiltrator_contract::aggregator::AggregationDraft;
use infiltrator_contract::app_routing::{
    AppRoutingMode, AppRoutingPackageSelection, AppRoutingRule,
};
use infiltrator_contract::command::{CommandIntent, CoreLogLevel, RequestId};
#[cfg(any(feature = "capture-tools", test))]
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::dns::DnsSettingsPatch;
use infiltrator_contract::dns_cache::DnsCacheOperationId;
use infiltrator_contract::dns_query::{DnsQueryOperationId, DnsQueryRequest};
use infiltrator_contract::error::Failure;
use infiltrator_contract::lan::LanCredentials;
use infiltrator_contract::language::LanguagePreference;
use infiltrator_contract::log_export::LogExportIdentity;
use infiltrator_contract::outbound::{
    preferences::OutboundUiPreferenceRequest,
    probe::{OutboundSelectionRequest, ProbeOutboundBatchRequest, ProbeOutboundRequest},
};
use infiltrator_contract::overview_layout::OverviewCardKind;
use infiltrator_contract::profile_configuration::ProfileConfigurationRequest;
use infiltrator_contract::profile_document::DocumentInspection;
#[cfg(any(feature = "capture-tools", test))]
use infiltrator_contract::profile_document::DocumentValidation;
use infiltrator_contract::profile_document_identity::ProfileDocumentIdentity;
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use infiltrator_contract::profile_protection::ProfileWriteAuthorization;
use infiltrator_contract::protocol_fidelity::ProtocolDraft;
use infiltrator_contract::protocol_trust::TlsTrustParams;
use infiltrator_contract::routing_intent::RoutingIntent;
use infiltrator_contract::rule_document::RuleListCommit;
use infiltrator_contract::rule_source::RuleSourceIdentity;
use infiltrator_contract::rule_trace_run::{RuleTraceOperationId, RuleTraceRequest};
use infiltrator_contract::rule_tracer::TracerRuleOverride;
use infiltrator_contract::runtime_settings::RuntimeProbeSettings;
use infiltrator_contract::script_export_review::{ScriptExportDraft, ScriptExportIdentity};
use infiltrator_contract::script_run::{ScriptOperationId, ScriptRunRequest};
use infiltrator_contract::speedtest::{SpeedtestConcurrency, SpeedtestTarget};
use infiltrator_contract::subscription_import::{
    SubscriptionFilterDraft, SubscriptionImportChannel, SubscriptionScheduleDraft,
};
use infiltrator_contract::tun::TunStack;
use infiltrator_ports::profile_document_codec::ProfileDocumentSnippetOutcome;
#[cfg(any(test, feature = "capture-tools"))]
use infiltrator_ports::profile_document_codec::{
    ProfileMixinCascade, ProfileMixinCascadeStage, ProfileMixinCascadeStageId, ProfileMixinColumn,
    ProfileMixinColumnRole, ProfileMixinComposition, ProfileMixinPreflight,
    ProfileMixinStageDeclaration, ProfileMixinToggle, ProfileMixinToggleProjection,
};
use infiltrator_ports::profile_document_codec::{
    ProfileMixinStudioProjection, ProfileMixinToggleId,
};
#[cfg(any(feature = "capture-tools", test))]
use std::collections::VecDeque;
#[cfg(any(feature = "capture-tools", test))]
use std::mem::take;
use std::sync::Arc;
#[cfg(any(feature = "capture-tools", test))]
use std::sync::Mutex;
#[cfg(any(feature = "capture-tools", test))]
use std::sync::atomic::{AtomicU64, Ordering};

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
    /// Choose a shared traffic-routing policy.
    SetRoutingIntent(RoutingIntent),
    /// Select a member from a neutral outbound collection, fenced to its
    /// observed inventory revision.
    SelectOutbound { request: OutboundSelectionRequest },
    /// Probe one outbound using its opaque identity and observed inventory revision.
    ProbeOutbound { request: ProbeOutboundRequest },
    /// Probe an observed neutral outbound set at one exact inventory revision.
    ProbeOutboundBatch { request: ProbeOutboundBatchRequest },
    /// Apply an outbound presentation preference using its observed revision.
    ApplyOutboundUiPreference { request: OutboundUiPreferenceRequest },
    /// Set the Core-owned speedtest runner's bounded concurrency value.
    StartSpeedtest { target: SpeedtestTarget },
    SetSpeedtestConcurrency { concurrency: SpeedtestConcurrency },
    /// Cancel exactly the run shown in the Core-owned speedtest snapshot.
    CancelSpeedtestRun { run_id: infiltrator_contract::speedtest::SpeedtestRunId },
    SetRuntimeProbeSettings { settings: RuntimeProbeSettings },
    /// Activate a subscription configuration profile.
    ActivateProfile { id: String },
    /// Trigger an immediate remote update for a profile.
    UpdateProfile { id: String },
    /// DUAL-07-11: refresh every subscription profile now, ignoring schedules.
    UpdateAllSubscriptions,
    /// DUAL-07-13: restore a profile's transient pre-save backup copy.
    RestoreSubscriptionBackup { id: String },
    /// DUAL-07-09: persist the post-update core-reload preference. A host
    /// without a Core-owner profile port rejects `enabled = true` with a
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
        source: ProfileWorkspaceIdentity,
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
    /// Apply a typed profile configuration update bound to its observed revision.
    ApplyProfileConfiguration(Box<ProfileConfigurationRequest>),
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
    /// Set one application's split-tunneling selection explicitly.
    SetAppRoutingPackageSelection {
        app_id: String,
        selection: AppRoutingPackageSelection,
    },
    /// Set app routing split tunneling mode.
    SetAppRoutingMode { mode: AppRoutingMode },
    /// Set specific app routing action rule.
    SetAppRule { app_id: String, rule: AppRoutingRule },
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
        source: ProfileWorkspaceIdentity,
        content: String,
        authorization: ProfileWriteAuthorization,
    },
    /// DUAL-09-14: load a profile's Mixin/filter sidecar for the editor panes.
    LoadProfileOptions { profile: Option<String> },
    /// DUAL-09-14: commit an edited Mixin overlay through the shared use-case.
    SaveMixinOverlay { source: ProfileWorkspaceIdentity, mixin_yaml: String },
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
    /// Pure, identity-scoped editor codec operations. These synchronous
    /// queries do not enter the Core command pump or mutate storage/runtime.
    fn inspect_profile_buffer(
        &self,
        _identity: &ProfileDocumentIdentity,
        _content: &str,
    ) -> Result<DocumentInspection, Failure> {
        Err(Failure::unsupported(
            "profile document inspection is not composed",
        ))
    }
    fn insert_profile_snippet(
        &self,
        _identity: &ProfileDocumentIdentity,
        _content: &str,
        _snippet_id: &str,
        _line: usize,
        _column: usize,
    ) -> Result<ProfileDocumentSnippetOutcome, Failure> {
        Err(Failure::unsupported(
            "profile document snippet insertion is not composed",
        ))
    }
    fn inspect_mixin_draft(
        &self,
        _identity: &ProfileDocumentIdentity,
        _document_content: &str,
        _mixin_yaml: &str,
    ) -> Result<(), Failure> {
        Err(Failure::unsupported(
            "profile Mixin draft inspection is not composed",
        ))
    }
    fn inspect_mixin_toggle(
        &self,
        _identity: &ProfileDocumentIdentity,
        _mixin_yaml: &str,
        _toggle_id: ProfileMixinToggleId,
    ) -> Result<bool, Failure> {
        Err(Failure::unsupported(
            "Mixin toggle inspection is not composed",
        ))
    }
    fn set_mixin_toggle(
        &self,
        _identity: &ProfileDocumentIdentity,
        _mixin_yaml: &str,
        _toggle_id: ProfileMixinToggleId,
        _enabled: bool,
    ) -> Result<String, Failure> {
        Err(Failure::unsupported("Mixin toggle editing is not composed"))
    }
    fn inspect_mixin_studio(
        &self,
        _identity: &ProfileDocumentIdentity,
        _document_content: &str,
        _mixin_yaml: &str,
    ) -> Result<ProfileMixinStudioProjection, Failure> {
        Err(Failure::unsupported(
            "Mixin studio projection is not composed",
        ))
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
    pub fn inspect_profile_buffer(
        &self,
        identity: &ProfileDocumentIdentity,
        content: &str,
    ) -> Result<DocumentInspection, Failure> {
        self.0.inspect_profile_buffer(identity, content)
    }
    pub fn insert_profile_snippet(
        &self,
        identity: &ProfileDocumentIdentity,
        content: &str,
        snippet_id: &str,
        line: usize,
        column: usize,
    ) -> Result<ProfileDocumentSnippetOutcome, Failure> {
        self.0
            .insert_profile_snippet(identity, content, snippet_id, line, column)
    }
    pub fn inspect_mixin_draft(
        &self,
        identity: &ProfileDocumentIdentity,
        document_content: &str,
        mixin_yaml: &str,
    ) -> Result<(), Failure> {
        self.0
            .inspect_mixin_draft(identity, document_content, mixin_yaml)
    }
    pub fn inspect_mixin_toggle(
        &self,
        identity: &ProfileDocumentIdentity,
        mixin_yaml: &str,
        toggle_id: ProfileMixinToggleId,
    ) -> Result<bool, Failure> {
        self.0.inspect_mixin_toggle(identity, mixin_yaml, toggle_id)
    }
    pub fn set_mixin_toggle(
        &self,
        identity: &ProfileDocumentIdentity,
        mixin_yaml: &str,
        toggle_id: ProfileMixinToggleId,
        enabled: bool,
    ) -> Result<String, Failure> {
        self.0
            .set_mixin_toggle(identity, mixin_yaml, toggle_id, enabled)
    }
    pub fn inspect_mixin_studio(
        &self,
        identity: &ProfileDocumentIdentity,
        document_content: &str,
        mixin_yaml: &str,
    ) -> Result<ProfileMixinStudioProjection, Failure> {
        self.0
            .inspect_mixin_studio(identity, document_content, mixin_yaml)
    }
}

/// Explicit demo/headless command sink recording commands and returning a
/// typed routing-intent readback. Product composition injects its owner sink.
#[cfg(any(feature = "capture-tools", test))]
#[derive(Clone, Debug, Default)]
pub struct DemoCommandSink {
    next_request: Arc<AtomicU64>,
    history: Arc<Mutex<Vec<UiCommand>>>,
    results: Arc<Mutex<Vec<CommandExecutedEvent>>>,
    editor_inspections: Arc<Mutex<VecDeque<Result<DocumentInspection, Failure>>>>,
    editor_snippets: Arc<Mutex<VecDeque<Result<ProfileDocumentSnippetOutcome, Failure>>>>,
    mixin_preflights: Arc<Mutex<VecDeque<Result<(), Failure>>>>,
    mixin_studios: Arc<Mutex<VecDeque<Result<ProfileMixinStudioProjection, Failure>>>>,
    mixin_toggle_inspections: Arc<Mutex<VecDeque<Result<bool, Failure>>>>,
    mixin_toggle_updates: Arc<Mutex<VecDeque<Result<String, Failure>>>>,
}

#[cfg(any(feature = "capture-tools", test))]
impl DemoCommandSink {
    /// Create an active demo command sink.
    pub fn accepting() -> Self {
        Self {
            history: Arc::new(Mutex::new(Vec::new())),
            next_request: Arc::new(AtomicU64::new(0)),
            results: Arc::new(Mutex::new(Vec::new())),
            editor_inspections: Arc::new(Mutex::new(VecDeque::new())),
            editor_snippets: Arc::new(Mutex::new(VecDeque::new())),
            mixin_preflights: Arc::new(Mutex::new(VecDeque::new())),
            mixin_studios: Arc::new(Mutex::new(VecDeque::new())),
            mixin_toggle_inspections: Arc::new(Mutex::new(VecDeque::new())),
            mixin_toggle_updates: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Queue a deterministic codec inspection for the next buffer check.
    pub fn queue_profile_inspection(&self, result: Result<DocumentInspection, Failure>) {
        self.editor_inspections
            .lock()
            .expect("demo editor inspection queue")
            .push_back(result);
    }

    /// Queue a deterministic codec result for the next snippet insertion.
    pub fn queue_profile_snippet(&self, result: Result<ProfileDocumentSnippetOutcome, Failure>) {
        self.editor_snippets
            .lock()
            .expect("demo editor snippet queue")
            .push_back(result);
    }

    /// Queue a deterministic result for the next Mixin codec preflight.
    pub fn queue_mixin_preflight(&self, result: Result<(), Failure>) {
        self.mixin_preflights
            .lock()
            .expect("demo Mixin preflight queue")
            .push_back(result);
    }

    /// Queue the next engine-neutral Mixin studio projection for this fake host.
    pub fn queue_mixin_studio(&self, result: Result<ProfileMixinStudioProjection, Failure>) {
        self.mixin_studios
            .lock()
            .expect("demo Mixin studio queue")
            .push_back(result);
    }

    /// Queue the next typed toggle inspection for this fake host.
    pub fn queue_mixin_toggle_inspection(&self, result: Result<bool, Failure>) {
        self.mixin_toggle_inspections
            .lock()
            .expect("demo Mixin toggle inspection queue")
            .push_back(result);
    }

    /// Queue the next typed toggle update for this fake host.
    pub fn queue_mixin_toggle_update(&self, result: Result<String, Failure>) {
        self.mixin_toggle_updates
            .lock()
            .expect("demo Mixin toggle update queue")
            .push_back(result);
    }

    /// Read all commands submitted so far.
    pub fn submitted(&self) -> Vec<UiCommand> {
        self.history.lock().expect("sink poisoned").clone()
    }

    /// Clear recorded command history and queued demo results.
    pub fn clear(&self) {
        self.history.lock().expect("sink poisoned").clear();
        self.results.lock().expect("sink poisoned").clear();
    }
}

#[cfg(any(feature = "capture-tools", test))]
impl UiCommandSink for DemoCommandSink {
    fn submit_tracked(&self, command: UiCommand) -> Option<RequestId> {
        let id = RequestId(self.next_request.fetch_add(1, Ordering::Relaxed) + 1);
        self.submit(command.clone());
        if let UiCommand::SetRoutingIntent(intent) = command.clone() {
            self.results
                .lock()
                .expect("sink poisoned")
                .push(CommandExecutedEvent {
                    command,
                    request_id: id,
                    result: Ok(CommandOutput::RoutingIntentApplied { observed: intent }),
                });
        }
        Some(id)
    }
    fn submit(&self, command: UiCommand) {
        self.history.lock().expect("sink poisoned").push(command);
    }
    fn drain_results(&self) -> Vec<CommandExecutedEvent> {
        take(&mut *self.results.lock().expect("sink poisoned"))
    }
    fn inspect_profile_buffer(
        &self,
        _identity: &ProfileDocumentIdentity,
        _content: &str,
    ) -> Result<DocumentInspection, Failure> {
        self.editor_inspections
            .lock()
            .expect("demo editor inspection queue")
            .pop_front()
            .unwrap_or(Ok(DocumentInspection::Inspected(DocumentValidation::Valid)))
    }
    fn insert_profile_snippet(
        &self,
        _identity: &ProfileDocumentIdentity,
        _content: &str,
        _snippet_id: &str,
        _line: usize,
        _column: usize,
    ) -> Result<ProfileDocumentSnippetOutcome, Failure> {
        self.editor_snippets
            .lock()
            .expect("demo editor snippet queue")
            .pop_front()
            .unwrap_or_else(|| {
                Err(Failure::unsupported(
                    "profile snippet fixture is not queued",
                ))
            })
    }
    fn inspect_mixin_draft(
        &self,
        _identity: &ProfileDocumentIdentity,
        _document_content: &str,
        _mixin_yaml: &str,
    ) -> Result<(), Failure> {
        self.mixin_preflights
            .lock()
            .expect("demo Mixin preflight queue")
            .pop_front()
            .unwrap_or(Ok(()))
    }
    fn inspect_mixin_toggle(
        &self,
        _identity: &ProfileDocumentIdentity,
        _mixin_yaml: &str,
        _toggle_id: ProfileMixinToggleId,
    ) -> Result<bool, Failure> {
        self.mixin_toggle_inspections
            .lock()
            .expect("demo Mixin toggle inspection queue")
            .pop_front()
            .unwrap_or(Ok(false))
    }
    fn set_mixin_toggle(
        &self,
        _identity: &ProfileDocumentIdentity,
        mixin_yaml: &str,
        toggle_id: ProfileMixinToggleId,
        enabled: bool,
    ) -> Result<String, Failure> {
        self.mixin_toggle_updates
            .lock()
            .expect("demo Mixin toggle update queue")
            .pop_front()
            .unwrap_or_else(|| Ok(format!("{mixin_yaml}{}: {enabled}\n", toggle_id.as_str())))
    }
    fn inspect_mixin_studio(
        &self,
        _identity: &ProfileDocumentIdentity,
        document_content: &str,
        mixin_yaml: &str,
    ) -> Result<ProfileMixinStudioProjection, Failure> {
        self.mixin_studios
            .lock()
            .expect("demo Mixin studio queue")
            .pop_front()
            .unwrap_or_else(|| Ok(demo_mixin_studio_projection(document_content, mixin_yaml)))
    }
}

#[cfg(any(feature = "capture-tools", test))]
fn demo_mixin_studio_projection(
    document_content: &str,
    mixin_yaml: &str,
) -> ProfileMixinStudioProjection {
    let base_lines = document_content.lines().count();
    let overlay_lines = mixin_yaml.lines().count();
    let composed = format!("{document_content}{mixin_yaml}");
    let composed_lines = composed.lines().count();
    ProfileMixinStudioProjection {
        preflight: ProfileMixinPreflight::Accepted,
        cascade: ProfileMixinCascade::Merged {
            stages: vec![
                ProfileMixinCascadeStage {
                    id: ProfileMixinCascadeStageId::Base,
                    declaration: ProfileMixinStageDeclaration::Applied,
                    line_count: base_lines,
                },
                ProfileMixinCascadeStage {
                    id: ProfileMixinCascadeStageId::Subscription,
                    declaration: ProfileMixinStageDeclaration::Undeclared,
                    line_count: 0,
                },
                ProfileMixinCascadeStage {
                    id: ProfileMixinCascadeStageId::Merge,
                    declaration: ProfileMixinStageDeclaration::Applied,
                    line_count: composed_lines,
                },
                ProfileMixinCascadeStage {
                    id: ProfileMixinCascadeStageId::PreMixin,
                    declaration: ProfileMixinStageDeclaration::Undeclared,
                    line_count: 0,
                },
                ProfileMixinCascadeStage {
                    id: ProfileMixinCascadeStageId::PostMixin,
                    declaration: ProfileMixinStageDeclaration::Undeclared,
                    line_count: 0,
                },
            ],
            line_count: composed_lines,
        },
        columns: infiltrator_ports::profile_document_codec::ProfileMixinColumns {
            base: ProfileMixinColumn {
                role: ProfileMixinColumnRole::Base,
                content: document_content.into(),
                editable: false,
                line_count: base_lines,
            },
            overlay: ProfileMixinColumn {
                role: ProfileMixinColumnRole::Overlay,
                content: mixin_yaml.into(),
                editable: true,
                line_count: overlay_lines,
            },
            composed: ProfileMixinComposition::Produced(ProfileMixinColumn {
                role: ProfileMixinColumnRole::Composed,
                content: composed,
                editable: false,
                line_count: composed_lines,
            }),
        },
        toggles: ProfileMixinToggleProjection::Available(
            ProfileMixinToggleId::ALL
                .into_iter()
                .map(|id| ProfileMixinToggle { id, enabled: false })
                .collect(),
        ),
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
    use infiltrator_contract::android_control::AndroidControlIntent;
    use infiltrator_contract::core_identity::EngineId;
    use infiltrator_contract::outbound::identity::OutboundId;
    use infiltrator_contract::outbound::identity::SnapshotRevision;
    use infiltrator_contract::outbound::preferences::OutboundSortOrder;
    use infiltrator_contract::outbound::preferences::OutboundUiPreferenceIntent;
    use infiltrator_contract::outbound::preferences::OutboundUiPreferenceRequest;
    use infiltrator_contract::outbound::probe::OutboundProbeBatchScope;
    use infiltrator_contract::outbound::probe::OutboundProbeConcurrency;
    use infiltrator_contract::outbound::probe::ProbeOutboundBatchRequest;
    use infiltrator_contract::outbound::probe::ProbeOutboundRequest;

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
        let outbound_request = OutboundSelectionRequest::new(
            OutboundId::new(EngineId::new("mihomo").unwrap(), "GLOBAL").unwrap(),
            OutboundId::new(EngineId::new("mihomo").unwrap(), "HK-01").unwrap(),
            SnapshotRevision::new(9),
        )
        .unwrap();
        assert_eq!(
            UiCommand::SelectOutbound {
                request: outbound_request.clone(),
            }
            .to_intent(),
            Some(CommandIntent::SelectOutbound {
                request: outbound_request,
            })
        );
        assert_eq!(
            UiCommand::SetRoutingIntent(RoutingIntent::RouteAllThroughProxy).to_intent(),
            Some(CommandIntent::SetRoutingIntent {
                intent: RoutingIntent::RouteAllThroughProxy,
            })
        );
        assert!(
            UiCommand::ApplyOutboundUiPreference {
                request: OutboundUiPreferenceRequest::new(
                    SnapshotRevision::new(9),
                    OutboundUiPreferenceIntent::SetSortOrder(OutboundSortOrder::LatencyDescending,),
                )
                .unwrap(),
            }
            .to_intent()
            .is_some()
        );
        let probe = ProbeOutboundRequest::new(
            OutboundId::new(EngineId::new("mihomo").unwrap(), "HK-01").unwrap(),
            SnapshotRevision::new(9),
            "https://example.test/204",
            5000,
        )
        .unwrap();
        assert_eq!(
            UiCommand::ProbeOutbound {
                request: probe.clone(),
            }
            .to_intent(),
            Some(CommandIntent::ProbeOutbound { request: probe })
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
    fn speedtest_commands_carry_validated_concurrency_and_typed_batch_request() {
        let concurrency = SpeedtestConcurrency::new(12).expect("bounded concurrency");
        assert_eq!(
            UiCommand::SetSpeedtestConcurrency { concurrency }.to_intent(),
            Some(CommandIntent::SetSpeedtestConcurrency { concurrency })
        );
        let run_id = infiltrator_contract::speedtest::SpeedtestRunId::new(7)
            .expect("valid speedtest run ID");
        assert_eq!(
            UiCommand::CancelSpeedtestRun { run_id }.to_intent(),
            Some(CommandIntent::CancelSpeedtestRun { run_id })
        );
        // The request is built from accepted probe settings, neutral scope,
        // and the observed inventory revision.
        let batch_request = ProbeOutboundBatchRequest::new(
            OutboundProbeBatchScope::All,
            SnapshotRevision::new(9),
            "https://cp.cloudflare.com/generate_204",
            5000,
            OutboundProbeConcurrency::new(4).unwrap(),
        )
        .unwrap();
        assert_eq!(
            UiCommand::ProbeOutboundBatch {
                request: batch_request.clone(),
            }
            .to_intent(),
            Some(CommandIntent::ProbeOutboundBatch {
                request: batch_request,
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
        assert_eq!(
            UiCommand::StartCore.android_control_intent(),
            Some(AndroidControlIntent::ResumeKernel)
        );
        assert_eq!(
            UiCommand::StopCore.android_control_intent(),
            Some(AndroidControlIntent::PauseKernel)
        );
        assert_eq!(
            UiCommand::StartVpn.android_control_intent(),
            Some(AndroidControlIntent::StartProxy)
        );
        assert_eq!(UiCommand::StopVpn.android_control_intent(), None);
        assert_eq!(UiCommand::StopVpn.to_intent(), Some(CommandIntent::StopVpn));
    }
}

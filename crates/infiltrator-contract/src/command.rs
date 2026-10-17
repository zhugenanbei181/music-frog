use crate::aggregator::AggregationDraft;
use crate::app_routing::{AppRoutingMode, AppRoutingPackageSelection, AppRoutingRule};
use crate::command_output::CommandOutput;
use crate::dns::DnsSettingsPatch;
use crate::dns_cache::DnsCacheOperationId;
use crate::dns_query::{DnsQueryOperationId, DnsQueryRequest};
use crate::lan::LanCredentials;
use crate::language::LanguagePreference;
use crate::log_export::LogExportIdentity;
use crate::overview_layout::OverviewCardKind;
use crate::profile_configuration::ProfileConfigurationRequest;
use crate::protocol_fidelity::ProtocolDraft;
use crate::protocol_trust::TlsTrustParams;
use crate::rule_document::RuleListCommit;
use crate::rule_edit::RuleMoveDirection;
use crate::rule_provider_snapshot::RuleProviderControllerReference;
use crate::rule_source::RuleSourceIdentity;
use crate::rule_trace_run::{RuleTraceOperationId, RuleTraceRequest};
use crate::rule_tracer::TracerRuleOverride;
use crate::runtime_settings::RuntimeProbeSettings;
use crate::script_export_review::{ScriptExportDraft, ScriptExportIdentity};
use crate::script_run::{ScriptOperationId, ScriptRunRequest};
use crate::snapshot_restore::{SnapshotRestoreIdentity, SnapshotRestoreTarget};
use crate::speedtest::{SpeedtestConcurrency, SpeedtestRunId, SpeedtestTarget};
use crate::subscription_import::{
    SubscriptionFilterDraft, SubscriptionImportChannel, SubscriptionScheduleDraft,
};
use crate::tun::TunStack;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::outbound::preferences::OutboundUiPreferenceRequest;
use lurker_core_contract::outbound::probe::{
    OutboundSelectionRequest, ProbeOutboundBatchRequest, ProbeOutboundRequest,
};
use lurker_core_contract::profile_document_identity::ProfileWorkspaceIdentity;
use lurker_core_contract::profile_protection::ProfileWriteAuthorization;
use lurker_core_contract::routing_intent::RoutingIntent;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Correlates an asynchronous command with its result and events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestId(pub u64);

impl RequestId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

/// Shared canonical keys for live core log verbosity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreLogLevel {
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl CoreLogLevel {
    pub const ALL: [Self; 4] = [Self::Debug, Self::Info, Self::Warn, Self::Error];

    pub const fn as_key(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }

    pub fn from_key(raw: &str) -> Option<Self> {
        match raw.trim() {
            "debug" => Some(Self::Debug),
            "info" => Some(Self::Info),
            "warn" => Some(Self::Warn),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

impl fmt::Display for CoreLogLevel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_key())
    }
}

/// A shared user intention. UI-local actions such as opening a drawer or
/// changing a Bevy scene do not belong here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandIntent {
    RunScriptSandbox {
        request: ScriptRunRequest,
    },
    ClearScriptSandbox {
        operation: ScriptOperationId,
    },
    PrepareScriptExport {
        draft: ScriptExportDraft,
    },
    SaveScriptExport {
        identity: ScriptExportIdentity,
    },
    CancelScriptExport {
        identity: ScriptExportIdentity,
    },
    StartCore,
    StopCore,
    RestartCore,
    StartProxy,
    StopProxy,
    ResumeKernel,
    PauseKernel,
    RestartKernel,
    PrepareServiceMode,
    RepairPortConflicts,
    SetCoreLogLevel {
        level: CoreLogLevel,
    },
    SetTunStack {
        stack: TunStack,
    },
    SwitchProfile {
        profile_id: String,
    },
    SetRoutingIntent {
        intent: RoutingIntent,
    },
    GetRoutingIntent,
    ReadOutboundInventory,
    SelectOutbound {
        request: OutboundSelectionRequest,
    },
    ProbeOutbound {
        request: ProbeOutboundRequest,
    },
    ProbeOutboundBatch {
        request: ProbeOutboundBatchRequest,
    },
    ApplyOutboundUiPreference {
        request: OutboundUiPreferenceRequest,
    },
    StartSpeedtest {
        target: SpeedtestTarget,
    },
    SetSpeedtestConcurrency {
        concurrency: SpeedtestConcurrency,
    },
    CancelSpeedtestRun {
        run_id: SpeedtestRunId,
    },
    SetRuntimeProbeSettings {
        settings: RuntimeProbeSettings,
    },
    ReorderOverviewCards {
        order: Vec<OverviewCardKind>,
    },
    ResetOverviewCardOrder,
    UpdateProfile {
        profile_id: String,
    },
    /// DUAL-07-11: refresh every subscription profile now, ignoring its
    /// schedule, through the shared conditional batch path.
    UpdateAllSubscriptions,
    /// DUAL-07-13: restore a profile's transient pre-save backup copy.
    RestoreSubscriptionBackup {
        profile_id: String,
    },
    /// Persist a profile's subscription fetch options: custom User-Agent and
    /// the insecure-TLS preference used by the conditional update.
    UpdateSubscriptionFetchSettings {
        profile_id: String,
        user_agent: Option<String>,
        insecure_skip_verify: bool,
    },
    /// DUAL-07-09: persist whether the active profile is reloaded into the
    /// running core after a successful subscription update. Hosts without a
    /// Core-owner profile port reject `enabled = true` with a typed
    /// `Unsupported` failure instead of storing a preference that no-ops.
    SetSubscriptionAutoReload {
        profile_id: String,
        enabled: bool,
    },
    /// DUAL-07-14: persist a profile's subscription URL, auto-update flag,
    /// interval, and cron schedule through the shared application.
    UpdateSubscriptionSchedule {
        profile_id: String,
        draft: SubscriptionScheduleDraft,
    },
    /// DUAL-07-08: apply and persist a profile's subscription node-keyword
    /// filter (include/exclude/protocol/rename/dedupe) through the shared
    /// pipeline.
    SaveSubscriptionFilter {
        workspace_identity: ProfileWorkspaceIdentity,
        filter: SubscriptionFilterDraft,
    },
    /// DUAL-07-01: import a profile from a URL, local file, or the clipboard
    /// through the shared import application + host import port.
    ImportSubscription {
        profile_id: String,
        channel: SubscriptionImportChannel,
        source: String,
    },
    DeleteProfile {
        profile_id: String,
    },
    /// Rename a profile's display name using the shared owner.
    RenameProfile {
        profile_id: String,
        name: String,
    },
    /// Reorder profiles in the catalog using snapshot-fenced order.
    ReorderProfiles {
        order: Vec<String>,
    },
    /// Clear the active profile selection through the shared owner.
    ClearActiveProfile,
    /// Create a new named YAML override.
    CreateNamedOverride {
        name: String,
        content: String,
    },
    /// Update an existing named YAML override.
    UpdateNamedOverride {
        id: String,
        name: Option<String>,
        content: Option<String>,
    },
    /// Delete an existing named YAML override.
    DeleteNamedOverride {
        id: String,
    },
    /// Bind or toggle an override for a profile.
    SetProfileOverrideBinding {
        profile_id: String,
        override_id: String,
        enabled: bool,
    },
    /// Reorder overrides bound to a profile.
    ReorderProfileOverrides {
        profile_id: String,
        order: Vec<String>,
    },
    /// DUAL-08-01/08-11: persist the aggregation draft and recompute the
    /// shared preview from the real source contents. Both surfaces render the
    /// resulting `AggregationReport`; neither clusters or dedups locally.
    PreviewProfileAggregation {
        draft: AggregationDraft,
    },
    /// DUAL-08-06: materialise the persisted aggregation draft into a brand
    /// new profile, leaving every source profile untouched.
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
    /// DUAL-08-07: re-read the sources of a saved template, re-run the shared
    /// aggregation and overwrite the profile the template produced.
    ReAggregateProfile {
        template_name: String,
    },
    RefreshRuleProviders,
    /// Refresh one provider using the exact controller identity and
    /// inventory revision exposed by the accepted rules-page observation.
    RefreshRuleProvider {
        reference: RuleProviderControllerReference,
    },
    SimulateRuleTrace {
        operation: RuleTraceOperationId,
        request: RuleTraceRequest,
    },
    /// DUAL-12-08: rewrite the traced rule's outbound target and commit the
    /// whole rule list through the atomic apply transaction.
    ApplyTracerRuleOverride {
        request: TracerRuleOverride,
    },
    ResetRuleHitCounters {
        expected_source: RuleSourceIdentity,
    },
    UnpackRuleProvider {
        provider_name: String,
    },
    /// DUAL-11-07: delete the kernel's cached rule-provider files and report
    /// the real file count / freed bytes through the refreshed read model.
    PurgeRuleProviderCache,
    /// DUAL-11-09: invert one rule's enabled flag in the active profile. The
    /// disabled form is persisted as a `#`-prefixed entry via the shared
    /// `format_rule_entry`.
    ToggleRuleEnabled {
        index: usize,
    },
    /// Commit the complete staged list only against its confirmed source identity.
    CommitRuleList {
        request: RuleListCommit,
    },
    /// DUAL-11-10: move one rule a single step up or down in the active
    /// profile's rule order.
    MoveRule {
        index: usize,
        direction: RuleMoveDirection,
    },
    /// DUAL-11-11: insert a wizard-built custom rule at the top of the list,
    /// applying the shared logical-rule validation.
    AddCustomRule {
        rule_type: String,
        payload: String,
        target: String,
    },
    /// DUAL-11-12: prepend the built-in game-routing preset rules for the
    /// supplied outbound target.
    ApplyGameRoutingPresets {
        target: String,
    },
    /// Trigger the kernel's GeoIP/GeoSite database upgrade through the
    /// scope-bound GeoDatabasePort. Acceptance is asynchronous, not completion.
    UpgradeGeoDatabases,
    /// Apply a typed profile section only to the engine/format/schema/revision
    /// from which the editor buffer was read.
    ApplyProfileConfiguration {
        request: Box<ProfileConfigurationRequest>,
    },
    CloseConnection {
        id: String,
    },
    CloseAllConnections,
    /// Clear closed connection history records (SLINT-007).
    ClearClosedConnections,
    /// Clear hourly traffic usage history buckets (SLINT-007).
    ClearTrafficHistory,
    ClearLogs,
    PrepareLogExport,
    SaveLogExport {
        identity: LogExportIdentity,
    },
    CancelLogExport {
        identity: LogExportIdentity,
    },
    SetLogLevelFilter {
        level: Option<String>,
    },
    ClearDnsCache {
        operation: DnsCacheOperationId,
    },
    QueryDns {
        operation: DnsQueryOperationId,
        request: DnsQueryRequest,
    },
    TestDnsLatency,
    /// DUAL-14-08: run the shared DNS leak cross-source probe (random
    /// subdomains under every configured echo authority).
    TestDnsLeak,
    /// DUAL-14-09 (re-scoped): probe this host/process's UDP egress mapping
    /// through the configured STUN server and compare it against the expected
    /// proxied egress. Not a browser WebRTC measurement.
    RunStunProbe,
    /// Apply a shared DNS workbench patch (switches / mapping mode / filter mode).
    ApplyDnsSettings {
        patch: DnsSettingsPatch,
    },
    RunDoctorDiagnostics,
    BootstrapDoctor,
    RepairDoctorIssue {
        check_id: String,
    },
    RepairAllDoctorIssues,
    ToggleTun {
        enabled: bool,
    },
    SetTunAutoRoute {
        enabled: bool,
    },
    SetTunStrictRoute {
        enabled: bool,
    },
    ProbeTunMtu,
    RefreshPublicIpProbe,
    SetSystemProxy {
        enabled: bool,
    },
    /// Recover orphaned system proxy state from journal.
    RecoverSystemProxy,
    SetLanSharing {
        enabled: bool,
        mixed_port: u16,
        bind_address: String,
    },
    SetLanSecurity {
        allowed_ips: Vec<String>,
        disallowed_ips: Vec<String>,
        skip_auth_prefixes: Vec<String>,
        authentication_enabled: bool,
        credentials: Option<LanCredentials>,
    },
    SetIpv6Routing {
        enabled: bool,
    },
    ScanUwpApps,
    SetUwpAppExemption {
        sid: String,
        exempt: bool,
    },
    SetAllUwpExemptions {
        exempt: bool,
    },
    ApplyPac {
        enabled: bool,
        bypass_domains: Vec<String>,
        bypass_lan: bool,
        minify: bool,
    },
    RefreshNetworkRoaming,
    RepairNetworkRoutes,
    CheckVpnAuthorization,
    StartVpn,
    StopVpn,
    RunPrivilegedNetworkRegression,
    SetAppRoutingPackageSelection {
        app_id: String,
        selection: AppRoutingPackageSelection,
    },
    SetAppRoutingMode {
        mode: AppRoutingMode,
    },
    SetAppRule {
        app_id: String,
        rule: AppRoutingRule,
    },
    SyncNow,
    CreateBackupSnapshot {
        profile: Option<String>,
    },
    ResolveConflictKeepLocal,
    ResolveConflictTakeRemote,
    PrepareSnapshotRestore {
        target: SnapshotRestoreTarget,
    },
    ConfirmSnapshotRestore {
        identity: SnapshotRestoreIdentity,
    },
    CancelSnapshotRestore {
        identity: SnapshotRestoreIdentity,
    },
    /// DUAL-09-08: compute a real snapshot-vs-current AST diff and publish it
    /// within the product instance for the surface snapshot. `snapshot_id = None` selects the
    /// newest snapshot of the active profile.
    LoadSnapshotDiff {
        profile: Option<String>,
        snapshot_id: Option<String>,
    },
    /// DUAL-09-06/07: refresh the active profile's snapshot history and publish
    /// it (entries + the shared prune view) for both surfaces.
    LoadSnapshotHistory {
        profile: Option<String>,
        keep: usize,
    },
    /// DUAL-09-07: run the shared dedupe+LRU prune now. `keep = None` keeps the
    /// default retention.
    PruneSnapshots {
        profile: Option<String>,
        keep: Option<usize>,
    },
    /// DUAL-09-03/14: load the active profile's stored document (content +
    /// protection + identity-scoped codec inspection) for the editor surfaces.
    LoadProfileDocument {
        profile: Option<String>,
    },
    /// DUAL-09-14: commit an editor buffer through the shared guarded write
    /// path, comparing the exact document/options source the user observed.
    SaveProfileDocument {
        workspace_identity: ProfileWorkspaceIdentity,
        content: String,
        authorization: ProfileWriteAuthorization,
    },
    /// DUAL-09-14: load a profile's option sidecar (Mixin overlay + filter
    /// draft) through the shared use-case and publish it for the editor panes.
    /// `profile = None` selects the active profile.
    LoadProfileOptions {
        profile: Option<String>,
    },
    /// DUAL-09-14: commit an edited Mixin overlay through the shared use-case
    /// (strip the outgoing mixin's injected rule lines, merge with the
    /// byte-faithful engine, apply through the transaction and persist the
    /// sidecar). The same call the Iced editor makes.
    SaveMixinOverlay {
        workspace_identity: ProfileWorkspaceIdentity,
        mixin_yaml: String,
    },
    /// Select the last installed, locally recorded core version.
    RollbackCore,
    /// DUAL-05-14: decode a share link into the shared protocol draft and
    /// publish the typed report (cipher family / REALITY / smux) for both
    /// surfaces. Never writes a profile.
    /// Publish a staged protocol draft without persisting any profile.
    PrepareCustomNodeDraft {
        draft: Box<ProtocolDraft>,
    },
    ImportCustomNodeUri {
        uri: String,
    },
    /// DUAL-05-14: commit a shared draft into the active profile document,
    /// preserving every other section and every unknown node key.
    SaveCustomNodeDraft {
        /// Boxed: the typed draft grew with the DUAL-05 parameter blocks, and
        /// the intent enum must stay small (clippy::large_enum_variant).
        draft: Box<ProtocolDraft>,
    },
    /// DUAL-05-09/10: analyse the active profile's dialer/relay graph and
    /// publish the resolved chains + typed loop findings for both surfaces.
    ScanDialerChains,
    /// DUAL-05-09/13: edit one whitelisted field of the published custom-node
    /// draft (dialer hop / CA carriers). An unknown field is a typed failure,
    /// so a surface can never invent a schema key.
    UpdateCustomNodeDraftField {
        field: String,
        value: String,
    },
    /// DUAL-05-13: resolve a draft's custom-CA request against the host reader
    /// and publish the typed outcome (loaded / unsupported / failed).
    ResolveCertificateAuthority {
        /// Boxed for the same reason as `SaveCustomNodeDraft`.
        trust: Box<TlsTrustParams>,
    },
    SetLanguage {
        preference: LanguagePreference,
    },
    UpdateSetting {
        key: String,
        value: String,
    },
    CheckUpdates,
}

/// Coarser command category used in event streams and telemetry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandKind {
    CoreLifecycle,
    Profile,
    Proxy,
    Runtime,
    Network,
    Sync,
    Update,
}

/// Result returned by an application facade without leaking its executor or
/// transport implementation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandResult {
    Accepted {
        request_id: RequestId,
    },
    Completed {
        request_id: RequestId,
    },
    Produced {
        request_id: RequestId,
        output: Box<CommandOutput>,
    },
    Rejected {
        request_id: RequestId,
        failure: Failure,
    },
}

impl CommandIntent {
    pub const fn kind(&self) -> CommandKind {
        match self {
            Self::StartCore
            | Self::StopCore
            | Self::RestartCore
            | Self::StartProxy
            | Self::StopProxy
            | Self::ResumeKernel
            | Self::PauseKernel
            | Self::RestartKernel => CommandKind::CoreLifecycle,
            Self::PrepareServiceMode => CommandKind::Network,
            Self::RepairPortConflicts => CommandKind::Network,
            Self::SetCoreLogLevel { .. } | Self::SetTunStack { .. } => CommandKind::Runtime,
            Self::SwitchProfile { .. }
            | Self::RunScriptSandbox { .. }
            | Self::ClearScriptSandbox { .. }
            | Self::PrepareScriptExport { .. }
            | Self::SaveScriptExport { .. }
            | Self::CancelScriptExport { .. }
            | Self::UpdateProfile { .. }
            | Self::UpdateAllSubscriptions
            | Self::RestoreSubscriptionBackup { .. }
            | Self::UpdateSubscriptionFetchSettings { .. }
            | Self::SetSubscriptionAutoReload { .. }
            | Self::UpdateSubscriptionSchedule { .. }
            | Self::SaveSubscriptionFilter { .. }
            | Self::ImportSubscription { .. }
            | Self::DeleteProfile { .. }
            | Self::RenameProfile { .. }
            | Self::ReorderProfiles { .. }
            | Self::ClearActiveProfile
            | Self::CreateNamedOverride { .. }
            | Self::UpdateNamedOverride { .. }
            | Self::DeleteNamedOverride { .. }
            | Self::SetProfileOverrideBinding { .. }
            | Self::ReorderProfileOverrides { .. }
            | Self::PreviewProfileAggregation { .. }
            | Self::CreateAggregatedProfile { .. }
            | Self::SaveAggregationTemplate { .. }
            | Self::DeleteAggregationTemplate { .. }
            | Self::ReAggregateProfile { .. }
            | Self::PrepareCustomNodeDraft { .. }
            | Self::ImportCustomNodeUri { .. }
            | Self::SaveCustomNodeDraft { .. }
            | Self::ScanDialerChains
            | Self::ResolveCertificateAuthority { .. }
            | Self::UpdateCustomNodeDraftField { .. }
            | Self::RefreshRuleProviders
            | Self::RefreshRuleProvider { .. }
            | Self::UnpackRuleProvider { .. }
            | Self::PurgeRuleProviderCache => CommandKind::Profile,
            Self::ToggleRuleEnabled { .. }
            | Self::CommitRuleList { .. }
            | Self::MoveRule { .. }
            | Self::AddCustomRule { .. }
            | Self::ApplyGameRoutingPresets { .. } => CommandKind::Profile,
            Self::UpgradeGeoDatabases => CommandKind::Runtime,
            Self::ApplyProfileConfiguration { .. } => CommandKind::Profile,
            Self::SimulateRuleTrace { .. }
            | Self::ApplyTracerRuleOverride { .. }
            | Self::ResetRuleHitCounters { .. }
            | Self::ReorderOverviewCards { .. }
            | Self::ResetOverviewCardOrder => CommandKind::Runtime,
            Self::SetRoutingIntent { .. }
            | Self::GetRoutingIntent
            | Self::ReadOutboundInventory
            | Self::SelectOutbound { .. }
            | Self::ProbeOutbound { .. }
            | Self::ProbeOutboundBatch { .. }
            | Self::ApplyOutboundUiPreference { .. }
            | Self::StartSpeedtest { .. }
            | Self::SetSpeedtestConcurrency { .. }
            | Self::CancelSpeedtestRun { .. }
            | Self::SetRuntimeProbeSettings { .. } => CommandKind::Proxy,
            Self::CloseConnection { .. }
            | Self::CloseAllConnections
            | Self::ClearClosedConnections
            | Self::ClearTrafficHistory
            | Self::ClearDnsCache { .. } => CommandKind::Runtime,
            Self::ClearLogs
            | Self::PrepareLogExport
            | Self::SaveLogExport { .. }
            | Self::CancelLogExport { .. }
            | Self::SetLogLevelFilter { .. }
            | Self::QueryDns { .. }
            | Self::TestDnsLatency
            | Self::TestDnsLeak
            | Self::RunStunProbe
            | Self::ApplyDnsSettings { .. }
            | Self::RunDoctorDiagnostics
            | Self::BootstrapDoctor
            | Self::RepairDoctorIssue { .. }
            | Self::RepairAllDoctorIssues => CommandKind::Runtime,
            Self::ToggleTun { .. }
            | Self::SetTunAutoRoute { .. }
            | Self::SetTunStrictRoute { .. }
            | Self::ProbeTunMtu
            | Self::RefreshPublicIpProbe
            | Self::SetSystemProxy { .. }
            | Self::RecoverSystemProxy
            | Self::SetLanSharing { .. }
            | Self::SetLanSecurity { .. }
            | Self::SetIpv6Routing { .. }
            | Self::ScanUwpApps
            | Self::SetUwpAppExemption { .. }
            | Self::SetAllUwpExemptions { .. }
            | Self::ApplyPac { .. }
            | Self::RefreshNetworkRoaming
            | Self::RepairNetworkRoutes
            | Self::CheckVpnAuthorization
            | Self::StartVpn
            | Self::StopVpn
            | Self::RunPrivilegedNetworkRegression
            | Self::SetAppRoutingPackageSelection { .. }
            | Self::SetAppRoutingMode { .. }
            | Self::SetAppRule { .. } => CommandKind::Network,
            Self::SyncNow
            | Self::CreateBackupSnapshot { .. }
            | Self::ResolveConflictKeepLocal
            | Self::ResolveConflictTakeRemote
            | Self::PrepareSnapshotRestore { .. }
            | Self::ConfirmSnapshotRestore { .. }
            | Self::CancelSnapshotRestore { .. }
            | Self::LoadSnapshotDiff { .. } => CommandKind::Sync,
            Self::LoadSnapshotHistory { .. } | Self::PruneSnapshots { .. } => CommandKind::Sync,
            Self::LoadProfileDocument { .. } | Self::SaveProfileDocument { .. } => {
                CommandKind::Profile
            }
            Self::LoadProfileOptions { .. } | Self::SaveMixinOverlay { .. } => CommandKind::Profile,
            Self::RollbackCore
            | Self::SetLanguage { .. }
            | Self::UpdateSetting { .. }
            | Self::CheckUpdates => CommandKind::Update,
        }
    }
}

impl CommandResult {
    pub fn into_output(self) -> Result<CommandOutput, Failure> {
        match self {
            Self::Completed { .. } => Ok(CommandOutput::Unit),
            Self::Produced { output, .. } => Ok(*output),
            Self::Rejected { failure, .. } => Err(failure),
            Self::Accepted { .. } => Err(Failure::new(
                ErrorCode::NotReady,
                "Command has no terminal acknowledgment",
                true,
            )),
        }
    }

    pub fn into_unit(self) -> Result<(), Failure> {
        self.into_output()?.into_unit()
    }
}

//! The application-wide [`Message`] bus plus its compact `Debug`
//! implementation used by tracing and tests.

use super::app::{
    ConfirmAction, CoreDownloadProgress, Route, SyncProgress, SyncSummary, ToastStatus, UwpAppItem,
};
use super::dns::{AdvancedConfigsBundle, AdvancedEditMode, DnsTab};
use super::runtime::{IpProbeResult, RuntimeStreamKind, RuntimeStreamState};
use crate::state::core_operations::{CoreDownloadRunId, CoreUpdateCheckId};
use crate::types::{
    app::SnapshotDiffMode,
    dns_query::QueryAction,
    options::{EditorPane, MrsProviderDetail, SyncDiffBundle},
    profile_edit::{ProfileDocumentReadReply, ProfileEditReply, ProfileOptionsReadReply},
    rule_list::RuleListAction,
    rule_trace::RuleTraceAction,
    script::ScriptAction,
    snapshot_restore::RestoreAction,
};
use crate::{admin_server::AdminHostCommand, tray::spec::TrayEvent};
use iced::Rectangle;
use iced::widget::text_editor;
use iced::window::{self, Screenshot};
use infiltrator_application::profile_application::ProfileActivationOutcome;
use infiltrator_application::routing_intent_actions::PendingRoutingIntentChange;
use infiltrator_application::rule_provider_application::ProviderUnpackPlan;
use infiltrator_application::rule_statistics_workbench::StatisticsAction;
use infiltrator_application::runtime_configuration_application::RuntimeConfigurationReadout;
use infiltrator_contract::command::CoreLogLevel;
use infiltrator_contract::outbound::{identity, inventory, preferences, probe};
use infiltrator_contract::rule_provider_snapshot::RuleProviderControllerReference;
use infiltrator_contract::{
    aggregator, command_catalogue, command_output, core_control, core_identity, dialer_chain, dns,
    dns_latency, dns_leak, doctor, error, ime, lan, mini_hud, network_roaming, overview_layout,
    pac, port_conflict, privileged_network, profile_protection, protocol_fidelity, protocol_form,
    provider_cache, routing_intent, rule_document, rules_workspace, runtime_settings, service_mode,
    shortcuts, snapshot, snapshot_history, speedtest, stun_probe, subscription_filter_form,
    subscription_filter_result, subscription_import, surface_snapshot, system_proxy, uwp, version,
    vpn, yaml_ast_diff,
};
use infiltrator_desktop::process_enumerator::{ExtendedProcessInfo, ProcessCategory};
use infiltrator_domain::{
    app_routing::{AppRoutingConfig, AppRoutingMode, AppRoutingRule},
    connection_view::ConnectionGroupingMode,
    profiles::ProfileInfo,
    rules::RuleProviderDiff,
    runtime::{ConnectionSnapshot, TrafficData},
    settings::AppSettings,
};
use infiltrator_ports::host_runtime::TunServiceStatus;
use infiltrator_ports::profile_document_codec::ProfileMixinToggleId;
use std::{path::PathBuf, time::Instant};
pub mod mtu_probe;
use mtu_probe::MtuProbeCompletion;

#[derive(Clone)]
pub enum Message {
    Noop,
    /// Shared 11-page application snapshot delivered by a host source.
    SurfaceSnapshotUpdated(Box<surface_snapshot::SurfaceSnapshot>),
    /// Window content area changed. Drives the shared 4-tier responsive
    /// projection ([`infiltrator_contract::responsive_viewport`]); the payload
    /// is `(width_px, height_px)`.
    WindowResized(f32, f32),
    /// Window focus changed. Drives the shared render-cadence policy
    /// ([`infiltrator_contract::cadence`], DUAL-15-08): foreground keeps the
    /// 60 FPS tick, background drops to 2 FPS.
    WindowFocusChanged(bool),
    /// One OS IME composition transition ([`infiltrator_contract::ime`],
    /// DUAL-15-11). The toolkit text widget owns the field text; the shell
    /// records the session and stops treating composing keys as chords.
    ImeComposition(ime::ImeCompositionEvent),
    Navigate(Route),
    NavigateBack,
    NavigateForward,
    StartCore,
    StopCore,
    CoreStopped,
    CoreControlFinished {
        action: core_control::CoreControlAction,
        token: u64,
        result: Result<(), error::Failure>,
        requested_scope: Option<core_identity::CoreScope>,
        terminal_scope: Option<core_identity::CoreScope>,
        snapshot: Box<snapshot::CoreSnapshot>,
    },
    SettingsLoaded(Result<AppSettings, error::InfiltratorError>),
    LoadProfiles,
    ProfilesLoaded(Result<Vec<ProfileInfo>, error::InfiltratorError>),
    SetActiveProfile(String),
    ProfileActivationFinished(Result<ProfileActivationOutcome, error::InfiltratorError>),
    UpdateImportUrl(String),
    UpdateImportName(String),
    UpdateImportActivate(bool),
    ImportProfile,
    ProfileImported(Result<Option<ProfileActivationOutcome>, error::InfiltratorError>),
    DeleteProfile(String),
    ProfileDeleted(Result<(), error::InfiltratorError>),
    UpdateLocalImportPath(String),
    BrowseLocalImportFile,
    LocalImportFilePicked(Option<PathBuf>),
    UpdateLocalImportName(String),
    UpdateLocalImportActivate(bool),
    ImportLocalProfile,
    LocalProfileImported(Result<Option<ProfileActivationOutcome>, error::InfiltratorError>),
    SelectSubscriptionProfile(String),
    UpdateSubscriptionUrl(String),
    UpdateSubscriptionAutoUpdate(bool),
    UpdateSubscriptionInterval(String),
    UpdateSubscriptionCron(String),
    UpdateSubscriptionUserAgent(String),
    UpdateSubscriptionInsecureSkipVerify(bool),
    /// DUAL-07-09: toggle the post-update core reload preference.
    UpdateSubscriptionAutoReload(bool),
    SaveSubscriptionSettings,
    SubscriptionSettingsSaved(Result<(), error::InfiltratorError>),
    UpdateSubscriptionNow,
    SubscriptionUpdatedNow(
        Result<subscription_import::SubscriptionUpdateReport, error::InfiltratorError>,
    ),
    /// DUAL-07-13: restore the selected profile's transient pre-save backup.
    RestoreSubscriptionBackup,
    SubscriptionBackupRestored(Result<bool, error::InfiltratorError>),
    SubscriptionAutoUpdated(Result<(Vec<String>, bool), error::InfiltratorError>),
    // Tray entries: update every subscription now (ignoring schedules) and
    // flip one profile's auto-update flag straight from the menu.
    UpdateAllSubscriptionsNow,
    AllSubscriptionsUpdated(
        Result<subscription_import::SubscriptionBatchReport, error::InfiltratorError>,
    ),
    SetProfileAutoUpdate {
        name: String,
        enabled: bool,
    },
    ProfileAutoUpdateSet(Result<String, error::InfiltratorError>),
    UpdateProfilesFilter(String),
    ClearProfiles,
    ProfilesCleared(Result<(), error::InfiltratorError>),
    RefreshOutboundInventory,
    SelectOutbound {
        scope: core_identity::CoreScope,
        request: probe::OutboundSelectionRequest,
    },
    PrepareRuntimeOutboundSelection {
        scope: core_identity::CoreScope,
        request: probe::OutboundSelectionRequest,
    },
    OutboundInventoryReadFinished {
        requested_scope: Option<core_identity::CoreScope>,
        terminal_scope: Option<core_identity::CoreScope>,
        result: Result<inventory::OutboundInventorySnapshot, error::Failure>,
    },
    OutboundSelectionFinished {
        scope: core_identity::CoreScope,
        collection_id: identity::OutboundId,
        outbound_id: identity::OutboundId,
        expected_revision: identity::SnapshotRevision,
        terminal_scope: Option<core_identity::CoreScope>,
        result: Result<inventory::OutboundInventorySnapshot, error::Failure>,
    },
    ProbeInspectedOutbound,
    StartOutboundProbe {
        scope: core_identity::CoreScope,
        request: probe::ProbeOutboundRequest,
    },
    StartOutboundProbeBatch {
        scope: core_identity::CoreScope,
        target_scope: probe::OutboundProbeBatchScope,
        expected_inventory_revision: identity::SnapshotRevision,
    },
    OutboundProbeFinished {
        token: u64,
        scope: core_identity::CoreScope,
        request: probe::ProbeOutboundRequest,
        terminal_scope: Option<core_identity::CoreScope>,
        result: Result<inventory::OutboundProbeSnapshot, error::Failure>,
    },
    OutboundProbeBatchFinished {
        token: u64,
        scope: core_identity::CoreScope,
        request: probe::ProbeOutboundBatchRequest,
        terminal_scope: Option<core_identity::CoreScope>,
        result: Result<probe::OutboundProbeBatchSnapshot, error::Failure>,
    },
    DismissOutboundProbeBatch,
    OutboundUiPreferenceFinished {
        token: u64,
        scope: core_identity::CoreScope,
        request: preferences::OutboundUiPreferenceRequest,
        terminal_scope: Option<core_identity::CoreScope>,
        result: Result<preferences::OutboundUiPreferencesSnapshot, error::Failure>,
    },
    FilterProxies(String),
    ToggleFilterAlive(bool),
    ToggleFavoriteOutbound(identity::OutboundId),
    ToggleOutboundGroupExpanded(identity::OutboundId),
    InspectOutbound(Option<identity::OutboundId>),
    ToggleProxyCompactView,
    OpenAddCustomNodeModal(bool),
    UpdateNewNodeType(String),
    UpdateNewNodeName(String),
    UpdateNewNodeServer(String),
    UpdateNewNodePort(String),
    UpdateNewNodeCredential(String),
    UpdateNewNodeCipher(String),
    UpdateNewNodeTls(bool),
    SubmitAddCustomNode,
    CustomNodeAdded(Result<(), error::InfiltratorError>),
    SetOutboundSortOrder(preferences::OutboundSortOrder),
    SetRuntimePanelDelaySort(runtime_settings::RuntimeDelaySortOrder),
    OpenRuntimeProbeSettings,
    ApplyRuntimeProbeSettings,
    CancelRuntimeProbeSettings,
    RuntimeProbeSettingsApplied {
        token: u64,
        result: Result<(), error::Failure>,
    },
    UpdateRuntimeProbeUrlDraft(String),
    UpdateRuntimeProbeTimeoutDraft(String),
    UpdateRuntimeSelectedGroup(String),
    ApplyRuntimeSelectedProxy,
    UpdateRuntimeConnectionFilter(String),
    UpdateRuntimeConnectionSort(String),
    RefreshRuntimeNow,
    TrafficReceived(TrafficData),
    IpInfoReceived(Result<IpProbeResult, error::InfiltratorError>, usize),
    ConnectionsReceived(ConnectionSnapshot),
    LogReceived(String),
    RuntimeStreamLogReceived(core_identity::CoreScope, String),
    RuntimeStreamTrafficReceived(core_identity::CoreScope, TrafficData),
    RuntimeStreamConnectionsReceived(core_identity::CoreScope, ConnectionSnapshot),
    RuntimeStreamStateChanged {
        kind: RuntimeStreamKind,
        scope: core_identity::CoreScope,
        state: RuntimeStreamState,
    },
    ClearRuntimeLogs,
    ConfirmLogExport,
    CancelLogExport,
    RetryLogExport,
    LogExportFinished {
        operation: u64,
        result: Result<command_output::CommandOutput, error::Failure>,
    },
    ToggleLogFollow,
    LogsScrolled {
        offset: f32,
        content: f32,
        viewport: f32,
    },
    LogsCommandFinished {
        generation: u64,
        result: Result<(), error::Failure>,
    },
    SetCoreLogLevel(CoreLogLevel),
    CoreLogLevelFinished(Result<(), error::InfiltratorError>, String),
    CloseConnection(String),
    CloseAllConnections,
    CloseFilteredConnections,
    /// DUAL-13-11: change the idle timeout used by the sweeper.
    SetConnectionIdleTimeout(u64),
    /// DUAL-13-11: terminate connections idle past the configured timeout.
    SweepIdleConnections,
    ConnectionsPrevPage,
    ConnectionsNextPage,
    FetchRuntimeConfig,
    FetchIpInfo,
    RuntimeConfigFetched(Result<RuntimeConfigurationReadout, error::Failure>, u64),
    SetRoutingIntent(routing_intent::RoutingIntent),
    SetIpv6Routing(bool),
    SetTunEnabled(bool),
    InstallTunService,
    RefreshTunServiceStatus,
    TunServiceStatusLoaded(Result<TunServiceStatus, error::InfiltratorError>),
    TunServiceInstalled(Result<(), error::InfiltratorError>),
    ServiceModePrepared(Result<service_mode::ServiceModeSnapshot, error::InfiltratorError>),
    RepairPortConflicts,
    PortConflictsRepaired(Result<port_conflict::PortConflictSnapshot, error::InfiltratorError>),
    SetTunStack(String),
    SetTunAutoRoute(bool),
    SetTunStrictRoute(bool),
    SetSnifferEnabled(bool),
    RoutingIntentFinished {
        request: PendingRoutingIntentChange,
        result: Result<routing_intent::RoutingIntent, error::Failure>,
    },
    RetryRoutingIntent,
    DismissRoutingIntentFailure,
    RuntimePatchResult(Result<(), error::InfiltratorError>, u64, u64),
    OperationResult(Result<(), error::InfiltratorError>),
    LoadRules,
    RulesLoaded(Result<rule_document::RuleDocumentSnapshot, error::InfiltratorError>),
    SetRulesTab(rules_workspace::RulesTab),
    SetRulesJsonTab(rules_workspace::RulesJsonSection),
    ToggleRulesProvidersExpanded,
    RulesPrevPage,
    RulesNextPage,
    RulesSetPage(usize),
    /// DUAL-11-08: the rules list viewport moved. `offset_px` is the absolute
    /// scroll offset and `viewport_px` the measured viewport height reported
    /// by the scrollable; both drive the shared render window.
    RulesListScrolled {
        offset_px: f32,
        viewport_px: f32,
    },
    EnsureRuleProvidersEditorLoaded,
    EnsureProxyProvidersEditorLoaded,
    EnsureSnifferEditorLoaded,
    ActivateRulesHeavyView,
    UpdateRuleProvider(RuleProviderControllerReference),
    FilterRules(String),
    UpdateRulesTracerInput(String),
    UpdateTracerSourceIp(String),
    RunRulesTracer,
    /// DUAL-12-08: outbound target typed into the reverse-apply chooser.
    UpdateTracerOverrideTarget(String),
    /// DUAL-12-08: rewrite the matched rule's outbound and apply it.
    ApplyTracerRuleOverride {
        rule_index: usize,
    },
    /// DUAL-12-08: shared typed result of the reverse-apply attempt.
    UpdateFilteredGroups,
    UpdateNewRuleType(String),
    UpdateNewRulePayload(String),
    UpdateNewRuleTarget(String),
    AddCustomRule,
    ToggleRuleEnabled(rule_document::RuleRowId),
    MoveRuleUp(rule_document::RuleRowId),
    MoveRuleDown(rule_document::RuleRowId),
    SaveRules,
    ApplyGameRoutingPresets,
    UpdateGeoDatabases,
    GeoDatabasesUpdated(Result<(), error::InfiltratorError>),
    InspectRuleProviderDiff(Option<String>),
    UnpackRuleProvider(String),
    RuleProviderDiffLoaded(Result<RuleProviderDiff, error::InfiltratorError>),
    RuleProvidersEditorAction(text_editor::Action),
    SaveRuleProvidersJson,
    RuleProvidersJsonSaved(Result<(), error::InfiltratorError>),
    ProxyProvidersEditorAction(text_editor::Action),
    SaveProxyProvidersJson,
    ProxyProvidersJsonSaved(Result<(), error::InfiltratorError>),
    SnifferEditorAction(text_editor::Action),
    SaveSnifferJson,
    SnifferJsonSaved(Result<(), error::InfiltratorError>),
    LoadAdvancedConfigs,
    AdvancedConfigsBundleLoaded(Result<Box<AdvancedConfigsBundle>, error::InfiltratorError>),
    SetDnsTab(DnsTab),
    SetAdvancedMode(DnsTab, AdvancedEditMode),
    RefreshDnsOnly,
    RefreshFakeIpOnly,
    RefreshTunOnly,
    EnsureDnsEditorLoaded,
    EnsureFakeIpEditorLoaded,
    EnsureTunEditorLoaded,
    ActivateDnsHeavyView,
    DnsConfigJsonLoaded(Result<String, error::InfiltratorError>),
    FakeIpConfigJsonLoaded(Result<String, error::InfiltratorError>),
    TunConfigJsonLoaded(Result<String, error::InfiltratorError>),
    UpdateDnsFormEnable(bool),
    UpdateDnsFormBootstrapNameserver(String),
    UpdateDnsFormNameserver(String),
    UpdateDnsFormFallback(String),
    UpdateDnsFormFallbackGeoip(bool),
    UpdateDnsFormFallbackGeoipCode(String),
    UpdateDnsFormFallbackTrigger(String),
    UpdateDnsFormEnhancedMode(dns::DnsEnhancedMode),
    UpdateDnsFormFilterMode(dns::DnsFakeIpFilterMode),
    UpdateDnsFormFakeIpRange(String),
    UpdateDnsFormFakeIpFilter(String),
    UpdateDnsFormIpv6(bool),
    UpdateDnsFormCache(bool),
    UpdateDnsFormUseHosts(bool),
    UpdateDnsFormUseSystemHosts(bool),
    UpdateDnsFormRespectRules(bool),
    UpdateDnsFormProxyServerNameserver(String),
    UpdateDnsFormDirectNameserver(String),
    UpdateFakeIpFormRange(String),
    UpdateFakeIpFormFilter(String),
    UpdateFakeIpFormStore(bool),
    /// DUAL-14-06: filter the observed Fake-IP mapping pool (view-local).
    UpdateDnsFakeIpQuery(String),
    /// DUAL-14-11: edit the shared `dns.hosts` draft rows.
    UpdateDnsHostsAddress(String),
    UpdateDnsHostsDomain(String),
    AddDnsHostRow,
    OpenDnsHostsEditor,
    CancelDnsHostsEditor,
    CancelDnsHostRowInput,
    EditDnsHostRow(u64),
    ImportLegacyDnsHosts,
    RemoveDnsHostRow(u64),
    SaveDnsHosts,
    DnsHostsCommandFinished {
        token: u64,
        result: Result<(), error::Failure>,
    },
    UpdateTunFormEnable(bool),
    UpdateTunFormStack(String),
    UpdateTunFormMtu(String),
    UpdateTunFormDnsHijack(String),
    UpdateTunFormAutoRoute(bool),
    UpdateTunFormAutoDetectInterface(bool),
    UpdateTunFormStrictRoute(bool),
    DnsConfigEditorAction(text_editor::Action),
    FakeIpConfigEditorAction(text_editor::Action),
    TunConfigEditorAction(text_editor::Action),
    TickSubUpdate,
    TickWebDavSync,
    TickRuntimeRefresh,
    TickFrame(Instant),
    CaptureRegionMeasured(Option<Rectangle>),
    CaptureFrameRendered {
        revision: u64,
        bounds: Option<Rectangle>,
        screenshot: Screenshot,
    },
    TrayEvent(TrayEvent),
    Exit,
    UpdateDnsServer(usize, String),
    UpdateDnsEnhancedMode(String),
    AddDnsServer,
    AddDnsServerTemplate(String),
    RemoveDnsServer(usize),
    UpdateFallbackDnsServer(usize, String),
    AddFallbackDnsServer,
    RemoveFallbackDnsServer(usize),
    SaveDns,
    DnsSaved(Result<(), error::InfiltratorError>),
    SaveFakeIpConfig,
    FakeIpConfigSaved(Result<(), error::InfiltratorError>),
    SaveTunConfig,
    TunConfigSaved(Result<(), error::InfiltratorError>),
    SetAutostart(bool),
    AutostartSet(Result<(), error::InfiltratorError>),
    UpdateNotificationsEnabled(bool),
    UpdateCloseToTray(bool),
    UpdateWebDavEnabled(bool),
    UpdateWebDavUrl(String),
    UpdateWebDavUser(String),
    UpdateWebDavPass(String),
    UpdateWebDavSyncInterval(String),
    UpdateWebDavSyncOnStartup(bool),
    SaveAppSettings,
    AppSettingsSaved(Result<(), error::InfiltratorError>),
    UpdateEditorPathSetting(String),
    SetAdminEnabled(bool),
    UpdateAdminPort(String),
    ApplyAdminSettings,
    AdminSettingsSaved(Result<(), error::InfiltratorError>),
    AdminServerStarted(Result<String, error::InfiltratorError>),
    AdminHostCommand(AdminHostCommand),
    ExternalSettingsLoaded(Result<AppSettings, error::InfiltratorError>),
    SyncUpload,
    SyncDownload,
    SyncFinished(Result<SyncSummary, error::InfiltratorError>),
    SyncProgress(SyncProgress),
    ResolveSyncConflict(String),
    DismissSyncConflict(String),
    SyncConflictResolved(Result<String, error::InfiltratorError>),
    SyncConflictDismissed(Result<String, error::InfiltratorError>),
    CancelWebDavSync,
    TestWebDavConnection,
    WebDavConnectionTested(Result<(), error::InfiltratorError>),
    SetSystemProxy(bool),
    UpdateSystemProxyBypass(String),
    SystemProxySet(Result<system_proxy::SystemProxySnapshot, error::InfiltratorError>),
    SystemProxyReconciled(system_proxy::SystemProxySnapshot),
    SystemProxyRecoveryFinished(system_proxy::SystemProxyRecoverySnapshot),
    RequestAdminPrivilege,
    RequestConfirmation(ConfirmAction),
    ConfirmAction,
    CancelConfirmation,
    ClearError,
    EditProfile(PathBuf),
    /// Open a profile in the Editor with a specific pane preselected
    /// (one-click 覆写/过滤 entry points).
    EditProfileAs(PathBuf, EditorPane),
    ProfileContentLoaded(ProfileDocumentReadReply),
    LoadProfileSnapshots,
    /// DUAL-09-06/07: the shared history read model (entries + prune view).
    ProfileSnapshotsLoaded(
        Result<snapshot_history::SnapshotHistorySnapshot, error::InfiltratorError>,
    ),
    /// DUAL-09-06: write a manual snapshot of the edited profile now.
    BackupProfileSnapshot,
    ProfileSnapshotBackedUp(Result<(), error::InfiltratorError>),
    /// DUAL-09-07: select the retention the manual prune keeps.
    SetSnapshotPruneKeep(usize),
    /// DUAL-09-07: run the shared dedupe+LRU prune with the selected retention.
    PruneProfileSnapshots,
    ProfileSnapshotsPruned(Result<snapshot_history::SnapshotPruneReport, error::InfiltratorError>),
    /// DUAL-09-09: arm the history-panel restore confirmation (first step).
    ArmRestoreProfileSnapshot(PathBuf),
    CancelRestoreProfileSnapshot,
    RestoreProfileSnapshot(PathBuf),
    EditorAction(text_editor::Action),
    SaveProfile,
    DiscardProfileDraft,
    DiscardMixinDraft,
    ProfileSaved(ProfileEditReply),
    // Profile options: mixin overlay editor (Editor page second pane).
    SetEditorPane(EditorPane),
    MixinEditorAction(text_editor::Action),
    MixinLoaded(ProfileOptionsReadReply),
    SaveMixin,
    MixinSaved(ProfileEditReply),
    /// DUAL-10-11: flip one shared Mixin preset toggle in the overlay buffer.
    ToggleMixinPreset(ProfileMixinToggleId, bool),
    // Profile options: subscription filter editor (Profiles page card).
    LoadProfileFilter,
    ProfileFilterLoaded {
        token: u64,
        profile: String,
        result: Result<subscription_filter_form::FilterObservation, error::Failure>,
    },
    UpdateFilterInclude(String),
    UpdateFilterExclude(String),
    UpdateFilterExcludeTypes(String),
    UpdateFilterRenames(String),
    UpdateFilterAdvancedPolicy(String),
    UpdateFilterDedup(usize),
    SaveProfileFilter,
    ProfileFilterSaved {
        token: u64,
        result: Result<subscription_filter_result::SubscriptionFilterApplied, error::Failure>,
    },
    DiscardProfileFilter,
    // MRS rule-provider detail scan (Rules page providers tab).
    ScanMrsProviders,
    MrsDetailsReady(Result<Vec<MrsProviderDetail>, error::InfiltratorError>),
    // Sync conflict key-level diff merge (Sync page).
    LoadSyncDiff(String),
    SyncDiffLoaded(Result<SyncDiffBundle, error::InfiltratorError>),
    PickSyncDiffKey(String, bool),
    SetSyncDiffPicks(bool),
    ApplySyncDiffMerge,
    SyncDiffMerged(Result<String, error::InfiltratorError>),
    CloseSyncDiff,
    OpenConfigDirFinished(Result<(), error::InfiltratorError>),
    LoadKernels,
    KernelsLoaded(Result<Vec<version::InstalledCoreVersion>, error::InfiltratorError>),
    CheckCoreUpdate,
    CoreUpdateFinished {
        check_id: CoreUpdateCheckId,
        result: Result<String, error::InfiltratorError>,
    },
    SetCoreChannel(String),
    DownloadCore(String),
    CoreDownloadProgress {
        run_id: CoreDownloadRunId,
        progress: CoreDownloadProgress,
    },
    CoreDownloadFinished {
        result: Result<String, error::Failure>,
        engine_id: Option<core_identity::EngineId>,
        run_id: CoreDownloadRunId,
    },
    CancelCoreDownload,
    DeleteKernel(String),
    SetDefaultKernel(String),
    RollbackCore,
    KernelOperationFinished(Result<(), error::InfiltratorError>),
    FactoryReset,
    FactoryResetFinished(Result<(), error::InfiltratorError>),
    OpenConfigDir,
    FlushFakeIpCache,
    ConfirmDnsCacheFlush,
    CancelDnsCacheFlush,
    RetryDnsCacheFlush,
    DnsCacheCommandFinished {
        token: u64,
        result: Result<(), error::Failure>,
    },
    WindowClosed(window::Id),
    HideWindow,
    ShowWindow,
    UpdateRuntimeAutoRefresh(bool),
    RuntimePanelSettingsSaved(Result<(), error::InfiltratorError>),
    ClearConfigurationSaveState,
    TogglePerfPanel,
    ToggleTheme,
    SetTheme(String),
    /// The OS appearance changed (`true` = the OS prefers dark).
    SystemThemeChanged(bool),
    /// Advance the shared appearance preference through its cycle.
    CycleThemePreference,
    SetLanguage(String),
    RetryLanguageChoice,
    LanguageChoiceApplied {
        token: u64,
        result: Result<(), error::Failure>,
    },
    ShowToast(String, ToastStatus),
    RemoveToast(u64),
    /// Cancel exactly the Core-owned speedtest run shown by the surface.
    CancelSpeedtestRun(speedtest::SpeedtestRunId),
    /// Adjust the shared engine's concurrency bound by a signed
    /// step. The effective value is read back from `snapshot.concurrency`.
    AdjustSpeedtestConcurrency(i32),
    /// Open the typed outbound speedtest detail modal. The modal only
    /// reads the shared snapshot; it owns no metrics of its own.
    OpenSpeedtestDetail,
    /// Close the outbound speedtest detail modal.
    CloseSpeedtestDetail,
    /// Move one Overview card one slot up in the shared layout order.
    MoveOverviewCardUp(overview_layout::OverviewCardKind),
    /// Move one Overview card one slot down in the shared layout order.
    MoveOverviewCardDown(overview_layout::OverviewCardKind),
    /// Reset the Overview card order to the canonical default.
    ResetOverviewCardOrder,
    // Doctor 体检面板（诊断域）：经内嵌 admin server 的 loopback HTTP 调用
    // /admin/api/doctor* 与 /admin/api/bootstrap，报告写回 diag.doctor。
    RunDoctor,
    DoctorReportReady(Result<doctor::DoctorReport, error::InfiltratorError>),
    DoctorCommandFinished {
        token: u64,
        result: Result<(), error::Failure>,
    },
    RetryDoctorCommand,
    RunDoctorFix,
    RepairDoctorIssue(String),
    DoctorFixApplied(Result<doctor::DoctorFixReport, error::InfiltratorError>),
    RunBootstrap,
    BootstrapFinished(Result<doctor::BootstrapReport, error::InfiltratorError>),
    // Command Palette
    ToggleCommandPalette,
    OpenCommandPalette,
    CloseCommandPalette,
    SetCommandQuery(String),
    SelectNextCommand,
    SelectPrevCommand,
    ExecuteCommand(command_catalogue::CommandTarget),
    ExecuteSelectedCommand,
    // Connection Deep Telemetry Inspector Drawer
    InspectConnection(Option<String>),
    CopyConnectionHost,
    CloseSingleConnection(String),
    // YAML Editor Snippets & Format
    InsertYamlSnippet(&'static str),
    FormatYamlEditor,
    // App Routing (应用分流)
    RefreshAppRoutingProcesses,
    AppRoutingProcessesLoaded(Vec<ExtendedProcessInfo>),
    AppRoutingConfigLoaded(Result<AppRoutingConfig, error::InfiltratorError>),
    AppRoutingPersisted(Result<(), error::InfiltratorError>),
    SetAppRoutingFilter(String),
    SetAppRoutingMode(AppRoutingMode),
    SetAppRouteRule {
        process: String,
        rule: AppRoutingRule,
    },
    SetAppRoutingCategory(Option<ProcessCategory>),
    // Proxy Group Reorder (策略组重排)
    MoveProxyGroupUp(identity::OutboundId),
    MoveProxyGroupDown(identity::OutboundId),
    ResetProxyGroupOrder,
    ApplyProxyGroupOrder,
    CancelProxyGroupOrder,
    // Mini HUD Mode (迷你网速悬浮窗)
    ToggleMiniHudMode,
    SetAlwaysOnTop(bool),
    MiniHudMoved {
        x: f32,
        y: f32,
    },
    MiniHudDragReleased,
    MiniHudPlacementUpdated(Result<mini_hud::MiniHudPlacement, String>),
    MiniHudDisplayKnown(Option<iced::Size>),
    WindowIdResolved(Option<window::Id>),
    // Frameless window chrome (无边框窗口拖拽, DUAL-15-13)
    WindowChromeDragRequested,
    WindowChromeToggleMaximize,
    WindowChromeMinimize,
    WindowChromeClose,
    Script(ScriptAction),
    SnapshotRestore(RestoreAction),

    // DNS Leak & Privacy Probe (Category 1)
    RunDnsLeakProbe,
    RetryDnsLeakProbe,
    DnsLeakCommandFinished {
        token: u64,
        result: Result<(), error::Failure>,
    },
    /// DUAL-14-08: the shared cross-source leak report, or the typed host
    /// refusal.
    DnsLeakProbed {
        token: u64,
        result: Result<dns_leak::DnsLeakReport, error::Failure>,
    },
    /// DUAL-14-09 (re-scoped): probe this host/process's UDP egress mapping
    /// through the configured STUN server. Not a browser WebRTC measurement.
    RunStunProbe,
    /// DUAL-14-09 (re-scoped): the shared STUN egress report, or the typed host
    /// refusal.
    StunProbed(Result<stun_probe::StunProbeReport, error::Failure>),
    /// DUAL-14-10: measure every configured nameserver through the shared
    /// host prober.
    DnsQuery(QueryAction),
    RuleTrace(RuleTraceAction),
    RuleStatistics(StatisticsAction),
    RuleList(RuleListAction),
    RunDnsLatencyProbe,
    /// DUAL-14-10: the shared probe report, or the typed host refusal.
    DnsLatencyProbed(Result<dns_latency::DnsLatencyReport, error::Failure>),
    // Custom Node Modal & Universal URI Codec (Category 2, DUAL-05)
    OpenCustomNodeModal,
    CloseCustomNodeModal,
    UpdateCustomNodeUriInput(String),
    ParseAndImportCustomUri,
    /// DUAL-05-14: the whole shared draft after a form edit. The view builds
    /// the next draft, the update layer re-derives the shared report.
    UpdateCustomNodeDraft(Box<protocol_fidelity::ProtocolDraft>),
    UpdateCustomNodeField(protocol_form::ProtocolField, String),
    /// DUAL-05-14: re-encode the draft into a share link preview.
    ExportCustomNodeUri,
    SaveCustomNodeForm,
    CustomNodeSaved(Result<(), error::InfiltratorError>),
    /// DUAL-05-09/10: analyse the active profile's dialer/relay graph and
    /// publish the shared chain + loop report.
    ScanCustomNodeDialer,
    /// DUAL-05-09/10: the shared analyzer's report (published for both surfaces).
    CustomNodeDialerScanned(Result<dialer_chain::DialerChainReport, error::InfiltratorError>),
    /// DUAL-05-13: resolve the draft's custom-CA request against this host's
    /// reader and publish the typed outcome.
    VerifyCustomNodeCertificateAuthority,
    // Multi-Profile Aggregator (Category 3, DUAL-08)
    OpenAggregatorModal,
    CloseAggregatorModal,
    ToggleAggregatorProfileSelection(String),
    UpdateAggregatorName(String),
    ToggleAggregatorDeduplicate,
    ToggleAggregatorGeoCluster,
    ToggleAggregatorGenerateGroups,
    ToggleAggregatorRemoveEmojis,
    /// DUAL-08-11: run the shared aggregation preview over the real sources.
    PreviewProfileAggregation,
    AggregationPreviewFinished(Result<aggregator::AggregationReport, error::InfiltratorError>),
    /// DUAL-08-06/08-12: save the preview as a brand new independent profile,
    /// optionally activating it through the shared activation path.
    CreateAggregatedProfile,
    AggregatedProfileCreated(Result<aggregator::AggregatedProfileOutcome, error::InfiltratorError>),
    /// DUAL-08-09: toggle the required-field precheck filter.
    ToggleAggregatorAvailabilityPrecheck,
    /// DUAL-08-12: switch the "set as active profile" wizard toggle.
    ToggleAggregatorActivateAfterCreate,
    /// DUAL-08-08: edit the regex rename rules (`模式 => 替换`, one per line).
    UpdateAggregatorRenames(String),
    /// DUAL-08-10: custom group name input.
    UpdateAggregatorCustomGroupName(String),
    /// DUAL-08-10: custom group member keywords input.
    UpdateAggregatorCustomGroupKeywords(String),
    /// DUAL-08-10: append the typed custom group to the draft.
    AddAggregatorCustomGroup,
    /// DUAL-08-10: drop the custom group at this index.
    RemoveAggregatorCustomGroup(usize),
    /// DUAL-08-13: load the persisted aggregation template library.
    LoadAggregatorTemplates,
    AggregatorTemplatesLoaded(
        Result<Vec<aggregator::AggregationTemplate>, error::InfiltratorError>,
    ),
    /// DUAL-08-13: prefill the wizard from a saved template.
    ApplyAggregatorTemplate(String),
    /// DUAL-08-13: persist the current draft under the typed template name.
    SaveAggregatorTemplate,
    /// DUAL-08-13: template name input.
    UpdateAggregatorTemplateName(String),
    AggregatorTemplateSaved(Result<String, error::InfiltratorError>),
    /// DUAL-08-13: delete a saved template.
    DeleteAggregatorTemplate(String),
    AggregatorTemplateDeleted(Result<(String, bool), error::InfiltratorError>),
    /// DUAL-08-07: re-read the sources and refresh the generated profile.
    ReAggregateProfile(String),
    AggregationReaggregated(Result<aggregator::AggregatedProfileOutcome, error::InfiltratorError>),
    // Connection Grouping & Quick Rule (Category 4)
    SetConnectionGroupingMode(ConnectionGroupingMode),
    AddQuickRuleFromConnection {
        pattern: String,
        target: String,
    },
    // Config Snapshot Visual Diff & Rollback (Category 5)
    OpenSnapshotDiff(String),
    CloseSnapshotDiff,
    /// DUAL-09-08: the shared application finished computing the diff.
    SnapshotDiffLoaded(Result<yaml_ast_diff::YamlAstDiffSnapshot, error::InfiltratorError>),
    /// DUAL-09-08: inline vs split layout for the same shared diff.
    SetSnapshotDiffMode(SnapshotDiffMode),
    /// DUAL-09-14: recompute the open diff from the shared snapshot
    /// application (the Bevy card's 「刷新差异」 is the same action).
    RefreshSnapshotDiff,
    /// DUAL-09-09: arm the two-step rollback confirmation.
    RollbackToSnapshot(String),
    /// DUAL-09-12: allow direct edits of a protected remote subscription for
    /// this session (the application still receives the explicit flag).
    SetProfileWriteAuthorization(profile_protection::ProfileWriteAuthorization),
    // Global Hotkey Manager (Category 6) — shared contract registry.
    BeginHotkeyCapture(shortcuts::ShortcutAction),
    CancelHotkeyCapture,
    /// One raw key press forwarded from the window: capture and dispatch are
    /// resolved against the live registry in `update`.
    KeyboardChord {
        key: String,
        modifiers: shortcuts::KeyModifiers,
    },
    ToggleHotkeyEnabled(shortcuts::ShortcutAction),
    ResetHotkey(shortcuts::ShortcutAction),
    ShortcutsUpdated(Result<shortcuts::ShortcutRegistry, String>),
    // Wave 3 Category 1: PCAP Exporter
    TogglePcapCapture,
    ExportPcapBuffer,
    // Wave 3 Category 2: Logical Sub-Rules Builder
    UpdateSubRuleOperator(String),
    AddSubRuleCondition(String),
    RemoveSubRuleCondition(usize),
    UpdateSubRuleTarget(String),
    InsertSubRuleIntoRules,
    // Wave 3 Category 3: Node Speedtest & Jitter
    StartSpeedtest(speedtest::SpeedtestTarget),
    /// Core accepted the exact run and returned its canonical owner snapshot.
    SpeedtestStarted(
        Result<
            (
                speedtest::SpeedtestStartReceipt,
                speedtest::SpeedtestSnapshot,
            ),
            error::Failure,
        >,
    ),
    /// Core handled cancellation for the exact run and returned its canonical owner snapshot.
    SpeedtestCancelled(
        Result<
            (
                speedtest::SpeedtestCancelOutcome,
                speedtest::SpeedtestSnapshot,
            ),
            error::Failure,
        >,
    ),
    /// Core applied the new speedtest concurrency and returned its canonical owner snapshot.
    SpeedtestConcurrencySet(Result<speedtest::SpeedtestSnapshot, error::Failure>),
    // Wave 3 Category 4: Geo Data Updater
    CheckGeoDataUpdates,
    TriggerGeoDataUpdate,
    GeoDataUpdateResult(Result<(), String>),
    // Wave 3 Category 5: Windows UWP Loopback Utility
    ScanUwpApps,
    UwpAppsLoaded(Vec<UwpAppItem>),
    UwpSnapshotLoaded(uwp::UwpLoopbackSnapshot),
    ExemptAllUwpApps,
    ClearAllUwpExemptions,
    ToggleUwpAppExemption(String),
    UwpExemptionsChanged(Result<uwp::UwpLoopbackSnapshot, error::InfiltratorError>),
    // Wave 3 Category 6: Encrypted Backup Package
    UpdateEncryptedBackupPassphrase(String),
    ExportEncryptedPackage,
    ImportEncryptedPackage,
    // Wave 4 Category 1: Network Interface Roaming
    PollNetworkInterfaces,
    NetworkInterfacesPolled(network_roaming::NetworkRoamingSnapshot),
    ForceGatewayReconnect,
    NetworkRoamingRepaired(
        Result<network_roaming::NetworkRoamingSnapshot, error::InfiltratorError>,
    ),
    StartVpn,
    StopVpn,
    VpnSessionUpdated(Result<vpn::VpnSessionSnapshot, error::InfiltratorError>),
    RunPrivilegedNetworkRegression,
    PrivilegedNetworkRegressionUpdated(
        Result<privileged_network::PrivilegedNetworkSnapshot, error::InfiltratorError>,
    ),
    // Wave 4 Category 2: Crash Watchdog & Forensics
    CheckCrashWatchdog,
    RecoverOrphanedState,
    ExportCrashDiagnostics,
    // Wave 4 Category 3: External Web Dashboard
    LaunchWebDashboard(&'static str),
    // Wave 4 Category 4: Log Regex & Redacted Export
    UpdateLogRegexFilter(String),
    SetLogLevelFilter(String),
    ExportRedactedLogs,
    // Wave 4 Category 5: Subscription Quota & Cron
    // Wave 4 Category 6: PAC Auto-Proxy & Bypass CIDR
    UpdatePacBypassSubnets(String),
    CompileAndValidatePac,
    PacApplied(Result<pac::PacSnapshot, error::InfiltratorError>),
    TogglePacMode(bool),
    // Wave 5 Category 1: Rule Hit Counter & Stale Rule Audit
    // Wave 5 Category 2: Latency Time-Series & Stability Radar
    // Wave 5 Category 3: TUN Multi-Stack & MTU Negotiator
    SelectTunStack(String),
    ProbeOptimalMtu,
    MtuProbed(u32),
    MtuProbeFinished(MtuProbeCompletion),
    // Wave 5 Category 4: Rule-Provider Lifecycle & Rule Unpacker
    UnpackRuleProviderToCustom(String),
    PurgeRuleProviderCache,
    /// DUAL-11-06: the real provider rules a host read produced.
    RuleProviderUnpacked(Result<ProviderUnpackPlan, error::InfiltratorError>),
    /// DUAL-11-07: the real files/bytes a host purge removed.
    RuleProviderCachePurged(Result<provider_cache::ProviderCachePurge, error::InfiltratorError>),
    // Wave 5 Category 5: Config Apply Multi-Stage Transaction Guard
    // Wave 5 Category 6: LAN Proxy Sharing & Client Access Whitelist
    ToggleLanSharing(bool),
    UpdateLanSharingPort(u16),
    UpdateLanBindAddress(String),
    UpdateLanAclWhitelist(String),
    ApplyLanSharing,
    LanSharingSet(
        Result<lan::LanSharingSnapshot, error::InfiltratorError>,
        u64,
    ),
    UpdateLanAllowedIps(String),
    UpdateLanDisallowedIps(String),
    UpdateLanSkipAuthPrefixes(String),
    ToggleLanAuthentication(bool),
    UpdateLanAuthUsername(String),
    UpdateLanAuthPassword(String),
    ApplyLanSecurity,
    LanSecuritySet(
        Result<lan::LanSecuritySnapshot, error::InfiltratorError>,
        u64,
    ),
}

mod debug;

//! The application-wide [`Message`] bus plus its compact `Debug`
//! implementation used by tracing and tests.

use super::app::{
    ConfirmAction, CoreDownloadProgress, Route, SyncProgress, SyncSummary, ToastStatus,
};
use super::dns::{AdvancedConfigsBundle, AdvancedEditMode, DnsTab};
use super::rules::RulesLoadBundle;
use super::runtime::{IpProbeResult, RuntimeConfig, RuntimeStreamKind, RuntimeStreamState};
use iced::{widget::text_editor, window};
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::rules_workspace::{RulesJsonSection, RulesTab};
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::subscription_import::{
    SubscriptionBatchReport, SubscriptionUpdateReport,
};
use infiltrator_contract::version::InstalledCoreVersion;
use infiltrator_domain::profiles::ProfileInfo;
use infiltrator_domain::proxy::Proxy;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_domain::runtime::{
    ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider, TrafficData,
};
use infiltrator_domain::settings::AppSettings;
use infiltrator_ports::host_runtime::HostRuntime;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

/// Shared batch report of the "update all subscriptions" entry. Both the tray
/// and the Profiles toolbar consume the same application-produced counts.
pub type SubscriptionUpdateOutcomes = Result<SubscriptionBatchReport, InfiltratorError>;

#[derive(Clone)]
pub struct MtuProbeCompletion {
    pub snapshot: infiltrator_contract::mtu::MtuNegotiationSnapshot,
    pub generation: u64,
    pub session_token: Option<SessionToken>,
}

#[derive(Clone)]
pub enum Message {
    Noop,
    /// Shared 11-page application snapshot delivered by a host source.
    SurfaceSnapshotUpdated(Box<infiltrator_contract::surface_snapshot::SurfaceSnapshot>),
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
    ImeComposition(infiltrator_contract::ime::ImeCompositionEvent),
    Navigate(Route),
    NavigateBack,
    NavigateForward,
    StartProxy,
    StopProxy,
    /// Core boot result; the bool reports whether the external-controller
    /// port had to be rotated during the boot retry loop.
    ProxyStarted(Result<(Arc<dyn HostRuntime>, bool), InfiltratorError>, u64),
    ProxyStopped,
    SettingsLoaded(Result<AppSettings, InfiltratorError>),
    LoadProfiles,
    ProfilesLoaded(Result<Vec<ProfileInfo>, InfiltratorError>),
    SetActiveProfile(String),
    ProfileActivationFinished(Result<bool, InfiltratorError>),
    UpdateImportUrl(String),
    UpdateImportName(String),
    UpdateImportActivate(bool),
    ImportProfile,
    ProfileImported(Result<bool, InfiltratorError>),
    DeleteProfile(String),
    ProfileDeleted(Result<(), InfiltratorError>),
    UpdateLocalImportPath(String),
    BrowseLocalImportFile,
    LocalImportFilePicked(Option<PathBuf>),
    UpdateLocalImportName(String),
    UpdateLocalImportActivate(bool),
    ImportLocalProfile,
    LocalProfileImported(Result<bool, InfiltratorError>),
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
    SubscriptionSettingsSaved(Result<(), InfiltratorError>),
    UpdateSubscriptionNow,
    SubscriptionUpdatedNow(Result<SubscriptionUpdateReport, InfiltratorError>),
    /// DUAL-07-13: restore the selected profile's transient pre-save backup.
    RestoreSubscriptionBackup,
    SubscriptionBackupRestored(Result<bool, InfiltratorError>),
    SubscriptionAutoUpdated(Result<(Vec<String>, bool), InfiltratorError>),
    // Tray entries: update every subscription now (ignoring schedules) and
    // flip one profile's auto-update flag straight from the menu.
    UpdateAllSubscriptionsNow,
    AllSubscriptionsUpdated(SubscriptionUpdateOutcomes),
    SetProfileAutoUpdate {
        name: String,
        enabled: bool,
    },
    ProfileAutoUpdateSet(Result<String, InfiltratorError>),
    UpdateProfilesFilter(String),
    ClearProfiles,
    ProfilesCleared(Result<(), InfiltratorError>),
    LoadProxies,
    ProxiesLoaded(Result<HashMap<String, Proxy>, InfiltratorError>),
    SelectProxy(String, String),
    FilterProxies(String),
    ToggleFilterAlive(bool),
    ToggleFavoriteProxy(String),
    ToggleProxyCompactView,
    InspectProxy(Option<String>),
    OpenAddCustomNodeModal(bool),
    UpdateNewNodeType(String),
    UpdateNewNodeName(String),
    UpdateNewNodeServer(String),
    UpdateNewNodePort(String),
    UpdateNewNodeCredential(String),
    UpdateNewNodeCipher(String),
    UpdateNewNodeTls(bool),
    SubmitAddCustomNode,
    CustomNodeAdded(Result<(), InfiltratorError>),
    ToggleProxySort,
    UpdateProxyDelaySort(String),
    UpdateDelayTestUrl(String),
    UpdateDelayTimeoutMs(String),
    UpdateRuntimeSelectedGroup(String),
    UpdateRuntimeSelectedProxy(String),
    ApplyRuntimeSelectedProxy,
    UpdateRuntimeConnectionFilter(String),
    UpdateRuntimeConnectionSort(String),
    RefreshRuntimeNow,
    TrafficReceived(TrafficData),
    MemoryReceived(MemoryData),
    IpInfoReceived(Result<IpProbeResult, InfiltratorError>, usize),
    ConnectionsReceived(ConnectionSnapshot),
    LogReceived(String),
    RuntimeStreamLogReceived(u64, String),
    RuntimeStreamTrafficReceived(u64, TrafficData),
    RuntimeStreamConnectionsReceived(u64, ConnectionSnapshot),
    RuntimeStreamStateChanged {
        kind: RuntimeStreamKind,
        generation: u64,
        state: RuntimeStreamState,
    },
    RuntimePollFailed(String),
    ClearRuntimeLogs,
    SetLogLevel(String),
    SetCoreLogLevel(String),
    CoreLogLevelFinished(Result<(), InfiltratorError>, String),
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
    RuntimeConfigFetched(Result<RuntimeConfig, InfiltratorError>, u64),
    SetProxyMode(String),
    SetIpv6Routing(bool),
    SetTunEnabled(bool),
    InstallTunService,
    RefreshTunServiceStatus,
    TunServiceStatusLoaded(
        Result<infiltrator_ports::host_runtime::TunServiceStatus, InfiltratorError>,
    ),
    TunServiceInstalled(Result<(), InfiltratorError>),
    ServiceModePrepared(
        Result<infiltrator_contract::service_mode::ServiceModeSnapshot, InfiltratorError>,
    ),
    RepairPortConflicts,
    PortConflictsRepaired(
        Result<infiltrator_contract::port_conflict::PortConflictSnapshot, InfiltratorError>,
    ),
    SetTunStack(String),
    SetTunAutoRoute(bool),
    SetTunStrictRoute(bool),
    SetSnifferEnabled(bool),
    ModeSetResult(Result<(), InfiltratorError>),
    RuntimePatchResult(Result<(), InfiltratorError>, u64, u64),
    OperationResult(Result<(), InfiltratorError>),
    LoadRules,
    RulesBundleLoaded(Result<RulesLoadBundle, InfiltratorError>),
    RulesLoaded(Result<Vec<RuleEntry>, InfiltratorError>),
    SetRulesTab(RulesTab),
    SetRulesJsonTab(RulesJsonSection),
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
    RuleProvidersJsonLoaded(Result<String, InfiltratorError>),
    ProxyProvidersJsonLoaded(Result<String, InfiltratorError>),
    SnifferJsonLoaded(Result<String, InfiltratorError>),
    LoadProviders,
    ProvidersLoaded(Result<(Vec<ProxyProvider>, Vec<RuleProvider>), InfiltratorError>),
    UpdateProxyProvider(String),
    UpdateRuleProvider(String),
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
    TracerRuleOverrideApplied(infiltrator_contract::rule_tracer::TracerRuleOverrideResult),
    UpdateFilteredGroups,
    UpdateNewRuleType(String),
    UpdateNewRulePayload(String),
    UpdateNewRuleTarget(String),
    AddCustomRule,
    RuleAdded(Result<(), InfiltratorError>),
    ToggleRuleEnabled(usize),
    MoveRuleUp(usize),
    MoveRuleDown(usize),
    SaveRules,
    ApplyGameRoutingPresets,
    UpdateGeoDatabases,
    GeoDatabasesUpdated(Result<(), InfiltratorError>),
    RulesSaved(Result<(), InfiltratorError>),
    InspectRuleProviderDiff(Option<String>),
    UnpackRuleProvider(String),
    RuleProviderDiffLoaded(Result<infiltrator_domain::rules::RuleProviderDiff, InfiltratorError>),
    RuleProvidersEditorAction(text_editor::Action),
    SaveRuleProvidersJson,
    RuleProvidersJsonSaved(Result<(), InfiltratorError>),
    ProxyProvidersEditorAction(text_editor::Action),
    SaveProxyProvidersJson,
    ProxyProvidersJsonSaved(Result<(), InfiltratorError>),
    SnifferEditorAction(text_editor::Action),
    SaveSnifferJson,
    SnifferJsonSaved(Result<(), InfiltratorError>),
    LoadAdvancedConfigs,
    AdvancedConfigsBundleLoaded(Result<Box<AdvancedConfigsBundle>, InfiltratorError>),
    SetDnsTab(DnsTab),
    SetAdvancedMode(DnsTab, AdvancedEditMode),
    RefreshDnsOnly,
    RefreshFakeIpOnly,
    RefreshTunOnly,
    EnsureDnsEditorLoaded,
    EnsureFakeIpEditorLoaded,
    EnsureTunEditorLoaded,
    ActivateDnsHeavyView,
    DnsConfigJsonLoaded(Result<String, InfiltratorError>),
    FakeIpConfigJsonLoaded(Result<String, InfiltratorError>),
    TunConfigJsonLoaded(Result<String, InfiltratorError>),
    UpdateDnsFormEnable(bool),
    UpdateDnsFormBootstrapNameserver(String),
    UpdateDnsFormNameserver(String),
    UpdateDnsFormFallback(String),
    UpdateDnsFormFallbackGeoip(bool),
    UpdateDnsFormFallbackGeoipCode(String),
    UpdateDnsFormFallbackTrigger(String),
    UpdateDnsFormEnhancedMode(infiltrator_contract::dns::DnsEnhancedMode),
    UpdateDnsFormFilterMode(infiltrator_contract::dns::DnsFakeIpFilterMode),
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
    RemoveDnsHostRow(usize),
    SaveDnsHosts,
    DnsHostsSaved(Result<(), InfiltratorError>),
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
    TrayEvent(crate::tray::spec::TrayEvent),
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
    DnsSaved(Result<(), InfiltratorError>),
    SaveFakeIpConfig,
    FakeIpConfigSaved(Result<(), InfiltratorError>),
    SaveTunConfig,
    TunConfigSaved(Result<(), InfiltratorError>),
    SetAutostart(bool),
    AutostartSet(Result<(), InfiltratorError>),
    UpdateNotificationsEnabled(bool),
    UpdateCloseToTray(bool),
    UpdateWebDavEnabled(bool),
    UpdateWebDavUrl(String),
    UpdateWebDavUser(String),
    UpdateWebDavPass(String),
    UpdateWebDavSyncInterval(String),
    UpdateWebDavSyncOnStartup(bool),
    SaveAppSettings,
    AppSettingsSaved(Result<(), InfiltratorError>),
    UpdateEditorPathSetting(String),
    SetAdminEnabled(bool),
    UpdateAdminPort(String),
    ApplyAdminSettings,
    AdminSettingsSaved(Result<(), InfiltratorError>),
    AdminServerStarted(Result<String, InfiltratorError>),
    AdminHostCommand(crate::admin_server::AdminHostCommand),
    ExternalSettingsLoaded(Result<AppSettings, InfiltratorError>),
    SyncUpload,
    SyncDownload,
    SyncFinished(Result<SyncSummary, InfiltratorError>),
    SyncProgress(SyncProgress),
    ResolveSyncConflict(String),
    DismissSyncConflict(String),
    SyncConflictResolved(Result<String, InfiltratorError>),
    SyncConflictDismissed(Result<String, InfiltratorError>),
    CancelWebDavSync,
    TestWebDavConnection,
    WebDavConnectionTested(Result<(), InfiltratorError>),
    SetSystemProxy(bool),
    UpdateSystemProxyBypass(String),
    SystemProxySet(
        Result<infiltrator_contract::system_proxy::SystemProxySnapshot, InfiltratorError>,
    ),
    SystemProxyReconciled(infiltrator_contract::system_proxy::SystemProxySnapshot),
    SystemProxyRecoveryFinished(infiltrator_contract::system_proxy::SystemProxyRecoverySnapshot),
    RequestAdminPrivilege,
    RequestConfirmation(ConfirmAction),
    ConfirmAction,
    CancelConfirmation,
    ClearError,
    EditProfile(PathBuf),
    /// Open a profile in the Editor with a specific pane preselected
    /// (one-click 覆写/过滤 entry points).
    EditProfileAs(PathBuf, crate::types::options::EditorPane),
    ProfileContentLoaded(Result<(PathBuf, String), InfiltratorError>),
    LoadProfileSnapshots,
    /// DUAL-09-06/07: the shared history read model (entries + prune view).
    ProfileSnapshotsLoaded(
        Result<infiltrator_contract::snapshot_history::SnapshotHistorySnapshot, InfiltratorError>,
    ),
    /// DUAL-09-06: write a manual snapshot of the edited profile now.
    BackupProfileSnapshot,
    ProfileSnapshotBackedUp(Result<(), InfiltratorError>),
    /// DUAL-09-07: select the retention the manual prune keeps.
    SetSnapshotPruneKeep(usize),
    /// DUAL-09-07: run the shared dedupe+LRU prune with the selected retention.
    PruneProfileSnapshots,
    ProfileSnapshotsPruned(
        Result<infiltrator_contract::snapshot_history::SnapshotPruneReport, InfiltratorError>,
    ),
    /// DUAL-09-09: arm the history-panel restore confirmation (first step).
    ArmRestoreProfileSnapshot(PathBuf),
    CancelRestoreProfileSnapshot,
    RestoreProfileSnapshot(PathBuf),
    ProfileSnapshotRestored(Result<(), InfiltratorError>),
    EditorAction(text_editor::Action),
    SaveProfile,
    ProfileSaved(Result<(), InfiltratorError>),
    // Profile options: mixin overlay editor (Editor page second pane).
    SetEditorPane(crate::types::options::EditorPane),
    MixinEditorAction(text_editor::Action),
    MixinLoaded(Result<String, InfiltratorError>),
    SaveMixin,
    MixinSaved(Result<(), InfiltratorError>),
    /// DUAL-10-11: flip one shared Mixin preset toggle in the overlay buffer.
    ToggleMixinPreset(String, bool),
    // Profile options: subscription filter editor (Profiles page card).
    LoadProfileFilter,
    ProfileFilterLoaded(
        Result<
            infiltrator_contract::subscription_import::SubscriptionFilterDraft,
            InfiltratorError,
        >,
    ),
    UpdateFilterInclude(String),
    UpdateFilterExclude(String),
    UpdateFilterExcludeTypes(String),
    UpdateFilterRenames(String),
    UpdateFilterDedup(usize),
    SaveProfileFilter,
    ProfileFilterSaved(Result<infiltrator_domain::filter::FilterReport, InfiltratorError>),
    // MRS rule-provider detail scan (Rules page providers tab).
    ScanMrsProviders,
    MrsDetailsReady(Result<Vec<crate::types::options::MrsProviderDetail>, InfiltratorError>),
    // Sync conflict key-level diff merge (Sync page).
    LoadSyncDiff(String),
    SyncDiffLoaded(Result<crate::types::options::SyncDiffBundle, InfiltratorError>),
    PickSyncDiffKey(String, bool),
    SetSyncDiffPicks(bool),
    ApplySyncDiffMerge,
    SyncDiffMerged(Result<String, InfiltratorError>),
    CloseSyncDiff,
    OpenConfigDirFinished(Result<(), InfiltratorError>),
    LoadKernels,
    KernelsLoaded(Result<Vec<InstalledCoreVersion>, InfiltratorError>),
    CheckCoreUpdate,
    CoreUpdateInfo(Result<String, InfiltratorError>), // Latest version string
    SetCoreChannel(String),
    DownloadCore(String),
    CoreDownloadProgress(CoreDownloadProgress, u64),
    CoreDownloadFinished(Result<String, InfiltratorError>, u64),
    CancelCoreDownload,
    DeleteKernel(String),
    SetDefaultKernel(String),
    RollbackCore,
    KernelOperationFinished(Result<(), InfiltratorError>),
    FactoryReset,
    FactoryResetFinished(Result<(), InfiltratorError>),
    OpenConfigDir,
    FlushFakeIpCache,
    DnsCacheFlushed(Result<infiltrator_contract::dns::DnsCacheFlushReport, String>),
    TestProxyDelay(String),
    TestGroupDelay(String),
    ProxyTested(String, Result<u64, InfiltratorError>),
    WindowClosed(window::Id),
    HideWindow,
    ShowWindow,
    UpdateRuntimeAutoRefresh(bool),
    RuntimePanelSettingsSaved(Result<(), InfiltratorError>),
    RuntimeRebuildFinished(Result<Arc<dyn HostRuntime>, InfiltratorError>),
    ClearRebuildFlow,
    TogglePerfPanel,
    ToggleTheme,
    SetTheme(String),
    /// The OS appearance changed (`true` = the OS prefers dark).
    SystemThemeChanged(bool),
    /// Advance the shared appearance preference through its cycle.
    CycleThemePreference,
    SetLanguage(String),
    ShowToast(String, ToastStatus),
    RemoveToast(u64),
    TestAllProxyDelays,
    /// Result of a scope-wide speedtest (all groups or one group) driven
    /// through the shared engine port. `Err` carries the typed port failure.
    SpeedtestScopeUpdated(
        Result<
            infiltrator_contract::speedtest::SpeedtestSnapshot,
            infiltrator_ports::error::PortError,
        >,
    ),
    /// Request cancellation of the active shared speedtest batch.
    CancelSpeedtest,
    /// DUAL-06-03: store the user-typed speedtest target URL. The typed value
    /// is handed to `run_scope` / `probe_node`; the engine owns the effective
    /// target when the field is blank.
    UpdateSpeedtestTestUrl(String),
    /// DUAL-06-01: adjust the shared engine's concurrency bound by a signed
    /// step. The effective value is read back from `snapshot.config.concurrency`.
    AdjustSpeedtestConcurrency(i32),
    /// DUAL-06-13: open the per-node speedtest detail modal. The modal only
    /// reads the shared snapshot; it owns no metrics of its own.
    OpenSpeedtestDetail,
    /// DUAL-06-13: close the speedtest detail modal.
    CloseSpeedtestDetail,
    /// Move one Overview card one slot up in the shared layout order.
    MoveOverviewCardUp(infiltrator_contract::overview_layout::OverviewCardKind),
    /// Move one Overview card one slot down in the shared layout order.
    MoveOverviewCardDown(infiltrator_contract::overview_layout::OverviewCardKind),
    /// Reset the Overview card order to the canonical default.
    ResetOverviewCardOrder,
    // ui-wave2-p: proxies page — expand/collapse one proxy-group card
    // (view-only UI state; flips AppState::proxy_groups_expanded).
    ToggleProxyGroupExpanded(String),
    // Doctor 体检面板（诊断域）：经内嵌 admin server 的 loopback HTTP 调用
    // /admin/api/doctor* 与 /admin/api/bootstrap，报告写回 diag.doctor。
    RunDoctor,
    DoctorReportReady(Result<crate::types::doctor::DoctorReport, InfiltratorError>),
    RunDoctorFix,
    DoctorFixApplied(Result<crate::types::doctor::DoctorFixReport, InfiltratorError>),
    RunBootstrap,
    BootstrapFinished(Result<crate::types::doctor::BootstrapReport, InfiltratorError>),
    // Command Palette
    ToggleCommandPalette,
    OpenCommandPalette,
    CloseCommandPalette,
    SetCommandQuery(String),
    SelectNextCommand,
    SelectPrevCommand,
    ExecuteCommand(infiltrator_contract::command_catalogue::CommandTarget),
    // Connection Deep Telemetry Inspector Drawer
    InspectConnection(Option<String>),
    CloseSingleConnection(String),
    // YAML Editor Snippets & Format
    InsertYamlSnippet(&'static str),
    FormatYamlEditor,
    // App Routing (应用分流)
    RefreshAppRoutingProcesses,
    AppRoutingProcessesLoaded(Vec<crate::host::process_enumerator::ExtendedProcessInfo>),
    AppRoutingConfigLoaded(
        Result<infiltrator_domain::app_routing::AppRoutingConfig, InfiltratorError>,
    ),
    AppRoutingPersisted(Result<(), InfiltratorError>),
    SetAppRoutingFilter(String),
    SetAppRoutingMode(super::app_routing::AppRoutingMode),
    SetAppRouteRule {
        process: String,
        rule: super::app_routing::AppRouteRule,
    },
    SetAppRoutingCategory(Option<crate::host::process_enumerator::ProcessCategory>),
    // Proxy Group Reorder (策略组重排)
    MoveProxyGroupUp(String),
    MoveProxyGroupDown(String),
    ResetProxyGroupOrder,
    // Mini HUD Mode (迷你网速悬浮窗)
    ToggleMiniHudMode,
    SetAlwaysOnTop(bool),
    MiniHudMoved {
        x: f32,
        y: f32,
    },
    MiniHudDragReleased,
    MiniHudPlacementUpdated(Result<infiltrator_contract::mini_hud::MiniHudPlacement, String>),
    MiniHudDisplayKnown(Option<iced::Size>),
    WindowIdResolved(Option<iced::window::Id>),
    // Frameless window chrome (无边框窗口拖拽, DUAL-15-13)
    WindowChromeDragRequested,
    WindowChromeToggleMaximize,
    WindowChromeMinimize,
    WindowChromeClose,
    // Script Sandbox Console (脚本沙箱控制台)
    RunScriptSandboxTest,
    SelectScriptPreset(String),
    UpdateScriptSandboxCode(String),
    UpdateScriptSandboxInputYaml(String),
    ClearScriptSandbox,
    /// DUAL-10-12: export one artifact through the shared export use-case and
    /// the host save-file port (typed unsupported when the host has none).
    ExportScriptDraft(infiltrator_contract::script_export::ScriptExportKind),
    /// DUAL-10-12: the shared export projection (file name/bytes/checksum +
    /// typed host outcome) the console renders for both surfaces.
    ScriptExportFinished(
        Result<infiltrator_contract::script_export::ScriptExportSnapshot, InfiltratorError>,
    ),
    // DNS Leak & Privacy Probe (Category 1)
    RunDnsLeakProbe,
    /// DUAL-14-08: the shared cross-source leak report, or the typed host
    /// refusal.
    DnsLeakProbed(
        Result<infiltrator_contract::dns_leak::DnsLeakReport, infiltrator_contract::error::Failure>,
    ),
    /// DUAL-14-09 (re-scoped): probe this host/process's UDP egress mapping
    /// through the configured STUN server. Not a browser WebRTC measurement.
    RunStunProbe,
    /// DUAL-14-09 (re-scoped): the shared STUN egress report, or the typed host
    /// refusal.
    StunProbed(
        Result<
            infiltrator_contract::stun_probe::StunProbeReport,
            infiltrator_contract::error::Failure,
        >,
    ),
    /// DUAL-14-10: measure every configured nameserver through the shared
    /// host prober.
    RunDnsLatencyProbe,
    /// DUAL-14-10: the shared probe report, or the typed host refusal.
    DnsLatencyProbed(
        Result<
            infiltrator_contract::dns_latency::DnsLatencyReport,
            infiltrator_contract::error::Failure,
        >,
    ),
    // Custom Node Modal & Universal URI Codec (Category 2, DUAL-05)
    OpenCustomNodeModal,
    CloseCustomNodeModal,
    UpdateCustomNodeUriInput(String),
    ParseAndImportCustomUri,
    /// DUAL-05-14: the whole shared draft after a form edit. The view builds
    /// the next draft, the update layer re-derives the shared report.
    UpdateCustomNodeDraft(Box<infiltrator_contract::protocol_fidelity::ProtocolDraft>),
    /// DUAL-05-14: re-encode the draft into a share link preview.
    ExportCustomNodeUri,
    SaveCustomNodeForm,
    CustomNodeSaved(Result<(), InfiltratorError>),
    /// DUAL-05-09/10: analyse the active profile's dialer/relay graph and
    /// publish the shared chain + loop report.
    ScanCustomNodeDialer,
    /// DUAL-05-09/10: the shared analyzer's report (published for both surfaces).
    CustomNodeDialerScanned(
        Result<infiltrator_contract::dialer_chain::DialerChainReport, InfiltratorError>,
    ),
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
    AggregationPreviewFinished(
        Result<infiltrator_contract::aggregator::AggregationReport, InfiltratorError>,
    ),
    /// DUAL-08-06/08-12: save the preview as a brand new independent profile,
    /// optionally activating it through the shared activation path.
    CreateAggregatedProfile,
    AggregatedProfileCreated(
        Result<infiltrator_contract::aggregator::AggregatedProfileOutcome, InfiltratorError>,
    ),
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
        Result<Vec<infiltrator_contract::aggregator::AggregationTemplate>, InfiltratorError>,
    ),
    /// DUAL-08-13: prefill the wizard from a saved template.
    ApplyAggregatorTemplate(String),
    /// DUAL-08-13: persist the current draft under the typed template name.
    SaveAggregatorTemplate,
    /// DUAL-08-13: template name input.
    UpdateAggregatorTemplateName(String),
    AggregatorTemplateSaved(Result<String, InfiltratorError>),
    /// DUAL-08-13: delete a saved template.
    DeleteAggregatorTemplate(String),
    AggregatorTemplateDeleted(Result<(String, bool), InfiltratorError>),
    /// DUAL-08-07: re-read the sources and refresh the generated profile.
    ReAggregateProfile(String),
    AggregationReaggregated(
        Result<infiltrator_contract::aggregator::AggregatedProfileOutcome, InfiltratorError>,
    ),
    // Connection Grouping & Quick Rule (Category 4)
    SetConnectionGroupingMode(infiltrator_domain::connection_view::ConnectionGroupingMode),
    AddQuickRuleFromConnection {
        pattern: String,
        target: String,
    },
    // Config Snapshot Visual Diff & Rollback (Category 5)
    OpenSnapshotDiff(String),
    CloseSnapshotDiff,
    /// DUAL-09-08: the shared application finished computing the diff.
    SnapshotDiffLoaded(
        Result<infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot, InfiltratorError>,
    ),
    /// DUAL-09-08: inline vs split layout for the same shared diff.
    SetSnapshotDiffMode(crate::types::app::SnapshotDiffMode),
    /// DUAL-09-14: recompute the open diff from the shared snapshot
    /// application (the Bevy card's 「刷新差异」 is the same action).
    RefreshSnapshotDiff,
    /// DUAL-09-09: arm the two-step rollback confirmation.
    ArmSnapshotRollback,
    CancelSnapshotRollback,
    RollbackToSnapshot(String),
    /// DUAL-09-12: allow direct edits of a protected remote subscription for
    /// this session (the application still receives the explicit flag).
    SetProfileProtectionOverride(bool),
    // Global Hotkey Manager (Category 6) — shared contract registry.
    BeginHotkeyCapture(infiltrator_contract::shortcuts::ShortcutAction),
    CancelHotkeyCapture,
    /// One raw key press forwarded from the window: capture and dispatch are
    /// resolved against the live registry in `update`.
    KeyboardChord {
        key: String,
        modifiers: infiltrator_contract::shortcuts::KeyModifiers,
    },
    ToggleHotkeyEnabled(infiltrator_contract::shortcuts::ShortcutAction),
    ResetHotkey(infiltrator_contract::shortcuts::ShortcutAction),
    ShortcutsUpdated(Result<infiltrator_contract::shortcuts::ShortcutRegistry, String>),
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
    RunNodeSpeedtest(String),
    /// Result of a real speedtest probe from the host port. `Err` carries the
    /// typed port failure (e.g. unsupported host) so the UI can show it.
    SpeedtestSnapshotUpdated(
        Result<
            infiltrator_contract::speedtest::SpeedtestSnapshot,
            infiltrator_ports::error::PortError,
        >,
    ),
    // Wave 3 Category 4: Geo Data Updater
    CheckGeoDataUpdates,
    TriggerGeoDataUpdate,
    GeoDataUpdateResult(Result<(), String>),
    // Wave 3 Category 5: Windows UWP Loopback Utility
    ScanUwpApps,
    UwpAppsLoaded(Vec<super::app::UwpAppItem>),
    UwpSnapshotLoaded(infiltrator_contract::uwp::UwpLoopbackSnapshot),
    ExemptAllUwpApps,
    ClearAllUwpExemptions,
    ToggleUwpAppExemption(String),
    UwpExemptionsChanged(Result<infiltrator_contract::uwp::UwpLoopbackSnapshot, InfiltratorError>),
    // Wave 3 Category 6: Encrypted Backup Package
    UpdateEncryptedBackupPassphrase(String),
    ExportEncryptedPackage,
    ImportEncryptedPackage,
    // Wave 4 Category 1: Network Interface Roaming
    PollNetworkInterfaces,
    NetworkInterfacesPolled(infiltrator_contract::network_roaming::NetworkRoamingSnapshot),
    ForceGatewayReconnect,
    NetworkRoamingRepaired(
        Result<infiltrator_contract::network_roaming::NetworkRoamingSnapshot, InfiltratorError>,
    ),
    StartVpn,
    StopVpn,
    VpnSessionUpdated(Result<infiltrator_contract::vpn::VpnSessionSnapshot, InfiltratorError>),
    RunPrivilegedNetworkRegression,
    PrivilegedNetworkRegressionUpdated(
        Result<
            infiltrator_contract::privileged_network::PrivilegedNetworkSnapshot,
            InfiltratorError,
        >,
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
    EvaluateSubscriptionQuota,
    UpdateCronScheduleHours(u32),
    // Wave 4 Category 6: PAC Auto-Proxy & Bypass CIDR
    UpdatePacBypassSubnets(String),
    CompileAndValidatePac,
    PacApplied(Result<infiltrator_contract::pac::PacSnapshot, InfiltratorError>),
    TogglePacMode(bool),
    // Wave 5 Category 1: Rule Hit Counter & Stale Rule Audit
    AuditStaleRules,
    DisableZeroHitRules,
    ClearRuleHitCounters,
    // Wave 5 Category 2: Latency Time-Series & Stability Radar
    SelectRadarNode(String),
    RecordRadarLatencySample {
        node: String,
        latency_ms: u64,
    },
    // Wave 5 Category 3: TUN Multi-Stack & MTU Negotiator
    SelectTunStack(String),
    ProbeOptimalMtu,
    MtuProbed(u32),
    MtuProbeFinished(MtuProbeCompletion),
    // Wave 5 Category 4: Rule-Provider Lifecycle & Rule Unpacker
    UnpackRuleProviderToCustom(String),
    PurgeRuleProviderCache,
    /// DUAL-11-06: the real provider rules a host read produced.
    RuleProviderUnpacked(
        Result<
            infiltrator_application::rule_provider_application::ProviderUnpackPlan,
            InfiltratorError,
        >,
    ),
    /// DUAL-11-07: the real files/bytes a host purge removed.
    RuleProviderCachePurged(
        Result<infiltrator_contract::provider_cache::ProviderCachePurge, InfiltratorError>,
    ),
    // Wave 5 Category 5: Config Apply Multi-Stage Transaction Guard
    TriggerAtomicConfigApply,
    ApplyTransactionStageChanged(super::runtime::ApplyTransactionStage),
    // Wave 5 Category 6: LAN Proxy Sharing & Client Access Whitelist
    ToggleLanSharing(bool),
    UpdateLanSharingPort(u16),
    UpdateLanBindAddress(String),
    UpdateLanAclWhitelist(String),
    ApplyLanSharing,
    LanSharingSet(
        Result<infiltrator_contract::lan::LanSharingSnapshot, InfiltratorError>,
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
        Result<infiltrator_contract::lan::LanSecuritySnapshot, InfiltratorError>,
        u64,
    ),
}

mod debug;

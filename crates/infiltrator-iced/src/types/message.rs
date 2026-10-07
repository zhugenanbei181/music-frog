//! The application-wide [`Message`] bus plus its compact `Debug`
//! implementation used by tracing and tests.

use super::app::{
    ConfirmAction, CoreDownloadProgress, Route, SyncProgress, SyncSummary, ToastStatus, UwpAppItem,
};
use super::dns::{AdvancedConfigsBundle, AdvancedEditMode, DnsTab};
use super::runtime::{IpProbeResult, RuntimeStreamKind, RuntimeStreamState};
use crate::admin_server::AdminHostCommand;
use crate::tray::spec::TrayEvent;
use crate::types::app::SnapshotDiffMode;
use crate::types::dns_query::QueryAction;
use crate::types::options::{EditorPane, MrsProviderDetail, SyncDiffBundle};
use crate::types::profile_edit::{
    ProfileDocumentReadReply, ProfileEditReply, ProfileOptionsReadReply,
};
use crate::types::rule_list::RuleListAction;
use crate::types::rule_trace::RuleTraceAction;
use crate::types::script::ScriptAction;
use crate::types::snapshot_restore::RestoreAction;
use iced::Rectangle;
use iced::widget::text_editor;
use iced::window;
use iced::window::Screenshot;
use infiltrator_application::proxy_mode_actions::PendingModeChange;
use infiltrator_application::rule_provider_application::ProviderUnpackPlan;
use infiltrator_application::rule_statistics_workbench::StatisticsAction;
use infiltrator_contract::aggregator::{
    AggregatedProfileOutcome, AggregationReport, AggregationTemplate,
};
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::command_catalogue::CommandTarget;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_control::CoreControlAction;
use infiltrator_contract::dialer_chain::DialerChainReport;
use infiltrator_contract::dns::{DnsEnhancedMode, DnsFakeIpFilterMode};
use infiltrator_contract::dns_latency::DnsLatencyReport;
use infiltrator_contract::dns_leak::DnsLeakReport;
use infiltrator_contract::doctor::{BootstrapReport, DoctorFixReport, DoctorReport};
use infiltrator_contract::error::{Failure, InfiltratorError};
use infiltrator_contract::ime::ImeCompositionEvent;
use infiltrator_contract::lan::{LanSecuritySnapshot, LanSharingSnapshot};
use infiltrator_contract::mini_hud::MiniHudPlacement;
use infiltrator_contract::mtu::MtuNegotiationSnapshot;
use infiltrator_contract::network_roaming::NetworkRoamingSnapshot;
use infiltrator_contract::overview_layout::OverviewCardKind;
use infiltrator_contract::pac::PacSnapshot;
use infiltrator_contract::port_conflict::PortConflictSnapshot;
use infiltrator_contract::privileged_network::PrivilegedNetworkSnapshot;
use infiltrator_contract::protocol_fidelity::ProtocolDraft;
use infiltrator_contract::protocol_form::ProtocolField;
use infiltrator_contract::provider_cache::ProviderCachePurge;
use infiltrator_contract::rule_document::RuleDocumentSnapshot;
use infiltrator_contract::rule_document::RuleRowId;
use infiltrator_contract::rules_workspace::{RulesJsonSection, RulesTab};
use infiltrator_contract::service_mode::ServiceModeSnapshot;
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::shortcuts::{KeyModifiers, ShortcutAction, ShortcutRegistry};
use infiltrator_contract::snapshot::CoreSnapshot;
use infiltrator_contract::snapshot_history::{SnapshotHistorySnapshot, SnapshotPruneReport};
use infiltrator_contract::speedtest::SpeedtestSnapshot;
use infiltrator_contract::stun_probe::StunProbeReport;
use infiltrator_contract::subscription_filter_form::FilterObservation;
use infiltrator_contract::subscription_filter_result::SubscriptionFilterApplied;
use infiltrator_contract::subscription_import::{
    SubscriptionBatchReport, SubscriptionUpdateReport,
};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::system_proxy::{SystemProxyRecoverySnapshot, SystemProxySnapshot};
use infiltrator_contract::uwp::UwpLoopbackSnapshot;
use infiltrator_contract::version::InstalledCoreVersion;
use infiltrator_contract::vpn::VpnSessionSnapshot;
use infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot;
use infiltrator_desktop::process_enumerator::{ExtendedProcessInfo, ProcessCategory};
use infiltrator_domain::app_routing::AppRoutingConfig;
use infiltrator_domain::app_routing::{AppRoutingMode, AppRoutingRule};
use infiltrator_domain::connection_view::ConnectionGroupingMode;
use infiltrator_domain::profiles::ProfileInfo;
use infiltrator_domain::proxy::Proxy;
use infiltrator_domain::rules::RuleProviderDiff;
use infiltrator_domain::runtime::ConfigSnapshot;
use infiltrator_domain::runtime::{
    ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider, TrafficData,
};
use infiltrator_domain::settings::AppSettings;
use infiltrator_ports::error::PortError;
use infiltrator_ports::host_runtime::{HostRuntime, TunServiceStatus};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

/// Shared batch report of the "update all subscriptions" entry. Both the tray
/// and the Profiles toolbar consume the same application-produced counts.
pub type SubscriptionUpdateOutcomes = Result<SubscriptionBatchReport, InfiltratorError>;

#[derive(Clone)]
pub struct MtuProbeCompletion {
    pub snapshot: MtuNegotiationSnapshot,
    pub generation: u64,
    pub session_token: Option<SessionToken>,
}

#[derive(Clone)]
pub enum Message {
    Noop,
    /// Shared 11-page application snapshot delivered by a host source.
    SurfaceSnapshotUpdated(Box<SurfaceSnapshot>),
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
    ImeComposition(ImeCompositionEvent),
    Navigate(Route),
    NavigateBack,
    NavigateForward,
    StartProxy,
    StopProxy,
    /// Core boot result; the bool reports whether the external-controller
    /// port had to be rotated during the boot retry loop.
    ProxyStarted(Result<(Arc<dyn HostRuntime>, bool), InfiltratorError>, u64),
    ProxyStopped,
    ProxyStopFinished(Result<(), Failure>, u64),
    CoreControlFinished {
        action: CoreControlAction,
        token: u64,
        result: Result<(), Failure>,
        snapshot: Box<CoreSnapshot>,
    },
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
    ProxiesLoadedForSession {
        generation: u64,
        session_token: Option<SessionToken>,
        result: Result<HashMap<String, Proxy>, InfiltratorError>,
    },
    SelectProxy(String, String),
    FilterProxies(String),
    ProxyPreferenceFinished {
        token: u64,
        result: Result<(), Failure>,
    },
    ProxySearchFinished {
        token: u64,
        query: String,
        result: Result<(), Failure>,
    },
    ToggleFilterAlive(bool),
    ToggleFavoriteProxy(String),
    ToggleProxyCompactView,
    InspectProxy(Option<String>),
    TestInspectedProxy,
    ProxyInspectionProbed {
        name: String,
        token: u64,
        result: Result<(), Failure>,
    },
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
    OpenProxyProbeOptions,
    ApplyProxyProbeOptions,
    CancelProxyProbeOptions,
    ProxyProbeOptionsApplied {
        token: u64,
        result: Result<(), Failure>,
    },
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
    ConfirmLogExport,
    CancelLogExport,
    RetryLogExport,
    LogExportFinished {
        operation: u64,
        result: Result<CommandOutput, Failure>,
    },
    ToggleLogFollow,
    LogsScrolled {
        offset: f32,
        content: f32,
        viewport: f32,
    },
    LogsCommandFinished {
        generation: u64,
        result: Result<(), Failure>,
    },
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
    RuntimeConfigFetched(Result<ConfigSnapshot, Failure>, u64),
    RuntimeConfigReadFinished {
        result: Result<ConfigSnapshot, Failure>,
        generation: u64,
        mode_epoch: u64,
    },
    SetProxyMode(String),
    SetIpv6Routing(bool),
    SetTunEnabled(bool),
    InstallTunService,
    RefreshTunServiceStatus,
    TunServiceStatusLoaded(Result<TunServiceStatus, InfiltratorError>),
    TunServiceInstalled(Result<(), InfiltratorError>),
    ServiceModePrepared(Result<ServiceModeSnapshot, InfiltratorError>),
    RepairPortConflicts,
    PortConflictsRepaired(Result<PortConflictSnapshot, InfiltratorError>),
    SetTunStack(String),
    SetTunAutoRoute(bool),
    SetTunStrictRoute(bool),
    SetSnifferEnabled(bool),
    ProxyModeFinished {
        request: PendingModeChange,
        result: Result<ProxyMode, Failure>,
    },
    RetryProxyMode,
    DismissProxyModeFailure,
    RuntimePatchResult(Result<(), InfiltratorError>, u64, u64),
    OperationResult(Result<(), InfiltratorError>),
    LoadRules,
    RulesLoaded(Result<RuleDocumentSnapshot, InfiltratorError>),
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
    UpdateFilteredGroups,
    UpdateNewRuleType(String),
    UpdateNewRulePayload(String),
    UpdateNewRuleTarget(String),
    AddCustomRule,
    ToggleRuleEnabled(RuleRowId),
    MoveRuleUp(RuleRowId),
    MoveRuleDown(RuleRowId),
    SaveRules,
    ApplyGameRoutingPresets,
    UpdateGeoDatabases,
    GeoDatabasesUpdated(Result<(), InfiltratorError>),
    InspectRuleProviderDiff(Option<String>),
    UnpackRuleProvider(String),
    RuleProviderDiffLoaded(Result<RuleProviderDiff, InfiltratorError>),
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
    UpdateDnsFormEnhancedMode(DnsEnhancedMode),
    UpdateDnsFormFilterMode(DnsFakeIpFilterMode),
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
        result: Result<(), Failure>,
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
    AdminHostCommand(AdminHostCommand),
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
    SystemProxySet(Result<SystemProxySnapshot, InfiltratorError>),
    SystemProxyReconciled(SystemProxySnapshot),
    SystemProxyRecoveryFinished(SystemProxyRecoverySnapshot),
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
    ProfileSnapshotsLoaded(Result<SnapshotHistorySnapshot, InfiltratorError>),
    /// DUAL-09-06: write a manual snapshot of the edited profile now.
    BackupProfileSnapshot,
    ProfileSnapshotBackedUp(Result<(), InfiltratorError>),
    /// DUAL-09-07: select the retention the manual prune keeps.
    SetSnapshotPruneKeep(usize),
    /// DUAL-09-07: run the shared dedupe+LRU prune with the selected retention.
    PruneProfileSnapshots,
    ProfileSnapshotsPruned(Result<SnapshotPruneReport, InfiltratorError>),
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
    ToggleMixinPreset(String, bool),
    // Profile options: subscription filter editor (Profiles page card).
    LoadProfileFilter,
    ProfileFilterLoaded {
        token: u64,
        profile: String,
        result: Result<FilterObservation, Failure>,
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
        result: Result<SubscriptionFilterApplied, Failure>,
    },
    DiscardProfileFilter,
    // MRS rule-provider detail scan (Rules page providers tab).
    ScanMrsProviders,
    MrsDetailsReady(Result<Vec<MrsProviderDetail>, InfiltratorError>),
    // Sync conflict key-level diff merge (Sync page).
    LoadSyncDiff(String),
    SyncDiffLoaded(Result<SyncDiffBundle, InfiltratorError>),
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
    CoreDownloadFinished(Result<String, Failure>, u64),
    CancelCoreDownload,
    DeleteKernel(String),
    SetDefaultKernel(String),
    RollbackCore,
    KernelOperationFinished(Result<(), InfiltratorError>),
    FactoryReset,
    FactoryResetFinished(Result<(), InfiltratorError>),
    OpenConfigDir,
    FlushFakeIpCache,
    ConfirmDnsCacheFlush,
    CancelDnsCacheFlush,
    RetryDnsCacheFlush,
    DnsCacheCommandFinished {
        token: u64,
        result: Result<(), Failure>,
    },
    TestProxyDelay(String),
    TestGroupDelay(String),
    ProxyTested(String, Result<u64, InfiltratorError>),
    ProxyDelayCompleted {
        name: String,
        result: Result<(), Failure>,
    },
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
    RetryLanguageChoice,
    LanguageChoiceApplied {
        token: u64,
        result: Result<(), Failure>,
    },
    ShowToast(String, ToastStatus),
    RemoveToast(u64),
    TestAllProxyDelays,
    /// Result of a scope-wide speedtest (all groups or one group) driven
    /// through the shared engine port. `Err` carries the typed port failure.
    SpeedtestScopeUpdated(Result<SpeedtestSnapshot, PortError>),
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
    MoveOverviewCardUp(OverviewCardKind),
    /// Move one Overview card one slot down in the shared layout order.
    MoveOverviewCardDown(OverviewCardKind),
    /// Reset the Overview card order to the canonical default.
    ResetOverviewCardOrder,
    // ui-wave2-p: proxies page — expand/collapse one proxy-group card
    // (view-only UI state; uses the shared ProxyUiPreferences expansion model).
    ToggleProxyGroupExpanded(String),
    // Doctor 体检面板（诊断域）：经内嵌 admin server 的 loopback HTTP 调用
    // /admin/api/doctor* 与 /admin/api/bootstrap，报告写回 diag.doctor。
    RunDoctor,
    DoctorReportReady(Result<DoctorReport, InfiltratorError>),
    DoctorCommandFinished {
        token: u64,
        result: Result<(), Failure>,
    },
    RetryDoctorCommand,
    RunDoctorFix,
    RepairDoctorIssue(String),
    DoctorFixApplied(Result<DoctorFixReport, InfiltratorError>),
    RunBootstrap,
    BootstrapFinished(Result<BootstrapReport, InfiltratorError>),
    // Command Palette
    ToggleCommandPalette,
    OpenCommandPalette,
    CloseCommandPalette,
    SetCommandQuery(String),
    SelectNextCommand,
    SelectPrevCommand,
    ExecuteCommand(CommandTarget),
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
    AppRoutingConfigLoaded(Result<AppRoutingConfig, InfiltratorError>),
    AppRoutingPersisted(Result<(), InfiltratorError>),
    SetAppRoutingFilter(String),
    SetAppRoutingMode(AppRoutingMode),
    SetAppRouteRule {
        process: String,
        rule: AppRoutingRule,
    },
    SetAppRoutingCategory(Option<ProcessCategory>),
    // Proxy Group Reorder (策略组重排)
    MoveProxyGroupUp(String),
    MoveProxyGroupDown(String),
    ResetProxyGroupOrder,
    OpenProxyGroupOrder,
    ApplyProxyGroupOrder,
    CancelProxyGroupOrder,
    ProxyGroupOrderApplied {
        token: u64,
        result: Result<(), Failure>,
    },
    // Mini HUD Mode (迷你网速悬浮窗)
    ToggleMiniHudMode,
    SetAlwaysOnTop(bool),
    MiniHudMoved {
        x: f32,
        y: f32,
    },
    MiniHudDragReleased,
    MiniHudPlacementUpdated(Result<MiniHudPlacement, String>),
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
        result: Result<(), Failure>,
    },
    /// DUAL-14-08: the shared cross-source leak report, or the typed host
    /// refusal.
    DnsLeakProbed(Result<DnsLeakReport, Failure>),
    /// DUAL-14-09 (re-scoped): probe this host/process's UDP egress mapping
    /// through the configured STUN server. Not a browser WebRTC measurement.
    RunStunProbe,
    /// DUAL-14-09 (re-scoped): the shared STUN egress report, or the typed host
    /// refusal.
    StunProbed(Result<StunProbeReport, Failure>),
    /// DUAL-14-10: measure every configured nameserver through the shared
    /// host prober.
    DnsQuery(QueryAction),
    RuleTrace(RuleTraceAction),
    RuleStatistics(StatisticsAction),
    RuleList(RuleListAction),
    RunDnsLatencyProbe,
    /// DUAL-14-10: the shared probe report, or the typed host refusal.
    DnsLatencyProbed(Result<DnsLatencyReport, Failure>),
    // Custom Node Modal & Universal URI Codec (Category 2, DUAL-05)
    OpenCustomNodeModal,
    CloseCustomNodeModal,
    UpdateCustomNodeUriInput(String),
    ParseAndImportCustomUri,
    /// DUAL-05-14: the whole shared draft after a form edit. The view builds
    /// the next draft, the update layer re-derives the shared report.
    UpdateCustomNodeDraft(Box<ProtocolDraft>),
    UpdateCustomNodeField(ProtocolField, String),
    /// DUAL-05-14: re-encode the draft into a share link preview.
    ExportCustomNodeUri,
    SaveCustomNodeForm,
    CustomNodeSaved(Result<(), InfiltratorError>),
    /// DUAL-05-09/10: analyse the active profile's dialer/relay graph and
    /// publish the shared chain + loop report.
    ScanCustomNodeDialer,
    /// DUAL-05-09/10: the shared analyzer's report (published for both surfaces).
    CustomNodeDialerScanned(Result<DialerChainReport, InfiltratorError>),
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
    AggregationPreviewFinished(Result<AggregationReport, InfiltratorError>),
    /// DUAL-08-06/08-12: save the preview as a brand new independent profile,
    /// optionally activating it through the shared activation path.
    CreateAggregatedProfile,
    AggregatedProfileCreated(Result<AggregatedProfileOutcome, InfiltratorError>),
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
    AggregatorTemplatesLoaded(Result<Vec<AggregationTemplate>, InfiltratorError>),
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
    AggregationReaggregated(Result<AggregatedProfileOutcome, InfiltratorError>),
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
    SnapshotDiffLoaded(Result<YamlAstDiffSnapshot, InfiltratorError>),
    /// DUAL-09-08: inline vs split layout for the same shared diff.
    SetSnapshotDiffMode(SnapshotDiffMode),
    /// DUAL-09-14: recompute the open diff from the shared snapshot
    /// application (the Bevy card's 「刷新差异」 is the same action).
    RefreshSnapshotDiff,
    /// DUAL-09-09: arm the two-step rollback confirmation.
    RollbackToSnapshot(String),
    /// DUAL-09-12: allow direct edits of a protected remote subscription for
    /// this session (the application still receives the explicit flag).
    SetProfileProtectionOverride(bool),
    // Global Hotkey Manager (Category 6) — shared contract registry.
    BeginHotkeyCapture(ShortcutAction),
    CancelHotkeyCapture,
    /// One raw key press forwarded from the window: capture and dispatch are
    /// resolved against the live registry in `update`.
    KeyboardChord {
        key: String,
        modifiers: KeyModifiers,
    },
    ToggleHotkeyEnabled(ShortcutAction),
    ResetHotkey(ShortcutAction),
    ShortcutsUpdated(Result<ShortcutRegistry, String>),
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
    SpeedtestSnapshotUpdated(Result<SpeedtestSnapshot, PortError>),
    // Wave 3 Category 4: Geo Data Updater
    CheckGeoDataUpdates,
    TriggerGeoDataUpdate,
    GeoDataUpdateResult(Result<(), String>),
    // Wave 3 Category 5: Windows UWP Loopback Utility
    ScanUwpApps,
    UwpAppsLoaded(Vec<UwpAppItem>),
    UwpSnapshotLoaded(UwpLoopbackSnapshot),
    ExemptAllUwpApps,
    ClearAllUwpExemptions,
    ToggleUwpAppExemption(String),
    UwpExemptionsChanged(Result<UwpLoopbackSnapshot, InfiltratorError>),
    // Wave 3 Category 6: Encrypted Backup Package
    UpdateEncryptedBackupPassphrase(String),
    ExportEncryptedPackage,
    ImportEncryptedPackage,
    // Wave 4 Category 1: Network Interface Roaming
    PollNetworkInterfaces,
    NetworkInterfacesPolled(NetworkRoamingSnapshot),
    ForceGatewayReconnect,
    NetworkRoamingRepaired(Result<NetworkRoamingSnapshot, InfiltratorError>),
    StartVpn,
    StopVpn,
    VpnSessionUpdated(Result<VpnSessionSnapshot, InfiltratorError>),
    RunPrivilegedNetworkRegression,
    PrivilegedNetworkRegressionUpdated(Result<PrivilegedNetworkSnapshot, InfiltratorError>),
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
    PacApplied(Result<PacSnapshot, InfiltratorError>),
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
    RuleProviderUnpacked(Result<ProviderUnpackPlan, InfiltratorError>),
    /// DUAL-11-07: the real files/bytes a host purge removed.
    RuleProviderCachePurged(Result<ProviderCachePurge, InfiltratorError>),
    // Wave 5 Category 5: Config Apply Multi-Stage Transaction Guard
    // Wave 5 Category 6: LAN Proxy Sharing & Client Access Whitelist
    ToggleLanSharing(bool),
    UpdateLanSharingPort(u16),
    UpdateLanBindAddress(String),
    UpdateLanAclWhitelist(String),
    ApplyLanSharing,
    LanSharingSet(Result<LanSharingSnapshot, InfiltratorError>, u64),
    UpdateLanAllowedIps(String),
    UpdateLanDisallowedIps(String),
    UpdateLanSkipAuthPrefixes(String),
    ToggleLanAuthentication(bool),
    UpdateLanAuthUsername(String),
    UpdateLanAuthPassword(String),
    ApplyLanSecurity,
    LanSecuritySet(Result<LanSecuritySnapshot, InfiltratorError>, u64),
}

mod debug;

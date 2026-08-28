use iced::{widget::text_editor, window};
pub use infiltrator_core::error::InfiltratorError;
use infiltrator_core::rules::RuleEntry;
use infiltrator_desktop::MihomoRuntime;
use mihomo_api::{ConnectionSnapshot, TrafficData};
use mihomo_config::Profile;
use mihomo_version::manager::VersionInfo;
use muda::MenuEvent;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tray_icon::TrayIconEvent;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Route {
    #[default]
    Overview,
    Profiles,
    Proxies,
    Runtime,
    Rules,
    Dns,
    Sync,
    Editor,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToastStatus {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    pub previous_route: Option<Route>,
    pub start_time: Option<Instant>,
    pub duration: std::time::Duration,
}

impl Default for Transition {
    fn default() -> Self {
        Self {
            previous_route: None,
            start_time: None,
            duration: std::time::Duration::from_millis(300),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum RuntimeStatus {
    #[default]
    Stopped,
    Starting,
    Running,
    Error(InfiltratorError),
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeConfig {
    pub mode: String,
    pub tun_enabled: bool,
    pub dns_nameservers: Vec<String>,
    pub dns_fallback: Vec<String>,
    pub dns_enhanced_mode: String,
    pub tun_stack: String,
    pub tun_auto_route: bool,
    pub tun_strict_route: bool,
    pub sniffer_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum RebuildFlowState {
    #[default]
    Idle,
    Saving {
        label: String,
    },
    Rebuilding {
        label: String,
    },
    Done {
        label: String,
    },
    Failed {
        label: String,
        error: String,
    },
}

#[derive(Debug, Clone, Default)]
pub struct RulesLoadBundle {
    pub rules: Vec<RuleEntry>,
    pub rule_providers_json: String,
    pub proxy_providers_json: String,
    pub sniffer_json: String,
}

#[derive(Debug, Clone, Default)]
pub struct AdvancedConfigsBundle {
    pub dns_json: String,
    pub fake_ip_json: String,
    pub tun_json: String,
    pub dns: infiltrator_core::dns::DnsConfig,
    pub fake_ip: infiltrator_core::fake_ip::FakeIpConfig,
    pub tun: infiltrator_core::tun::TunConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RulesTab {
    #[default]
    RulesList,
    Providers,
    JsonEditors,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RulesJsonTab {
    #[default]
    RuleProviders,
    ProxyProviders,
    Sniffer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DnsTab {
    #[default]
    Dns,
    FakeIp,
    Tun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AdvancedEditMode {
    #[default]
    Form,
    Json,
}

#[derive(Debug, Clone, Default)]
pub struct DnsFormDraft {
    pub enable: bool,
    pub nameserver: String,
    pub fallback: String,
    pub enhanced_mode: String,
    pub fake_ip_range: String,
    pub fake_ip_filter: String,
    pub ipv6: bool,
    pub cache: bool,
    pub use_hosts: bool,
    pub use_system_hosts: bool,
    pub respect_rules: bool,
    pub proxy_server_nameserver: String,
    pub direct_nameserver: String,
}

#[derive(Debug, Clone, Default)]
pub struct FakeIpFormDraft {
    pub fake_ip_range: String,
    pub fake_ip_filter: String,
    pub store_fake_ip: bool,
}

#[derive(Debug, Clone, Default)]
pub struct TunFormDraft {
    pub enable: bool,
    pub stack: String,
    pub mtu: String,
    pub dns_hijack: String,
    pub auto_route: bool,
    pub auto_detect_interface: bool,
    pub strict_route: bool,
}

#[derive(Debug, Clone, Default)]
pub struct AdvancedValidationState {
    pub dns: Option<String>,
    pub fake_ip: Option<String>,
    pub tun: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorLazyState {
    #[default]
    Unloaded,
    Loaded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuleBadgeKind {
    Domain,
    Ip,
    #[default]
    Other,
}

#[derive(Debug, Clone, Default)]
pub struct RuleRenderItem {
    pub source_index: usize,
    pub rule_type: String,
    pub payload: String,
    pub target: String,
    pub search_key: String,
    pub badge: RuleBadgeKind,
}

#[derive(Debug, Clone, Default)]
pub struct PerfSnapshot {
    pub navigate_to_first_paint_ms: Option<u128>,
    pub rules_cache_build_ms: u128,
    pub rules_with_text_apply_ms: u128,
    pub dns_with_text_apply_ms: u128,
    pub rules_visible_rows: usize,
}

#[derive(Clone)]
pub enum Message {
    Noop,
    Navigate(Route),
    StartProxy,
    StopProxy,
    ProxyStarted(Result<Arc<MihomoRuntime>, InfiltratorError>),
    ProxyStopped,
    SettingsLoaded(Result<infiltrator_core::settings::AppSettings, InfiltratorError>),
    LoadProfiles,
    ProfilesLoaded(Result<Vec<Profile>, InfiltratorError>),
    SetActiveProfile(String),
    UpdateImportUrl(String),
    UpdateImportName(String),
    UpdateImportActivate(bool),
    ImportProfile,
    ProfileImported(Result<(), InfiltratorError>),
    DeleteProfile(String),
    ProfileDeleted(Result<(), InfiltratorError>),
    UpdateLocalImportPath(String),
    BrowseLocalImportFile,
    LocalImportFilePicked(Option<PathBuf>),
    UpdateLocalImportName(String),
    UpdateLocalImportActivate(bool),
    ImportLocalProfile,
    LocalProfileImported(Result<(), InfiltratorError>),
    SelectSubscriptionProfile(String),
    UpdateSubscriptionUrl(String),
    UpdateSubscriptionAutoUpdate(bool),
    UpdateSubscriptionInterval(String),
    SaveSubscriptionSettings,
    SubscriptionSettingsSaved(Result<(), InfiltratorError>),
    UpdateSubscriptionNow,
    SubscriptionUpdatedNow(Result<(), InfiltratorError>),
    SubscriptionAutoUpdated(Result<(Vec<String>, bool), InfiltratorError>),
    UpdateProfilesFilter(String),
    ClearProfiles,
    ProfilesCleared(Result<(), InfiltratorError>),
    LoadProxies,
    ProxiesLoaded(Result<HashMap<String, mihomo_api::Proxy>, InfiltratorError>),
    SelectProxy(String, String),
    FilterProxies(String),
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
    MemoryReceived(mihomo_api::MemoryData),
    IpInfoReceived(Result<String, InfiltratorError>, usize),
    ConnectionsReceived(ConnectionSnapshot),
    LogReceived(String),
    ClearRuntimeLogs,
    SetLogLevel(String),
    CloseConnection(String),
    CloseAllConnections,
    FetchRuntimeConfig,
    FetchIpInfo,
    RuntimeConfigFetched(Result<RuntimeConfig, InfiltratorError>),
    SetProxyMode(String),
    SetTunEnabled(bool),
    SetTunStack(String),
    SetTunAutoRoute(bool),
    SetTunStrictRoute(bool),
    SetSnifferEnabled(bool),
    ModeSetResult(Result<(), InfiltratorError>),
    OperationResult(Result<(), InfiltratorError>),
    LoadRules,
    RulesBundleLoaded(Result<RulesLoadBundle, InfiltratorError>),
    RulesLoaded(Result<Vec<RuleEntry>, InfiltratorError>),
    SetRulesTab(RulesTab),
    SetRulesJsonTab(RulesJsonTab),
    ToggleRulesProvidersExpanded,
    RulesPrevPage,
    RulesNextPage,
    RulesSetPage(usize),
    EnsureRuleProvidersEditorLoaded,
    EnsureProxyProvidersEditorLoaded,
    EnsureSnifferEditorLoaded,
    ActivateRulesHeavyView,
    RuleProvidersJsonLoaded(Result<String, InfiltratorError>),
    ProxyProvidersJsonLoaded(Result<String, InfiltratorError>),
    SnifferJsonLoaded(Result<String, InfiltratorError>),
    LoadProviders,
    ProvidersLoaded(
        Result<
            (
                Vec<mihomo_api::ProxyProvider>,
                Vec<mihomo_api::RuleProvider>,
            ),
            InfiltratorError,
        >,
    ),
    UpdateProxyProvider(String),
    UpdateRuleProvider(String),
    FilterRules(String),
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
    RulesSaved(Result<(), InfiltratorError>),
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
    AdvancedConfigsBundleLoaded(Result<AdvancedConfigsBundle, InfiltratorError>),
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
    UpdateDnsFormNameserver(String),
    UpdateDnsFormFallback(String),
    UpdateDnsFormEnhancedMode(String),
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
    TrayIconEvent(TrayIconEvent),
    MenuEvent(MenuEvent),
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
    UpdateWebDavEnabled(bool),
    UpdateWebDavUrl(String),
    UpdateWebDavUser(String),
    UpdateWebDavPass(String),
    UpdateWebDavSyncInterval(String),
    UpdateWebDavSyncOnStartup(bool),
    SaveAppSettings,
    AppSettingsSaved(Result<(), InfiltratorError>),
    UpdateEditorPathSetting(String),
    SyncUpload,
    SyncDownload,
    SyncFinished(Result<(), InfiltratorError>),
    SetSystemProxy(bool),
    SystemProxySet(Result<(), InfiltratorError>),
    RequestAdminPrivilege,
    EditProfile(PathBuf),
    ProfileContentLoaded(Result<(PathBuf, String), InfiltratorError>),
    EditorAction(text_editor::Action),
    SaveProfile,
    ProfileSaved(Result<(), InfiltratorError>),
    LoadKernels,
    KernelsLoaded(Result<Vec<VersionInfo>, InfiltratorError>),
    CheckCoreUpdate,
    CoreUpdateInfo(Result<String, InfiltratorError>), // Latest version string
    DownloadCore(String),
    CoreDownloadProgress(f32),
    CoreDownloadFinished(Result<String, InfiltratorError>),
    DeleteKernel(String),
    SetDefaultKernel(String),
    FactoryReset,
    OpenConfigDir,
    FlushFakeIpCache,
    TestProxyDelay(String),
    TestGroupDelay(String),
    ProxyTested(String, Result<u64, InfiltratorError>),
    WindowClosed(window::Id),
    HideWindow,
    ShowWindow,
    UpdateRuntimeAutoRefresh(bool),
    RuntimePanelSettingsSaved(Result<(), InfiltratorError>),
    RuntimeRebuildFinished(Result<Arc<MihomoRuntime>, InfiltratorError>),
    ClearRebuildFlow,
    TogglePerfPanel,
    ToggleTheme,
    ShowToast(String, ToastStatus),
    RemoveToast(usize),
    TestAllProxyDelays,
    AllProxyDelaysTested(Result<(usize, usize), InfiltratorError>),
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Message::Noop => write!(f, "Noop"),
            Message::Navigate(route) => write!(f, "Navigate({:?})", route),
            Message::StartProxy => write!(f, "StartProxy"),
            Message::StopProxy => write!(f, "StopProxy"),
            Message::ProxyStarted(Ok(_)) => write!(f, "ProxyStarted(Ok)"),
            Message::ProxyStarted(Err(e)) => write!(f, "ProxyStarted(Err({:?}))", e),
            Message::ProxyStopped => write!(f, "ProxyStopped"),
            Message::SettingsLoaded(Ok(_)) => write!(f, "SettingsLoaded(Ok)"),
            Message::SettingsLoaded(Err(e)) => write!(f, "SettingsLoaded(Err({:?}))", e),
            Message::LoadProfiles => write!(f, "LoadProfiles"),
            Message::ProfilesLoaded(Ok(p)) => write!(f, "ProfilesLoaded(Ok({} profiles))", p.len()),
            Message::ProfilesLoaded(Err(e)) => write!(f, "ProfilesLoaded(Err({:?}))", e),
            Message::SetActiveProfile(name) => write!(f, "SetActiveProfile({})", name),
            Message::UpdateImportUrl(url) => write!(f, "UpdateImportUrl({})", url),
            Message::UpdateImportName(name) => write!(f, "UpdateImportName({})", name),
            Message::UpdateImportActivate(enabled) => {
                write!(f, "UpdateImportActivate({})", enabled)
            }
            Message::ImportProfile => write!(f, "ImportProfile"),
            Message::ProfileImported(Ok(_)) => write!(f, "ProfileImported(Ok)"),
            Message::ProfileImported(Err(e)) => write!(f, "ProfileImported(Err({:?}))", e),
            Message::DeleteProfile(name) => write!(f, "DeleteProfile({})", name),
            Message::ProfileDeleted(Ok(_)) => write!(f, "ProfileDeleted(Ok)"),
            Message::ProfileDeleted(Err(e)) => write!(f, "ProfileDeleted(Err({:?}))", e),
            Message::UpdateLocalImportPath(path) => write!(f, "UpdateLocalImportPath({})", path),
            Message::BrowseLocalImportFile => write!(f, "BrowseLocalImportFile"),
            Message::LocalImportFilePicked(Some(path)) => {
                write!(f, "LocalImportFilePicked(Some({:?}))", path)
            }
            Message::LocalImportFilePicked(None) => write!(f, "LocalImportFilePicked(None)"),
            Message::UpdateLocalImportName(name) => write!(f, "UpdateLocalImportName({})", name),
            Message::UpdateLocalImportActivate(enabled) => {
                write!(f, "UpdateLocalImportActivate({})", enabled)
            }
            Message::ImportLocalProfile => write!(f, "ImportLocalProfile"),
            Message::LocalProfileImported(Ok(_)) => write!(f, "LocalProfileImported(Ok)"),
            Message::LocalProfileImported(Err(e)) => {
                write!(f, "LocalProfileImported(Err({:?}))", e)
            }
            Message::SelectSubscriptionProfile(name) => {
                write!(f, "SelectSubscriptionProfile({})", name)
            }
            Message::UpdateSubscriptionUrl(url) => write!(f, "UpdateSubscriptionUrl({})", url),
            Message::UpdateSubscriptionAutoUpdate(enabled) => {
                write!(f, "UpdateSubscriptionAutoUpdate({})", enabled)
            }
            Message::UpdateSubscriptionInterval(v) => {
                write!(f, "UpdateSubscriptionInterval({})", v)
            }
            Message::SaveSubscriptionSettings => write!(f, "SaveSubscriptionSettings"),
            Message::SubscriptionSettingsSaved(Ok(_)) => {
                write!(f, "SubscriptionSettingsSaved(Ok)")
            }
            Message::SubscriptionSettingsSaved(Err(e)) => {
                write!(f, "SubscriptionSettingsSaved(Err({:?}))", e)
            }
            Message::UpdateSubscriptionNow => write!(f, "UpdateSubscriptionNow"),
            Message::SubscriptionUpdatedNow(Ok(_)) => write!(f, "SubscriptionUpdatedNow(Ok)"),
            Message::SubscriptionUpdatedNow(Err(e)) => {
                write!(f, "SubscriptionUpdatedNow(Err({:?}))", e)
            }
            Message::SubscriptionAutoUpdated(Ok((names, active_updated))) => write!(
                f,
                "SubscriptionAutoUpdated(Ok({} profiles, active_updated={}))",
                names.len(),
                active_updated
            ),
            Message::SubscriptionAutoUpdated(Err(e)) => {
                write!(f, "SubscriptionAutoUpdated(Err({:?}))", e)
            }
            Message::UpdateProfilesFilter(s) => write!(f, "UpdateProfilesFilter({})", s),
            Message::ClearProfiles => write!(f, "ClearProfiles"),
            Message::ProfilesCleared(Ok(_)) => write!(f, "ProfilesCleared(Ok)"),
            Message::ProfilesCleared(Err(e)) => write!(f, "ProfilesCleared(Err({:?}))", e),
            Message::LoadProxies => write!(f, "LoadProxies"),
            Message::ProxiesLoaded(Ok(p)) => write!(f, "ProxiesLoaded(Ok({} proxies))", p.len()),
            Message::ProxiesLoaded(Err(e)) => write!(f, "ProxiesLoaded(Err({:?}))", e),
            Message::SelectProxy(g, n) => write!(f, "SelectProxy({}, {})", g, n),
            Message::FilterProxies(s) => write!(f, "FilterProxies({})", s),
            Message::ToggleProxySort => write!(f, "ToggleProxySort"),
            Message::UpdateProxyDelaySort(s) => write!(f, "UpdateProxyDelaySort({})", s),
            Message::UpdateDelayTestUrl(s) => write!(f, "UpdateDelayTestUrl({})", s),
            Message::UpdateDelayTimeoutMs(s) => write!(f, "UpdateDelayTimeoutMs({})", s),
            Message::UpdateRuntimeSelectedGroup(s) => {
                write!(f, "UpdateRuntimeSelectedGroup({})", s)
            }
            Message::UpdateRuntimeSelectedProxy(s) => {
                write!(f, "UpdateRuntimeSelectedProxy({})", s)
            }
            Message::ApplyRuntimeSelectedProxy => write!(f, "ApplyRuntimeSelectedProxy"),
            Message::UpdateRuntimeConnectionFilter(s) => {
                write!(f, "UpdateRuntimeConnectionFilter({})", s)
            }
            Message::UpdateRuntimeConnectionSort(s) => {
                write!(f, "UpdateRuntimeConnectionSort({})", s)
            }
            Message::RefreshRuntimeNow => write!(f, "RefreshRuntimeNow"),
            Message::TrafficReceived(t) => {
                write!(f, "TrafficReceived(up: {}, down: {})", t.up, t.down)
            }
            Message::MemoryReceived(m) => write!(
                f,
                "MemoryReceived(in_use: {}, os_limit: {})",
                m.in_use, m.os_limit
            ),
            Message::IpInfoReceived(Ok(ip), id) => {
                write!(f, "IpInfoReceived(Ok({}), taskId: {})", ip, id)
            }
            Message::IpInfoReceived(Err(e), id) => {
                write!(f, "IpInfoReceived(Err({:?}), taskId: {})", e, id)
            }
            Message::ConnectionsReceived(c) => write!(
                f,
                "ConnectionsReceived({} connections)",
                c.connections.len()
            ),
            Message::LogReceived(l) => write!(f, "LogReceived({})", l),
            Message::ClearRuntimeLogs => write!(f, "ClearRuntimeLogs"),
            Message::SetLogLevel(l) => write!(f, "SetLogLevel({})", l),
            Message::CloseConnection(id) => write!(f, "CloseConnection({})", id),
            Message::CloseAllConnections => write!(f, "CloseAllConnections"),
            Message::FetchRuntimeConfig => write!(f, "FetchRuntimeConfig"),
            Message::FetchIpInfo => write!(f, "FetchIpInfo"),
            Message::RuntimeConfigFetched(Ok(config)) => {
                write!(
                    f,
                    "RuntimeConfigFetched({}, {}, {} DNS, {} FB, {}, {}, {}, {}, {})",
                    config.mode,
                    config.tun_enabled,
                    config.dns_nameservers.len(),
                    config.dns_fallback.len(),
                    config.dns_enhanced_mode,
                    config.tun_stack,
                    config.tun_auto_route,
                    config.tun_strict_route,
                    config.sniffer_enabled
                )
            }
            Message::RuntimeConfigFetched(Err(e)) => {
                write!(f, "RuntimeConfigFetched(Err({:?}))", e)
            }
            Message::SetProxyMode(m) => write!(f, "SetProxyMode({})", m),
            Message::SetTunEnabled(t) => write!(f, "SetTunEnabled({})", t),
            Message::SetTunStack(s) => write!(f, "SetTunStack({})", s),
            Message::SetTunAutoRoute(a) => write!(f, "SetTunAutoRoute({})", a),
            Message::SetTunStrictRoute(s) => write!(f, "SetTunStrictRoute({})", s),
            Message::SetSnifferEnabled(s) => write!(f, "SetSnifferEnabled({})", s),
            Message::ModeSetResult(Ok(_)) => write!(f, "ModeSetResult(Ok)"),
            Message::ModeSetResult(Err(e)) => write!(f, "ModeSetResult(Err({:?}))", e),
            Message::OperationResult(Ok(_)) => write!(f, "OperationResult(Ok)"),
            Message::OperationResult(Err(e)) => write!(f, "OperationResult(Err({:?}))", e),
            Message::LoadRules => write!(f, "LoadRules"),
            Message::RulesBundleLoaded(Ok(bundle)) => write!(
                f,
                "RulesBundleLoaded(Ok({} rules, rp:{} chars, pp:{} chars, sn:{} chars))",
                bundle.rules.len(),
                bundle.rule_providers_json.len(),
                bundle.proxy_providers_json.len(),
                bundle.sniffer_json.len()
            ),
            Message::RulesBundleLoaded(Err(e)) => write!(f, "RulesBundleLoaded(Err({:?}))", e),
            Message::RulesLoaded(Ok(r)) => write!(f, "RulesLoaded(Ok({} rules))", r.len()),
            Message::RulesLoaded(Err(e)) => write!(f, "RulesLoaded(Err({:?}))", e),
            Message::SetRulesTab(tab) => write!(f, "SetRulesTab({:?})", tab),
            Message::SetRulesJsonTab(tab) => write!(f, "SetRulesJsonTab({:?})", tab),
            Message::ToggleRulesProvidersExpanded => write!(f, "ToggleRulesProvidersExpanded"),
            Message::RulesPrevPage => write!(f, "RulesPrevPage"),
            Message::RulesNextPage => write!(f, "RulesNextPage"),
            Message::RulesSetPage(page) => write!(f, "RulesSetPage({})", page),
            Message::EnsureRuleProvidersEditorLoaded => {
                write!(f, "EnsureRuleProvidersEditorLoaded")
            }
            Message::EnsureProxyProvidersEditorLoaded => {
                write!(f, "EnsureProxyProvidersEditorLoaded")
            }
            Message::EnsureSnifferEditorLoaded => write!(f, "EnsureSnifferEditorLoaded"),
            Message::ActivateRulesHeavyView => write!(f, "ActivateRulesHeavyView"),
            Message::RuleProvidersJsonLoaded(Ok(json)) => {
                write!(f, "RuleProvidersJsonLoaded(Ok({} chars))", json.len())
            }
            Message::RuleProvidersJsonLoaded(Err(e)) => {
                write!(f, "RuleProvidersJsonLoaded(Err({:?}))", e)
            }
            Message::ProxyProvidersJsonLoaded(Ok(json)) => {
                write!(f, "ProxyProvidersJsonLoaded(Ok({} chars))", json.len())
            }
            Message::ProxyProvidersJsonLoaded(Err(e)) => {
                write!(f, "ProxyProvidersJsonLoaded(Err({:?}))", e)
            }
            Message::SnifferJsonLoaded(Ok(json)) => {
                write!(f, "SnifferJsonLoaded(Ok({} chars))", json.len())
            }
            Message::SnifferJsonLoaded(Err(e)) => write!(f, "SnifferJsonLoaded(Err({:?}))", e),
            Message::LoadProviders => write!(f, "LoadProviders"),
            Message::ProvidersLoaded(Ok((p, r))) => write!(
                f,
                "ProvidersLoaded(Ok({} proxies, {} rules))",
                p.len(),
                r.len()
            ),
            Message::ProvidersLoaded(Err(e)) => write!(f, "ProvidersLoaded(Err({:?}))", e),
            Message::UpdateProxyProvider(p) => write!(f, "UpdateProxyProvider({})", p),
            Message::UpdateRuleProvider(p) => write!(f, "UpdateRuleProvider({})", p),
            Message::FilterRules(s) => write!(f, "FilterRules({})", s),
            Message::UpdateFilteredGroups => write!(f, "UpdateFilteredGroups"),
            Message::UpdateNewRuleType(s) => write!(f, "UpdateNewRuleType({})", s),
            Message::UpdateNewRulePayload(s) => write!(f, "UpdateNewRulePayload({})", s),
            Message::UpdateNewRuleTarget(s) => write!(f, "UpdateNewRuleTarget({})", s),
            Message::AddCustomRule => write!(f, "AddCustomRule"),
            Message::RuleAdded(Ok(_)) => write!(f, "RuleAdded(Ok)"),
            Message::RuleAdded(Err(e)) => write!(f, "RuleAdded(Err({:?}))", e),
            Message::ToggleRuleEnabled(index) => write!(f, "ToggleRuleEnabled({})", index),
            Message::MoveRuleUp(index) => write!(f, "MoveRuleUp({})", index),
            Message::MoveRuleDown(index) => write!(f, "MoveRuleDown({})", index),
            Message::SaveRules => write!(f, "SaveRules"),
            Message::RulesSaved(Ok(_)) => write!(f, "RulesSaved(Ok)"),
            Message::RulesSaved(Err(e)) => write!(f, "RulesSaved(Err({:?}))", e),
            Message::RuleProvidersEditorAction(_) => write!(f, "RuleProvidersEditorAction"),
            Message::SaveRuleProvidersJson => write!(f, "SaveRuleProvidersJson"),
            Message::RuleProvidersJsonSaved(Ok(_)) => write!(f, "RuleProvidersJsonSaved(Ok)"),
            Message::RuleProvidersJsonSaved(Err(e)) => {
                write!(f, "RuleProvidersJsonSaved(Err({:?}))", e)
            }
            Message::ProxyProvidersEditorAction(_) => write!(f, "ProxyProvidersEditorAction"),
            Message::SaveProxyProvidersJson => write!(f, "SaveProxyProvidersJson"),
            Message::ProxyProvidersJsonSaved(Ok(_)) => write!(f, "ProxyProvidersJsonSaved(Ok)"),
            Message::ProxyProvidersJsonSaved(Err(e)) => {
                write!(f, "ProxyProvidersJsonSaved(Err({:?}))", e)
            }
            Message::SnifferEditorAction(_) => write!(f, "SnifferEditorAction"),
            Message::SaveSnifferJson => write!(f, "SaveSnifferJson"),
            Message::SnifferJsonSaved(Ok(_)) => write!(f, "SnifferJsonSaved(Ok)"),
            Message::SnifferJsonSaved(Err(e)) => write!(f, "SnifferJsonSaved(Err({:?}))", e),
            Message::LoadAdvancedConfigs => write!(f, "LoadAdvancedConfigs"),
            Message::AdvancedConfigsBundleLoaded(Ok(bundle)) => write!(
                f,
                "AdvancedConfigsBundleLoaded(Ok(dns:{} chars, fake:{} chars, tun:{} chars))",
                bundle.dns_json.len(),
                bundle.fake_ip_json.len(),
                bundle.tun_json.len()
            ),
            Message::AdvancedConfigsBundleLoaded(Err(e)) => {
                write!(f, "AdvancedConfigsBundleLoaded(Err({:?}))", e)
            }
            Message::SetDnsTab(tab) => write!(f, "SetDnsTab({:?})", tab),
            Message::SetAdvancedMode(tab, mode) => {
                write!(f, "SetAdvancedMode({:?}, {:?})", tab, mode)
            }
            Message::RefreshDnsOnly => write!(f, "RefreshDnsOnly"),
            Message::RefreshFakeIpOnly => write!(f, "RefreshFakeIpOnly"),
            Message::RefreshTunOnly => write!(f, "RefreshTunOnly"),
            Message::EnsureDnsEditorLoaded => write!(f, "EnsureDnsEditorLoaded"),
            Message::EnsureFakeIpEditorLoaded => write!(f, "EnsureFakeIpEditorLoaded"),
            Message::EnsureTunEditorLoaded => write!(f, "EnsureTunEditorLoaded"),
            Message::ActivateDnsHeavyView => write!(f, "ActivateDnsHeavyView"),
            Message::DnsConfigJsonLoaded(Ok(json)) => {
                write!(f, "DnsConfigJsonLoaded(Ok({} chars))", json.len())
            }
            Message::DnsConfigJsonLoaded(Err(e)) => {
                write!(f, "DnsConfigJsonLoaded(Err({:?}))", e)
            }
            Message::FakeIpConfigJsonLoaded(Ok(json)) => {
                write!(f, "FakeIpConfigJsonLoaded(Ok({} chars))", json.len())
            }
            Message::FakeIpConfigJsonLoaded(Err(e)) => {
                write!(f, "FakeIpConfigJsonLoaded(Err({:?}))", e)
            }
            Message::TunConfigJsonLoaded(Ok(json)) => {
                write!(f, "TunConfigJsonLoaded(Ok({} chars))", json.len())
            }
            Message::TunConfigJsonLoaded(Err(e)) => {
                write!(f, "TunConfigJsonLoaded(Err({:?}))", e)
            }
            Message::UpdateDnsFormEnable(v) => write!(f, "UpdateDnsFormEnable({})", v),
            Message::UpdateDnsFormNameserver(v) => write!(f, "UpdateDnsFormNameserver({})", v),
            Message::UpdateDnsFormFallback(v) => write!(f, "UpdateDnsFormFallback({})", v),
            Message::UpdateDnsFormEnhancedMode(v) => {
                write!(f, "UpdateDnsFormEnhancedMode({})", v)
            }
            Message::UpdateDnsFormFakeIpRange(v) => {
                write!(f, "UpdateDnsFormFakeIpRange({})", v)
            }
            Message::UpdateDnsFormFakeIpFilter(v) => {
                write!(f, "UpdateDnsFormFakeIpFilter({})", v)
            }
            Message::UpdateDnsFormIpv6(v) => write!(f, "UpdateDnsFormIpv6({})", v),
            Message::UpdateDnsFormCache(v) => write!(f, "UpdateDnsFormCache({})", v),
            Message::UpdateDnsFormUseHosts(v) => write!(f, "UpdateDnsFormUseHosts({})", v),
            Message::UpdateDnsFormUseSystemHosts(v) => {
                write!(f, "UpdateDnsFormUseSystemHosts({})", v)
            }
            Message::UpdateDnsFormRespectRules(v) => {
                write!(f, "UpdateDnsFormRespectRules({})", v)
            }
            Message::UpdateDnsFormProxyServerNameserver(v) => {
                write!(f, "UpdateDnsFormProxyServerNameserver({})", v)
            }
            Message::UpdateDnsFormDirectNameserver(v) => {
                write!(f, "UpdateDnsFormDirectNameserver({})", v)
            }
            Message::UpdateFakeIpFormRange(v) => write!(f, "UpdateFakeIpFormRange({})", v),
            Message::UpdateFakeIpFormFilter(v) => write!(f, "UpdateFakeIpFormFilter({})", v),
            Message::UpdateFakeIpFormStore(v) => write!(f, "UpdateFakeIpFormStore({})", v),
            Message::UpdateTunFormEnable(v) => write!(f, "UpdateTunFormEnable({})", v),
            Message::UpdateTunFormStack(v) => write!(f, "UpdateTunFormStack({})", v),
            Message::UpdateTunFormMtu(v) => write!(f, "UpdateTunFormMtu({})", v),
            Message::UpdateTunFormDnsHijack(v) => write!(f, "UpdateTunFormDnsHijack({})", v),
            Message::UpdateTunFormAutoRoute(v) => write!(f, "UpdateTunFormAutoRoute({})", v),
            Message::UpdateTunFormAutoDetectInterface(v) => {
                write!(f, "UpdateTunFormAutoDetectInterface({})", v)
            }
            Message::UpdateTunFormStrictRoute(v) => {
                write!(f, "UpdateTunFormStrictRoute({})", v)
            }
            Message::DnsConfigEditorAction(_) => write!(f, "DnsConfigEditorAction"),
            Message::FakeIpConfigEditorAction(_) => write!(f, "FakeIpConfigEditorAction"),
            Message::TunConfigEditorAction(_) => write!(f, "TunConfigEditorAction"),
            Message::TickSubUpdate => write!(f, "TickSubUpdate"),
            Message::TickWebDavSync => write!(f, "TickWebDavSync"),
            Message::TickRuntimeRefresh => write!(f, "TickRuntimeRefresh"),
            Message::TickFrame(now) => write!(f, "TickFrame({:?})", now),
            Message::TrayIconEvent(_) => write!(f, "TrayIconEvent"),
            Message::MenuEvent(e) => write!(f, "MenuEvent({:?})", e),
            Message::Exit => write!(f, "Exit"),
            Message::UpdateDnsServer(i, s) => write!(f, "UpdateDnsServer({}, {})", i, s),
            Message::UpdateDnsEnhancedMode(m) => write!(f, "UpdateDnsEnhancedMode({})", m),
            Message::AddDnsServer => write!(f, "AddDnsServer"),
            Message::AddDnsServerTemplate(s) => write!(f, "AddDnsServerTemplate({})", s),
            Message::RemoveDnsServer(i) => write!(f, "RemoveDnsServer({})", i),
            Message::UpdateFallbackDnsServer(i, s) => {
                write!(f, "UpdateFallbackDnsServer({}, {})", i, s)
            }
            Message::AddFallbackDnsServer => write!(f, "AddFallbackDnsServer"),
            Message::RemoveFallbackDnsServer(i) => write!(f, "RemoveFallbackDnsServer({})", i),
            Message::SaveDns => write!(f, "SaveDns"),
            Message::DnsSaved(Ok(_)) => write!(f, "DnsSaved(Ok)"),
            Message::DnsSaved(Err(e)) => write!(f, "DnsSaved(Err({:?}))", e),
            Message::SaveFakeIpConfig => write!(f, "SaveFakeIpConfig"),
            Message::FakeIpConfigSaved(Ok(_)) => write!(f, "FakeIpConfigSaved(Ok)"),
            Message::FakeIpConfigSaved(Err(e)) => write!(f, "FakeIpConfigSaved(Err({:?}))", e),
            Message::SaveTunConfig => write!(f, "SaveTunConfig"),
            Message::TunConfigSaved(Ok(_)) => write!(f, "TunConfigSaved(Ok)"),
            Message::TunConfigSaved(Err(e)) => write!(f, "TunConfigSaved(Err({:?}))", e),
            Message::SetAutostart(b) => write!(f, "SetAutostart({})", b),
            Message::AutostartSet(Ok(_)) => write!(f, "AutostartSet(Ok)"),
            Message::AutostartSet(Err(e)) => write!(f, "AutostartSet(Err({:?}))", e),
            Message::UpdateWebDavEnabled(b) => write!(f, "UpdateWebDavEnabled({})", b),
            Message::UpdateWebDavUrl(s) => write!(f, "UpdateWebDavUrl({})", s),
            Message::UpdateWebDavUser(s) => write!(f, "UpdateWebDavUser({})", s),
            Message::UpdateWebDavPass(_) => write!(f, "UpdateWebDavPass(***)"),
            Message::UpdateWebDavSyncInterval(s) => {
                write!(f, "UpdateWebDavSyncInterval({})", s)
            }
            Message::UpdateWebDavSyncOnStartup(b) => {
                write!(f, "UpdateWebDavSyncOnStartup({})", b)
            }
            Message::SaveAppSettings => write!(f, "SaveAppSettings"),
            Message::AppSettingsSaved(Ok(_)) => write!(f, "AppSettingsSaved(Ok)"),
            Message::AppSettingsSaved(Err(e)) => write!(f, "AppSettingsSaved(Err({:?}))", e),
            Message::UpdateEditorPathSetting(s) => write!(f, "UpdateEditorPathSetting({})", s),
            Message::SyncUpload => write!(f, "SyncUpload"),
            Message::SyncDownload => write!(f, "SyncDownload"),
            Message::SyncFinished(Ok(_)) => write!(f, "SyncFinished(Ok)"),
            Message::SyncFinished(Err(e)) => write!(f, "SyncFinished(Err({:?}))", e),
            Message::SetSystemProxy(b) => write!(f, "SetSystemProxy({})", b),
            Message::SystemProxySet(Ok(_)) => write!(f, "SystemProxySet(Ok)"),
            Message::SystemProxySet(Err(e)) => write!(f, "SystemProxySet(Err({:?}))", e),
            Message::RequestAdminPrivilege => write!(f, "RequestAdminPrivilege"),
            Message::EditProfile(p) => write!(f, "EditProfile({:?})", p),
            Message::ProfileContentLoaded(Ok((p, _))) => {
                write!(f, "ProfileContentLoaded(Ok({:?}))", p)
            }
            Message::ProfileContentLoaded(Err(e)) => {
                write!(f, "ProfileContentLoaded(Err({:?}))", e)
            }
            Message::EditorAction(_) => write!(f, "EditorAction"),
            Message::SaveProfile => write!(f, "SaveProfile"),
            Message::ProfileSaved(Ok(_)) => write!(f, "ProfileSaved(Ok)"),
            Message::ProfileSaved(Err(e)) => write!(f, "ProfileSaved(Err({:?}))", e),
            Message::LoadKernels => write!(f, "LoadKernels"),
            Message::KernelsLoaded(Ok(k)) => write!(f, "KernelsLoaded(Ok({} kernels))", k.len()),
            Message::KernelsLoaded(Err(e)) => write!(f, "KernelsLoaded(Err({:?}))", e),
            Message::CheckCoreUpdate => write!(f, "CheckCoreUpdate"),
            Message::CoreUpdateInfo(Ok(v)) => write!(f, "CoreUpdateInfo(Ok({}))", v),
            Message::CoreUpdateInfo(Err(e)) => write!(f, "CoreUpdateInfo(Err({:?}))", e),
            Message::DownloadCore(v) => write!(f, "DownloadCore({})", v),
            Message::CoreDownloadProgress(p) => {
                write!(f, "CoreDownloadProgress({:.2}%)", p * 100.0)
            }
            Message::CoreDownloadFinished(Ok(v)) => write!(f, "CoreDownloadFinished(Ok({}))", v),
            Message::CoreDownloadFinished(Err(e)) => {
                write!(f, "CoreDownloadFinished(Err({:?}))", e)
            }
            Message::DeleteKernel(v) => write!(f, "DeleteKernel({})", v),
            Message::SetDefaultKernel(v) => write!(f, "SetDefaultKernel({})", v),
            Message::FactoryReset => write!(f, "FactoryReset"),
            Message::OpenConfigDir => write!(f, "OpenConfigDir"),
            Message::FlushFakeIpCache => write!(f, "FlushFakeIpCache"),
            Message::TestProxyDelay(p) => write!(f, "TestProxyDelay({})", p),
            Message::TestGroupDelay(g) => write!(f, "TestGroupDelay({})", g),
            Message::ProxyTested(p, Ok(d)) => write!(f, "ProxyTested({}, Ok({}ms))", p, d),
            Message::ProxyTested(p, Err(e)) => write!(f, "ProxyTested({}, Err({:?}))", p, e),
            Message::WindowClosed(id) => write!(f, "WindowClosed({:?})", id),
            Message::HideWindow => write!(f, "HideWindow"),
            Message::ShowWindow => write!(f, "ShowWindow"),
            Message::UpdateRuntimeAutoRefresh(v) => write!(f, "UpdateRuntimeAutoRefresh({})", v),
            Message::RuntimePanelSettingsSaved(Ok(_)) => {
                write!(f, "RuntimePanelSettingsSaved(Ok)")
            }
            Message::RuntimePanelSettingsSaved(Err(e)) => {
                write!(f, "RuntimePanelSettingsSaved(Err({:?}))", e)
            }
            Message::RuntimeRebuildFinished(Ok(_)) => write!(f, "RuntimeRebuildFinished(Ok)"),
            Message::RuntimeRebuildFinished(Err(e)) => {
                write!(f, "RuntimeRebuildFinished(Err({:?}))", e)
            }
            Message::ClearRebuildFlow => write!(f, "ClearRebuildFlow"),
            Message::TogglePerfPanel => write!(f, "TogglePerfPanel"),
            Message::ToggleTheme => write!(f, "ToggleTheme"),
            Message::ShowToast(s, st) => write!(f, "ShowToast({}, {:?})", s, st),
            Message::RemoveToast(i) => write!(f, "RemoveToast({})", i),
            Message::TestAllProxyDelays => write!(f, "TestAllProxyDelays"),
            Message::AllProxyDelaysTested(Ok((s, f_cnt))) => {
                write!(
                    f,
                    "AllProxyDelaysTested(Ok(success={}, failed={}))",
                    s, f_cnt
                )
            }
            Message::AllProxyDelaysTested(Err(e)) => {
                write!(f, "AllProxyDelaysTested(Err({:?}))", e)
            }
        }
    }
}

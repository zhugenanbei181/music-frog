use serde::{Deserialize, Serialize};

use crate::error::Failure;
use crate::lan::LanCredentials;
use crate::tun::TunStack;

/// Correlates an asynchronous command with its result and events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestId(pub u64);

impl RequestId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

/// Mihomo's three controller modes, kept free of transport details.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProxyMode {
    #[default]
    Rule,
    Global,
    Direct,
    Script,
}

/// Log verbosity accepted by Mihomo's live configuration endpoint.
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

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "debug" => Some(Self::Debug),
            "info" => Some(Self::Info),
            "warn" | "warning" => Some(Self::Warn),
            "error" | "err" => Some(Self::Error),
            _ => None,
        }
    }
}

impl ProxyMode {
    pub const ALL: [Self; 4] = [Self::Rule, Self::Global, Self::Direct, Self::Script];

    pub fn to_wire(self) -> &'static str {
        match self {
            Self::Rule => "rule",
            Self::Global => "global",
            Self::Direct => "direct",
            Self::Script => "script",
        }
    }

    pub fn from_wire(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "rule" => Some(Self::Rule),
            "global" => Some(Self::Global),
            "direct" => Some(Self::Direct),
            "script" => Some(Self::Script),
            _ => None,
        }
    }

    pub const fn to_index(self) -> u8 {
        match self {
            Self::Rule => 0,
            Self::Global => 1,
            Self::Direct => 2,
            Self::Script => 3,
        }
    }

    pub const fn from_index(raw: u8) -> Self {
        match raw {
            1 => Self::Global,
            2 => Self::Direct,
            3 => Self::Script,
            _ => Self::Rule,
        }
    }
}

/// A shared user intention. UI-local actions such as opening a drawer or
/// changing a Bevy scene do not belong here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandIntent {
    StartCore,
    StopCore,
    RestartCore,
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
    SetProxyMode {
        mode: ProxyMode,
    },
    SelectProxyNode {
        group: String,
        node: String,
    },
    TestDelay {
        group: Option<String>,
        #[serde(default)]
        url: Option<String>,
        #[serde(default)]
        timeout_ms: Option<u32>,
    },
    RunSpeedtest {
        node: String,
        #[serde(default)]
        url: Option<String>,
    },
    /// Report a real downlink transfer measured by the host for one node.
    RecordSpeedtestBandwidth {
        node: String,
        total_bytes: u64,
        duration_ms: u64,
    },
    CancelSpeedtest,
    ToggleProxyGroupExpand {
        group: String,
    },
    SetProxyGroupExpanded {
        group: String,
        expanded: bool,
    },
    SetProxySortOrder {
        order: crate::proxies::ProxySortOrder,
    },
    ToggleFilterAlive {
        enabled: bool,
    },
    ToggleFavoriteProxy {
        proxy: String,
    },
    SetProxyCompactView {
        compact: bool,
    },
    ReorderProxyGroups {
        group_names: Vec<String>,
    },
    ResetProxyGroupOrder,
    ReorderOverviewCards {
        order: Vec<crate::overview_layout::OverviewCardKind>,
    },
    ResetOverviewCardOrder,
    UpdateProfile {
        profile_id: String,
    },
    /// Persist a profile's subscription fetch options: custom User-Agent and
    /// the insecure-TLS preference used by the conditional update.
    UpdateSubscriptionFetchSettings {
        profile_id: String,
        user_agent: Option<String>,
        insecure_skip_verify: bool,
    },
    DeleteProfile {
        profile_id: String,
    },
    RefreshRuleProviders,
    SimulateRuleTrace {
        query: String,
    },
    /// DUAL-12-10: set the simulated sandbox source IP / inbound environment
    /// the next rule-tracer replay must merge.
    SetRuleTracerContext {
        src_ip: Option<String>,
    },
    ResetRuleHitCounters,
    UnpackRuleProvider {
        provider_name: String,
    },
    CloseConnection {
        id: String,
    },
    CloseAllConnections,
    ClearLogs,
    SetLogLevelFilter {
        level: Option<String>,
    },
    ClearDnsCache,
    TestDnsLatency,
    RunDoctorDiagnostics,
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
    StartVpn,
    StopVpn,
    RunPrivilegedNetworkRegression,
    ToggleAppRouting {
        app_id: String,
        enabled: bool,
    },
    SetAppRoutingMode {
        mode: String,
    },
    ToggleIncludeSystemApps {
        include: bool,
    },
    SetAppRule {
        app_id: String,
        rule: String,
    },
    SyncNow,
    CreateBackupSnapshot,
    ResolveConflictKeepLocal,
    ResolveConflictTakeRemote,
    RestoreSnapshot {
        id: String,
    },
    /// Select the last installed, locally recorded core version.
    RollbackCore,
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
    Rejected {
        request_id: RequestId,
        failure: Failure,
    },
}

impl CommandIntent {
    pub const fn kind(&self) -> CommandKind {
        match self {
            Self::StartCore | Self::StopCore | Self::RestartCore => CommandKind::CoreLifecycle,
            Self::PrepareServiceMode => CommandKind::Network,
            Self::RepairPortConflicts => CommandKind::Network,
            Self::SetCoreLogLevel { .. } | Self::SetTunStack { .. } => CommandKind::Runtime,
            Self::SwitchProfile { .. }
            | Self::UpdateProfile { .. }
            | Self::UpdateSubscriptionFetchSettings { .. }
            | Self::DeleteProfile { .. }
            | Self::RefreshRuleProviders
            | Self::UnpackRuleProvider { .. } => CommandKind::Profile,
            Self::SimulateRuleTrace { .. }
            | Self::SetRuleTracerContext { .. }
            | Self::ResetRuleHitCounters
            | Self::ReorderOverviewCards { .. }
            | Self::ResetOverviewCardOrder => CommandKind::Runtime,
            Self::SetProxyMode { .. }
            | Self::SelectProxyNode { .. }
            | Self::TestDelay { .. }
            | Self::RunSpeedtest { .. }
            | Self::RecordSpeedtestBandwidth { .. }
            | Self::CancelSpeedtest
            | Self::ToggleProxyGroupExpand { .. }
            | Self::SetProxyGroupExpanded { .. }
            | Self::SetProxySortOrder { .. }
            | Self::ToggleFilterAlive { .. }
            | Self::ToggleFavoriteProxy { .. }
            | Self::SetProxyCompactView { .. }
            | Self::ReorderProxyGroups { .. }
            | Self::ResetProxyGroupOrder => CommandKind::Proxy,
            Self::CloseConnection { .. } | Self::CloseAllConnections | Self::ClearDnsCache => {
                CommandKind::Runtime
            }
            Self::ClearLogs
            | Self::SetLogLevelFilter { .. }
            | Self::TestDnsLatency
            | Self::RunDoctorDiagnostics
            | Self::RepairDoctorIssue { .. }
            | Self::RepairAllDoctorIssues => CommandKind::Runtime,
            Self::ToggleTun { .. }
            | Self::SetTunAutoRoute { .. }
            | Self::SetTunStrictRoute { .. }
            | Self::ProbeTunMtu
            | Self::RefreshPublicIpProbe
            | Self::SetSystemProxy { .. }
            | Self::SetLanSharing { .. }
            | Self::SetLanSecurity { .. }
            | Self::SetIpv6Routing { .. }
            | Self::ScanUwpApps
            | Self::SetUwpAppExemption { .. }
            | Self::SetAllUwpExemptions { .. }
            | Self::ApplyPac { .. }
            | Self::RefreshNetworkRoaming
            | Self::RepairNetworkRoutes
            | Self::StartVpn
            | Self::StopVpn
            | Self::RunPrivilegedNetworkRegression
            | Self::ToggleAppRouting { .. }
            | Self::SetAppRoutingMode { .. }
            | Self::ToggleIncludeSystemApps { .. }
            | Self::SetAppRule { .. } => CommandKind::Network,
            Self::SyncNow
            | Self::CreateBackupSnapshot
            | Self::ResolveConflictKeepLocal
            | Self::ResolveConflictTakeRemote
            | Self::RestoreSnapshot { .. } => CommandKind::Sync,
            Self::RollbackCore | Self::UpdateSetting { .. } | Self::CheckUpdates => {
                CommandKind::Update
            }
        }
    }
}

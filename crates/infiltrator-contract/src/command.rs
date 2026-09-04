use serde::{Deserialize, Serialize};

use crate::error::Failure;

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
}

impl ProxyMode {
    pub fn to_wire(self) -> &'static str {
        match self {
            Self::Rule => "rule",
            Self::Global => "global",
            Self::Direct => "direct",
        }
    }

    pub fn from_wire(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "rule" => Some(Self::Rule),
            "global" => Some(Self::Global),
            "direct" => Some(Self::Direct),
            _ => None,
        }
    }

    pub const fn to_index(self) -> u8 {
        match self {
            Self::Rule => 0,
            Self::Global => 1,
            Self::Direct => 2,
        }
    }

    pub const fn from_index(raw: u8) -> Self {
        match raw {
            1 => Self::Global,
            2 => Self::Direct,
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
    SwitchProfile { profile_id: String },
    SetProxyMode { mode: ProxyMode },
    SelectProxyNode { group: String, node: String },
    TestDelay { group: Option<String> },
    UpdateProfile { profile_id: String },
    DeleteProfile { profile_id: String },
    RefreshRuleProviders,
    CloseConnection { id: String },
    CloseAllConnections,
    ClearLogs,
    SetLogLevelFilter { level: Option<String> },
    ClearDnsCache,
    TestDnsLatency,
    RunDoctorDiagnostics,
    RepairDoctorIssue { check_id: String },
    RepairAllDoctorIssues,
    ToggleTun { enabled: bool },
    SetSystemProxy { enabled: bool },
    ToggleAppRouting { app_id: String, enabled: bool },
    SetAppRoutingMode { mode: String },
    ToggleIncludeSystemApps { include: bool },
    SetAppRule { app_id: String, rule: String },
    SyncNow,
    CreateBackupSnapshot,
    ResolveConflictKeepLocal,
    ResolveConflictTakeRemote,
    RestoreSnapshot { id: String },
    UpdateSetting { key: String, value: String },
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
            Self::SwitchProfile { .. }
            | Self::UpdateProfile { .. }
            | Self::DeleteProfile { .. }
            | Self::RefreshRuleProviders => CommandKind::Profile,
            Self::SetProxyMode { .. } | Self::SelectProxyNode { .. } | Self::TestDelay { .. } => {
                CommandKind::Proxy
            }
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
            | Self::SetSystemProxy { .. }
            | Self::ToggleAppRouting { .. }
            | Self::SetAppRoutingMode { .. }
            | Self::ToggleIncludeSystemApps { .. }
            | Self::SetAppRule { .. } => CommandKind::Network,
            Self::SyncNow
            | Self::CreateBackupSnapshot
            | Self::ResolveConflictKeepLocal
            | Self::ResolveConflictTakeRemote
            | Self::RestoreSnapshot { .. } => CommandKind::Sync,
            Self::UpdateSetting { .. } | Self::CheckUpdates => CommandKind::Update,
        }
    }
}

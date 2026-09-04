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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProxyMode {
    Rule,
    Global,
    Direct,
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
    UpdateProfile { profile_id: String },
    RefreshRuleProviders,
    CloseConnection { id: String },
    CloseAllConnections,
    ClearDnsCache,
    ToggleTun { enabled: bool },
    SetSystemProxy { enabled: bool },
    SyncNow,
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
            | Self::RefreshRuleProviders => CommandKind::Profile,
            Self::SetProxyMode { .. } | Self::SelectProxyNode { .. } => CommandKind::Proxy,
            Self::CloseConnection { .. } | Self::CloseAllConnections | Self::ClearDnsCache => {
                CommandKind::Runtime
            }
            Self::ToggleTun { .. } | Self::SetSystemProxy { .. } => CommandKind::Network,
            Self::SyncNow => CommandKind::Sync,
            Self::CheckUpdates => CommandKind::Update,
        }
    }
}

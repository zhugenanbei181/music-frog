use crate::command::{CommandKind, ProxyMode, RequestId};
use crate::error::Failure;
use crate::session::SessionToken;
use serde::{Deserialize, Serialize};

/// Stable lifecycle vocabulary for REST, UI projection, and FFI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreLifecycle {
    Stopped,
    Starting,
    Ready,
    Running,
    Stopping,
    Failed,
}

/// State of the application-owned crash recovery loop.
///
/// The value is deliberately a contract type rather than a UI status string:
/// Iced, Bevy, native mobile surfaces and REST/FFI adapters can all render
/// the same recovery decision without owning the retry policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreWatchdogState {
    Idle,
    Waiting { attempt: u32, retry_in_ms: u64 },
    Restarting { attempt: u32 },
    Recovered { attempts: u32 },
    Tripped { attempts: u32 },
}

/// Read-only crash-watchdog projection attached to the canonical core
/// snapshot. `last_error` is already typed and safe to pass to a surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreWatchdogSnapshot {
    pub state: CoreWatchdogState,
    pub session_token: Option<SessionToken>,
    pub consecutive_failures: u32,
    pub last_error: Option<Failure>,
}

impl Default for CoreWatchdogSnapshot {
    fn default() -> Self {
        Self {
            state: CoreWatchdogState::Idle,
            session_token: None,
            consecutive_failures: 0,
            last_error: None,
        }
    }
}

/// A read-only Core projection. Secrets and client objects never cross this
/// boundary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoreSnapshot {
    pub lifecycle: CoreLifecycle,
    pub generation: u64,
    /// Identity of the currently active core session. It is absent while
    /// stopped; a new start always receives a fresh token.
    #[serde(default)]
    pub session_token: Option<SessionToken>,
    pub revision: u64,
    pub proxy_mode: Option<ProxyMode>,
    pub core_version: Option<String>,
    pub sampled_at_epoch_ms: Option<i64>,
    pub failure: Option<Failure>,
    pub upload_bps: f64,
    pub download_bps: f64,
    pub active_connections: u32,
    pub memory_bytes: Option<u64>,
    /// Crash recovery state owned by the shared application, not by either UI.
    #[serde(default)]
    pub watchdog: CoreWatchdogSnapshot,
}

/// Result of a user-requested public-egress probe.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicIpSnapshot {
    pub ip: String,
    pub provider: String,
    pub checked_at_epoch_ms: i64,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub city: Option<String>,
}

/// Bounded, surface-neutral events emitted by the application layer.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CoreEvent {
    SnapshotUpdated(CoreSnapshot),
    CommandAccepted {
        request_id: RequestId,
        kind: CommandKind,
    },
    CommandCompleted {
        request_id: RequestId,
        kind: CommandKind,
    },
    CommandFailed {
        request_id: RequestId,
        kind: CommandKind,
        failure: Failure,
    },
}

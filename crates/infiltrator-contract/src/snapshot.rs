use crate::command::{CommandKind, RequestId};
use lurker_core_contract::error::Failure;
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, CoreScope};
use lurker_core_contract::lifecycle::{CoreLifecycle, CoreLifecycleSnapshot};
use lurker_core_contract::routing_intent::RoutingIntentSnapshot;
use lurker_core_contract::session::SessionToken;
use serde::{Deserialize, Serialize};

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
    /// Engine and configured instance that authored this observation.
    #[serde(default)]
    pub identity: Option<CoreIdentity>,
    pub lifecycle: CoreLifecycle,
    pub generation: u64,
    /// Identity of the currently active core session. It is absent while
    /// stopped; a new start always receives a fresh token.
    #[serde(default)]
    pub session_token: Option<SessionToken>,
    pub revision: u64,
    pub routing_intent: RoutingIntentSnapshot,
    pub core_version: Option<String>,
    pub sampled_at_epoch_ms: Option<i64>,
    pub failure: Option<Failure>,
    pub upload_bps: f64,
    pub download_bps: f64,
    pub active_connections: u32,
    /// Crash recovery state owned by the shared application, not by either UI.
    #[serde(default)]
    pub watchdog: CoreWatchdogSnapshot,
}

impl CoreSnapshot {
    pub fn lifecycle_snapshot(&self) -> CoreLifecycleSnapshot {
        CoreLifecycleSnapshot {
            identity: self.identity.clone(),
            lifecycle: self.lifecycle.clone(),
            generation: self.generation,
            session_token: self.session_token,
            revision: self.revision,
            failure: self.failure.clone(),
        }
    }

    pub fn scope(&self) -> Option<CoreScope> {
        self.identity.clone().map(|identity| CoreScope {
            identity,
            generation: CoreGeneration::from(self.generation),
            session_token: self.session_token,
        })
    }
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
        scope: Option<CoreScope>,
        request_id: RequestId,
        kind: CommandKind,
    },
    CommandCompleted {
        scope: Option<CoreScope>,
        request_id: RequestId,
        kind: CommandKind,
    },
    CommandFailed {
        scope: Option<CoreScope>,
        request_id: RequestId,
        kind: CommandKind,
        failure: Failure,
    },
}

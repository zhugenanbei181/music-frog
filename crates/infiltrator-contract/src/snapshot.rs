use crate::command::{CommandKind, RequestId};
use crate::core_identity::{CoreGeneration, CoreIdentity, CoreScope};
use crate::error::Failure;
use crate::routing_intent::RoutingIntentSnapshot;
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

/// Minimal lifecycle read model shared by every inbound surface. It is
/// derived from [`CoreSnapshot`] so lifecycle, generation, session identity
/// and failure cannot drift between UI implementations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreLifecycleSnapshot {
    #[serde(default)]
    pub identity: Option<CoreIdentity>,
    pub lifecycle: CoreLifecycle,
    pub generation: u64,
    #[serde(default, with = "core_lifecycle_session_token_wire")]
    pub session_token: Option<SessionToken>,
    pub revision: u64,
    pub failure: Option<Failure>,
}

impl Default for CoreLifecycleSnapshot {
    fn default() -> Self {
        Self {
            identity: None,
            lifecycle: CoreLifecycle::Stopped,
            generation: 0,
            session_token: None,
            revision: 0,
            failure: None,
        }
    }
}

impl CoreLifecycleSnapshot {
    pub fn scope(&self) -> Option<CoreScope> {
        self.identity.clone().map(|identity| CoreScope {
            identity,
            generation: CoreGeneration::from(self.generation),
            session_token: self.session_token,
        })
    }
}

mod core_lifecycle_session_token_wire {
    use crate::session::SessionToken;
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(token: &Option<SessionToken>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        token
            .map(|value| value.value().to_string())
            .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<SessionToken>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| {
                value
                    .parse::<u128>()
                    .map(SessionToken::new)
                    .map_err(D::Error::custom)
            })
            .transpose()
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
    pub memory_bytes: Option<u64>,
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

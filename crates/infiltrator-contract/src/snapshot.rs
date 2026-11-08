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
    /// The Core session the owner runs, with the instant it began and the
    /// length the owner measured when it published this readback.
    #[serde(default)]
    pub session: CoreSessionObservation,
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

/// The Core session an owner currently runs.
///
/// The session start is the owner's own fact: the owner stamps it when it
/// starts the Core process and republishes the session length it measures at
/// read time. A surface renders the session length it is given, and never
/// derives one from a lifecycle stage, a wall clock of its own, or a
/// fabricated zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "details", rename_all = "snake_case")]
pub enum CoreSessionObservation {
    /// No session has been observed yet.
    #[default]
    Unknown,
    /// The owner runs no session: it never started one, or the last one
    /// ended (the Core process is gone).
    Inactive,
    /// The owner runs a session that began at the published instant.
    Active {
        /// Wall-clock instant at which the owner began the session, in
        /// milliseconds since the Unix epoch.
        started_at_epoch_ms: u64,
        /// The session length the owner measured when it published this
        /// readback, in milliseconds.
        elapsed_ms: u64,
    },
}

impl CoreSessionObservation {
    /// The measured session length, or `None` when no session is running.
    pub fn elapsed_ms(&self) -> Option<u64> {
        match self {
            Self::Active { elapsed_ms, .. } => Some(*elapsed_ms),
            Self::Unknown | Self::Inactive => None,
        }
    }

    /// The instant the running session began, or `None` when no session is
    /// running.
    pub fn started_at_epoch_ms(&self) -> Option<u64> {
        match self {
            Self::Active {
                started_at_epoch_ms,
                ..
            } => Some(*started_at_epoch_ms),
            Self::Unknown | Self::Inactive => None,
        }
    }

    /// Whether the owner runs a session with a measured length.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active { .. })
    }
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

#[cfg(test)]
mod tests {
    use super::CoreSessionObservation;
    use lurker_core_contract::lifecycle::CoreLifecycle;
    use lurker_core_contract::routing_intent::RoutingIntentSnapshot;

    #[test]
    fn an_unobserved_session_publishes_no_length() {
        let unknown = CoreSessionObservation::Unknown;
        assert!(!unknown.is_active());
        assert_eq!(unknown.elapsed_ms(), None);
        assert_eq!(unknown.started_at_epoch_ms(), None);

        let inactive = CoreSessionObservation::Inactive;
        assert!(!inactive.is_active());
        assert_eq!(inactive.elapsed_ms(), None);
        assert_eq!(inactive.started_at_epoch_ms(), None);
    }

    #[test]
    fn an_active_session_publishes_its_start_and_its_measured_length() {
        let session = CoreSessionObservation::Active {
            started_at_epoch_ms: 1_700_000_000_000,
            elapsed_ms: 11_089_000,
        };
        assert!(session.is_active());
        assert_eq!(session.started_at_epoch_ms(), Some(1_700_000_000_000));
        assert_eq!(session.elapsed_ms(), Some(11_089_000));
    }

    #[test]
    fn the_session_observation_survives_the_wire() {
        let session = CoreSessionObservation::Active {
            started_at_epoch_ms: 1_700_000_000_000,
            elapsed_ms: 42_000,
        };
        let encoded = serde_json::to_string(&session).expect("serialize session");
        assert!(encoded.contains("\"state\":\"active\""));
        assert!(encoded.contains("\"started_at_epoch_ms\":1700000000000"));
        assert!(encoded.contains("\"elapsed_ms\":42000"));
        let decoded: CoreSessionObservation =
            serde_json::from_str(&encoded).expect("deserialize session");
        assert_eq!(decoded, session);
    }

    #[test]
    fn a_snapshot_without_a_session_field_reads_as_unobserved() {
        let snapshot = super::CoreSnapshot {
            identity: None,
            lifecycle: CoreLifecycle::Stopped,
            generation: 0,
            session_token: None,
            session: CoreSessionObservation::Unknown,
            revision: 0,
            routing_intent: RoutingIntentSnapshot::default(),
            core_version: None,
            sampled_at_epoch_ms: None,
            failure: None,
            upload_bps: 0.0,
            download_bps: 0.0,
            active_connections: 0,
            watchdog: Default::default(),
        };
        let mut value = serde_json::to_value(&snapshot).expect("serialize snapshot");
        value
            .as_object_mut()
            .expect("a snapshot is an object")
            .remove("session");
        let decoded: super::CoreSnapshot =
            serde_json::from_value(value).expect("deserialize snapshot");
        assert_eq!(decoded.session, CoreSessionObservation::Unknown);
    }
}

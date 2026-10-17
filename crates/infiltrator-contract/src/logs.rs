//! Neutral log semantics and session-fenced stream observations.
use crate::core_identity::{CoreGeneration, CoreIdentity, CoreScope};
use crate::error::Failure;
use lurker_core_contract::session::SessionToken;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
    #[default]
    Unknown,
}
impl LogLevel {
    pub const ALL: [Self; 5] = [
        Self::Debug,
        Self::Info,
        Self::Warn,
        Self::Error,
        Self::Unknown,
    ];
    pub fn from_identifier(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "debug" | "dbg" => Self::Debug,
            "info" | "inf" => Self::Info,
            "warn" | "warning" | "wrn" => Self::Warn,
            "error" | "err" | "fatal" => Self::Error,
            _ => Self::Unknown,
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
            Self::Unknown => "UNKNOWN",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogSession {
    #[serde(default)]
    identity: Option<CoreIdentity>,
    pub generation: CoreGeneration,
    pub token: SessionToken,
}

impl LogSession {
    /// Legacy/test session without engine identity. Production callers should
    /// construct sessions from a lifecycle scope.
    pub fn new(generation: u64, token: SessionToken) -> Self {
        Self {
            identity: None,
            generation: CoreGeneration::new(generation),
            token,
        }
    }

    pub fn from_core_scope(scope: CoreScope) -> Option<Self> {
        let token = scope.session_token.filter(|token| token.is_valid())?;
        Some(Self {
            identity: Some(scope.identity),
            generation: scope.generation,
            token,
        })
    }

    pub fn identity(&self) -> Option<&CoreIdentity> {
        self.identity.as_ref()
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogStreamState {
    #[default]
    Idle,
    Connecting,
    Live,
    Reconnecting(Failure),
    Failed(Failure),
    Unsupported(Failure),
    Cancelled,
}

/// Original controller bytes retained by the runtime-neutral log owner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawLogRecord {
    pub id: u64,
    pub raw: String,
}

/// Whether the stream is driven by a standalone consumer or the Core owner.
/// Each variant represents one valid owner/binding combination.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogBindingSnapshot {
    #[default]
    StandaloneUnbound,
    StandaloneBound(LogSession),
    CoreUnbound,
    CoreBound(LogSession),
}

impl LogBindingSnapshot {
    pub fn session(&self) -> Option<&LogSession> {
        match self {
            Self::StandaloneUnbound | Self::CoreUnbound => None,
            Self::StandaloneBound(session) | Self::CoreBound(session) => Some(session),
        }
    }
}

/// Raw facts observed from one scoped stream. Display parsing belongs to the
/// presentation layer and is intentionally absent from this contract.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogObservationSnapshot {
    #[default]
    Unobserved,
    Connecting,
    Reconnecting(Failure),
    Failed(Failure),
    Unsupported(Failure),
    Cancelled,
    Observed {
        session: LogSession,
        stream: LogStreamState,
        records: Vec<RawLogRecord>,
    },
}

impl LogObservationSnapshot {
    pub fn stream(&self) -> LogStreamState {
        match self {
            Self::Unobserved => LogStreamState::Idle,
            Self::Connecting => LogStreamState::Connecting,
            Self::Reconnecting(failure) => LogStreamState::Reconnecting(failure.clone()),
            Self::Failed(failure) => LogStreamState::Failed(failure.clone()),
            Self::Unsupported(failure) => LogStreamState::Unsupported(failure.clone()),
            Self::Cancelled => LogStreamState::Cancelled,
            Self::Observed { stream, .. } => stream.clone(),
        }
    }

    pub fn observed(&self) -> Option<(&LogSession, &[RawLogRecord])> {
        match self {
            Self::Observed {
                session, records, ..
            } => Some((session, records)),
            Self::Unobserved
            | Self::Connecting
            | Self::Reconnecting(_)
            | Self::Failed(_)
            | Self::Unsupported(_)
            | Self::Cancelled => None,
        }
    }
}

/// Application-owned raw log facts passed to a pure display projection.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogFactsSnapshot {
    pub binding: LogBindingSnapshot,
    pub observation: LogObservationSnapshot,
    pub active_level: Option<LogLevel>,
}

#[cfg(test)]
mod tests {
    use super::LogSession;
    use crate::core_identity::{CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId};
    use lurker_core_contract::session::SessionToken;

    #[test]
    fn log_session_round_trips_engine_instance_and_generation_identity() {
        let session = LogSession::from_core_scope(CoreScope {
            identity: CoreIdentity::new(
                EngineId::new("mihomo").expect("valid engine"),
                CoreInstanceId::new("desktop-main").expect("valid instance"),
                None,
            ),
            generation: CoreGeneration::new(7),
            session_token: Some(SessionToken::new(41)),
        })
        .expect("active identity-bound log session");

        let encoded = serde_json::to_string(&session).expect("serialize log session");
        let decoded =
            serde_json::from_str::<LogSession>(&encoded).expect("deserialize log session");

        assert_eq!(decoded, session);
        assert_eq!(
            decoded.identity().expect("engine identity").engine.as_str(),
            "mihomo"
        );
        assert_eq!(decoded.generation.value(), 7);
    }

    #[test]
    fn legacy_log_session_payload_remains_readable_without_engine_identity() {
        let decoded = serde_json::from_str::<LogSession>(r#"{"generation":5,"token":25}"#)
            .expect("deserialize prior log session payload");

        assert!(decoded.identity().is_none());
        assert_eq!(decoded.generation.value(), 5);
        assert_eq!(decoded.token, SessionToken::new(25));
    }

    #[test]
    fn log_session_requires_a_valid_active_token() {
        let scope = CoreScope {
            identity: CoreIdentity::new(
                EngineId::new("sing-box").expect("valid engine"),
                CoreInstanceId::new("android-vpn").expect("valid instance"),
                None,
            ),
            generation: CoreGeneration::new(3),
            session_token: Some(SessionToken::ZERO),
        };

        assert!(LogSession::from_core_scope(scope).is_none());
    }
}

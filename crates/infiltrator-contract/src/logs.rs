//! Neutral log semantics and session-fenced stream observations.
use lurker_core_contract::error::Failure;
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, CoreScope};
use lurker_core_contract::session::SessionToken;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// The quietest rung: the engine logged the line without saying more.
    ///
    /// The reference product's level ladder starts here
    /// (`logLevelOrder` in its logs page: silent, error, warning, info,
    /// debug), so a filter that cannot name this rung cannot express the
    /// reference's own "only the quietest lines" view.
    Silent,
    Debug,
    Info,
    Warn,
    Error,
    #[default]
    Unknown,
}
impl LogLevel {
    pub const ALL: [Self; 6] = [
        Self::Silent,
        Self::Debug,
        Self::Info,
        Self::Warn,
        Self::Error,
        Self::Unknown,
    ];
    pub fn from_identifier(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "silent" => Self::Silent,
            "debug" | "dbg" => Self::Debug,
            "info" | "inf" => Self::Info,
            "warn" | "warning" | "wrn" => Self::Warn,
            "error" | "err" | "fatal" => Self::Error,
            _ => Self::Unknown,
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::Silent => "SILENT",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
            Self::Unknown => "UNKNOWN",
        }
    }

    /// Where this level sits on the severity ladder.
    ///
    /// The numbers are the reference product's own (`logLevelOrder`): zero is
    /// the quietest rung and the highest number is the most verbose one, so
    /// "at least as severe as the filter" is "not above the filter's rung".
    /// `Unknown` sits above `Debug` because an unclassified line is not
    /// claimed to be a debug line - it is only shown when the reader asks for
    /// the unclassified lines themselves.
    pub const fn severity(self) -> u8 {
        match self {
            Self::Silent => 0,
            Self::Error => 1,
            Self::Warn => 2,
            Self::Info => 3,
            Self::Debug => 4,
            Self::Unknown => 5,
        }
    }

    /// Whether a line of this level belongs under a filter on `filter`.
    ///
    /// The filter is a threshold, not an equality: picking "warn" shows the
    /// warnings **and** the errors above them, exactly like the reference
    /// product's ladder does. Only the unclassified lines stay out until the
    /// reader asks for them by name.
    pub const fn passes_filter(self, filter: Self) -> bool {
        self.severity() <= filter.severity()
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
    use super::{LogLevel, LogSession};
    use lurker_core_contract::identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
    };
    use lurker_core_contract::session::SessionToken;

    #[test]
    fn the_silent_rung_is_part_of_the_level_vocabulary() {
        assert_eq!(LogLevel::from_identifier("SILENT"), LogLevel::Silent);
        assert_eq!(LogLevel::Silent.label(), "SILENT");
        assert!(LogLevel::ALL.contains(&LogLevel::Silent));
        let encoded = serde_json::to_string(&LogLevel::Silent).expect("serialize level");
        assert_eq!(encoded, "\"silent\"");
        assert_eq!(
            serde_json::from_str::<LogLevel>(&encoded).expect("deserialize level"),
            LogLevel::Silent
        );
    }

    #[test]
    fn a_level_filter_is_a_severity_threshold_not_an_equality() {
        // The reference ladder is silent < error < warning < info < debug;
        // picking "warn" must show warnings AND the errors above them.
        assert!(LogLevel::Error.passes_filter(LogLevel::Warn));
        assert!(LogLevel::Warn.passes_filter(LogLevel::Warn));
        assert!(!LogLevel::Info.passes_filter(LogLevel::Warn));
        assert!(!LogLevel::Debug.passes_filter(LogLevel::Warn));
        // "silent" shows only the quietest lines; "debug" shows every real
        // level. An unclassified line is never claimed to be a debug line.
        assert!(LogLevel::Silent.passes_filter(LogLevel::Silent));
        assert!(!LogLevel::Error.passes_filter(LogLevel::Silent));
        assert!(LogLevel::Debug.passes_filter(LogLevel::Debug));
        assert!(!LogLevel::Unknown.passes_filter(LogLevel::Debug));
        assert!(LogLevel::Unknown.passes_filter(LogLevel::Unknown));
    }

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

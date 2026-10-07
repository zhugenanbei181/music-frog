//! Neutral log semantics and session-fenced stream observations.
use crate::error::Failure;
use crate::session::SessionToken;
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogSession {
    pub generation: u64,
    pub token: SessionToken,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogStreamState {
    #[default]
    Idle,
    Connecting,
    Live,
    Reconnecting(Failure),
    Failed(Failure),
}

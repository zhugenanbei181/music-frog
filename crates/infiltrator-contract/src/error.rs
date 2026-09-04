use serde::{Deserialize, Serialize};

/// Stable application-facing error with no dependency on a transport, host,
/// executor, or UI toolkit. Adapters convert their concrete failures into
/// these owned strings at the boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InfiltratorError {
    Mihomo(String),
    Config(String),
    Io(String),
    Download(String),
    Sync(String),
    Auth(String),
    Internal(String),
    Privilege(String),
}

impl std::fmt::Display for InfiltratorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (label, message) = match self {
            Self::Mihomo(message) => ("Mihomo API error", message),
            Self::Config(message) => ("Configuration error", message),
            Self::Io(message) => ("IO error", message),
            Self::Download(message) => ("Download error", message),
            Self::Sync(message) => ("Sync error", message),
            Self::Auth(message) => ("Auth error", message),
            Self::Internal(message) => ("Internal error", message),
            Self::Privilege(message) => ("Privilege error", message),
        };
        write!(formatter, "{label}: {message}")
    }
}

impl std::error::Error for InfiltratorError {}

impl From<String> for InfiltratorError {
    fn from(message: String) -> Self {
        Self::Internal(message)
    }
}

impl From<std::io::Error> for InfiltratorError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<anyhow::Error> for InfiltratorError {
    fn from(error: anyhow::Error) -> Self {
        Self::Internal(error.to_string())
    }
}

/// Convert a concrete controller/transport failure at an inbound adapter
/// without making this contract crate depend on that transport's error type.
pub fn from_mihomo<E: std::fmt::Display>(error: E) -> InfiltratorError {
    InfiltratorError::Mihomo(error.to_string())
}

/// Stable machine-readable failure categories shared by every surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    InvalidInput,
    InvalidState,
    NotReady,
    Unsupported,
    Network,
    Authentication,
    Configuration,
    Storage,
    Permission,
    Canceled,
    Internal,
}

/// A user-presentable, serializable failure without a dependency on any
/// adapter's error type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    pub code: ErrorCode,
    pub message: String,
    pub retryable: bool,
}

impl Failure {
    pub fn new(code: ErrorCode, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
        }
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unsupported, message, false)
    }
}

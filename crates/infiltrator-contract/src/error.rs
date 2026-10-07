use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::{error, fmt, io};

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

impl Display for InfiltratorError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
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

impl error::Error for InfiltratorError {}

impl From<String> for InfiltratorError {
    fn from(message: String) -> Self {
        Self::Internal(message)
    }
}

impl From<io::Error> for InfiltratorError {
    fn from(error: io::Error) -> Self {
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
pub fn from_mihomo<E: Display>(error: E) -> InfiltratorError {
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
    /// Product-owned reason; `message` remains opaque diagnostic detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<FailureReason>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureReason {
    YamlSyntax { line: usize, column: usize },
    MixinYaml,
    ActiveProfileInconsistent,
    QuotaSourceChanged,
    CurrentTimeUnavailable,
    DownloadCanceled,
}

impl Failure {
    pub fn new(code: ErrorCode, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
            reason: None,
        }
    }

    pub fn with_reason(mut self, reason: FailureReason) -> Self {
        self.reason = Some(reason);
        self
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unsupported, message, false)
    }
}

impl Display for Failure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}
impl error::Error for Failure {}

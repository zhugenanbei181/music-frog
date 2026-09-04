use serde::{Deserialize, Serialize};

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

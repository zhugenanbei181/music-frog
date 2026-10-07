use infiltrator_contract::capability::Capability;
use infiltrator_contract::error::{ErrorCode, Failure};
use std::fmt::{Display, Formatter};
use std::{error, fmt};

/// Adapter failure without coupling the application boundary to a concrete
/// transport, OS, or runtime error type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PortError {
    /// An adapter's precise application category and recovery policy.
    Rejected(Failure),
    Unsupported {
        capability: Capability,
        reason: String,
    },
    PermissionDenied(String),
    NotFound(String),
    Io(String),
    Network(String),
    Failed(String),
}

impl PortError {
    pub fn unsupported(capability: Capability, reason: impl Into<String>) -> Self {
        Self::Unsupported {
            capability,
            reason: reason.into(),
        }
    }
}

impl PortError {
    pub fn error_code(&self) -> ErrorCode {
        match self {
            Self::Rejected(failure) => failure.code.clone(),
            Self::Unsupported { .. } => ErrorCode::Unsupported,
            Self::PermissionDenied(_) => ErrorCode::Permission,
            Self::NotFound(_) | Self::Io(_) => ErrorCode::Storage,
            Self::Network(_) => ErrorCode::Network,
            Self::Failed(_) => ErrorCode::Internal,
        }
    }
}

impl From<PortError> for Failure {
    fn from(error: PortError) -> Self {
        if let PortError::Rejected(failure) = error {
            return failure;
        }
        let retryable = matches!(&error, PortError::Network(_) | PortError::Io(_));
        Self::new(error.error_code(), error.to_string(), retryable)
    }
}

impl Display for PortError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(failure) => write!(formatter, "{}", failure.message),
            Self::Unsupported { capability, reason } => {
                write!(formatter, "{capability:?} unsupported: {reason}")
            }
            Self::PermissionDenied(message) => write!(formatter, "permission denied: {message}"),
            Self::NotFound(message) => write!(formatter, "not found: {message}"),
            Self::Io(message) => write!(formatter, "storage error: {message}"),
            Self::Network(message) => write!(formatter, "network error: {message}"),
            Self::Failed(message) => write!(formatter, "adapter failure: {message}"),
        }
    }
}

impl error::Error for PortError {}

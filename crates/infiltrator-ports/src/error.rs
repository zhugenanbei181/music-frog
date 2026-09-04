use infiltrator_contract::capability::Capability;
use infiltrator_contract::error::{ErrorCode, Failure};

/// Adapter failure without coupling the application boundary to a concrete
/// transport, OS, or runtime error type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PortError {
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
        let retryable = matches!(&error, PortError::Network(_) | PortError::Io(_));
        Self::new(error.error_code(), error.to_string(), retryable)
    }
}

impl std::fmt::Display for PortError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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

impl std::error::Error for PortError {}

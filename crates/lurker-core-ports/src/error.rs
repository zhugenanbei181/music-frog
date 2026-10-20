//! Neutral adapter failure vocabulary.
//!
//! A port never leaks a transport, OS or runtime error type into the
//! application boundary. It converts the concrete failure into a
//! [`PortError`], which carries either the adapter's precise application
//! category ([`PortError::Rejected`]) or the neutral fallback classes.

use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};
use std::fmt::{Display, Formatter};
use std::{error, fmt, io};

/// Adapter failure without coupling the application boundary to a concrete
/// transport, OS, or runtime error type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PortError {
    /// An adapter's precise application category and recovery policy.
    Rejected(Failure),
    /// The engine cannot perform the requested capability at all.
    Unsupported {
        /// The capability the engine does not provide.
        capability: Capability,
        /// Human-readable reason the capability is absent.
        reason: String,
    },
    /// The process lacks the permission the operation needs.
    PermissionDenied(String),
    /// The requested resource does not exist.
    NotFound(String),
    /// A filesystem or other input/output operation failed.
    Io(String),
    /// A network operation failed.
    Network(String),
    /// The adapter failed for a reason that has no neutral class.
    Failed(String),
}

impl PortError {
    /// Builds an `Unsupported` failure for one capability.
    pub fn unsupported(capability: Capability, reason: impl Into<String>) -> Self {
        Self::Unsupported {
            capability,
            reason: reason.into(),
        }
    }
}

impl From<io::Error> for PortError {
    fn from(error: io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl PortError {
    /// Maps the port failure to the neutral application error code.
    ///
    /// A [`PortError::Rejected`] keeps the precise code the adapter chose;
    /// every other variant maps to its neutral class.
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

#[cfg(test)]
mod tests {
    use super::PortError;
    use lurker_core_contract::capability::Capability;
    use lurker_core_contract::error::{ErrorCode, Failure};

    #[test]
    fn adapter_error_maps_to_contract_failure_without_runtime_types() {
        let failure: Failure = PortError::unsupported(
            Capability::SystemProxy,
            "Android VpnService owns proxy routing",
        )
        .into();
        assert_eq!(failure.code, ErrorCode::Unsupported);
        assert!(!failure.retryable);
        assert!(failure.message.contains("SystemProxy"));
    }
}

//! Engine-neutral failure vocabulary.
//!
//! Two layers live here: [`InfiltratorError`] is the coarse, owned error an
//! inbound adapter converts into, and [`Failure`] is the fine-grained,
//! user-presentable failure every surface renders. Both are transport-,
//! host- and toolkit-free so a neutral port can report a failure without
//! naming the backend that produced it.

use crate::owner_switch::{ActiveProfileRecoveryOutcome, CoreApplyFailureKind};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::{error, fmt, io};

/// Stable application-facing error with no dependency on a transport, host,
/// executor, or UI toolkit. Adapters convert their concrete failures into
/// these owned strings at the boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InfiltratorError {
    /// The proxy core or its control API failed.
    #[serde(alias = "Mihomo")]
    Backend(String),
    /// The configuration was rejected by the core or by this application.
    Config(String),
    /// A filesystem or other input/output operation failed.
    Io(String),
    /// A download of profiles or core binaries failed.
    Download(String),
    /// A synchronisation (for example WebDAV) failed.
    Sync(String),
    /// An authentication or authorisation step failed.
    Auth(String),
    /// An unexpected internal failure occurred.
    Internal(String),
    /// The operation needs privileges the current process does not hold.
    Privilege(String),
}

impl Display for InfiltratorError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let (label, message) = match self {
            Self::Backend(message) => ("Backend error", message),
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

/// Convert a concrete backend failure at an inbound adapter without making
/// this contract crate depend on that backend's error type.
pub fn from_backend<E: Display>(error: E) -> InfiltratorError {
    InfiltratorError::Backend(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{InfiltratorError, from_backend};

    #[test]
    fn backend_error_writes_neutral_tag_and_reads_legacy_tag() {
        let error = InfiltratorError::Backend("connection refused".to_owned());
        let serialized = serde_json::to_string(&error).unwrap();
        assert_eq!(serialized, r#"{"Backend":"connection refused"}"#);
        assert_eq!(
            serde_json::from_str::<InfiltratorError>(&serialized).unwrap(),
            error
        );
        assert_eq!(
            serde_json::from_str::<InfiltratorError>(r#"{"Mihomo":"connection refused"}"#).unwrap(),
            error
        );
        assert_eq!(error.to_string(), "Backend error: connection refused");
    }

    #[test]
    fn backend_conversion_preserves_displayed_message() {
        assert_eq!(
            from_backend("controller unavailable"),
            InfiltratorError::Backend("controller unavailable".to_owned())
        );
    }
}

/// Stable machine-readable failure categories shared by every surface.
///
/// The category is the part a program branches on; the accompanying message
/// is diagnostic text and must not be parsed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    /// The caller supplied an invalid argument or payload.
    InvalidInput,
    /// The operation is not allowed in the current state.
    InvalidState,
    /// The target is not ready yet.
    NotReady,
    /// The requested capability is not supported by this host or engine.
    Unsupported,
    /// A network operation failed.
    Network,
    /// Authentication or authorisation failed.
    Authentication,
    /// The configuration was rejected.
    Configuration,
    /// A storage operation failed.
    Storage,
    /// The process lacks the permission the operation needs.
    Permission,
    /// The operation was cancelled.
    Canceled,
    /// An unexpected internal failure occurred.
    Internal,
}

/// A user-presentable, serializable failure without a dependency on any
/// adapter's error type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    /// Machine-readable category of the failure.
    pub code: ErrorCode,
    /// Human-readable, already-localized-or-localizable diagnostic text.
    pub message: String,
    /// Whether retrying the same operation may succeed.
    pub retryable: bool,
    /// Product-owned reason; `message` remains opaque diagnostic detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<FailureReason>,
}

/// The typed reason behind a [`Failure`].
///
/// A reason is the machine-actionable part of a failure: it names the exact
/// condition (a stale revision, a rejected Core apply) so a caller can decide
/// what to do without matching on message text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureReason {
    /// The YAML document could not be parsed at the given position.
    YamlSyntax {
        /// Zero-based line of the syntax error.
        line: usize,
        /// Zero-based column of the syntax error.
        column: usize,
    },
    /// The YAML mixin could not be applied to the base document.
    MixinYaml,
    /// The persisted active-profile record is inconsistent with the selection.
    ActiveProfileInconsistent,
    /// The quota source changed while the request was in flight.
    QuotaSourceChanged,
    /// The current time could not be determined.
    CurrentTimeUnavailable,
    /// The download was cancelled by the caller.
    DownloadCanceled,
    /// The active-profile reference moved on before the write was committed.
    ActiveProfileReferenceStale {
        /// Revision the writer expected to observe.
        expected_revision: u64,
        /// Revision the writer actually observed.
        observed_revision: u64,
    },
    /// The Core apply step failed for a typed reason.
    CoreApplyFailure(CoreApplyFailureKind),
    /// The active-profile recovery step failed for a typed reason.
    ActiveProfileRecovery(ActiveProfileRecoveryOutcome),
}

impl Failure {
    /// Builds a failure without a typed reason.
    pub fn new(code: ErrorCode, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
            reason: None,
        }
    }

    /// Attaches a typed reason to the failure.
    pub fn with_reason(mut self, reason: FailureReason) -> Self {
        self.reason = Some(reason);
        self
    }

    /// Builds a non-retryable `Unsupported` failure.
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

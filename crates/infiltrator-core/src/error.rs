//! The typed error of the core layer (ENG-002.2).
//!
//! Core failures reuse the domain failure categories and add core IO context.
//! Third-party errors cross the boundary through one `From` impl per concrete
//! error type; `anyhow::Error` is only accepted at the layer boundary, never
//! produced inside the layer.

use infiltrator_domain::backup::BackupError;
use infiltrator_domain::error::{DomainError, DomainFailureCategory};
use infiltrator_http::reqwest;
use lurker_core_ports::error::PortError;
use mihomo_driver::error::MihomoError;
use serde_yaml_ng;
use std::error::Error;
use std::fmt;
use std::io;
use toml::{de, ser};
use zip::result::ZipError;

fn category_of_mihomo_error(error: &MihomoError) -> DomainFailureCategory {
    match error {
        MihomoError::Http(_) | MihomoError::Io(_) | MihomoError::WebSocket(_) => {
            DomainFailureCategory::Io
        }
        MihomoError::Service(_) | MihomoError::Proxy(_) => DomainFailureCategory::Io,
        MihomoError::Json(_) | MihomoError::YamlEmit(_) => DomainFailureCategory::Serialize,
        MihomoError::Yaml(_) | MihomoError::UrlParse(_) | MihomoError::Version(_) => {
            DomainFailureCategory::Parse
        }
        MihomoError::Config(_) => DomainFailureCategory::Invalid,
        MihomoError::Canceled => DomainFailureCategory::Other,
        MihomoError::NotFound(_) => DomainFailureCategory::NotFound,
    }
}

/// The typed error of the core layer: one category plus its context message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreError {
    category: DomainFailureCategory,
    message: String,
    io_kind: Option<io::ErrorKind>,
}

impl CoreError {
    /// Build an error from an explicit category.
    pub fn new(category: DomainFailureCategory, message: impl Into<String>) -> Self {
        Self {
            category,
            message: message.into(),
            io_kind: None,
        }
    }

    /// Build an IO error that keeps the concrete `std::io::ErrorKind`.
    pub fn io_with_kind(kind: io::ErrorKind, message: impl Into<String>) -> Self {
        Self {
            category: DomainFailureCategory::Io,
            message: message.into(),
            io_kind: Some(kind),
        }
    }

    /// Input validation rejected a value.
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(DomainFailureCategory::Invalid, message)
    }

    /// A document failed to parse.
    pub fn parse(message: impl Into<String>) -> Self {
        Self::new(DomainFailureCategory::Parse, message)
    }

    /// Serialization to a document format failed.
    pub fn serialize(message: impl Into<String>) -> Self {
        Self::new(DomainFailureCategory::Serialize, message)
    }

    /// Filesystem, network or process IO failed.
    pub fn io(message: impl Into<String>) -> Self {
        Self::new(DomainFailureCategory::Io, message)
    }

    /// A referenced entity does not exist.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(DomainFailureCategory::NotFound, message)
    }

    /// A failure that crosses the layer boundary without a closer category.
    pub fn other(message: impl Into<String>) -> Self {
        Self::new(DomainFailureCategory::Other, message)
    }

    /// The failure category.
    pub const fn category(&self) -> DomainFailureCategory {
        self.category
    }

    /// The context message of this failure.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The concrete IO error kind when this failure originated from IO.
    pub const fn io_kind(&self) -> Option<io::ErrorKind> {
        self.io_kind
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for CoreError {}

impl From<io::Error> for CoreError {
    fn from(error: io::Error) -> Self {
        Self::io_with_kind(error.kind(), error.to_string())
    }
}

impl From<serde_yaml_ng::Error> for CoreError {
    fn from(error: serde_yaml_ng::Error) -> Self {
        Self::parse(error.to_string())
    }
}

impl From<serde_json::Error> for CoreError {
    fn from(error: serde_json::Error) -> Self {
        Self::serialize(error.to_string())
    }
}

impl From<de::Error> for CoreError {
    fn from(error: de::Error) -> Self {
        Self::parse(error.to_string())
    }
}

impl From<ser::Error> for CoreError {
    fn from(error: ser::Error) -> Self {
        Self::serialize(error.to_string())
    }
}

impl From<url::ParseError> for CoreError {
    fn from(error: url::ParseError) -> Self {
        Self::parse(error.to_string())
    }
}

impl From<base64::DecodeError> for CoreError {
    fn from(error: base64::DecodeError) -> Self {
        Self::parse(error.to_string())
    }
}

impl From<ZipError> for CoreError {
    fn from(error: ZipError) -> Self {
        Self::other(error.to_string())
    }
}

/// A domain failure keeps its category when it crosses into core.
impl From<DomainError> for CoreError {
    fn from(error: DomainError) -> Self {
        Self::new(error.category(), error.message().to_owned())
    }
}

impl From<MihomoError> for CoreError {
    fn from(error: MihomoError) -> Self {
        if let MihomoError::Io(io) = &error {
            Self::io_with_kind(io.kind(), error.to_string())
        } else {
            Self::new(category_of_mihomo_error(&error), error.to_string())
        }
    }
}

impl From<PortError> for CoreError {
    fn from(error: PortError) -> Self {
        let category = match &error {
            PortError::NotFound(_) => DomainFailureCategory::NotFound,
            PortError::Io(_) | PortError::Network(_) | PortError::PermissionDenied(_) => {
                DomainFailureCategory::Io
            }
            PortError::Rejected(_) | PortError::Unsupported { .. } | PortError::Failed(_) => {
                DomainFailureCategory::Other
            }
        };
        Self::new(category, error.to_string())
    }
}

impl From<BackupError> for CoreError {
    fn from(error: BackupError) -> Self {
        let category = match &error {
            BackupError::Io(_) => DomainFailureCategory::Io,
            BackupError::Json(_) => DomainFailureCategory::Serialize,
            BackupError::Zip(_) | BackupError::EncryptionFailed(_) => DomainFailureCategory::Other,
            BackupError::InvalidFormat(_) | BackupError::IntegrityMismatch { .. } => {
                DomainFailureCategory::Parse
            }
            BackupError::EmptyPassword | BackupError::DecryptionFailed => {
                DomainFailureCategory::Invalid
            }
        };
        Self::new(category, error.to_string())
    }
}

impl From<reqwest::Error> for CoreError {
    fn from(error: reqwest::Error) -> Self {
        Self::io(error.to_string())
    }
}

/// The layer boundary: an opaque `anyhow::Error` collapses into `Other`.
impl From<anyhow::Error> for CoreError {
    fn from(error: anyhow::Error) -> Self {
        Self::other(error.to_string())
    }
}

/// Attach a category-preserving context message to a fallible value.
///
/// This is the typed replacement for `anyhow::Context`: the category comes
/// from the concrete error type, and an already-typed error keeps its own
/// category.
pub trait CoreContext<T> {
    /// Wrap the error with `message` as its outer context.
    fn core_context(self, message: impl Into<String>) -> Result<T, CoreError>;

    /// Wrap the error with a lazily-built context message.
    fn core_context_with(self, message: impl FnOnce() -> String) -> Result<T, CoreError>;
}

/// Flatten `context: message` into one line the way `anyhow` renders chains.
fn with_context(context: &str, error: &str) -> String {
    format!("{context}: {error}")
}

impl<T> CoreContext<T> for Result<T, io::Error> {
    fn core_context(self, message: impl Into<String>) -> Result<T, CoreError> {
        self.map_err(|error| {
            CoreError::io_with_kind(
                error.kind(),
                with_context(&message.into(), &error.to_string()),
            )
        })
    }

    fn core_context_with(self, message: impl FnOnce() -> String) -> Result<T, CoreError> {
        self.map_err(|error| {
            CoreError::io_with_kind(error.kind(), with_context(&message(), &error.to_string()))
        })
    }
}

impl<T> CoreContext<T> for Result<T, serde_yaml_ng::Error> {
    fn core_context(self, message: impl Into<String>) -> Result<T, CoreError> {
        self.map_err(|error| CoreError::parse(with_context(&message.into(), &error.to_string())))
    }

    fn core_context_with(self, message: impl FnOnce() -> String) -> Result<T, CoreError> {
        self.map_err(|error| CoreError::parse(with_context(&message(), &error.to_string())))
    }
}

impl<T> CoreContext<T> for Result<T, serde_json::Error> {
    fn core_context(self, message: impl Into<String>) -> Result<T, CoreError> {
        self.map_err(|error| {
            CoreError::serialize(with_context(&message.into(), &error.to_string()))
        })
    }

    fn core_context_with(self, message: impl FnOnce() -> String) -> Result<T, CoreError> {
        self.map_err(|error| CoreError::serialize(with_context(&message(), &error.to_string())))
    }
}

impl<T> CoreContext<T> for Result<T, DomainError> {
    fn core_context(self, message: impl Into<String>) -> Result<T, CoreError> {
        self.map_err(|error| {
            CoreError::new(
                error.category(),
                with_context(&message.into(), error.message()),
            )
        })
    }

    fn core_context_with(self, message: impl FnOnce() -> String) -> Result<T, CoreError> {
        self.map_err(|error| {
            CoreError::new(error.category(), with_context(&message(), error.message()))
        })
    }
}

impl<T> CoreContext<T> for Result<T, MihomoError> {
    fn core_context(self, message: impl Into<String>) -> Result<T, CoreError> {
        self.map_err(|error| {
            CoreError::new(
                category_of_mihomo_error(&error),
                with_context(&message.into(), &error.to_string()),
            )
        })
    }

    fn core_context_with(self, message: impl FnOnce() -> String) -> Result<T, CoreError> {
        self.map_err(|error| {
            CoreError::new(
                category_of_mihomo_error(&error),
                with_context(&message(), &error.to_string()),
            )
        })
    }
}

impl<T> CoreContext<T> for Result<T, PortError> {
    fn core_context(self, message: impl Into<String>) -> Result<T, CoreError> {
        self.map_err(|error| {
            let converted = CoreError::from(error);
            CoreError::new(
                converted.category,
                with_context(&message.into(), &converted.message),
            )
        })
    }

    fn core_context_with(self, message: impl FnOnce() -> String) -> Result<T, CoreError> {
        self.map_err(|error| {
            let converted = CoreError::from(error);
            CoreError::new(
                converted.category,
                with_context(&message(), &converted.message),
            )
        })
    }
}

impl<T> CoreContext<T> for Result<T, CoreError> {
    fn core_context(self, message: impl Into<String>) -> Result<T, CoreError> {
        self.map_err(|error| {
            CoreError::new(
                error.category,
                with_context(&message.into(), &error.message),
            )
        })
    }

    fn core_context_with(self, message: impl FnOnce() -> String) -> Result<T, CoreError> {
        self.map_err(|error| {
            CoreError::new(error.category, with_context(&message(), &error.message))
        })
    }
}

impl<T> CoreContext<T> for Option<T> {
    fn core_context(self, message: impl Into<String>) -> Result<T, CoreError> {
        self.ok_or_else(|| CoreError::not_found(message.into()))
    }

    fn core_context_with(self, message: impl FnOnce() -> String) -> Result<T, CoreError> {
        self.ok_or_else(|| CoreError::not_found(message()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_carry_their_category() {
        let error = CoreError::io("settings file missing");
        assert_eq!(error.category(), DomainFailureCategory::Io);
        assert_eq!(error.message(), "settings file missing");
        assert_eq!(error.to_string(), "settings file missing");
        assert_eq!(
            CoreError::invalid("profile name empty").category(),
            DomainFailureCategory::Invalid
        );
    }

    #[test]
    fn concrete_third_party_errors_convert_through_one_boundary() {
        let io: CoreError = io::Error::other("disk").into();
        assert_eq!(io.category(), DomainFailureCategory::Io);

        let yaml: CoreError = serde_yaml_ng::from_str::<serde_yaml_ng::Value>("{")
            .unwrap_err()
            .into();
        assert_eq!(yaml.category(), DomainFailureCategory::Parse);

        let json: CoreError = serde_json::from_str::<serde_json::Value>("{")
            .unwrap_err()
            .into();
        assert_eq!(json.category(), DomainFailureCategory::Serialize);
    }

    #[test]
    fn domain_errors_keep_their_category_in_core() {
        let domain = DomainError::not_found("profile missing");
        let core: CoreError = domain.into();
        assert_eq!(core.category(), DomainFailureCategory::NotFound);
        assert_eq!(core.message(), "profile missing");
    }

    #[test]
    fn context_preserves_the_category_of_the_source() {
        let io_error: Result<(), io::Error> = Err(io::Error::other("disk"));
        let wrapped = io_error.core_context("读取设置失败").unwrap_err();
        assert_eq!(wrapped.category(), DomainFailureCategory::Io);
        assert_eq!(wrapped.message(), "读取设置失败: disk");

        let missing: Option<u8> = None;
        let wrapped = missing.core_context("profile missing").unwrap_err();
        assert_eq!(wrapped.category(), DomainFailureCategory::NotFound);
    }

    #[test]
    fn the_anyhow_boundary_collapses_into_other() {
        let boundary: CoreError = anyhow::anyhow!("legacy failure").into();
        assert_eq!(boundary.category(), DomainFailureCategory::Other);
    }
}

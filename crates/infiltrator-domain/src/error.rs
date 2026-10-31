//! The typed error of the domain layer (ENG-002.2).
//!
//! Domain failures carry one reusable failure category plus their context.
//! The category is a closed set, so callers can branch on the failure kind
//! instead of parsing message text. Third-party errors cross the boundary
//! through one `From` impl per concrete error type; `anyhow::Error` is only
//! accepted at the layer boundary, never produced inside the layer.

use serde_yaml_ng;
use std::error::Error;
use std::fmt;
use std::io;
use std::net::AddrParseError;
use std::num::{ParseFloatError, ParseIntError};
use std::str::Utf8Error;
use std::string::FromUtf8Error;
use zip::result::ZipError;

/// The reusable failure categories of the domain layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DomainFailureCategory {
    /// Input validation rejected a value.
    Invalid,
    /// A document failed to parse.
    Parse,
    /// Serialization to a document format failed.
    Serialize,
    /// Filesystem or process IO failed.
    Io,
    /// A referenced entity does not exist.
    NotFound,
    /// A failure that crosses the layer boundary without a closer category.
    Other,
}

impl DomainFailureCategory {
    /// Every category in stable enumeration order.
    pub const ALL: [Self; 6] = [
        Self::Invalid,
        Self::Parse,
        Self::Serialize,
        Self::Io,
        Self::NotFound,
        Self::Other,
    ];

    /// The localized Chinese label of the category.
    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::Invalid => "取值无效",
            Self::Parse => "解析失败",
            Self::Serialize => "序列化失败",
            Self::Io => "读写失败",
            Self::NotFound => "未找到",
            Self::Other => "其他",
        }
    }
}

/// The typed error of the domain layer: one category plus its context message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainError {
    category: DomainFailureCategory,
    message: String,
}

impl DomainError {
    /// Build an error from an explicit category.
    pub fn new(category: DomainFailureCategory, message: impl Into<String>) -> Self {
        Self {
            category,
            message: message.into(),
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

    /// Filesystem or process IO failed.
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
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for DomainError {}

impl From<io::Error> for DomainError {
    fn from(error: io::Error) -> Self {
        Self::io(error.to_string())
    }
}

impl From<serde_yaml_ng::Error> for DomainError {
    fn from(error: serde_yaml_ng::Error) -> Self {
        Self::parse(error.to_string())
    }
}

impl From<serde_json::Error> for DomainError {
    fn from(error: serde_json::Error) -> Self {
        Self::serialize(error.to_string())
    }
}

impl From<url::ParseError> for DomainError {
    fn from(error: url::ParseError) -> Self {
        Self::parse(error.to_string())
    }
}

impl From<Utf8Error> for DomainError {
    fn from(error: Utf8Error) -> Self {
        Self::parse(error.to_string())
    }
}

impl From<FromUtf8Error> for DomainError {
    fn from(error: FromUtf8Error) -> Self {
        Self::parse(error.to_string())
    }
}

impl From<ParseIntError> for DomainError {
    fn from(error: ParseIntError) -> Self {
        Self::parse(error.to_string())
    }
}

impl From<ParseFloatError> for DomainError {
    fn from(error: ParseFloatError) -> Self {
        Self::parse(error.to_string())
    }
}

impl From<AddrParseError> for DomainError {
    fn from(error: AddrParseError) -> Self {
        Self::parse(error.to_string())
    }
}

impl From<regex::Error> for DomainError {
    fn from(error: regex::Error) -> Self {
        Self::parse(error.to_string())
    }
}

impl From<base64::DecodeError> for DomainError {
    fn from(error: base64::DecodeError) -> Self {
        Self::parse(error.to_string())
    }
}

impl From<ipnet::AddrParseError> for DomainError {
    fn from(error: ipnet::AddrParseError) -> Self {
        Self::parse(error.to_string())
    }
}

impl From<ZipError> for DomainError {
    fn from(error: ZipError) -> Self {
        Self::other(error.to_string())
    }
}

/// The layer boundary: an opaque `anyhow::Error` collapses into `Other`.
impl From<anyhow::Error> for DomainError {
    fn from(error: anyhow::Error) -> Self {
        Self::other(error.to_string())
    }
}

/// Attach a category-preserving context message to a fallible value.
///
/// This is the typed replacement for `anyhow::Context`: the category comes
/// from the concrete error type (IO stays IO, YAML stays Parse), and an
/// already-typed [`DomainError`] keeps its own category.
pub trait DomainContext<T> {
    /// Wrap the error with `message` as its outer context.
    fn domain_context(self, message: impl Into<String>) -> Result<T, DomainError>;

    /// Wrap the error with a lazily-built context message.
    fn domain_context_with(self, message: impl FnOnce() -> String) -> Result<T, DomainError>;
}

/// Flatten `context: message` into one line the way `anyhow` renders chains.
fn with_context(context: &str, error: &str) -> String {
    format!("{context}: {error}")
}

impl<T> DomainContext<T> for Result<T, io::Error> {
    fn domain_context(self, message: impl Into<String>) -> Result<T, DomainError> {
        self.map_err(|error| DomainError::io(with_context(&message.into(), &error.to_string())))
    }

    fn domain_context_with(self, message: impl FnOnce() -> String) -> Result<T, DomainError> {
        self.map_err(|error| DomainError::io(with_context(&message(), &error.to_string())))
    }
}

impl<T> DomainContext<T> for Result<T, serde_yaml_ng::Error> {
    fn domain_context(self, message: impl Into<String>) -> Result<T, DomainError> {
        self.map_err(|error| DomainError::parse(with_context(&message.into(), &error.to_string())))
    }

    fn domain_context_with(self, message: impl FnOnce() -> String) -> Result<T, DomainError> {
        self.map_err(|error| DomainError::parse(with_context(&message(), &error.to_string())))
    }
}

impl<T> DomainContext<T> for Result<T, serde_json::Error> {
    fn domain_context(self, message: impl Into<String>) -> Result<T, DomainError> {
        self.map_err(|error| {
            DomainError::serialize(with_context(&message.into(), &error.to_string()))
        })
    }

    fn domain_context_with(self, message: impl FnOnce() -> String) -> Result<T, DomainError> {
        self.map_err(|error| DomainError::serialize(with_context(&message(), &error.to_string())))
    }
}

impl<T> DomainContext<T> for Result<T, regex::Error> {
    fn domain_context(self, message: impl Into<String>) -> Result<T, DomainError> {
        self.map_err(|error| DomainError::parse(with_context(&message.into(), &error.to_string())))
    }

    fn domain_context_with(self, message: impl FnOnce() -> String) -> Result<T, DomainError> {
        self.map_err(|error| DomainError::parse(with_context(&message(), &error.to_string())))
    }
}

impl<T> DomainContext<T> for Result<T, Utf8Error> {
    fn domain_context(self, message: impl Into<String>) -> Result<T, DomainError> {
        self.map_err(|error| DomainError::parse(with_context(&message.into(), &error.to_string())))
    }

    fn domain_context_with(self, message: impl FnOnce() -> String) -> Result<T, DomainError> {
        self.map_err(|error| DomainError::parse(with_context(&message(), &error.to_string())))
    }
}

impl<T> DomainContext<T> for Result<T, FromUtf8Error> {
    fn domain_context(self, message: impl Into<String>) -> Result<T, DomainError> {
        self.map_err(|error| DomainError::parse(with_context(&message.into(), &error.to_string())))
    }

    fn domain_context_with(self, message: impl FnOnce() -> String) -> Result<T, DomainError> {
        self.map_err(|error| DomainError::parse(with_context(&message(), &error.to_string())))
    }
}

impl<T> DomainContext<T> for Result<T, DomainError> {
    fn domain_context(self, message: impl Into<String>) -> Result<T, DomainError> {
        self.map_err(|error| {
            DomainError::new(
                error.category,
                with_context(&message.into(), &error.message),
            )
        })
    }

    fn domain_context_with(self, message: impl FnOnce() -> String) -> Result<T, DomainError> {
        self.map_err(|error| {
            DomainError::new(error.category, with_context(&message(), &error.message))
        })
    }
}

impl<T> DomainContext<T> for Option<T> {
    fn domain_context(self, message: impl Into<String>) -> Result<T, DomainError> {
        self.ok_or_else(|| DomainError::not_found(message.into()))
    }

    fn domain_context_with(self, message: impl FnOnce() -> String) -> Result<T, DomainError> {
        self.ok_or_else(|| DomainError::not_found(message()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml_ng::Value;

    #[test]
    fn categories_are_labeled_and_enumerable() {
        assert_eq!(DomainFailureCategory::ALL.len(), 6);
        for category in DomainFailureCategory::ALL {
            assert!(!category.label_zh().is_empty());
        }
    }

    #[test]
    fn constructors_carry_their_category() {
        let error = DomainError::invalid("端口超出范围");
        assert_eq!(error.category(), DomainFailureCategory::Invalid);
        assert_eq!(error.message(), "端口超出范围");
        assert_eq!(error.to_string(), "端口超出范围");
        assert_eq!(
            DomainError::io("disk full").category(),
            DomainFailureCategory::Io
        );
    }

    #[test]
    fn concrete_third_party_errors_convert_through_one_boundary() {
        let io: DomainError = io::Error::other("disk").into();
        assert_eq!(io.category(), DomainFailureCategory::Io);

        let yaml: DomainError = serde_yaml_ng::from_str::<Value>("{").unwrap_err().into();
        assert_eq!(yaml.category(), DomainFailureCategory::Parse);

        let json: DomainError = serde_json::from_str::<serde_json::Value>("{")
            .unwrap_err()
            .into();
        assert_eq!(json.category(), DomainFailureCategory::Serialize);
    }

    #[test]
    fn context_preserves_the_category_of_the_source() {
        let io_error: Result<(), io::Error> = Err(io::Error::other("disk"));
        let wrapped = io_error.domain_context("读取设置失败").unwrap_err();
        assert_eq!(wrapped.category(), DomainFailureCategory::Io);
        assert_eq!(wrapped.message(), "读取设置失败: disk");

        let typed: Result<(), DomainError> = Err(DomainError::invalid("端口无效"));
        let wrapped = typed.domain_context("保存设置失败").unwrap_err();
        assert_eq!(wrapped.category(), DomainFailureCategory::Invalid);
        assert_eq!(wrapped.message(), "保存设置失败: 端口无效");
    }

    #[test]
    fn option_context_maps_to_not_found() {
        let missing: Option<u8> = None;
        assert_eq!(
            missing.domain_context("profile").unwrap_err().category(),
            DomainFailureCategory::NotFound
        );
    }

    #[test]
    fn the_anyhow_boundary_collapses_into_other() {
        let boundary: DomainError = anyhow::anyhow!("legacy failure").into();
        assert_eq!(boundary.category(), DomainFailureCategory::Other);
        assert_eq!(boundary.message(), "legacy failure");
    }
}

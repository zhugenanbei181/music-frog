use std::{io, result};
use thiserror::Error;
#[cfg(feature = "http")]
use tokio_tungstenite::tungstenite;

/// The typed failure of the Mihomo driver.
///
/// The variants name the concrete subsystem that failed; the driver converts
/// them into the neutral [`lurker_core_ports::error::PortError`] at the
/// adapter boundary.
#[derive(Error, Debug)]
pub enum MihomoError {
    /// The controller request failed at the HTTP layer.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// A filesystem or process operation failed.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// A controller payload could not be decoded.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// A native YAML document could not be scanned.
    #[error("YAML error: {0}")]
    Yaml(#[from] yaml_rust2::ScanError),

    /// A native YAML document could not be emitted.
    #[error("YAML emit error: {0}")]
    YamlEmit(String),

    /// A URL could not be parsed.
    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    /// The controller WebSocket failed.
    #[cfg(feature = "http")]
    #[error("WebSocket error: {0}")]
    WebSocket(Box<tungstenite::Error>),

    /// The driver was configured with an invalid value.
    #[error("Config error: {0}")]
    Config(String),

    /// The core service itself failed.
    #[error("Service error: {0}")]
    Service(String),

    /// The core version could not be determined or is unsupported.
    #[error("Version error: {0}")]
    Version(String),

    /// The operation was cancelled by the caller.
    #[error("Operation canceled")]
    Canceled,

    /// A proxy operation failed.
    #[error("Proxy error: {0}")]
    Proxy(String),

    /// The requested resource does not exist.
    #[error("Not found: {0}")]
    NotFound(String),
}

// Manual From implementation for WebSocket error to box it
#[cfg(feature = "http")]
impl From<tungstenite::Error> for MihomoError {
    fn from(err: tungstenite::Error) -> Self {
        MihomoError::WebSocket(Box::new(err))
    }
}

/// The result type of every fallible Mihomo driver operation.
pub type Result<T> = result::Result<T, MihomoError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_error_display() {
        let err = MihomoError::Config("invalid configuration".to_string());
        assert_eq!(err.to_string(), "Config error: invalid configuration");
    }

    #[test]
    fn test_service_error_display() {
        let err = MihomoError::Service("failed to start".to_string());
        assert_eq!(err.to_string(), "Service error: failed to start");
    }

    #[test]
    fn test_version_error_display() {
        let err = MihomoError::Version("version not found".to_string());
        assert_eq!(err.to_string(), "Version error: version not found");
    }

    #[test]
    fn test_proxy_error_display() {
        let err = MihomoError::Proxy("proxy unavailable".to_string());
        assert_eq!(err.to_string(), "Proxy error: proxy unavailable");
    }

    #[test]
    fn test_not_found_error_display() {
        let err = MihomoError::NotFound("resource not found".to_string());
        assert_eq!(err.to_string(), "Not found: resource not found");
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let mihomo_err: MihomoError = io_err.into();
        assert!(matches!(mihomo_err, MihomoError::Io(_)));
    }

    #[test]
    fn test_json_error_conversion() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let mihomo_err: MihomoError = json_err.into();
        assert!(matches!(mihomo_err, MihomoError::Json(_)));
    }

    #[test]
    fn test_url_parse_error_conversion() {
        let url_err = url::Url::parse("not a url").unwrap_err();
        let mihomo_err: MihomoError = url_err.into();
        assert!(matches!(mihomo_err, MihomoError::UrlParse(_)));
    }

    #[test]
    fn test_result_type() {
        fn returns_ok() -> Result<i32> {
            Ok(42)
        }

        fn returns_err() -> Result<i32> {
            Err(MihomoError::Config("test error".to_string()))
        }

        assert_eq!(returns_ok().unwrap(), 42);
        assert!(returns_err().is_err());
    }
}

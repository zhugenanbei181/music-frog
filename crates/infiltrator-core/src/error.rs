#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum InfiltratorError {
    #[error("Mihomo API error: {0}")]
    Mihomo(String),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("Download error: {0}")]
    Download(String),
    #[error("Sync error: {0}")]
    Sync(String),
    #[error("Auth error: {0}")]
    Auth(String),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Privilege error: {0}")]
    Privilege(String),
}

impl From<String> for InfiltratorError {
    fn from(s: String) -> Self {
        InfiltratorError::Internal(s)
    }
}

impl From<mihomo_api::MihomoError> for InfiltratorError {
    fn from(e: mihomo_api::MihomoError) -> Self {
        InfiltratorError::Mihomo(e.to_string())
    }
}

impl From<std::io::Error> for InfiltratorError {
    fn from(e: std::io::Error) -> Self {
        InfiltratorError::Io(e.to_string())
    }
}

impl From<anyhow::Error> for InfiltratorError {
    fn from(e: anyhow::Error) -> Self {
        InfiltratorError::Internal(e.to_string())
    }
}

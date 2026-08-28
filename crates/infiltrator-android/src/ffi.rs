use infiltrator_core::error::InfiltratorError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
#[repr(u32)]
pub enum FfiErrorCode {
    Ok = 0,
    InvalidState = 1,
    InvalidInput = 2,
    NotReady = 3,
    NotSupported = 4,
    Io = 5,
    Network = 6,
    Auth = 7,
    Sync = 8,
    Config = 9,
    Unknown = 255,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiStatus {
    pub code: FfiErrorCode,
    pub message: Option<String>,
}

impl FfiStatus {
    pub fn ok() -> Self {
        Self {
            code: FfiErrorCode::Ok,
            message: None,
        }
    }

    pub fn err(code: FfiErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: Some(message.into()),
        }
    }
}

impl From<InfiltratorError> for FfiStatus {
    fn from(err: InfiltratorError) -> Self {
        let code = match err {
            InfiltratorError::Mihomo(_) => FfiErrorCode::Network,
            InfiltratorError::Config(_) => FfiErrorCode::Config,
            InfiltratorError::Io(_) => FfiErrorCode::Io,
            InfiltratorError::Download(_) => FfiErrorCode::Network,
            InfiltratorError::Sync(_) => FfiErrorCode::Sync,
            InfiltratorError::Auth(_) => FfiErrorCode::Auth,
            InfiltratorError::Internal(_) => FfiErrorCode::Unknown,
            InfiltratorError::Privilege(_) => FfiErrorCode::InvalidState,
        };
        Self::err(code, err.to_string())
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiStringResult {
    pub status: FfiStatus,
    pub value: Option<String>,
}

impl FfiStringResult {
    pub fn ok(value: Option<String>) -> Self {
        Self {
            status: FfiStatus::ok(),
            value,
        }
    }

    pub fn err(code: FfiErrorCode, message: impl Into<String>) -> Self {
        Self {
            status: FfiStatus::err(code, message),
            value: None,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiBoolResult {
    pub status: FfiStatus,
    pub value: bool,
}

impl FfiBoolResult {
    pub fn ok(value: bool) -> Self {
        Self {
            status: FfiStatus::ok(),
            value,
        }
    }

    pub fn err(code: FfiErrorCode, message: impl Into<String>) -> Self {
        Self {
            status: FfiStatus::err(code, message),
            value: false,
        }
    }
}

pub trait FfiApi: Send + Sync {
    fn core_start(&self) -> FfiStatus;
    fn core_stop(&self) -> FfiStatus;
    fn core_is_running(&self) -> FfiBoolResult;
    fn controller_url(&self) -> FfiStringResult;

    fn credential_get(&self, service: &str, key: &str) -> FfiStringResult;
    fn credential_set(&self, service: &str, key: &str, value: &str) -> FfiStatus;
    fn credential_delete(&self, service: &str, key: &str) -> FfiStatus;

    fn data_dir(&self) -> FfiStringResult;
    fn cache_dir(&self) -> FfiStringResult;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_ok() {
        let status = FfiStatus::ok();
        assert_eq!(status.code, FfiErrorCode::Ok);
        assert!(status.message.is_none());
    }

    #[test]
    fn test_status_err() {
        let status = FfiStatus::err(FfiErrorCode::InvalidInput, "bad input");
        assert_eq!(status.code, FfiErrorCode::InvalidInput);
        assert_eq!(status.message, Some("bad input".to_string()));
    }

    #[test]
    fn test_ffi_status_from_infiltrator_error() {
        let err = InfiltratorError::Auth("invalid pass".to_string());
        let status: FfiStatus = err.into();
        assert_eq!(status.code, FfiErrorCode::Auth);
        assert!(status.message.unwrap().contains("invalid pass"));
    }
}

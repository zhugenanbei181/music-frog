use lurker_core_contract::error::InfiltratorError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum, serde::Serialize, serde::Deserialize)]
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
    Canceled = 10,
    Unknown = 255,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum, serde::Serialize, serde::Deserialize)]
#[repr(u32)]
pub enum FfiFailureLocalizationKey {
    InvalidInput = 0,
    InvalidState = 1,
    NotReady = 2,
    Unsupported = 3,
    Network = 4,
    Authentication = 5,
    Sync = 6,
    Configuration = 7,
    Storage = 8,
    Permission = 9,
    Canceled = 10,
    Internal = 11,
    YamlSyntax = 12,
    MixinYaml = 13,
    ActiveProfileInconsistent = 14,
    QuotaSourceChanged = 15,
    CurrentTimeUnavailable = 16,
    DownloadCanceled = 17,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record, serde::Serialize, serde::Deserialize)]
pub struct FfiFailureLocalization {
    pub key: FfiFailureLocalizationKey,
    pub line: Option<u64>,
    pub column: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record, serde::Serialize, serde::Deserialize)]
pub struct FfiStatus {
    pub code: FfiErrorCode,
    /// Opaque platform/application diagnostic. Present separately from the
    /// stable localization projection so UI never needs to parse this text.
    pub message: Option<String>,
    pub localization: Option<FfiFailureLocalization>,
}

impl FfiStatus {
    pub fn ok() -> Self {
        Self {
            code: FfiErrorCode::Ok,
            message: None,
            localization: None,
        }
    }

    pub fn err(code: FfiErrorCode, message: impl Into<String>) -> Self {
        Self::err_with_localization(code, message, FfiFailureLocalization::for_code(code))
    }

    pub fn err_with_localization(
        code: FfiErrorCode,
        message: impl Into<String>,
        localization: FfiFailureLocalization,
    ) -> Self {
        Self {
            code,
            message: Some(message.into()),
            localization: Some(localization),
        }
    }
}

impl FfiFailureLocalization {
    pub fn for_code(code: FfiErrorCode) -> Self {
        let key = match code {
            FfiErrorCode::Ok | FfiErrorCode::Unknown => FfiFailureLocalizationKey::Internal,
            FfiErrorCode::InvalidState => FfiFailureLocalizationKey::InvalidState,
            FfiErrorCode::InvalidInput => FfiFailureLocalizationKey::InvalidInput,
            FfiErrorCode::NotReady => FfiFailureLocalizationKey::NotReady,
            FfiErrorCode::NotSupported => FfiFailureLocalizationKey::Unsupported,
            FfiErrorCode::Io => FfiFailureLocalizationKey::Storage,
            FfiErrorCode::Network => FfiFailureLocalizationKey::Network,
            FfiErrorCode::Auth => FfiFailureLocalizationKey::Authentication,
            FfiErrorCode::Sync => FfiFailureLocalizationKey::Sync,
            FfiErrorCode::Config => FfiFailureLocalizationKey::Configuration,
            FfiErrorCode::Canceled => FfiFailureLocalizationKey::Canceled,
        };
        Self {
            key,
            line: None,
            column: None,
        }
    }
}

impl From<InfiltratorError> for FfiStatus {
    fn from(err: InfiltratorError) -> Self {
        let code = match err {
            InfiltratorError::Backend(_) => FfiErrorCode::Network,
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

#[derive(Debug, Clone, uniffi::Record, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record, serde::Serialize, serde::Deserialize)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_ok() {
        let status = FfiStatus::ok();
        assert_eq!(status.code, FfiErrorCode::Ok);
        assert!(status.message.is_none());
        assert!(status.localization.is_none());
    }

    #[test]
    fn test_status_err() {
        let status = FfiStatus::err(FfiErrorCode::InvalidInput, "bad input");
        assert_eq!(status.code, FfiErrorCode::InvalidInput);
        assert_eq!(status.message, Some("bad input".to_string()));
        assert_eq!(
            status.localization,
            Some(FfiFailureLocalization {
                key: FfiFailureLocalizationKey::InvalidInput,
                line: None,
                column: None,
            })
        );
    }

    #[test]
    fn test_ffi_status_from_infiltrator_error() {
        let err = InfiltratorError::Auth("invalid pass".to_string());
        let status: FfiStatus = err.into();
        assert_eq!(status.code, FfiErrorCode::Auth);
        assert!(status.message.unwrap().contains("invalid pass"));
    }

    #[test]
    fn backend_error_keeps_the_existing_network_classification() {
        let status = FfiStatus::from(InfiltratorError::Backend("backend unavailable".to_owned()));
        assert_eq!(status.code, FfiErrorCode::Network);
        assert_eq!(
            status.message.as_deref(),
            Some("Backend error: backend unavailable")
        );
        assert_eq!(
            status.localization.unwrap().key,
            FfiFailureLocalizationKey::Network
        );
    }
}

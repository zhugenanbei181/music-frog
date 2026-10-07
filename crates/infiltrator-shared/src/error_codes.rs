use crate::i18n_interpolator::localize;
use crate::locales::resolve_language_code;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum InfiltratorErrorCode {
    // Core
    PortInUse(u16),
    KernelCrash,
    KernelNotFound,
    ConfigInvalid(String),
    ReadinessTimeout,
    SecretMismatch,
    // Subscription
    SubscriptionFetchFailed(String),
    SubscriptionEmpty,
    InvalidSubscriptionUrl(String),
    SubscriptionDecodeError(String),
    // Sync
    WebDavAuthFailed,
    WebDavNetworkError(String),
    WebDavConflict,
    // Platform
    TunPrivilegeMissing,
    SystemProxyFailed(String),
    KeyringError(String),
    AutostartFailed(String),
    // Generic
    Internal(String),
    NetworkTimeout,
}

impl InfiltratorErrorCode {
    pub fn domain(&self) -> &'static str {
        match self {
            Self::PortInUse(_)
            | Self::KernelCrash
            | Self::KernelNotFound
            | Self::ConfigInvalid(_)
            | Self::ReadinessTimeout
            | Self::SecretMismatch => "Core",

            Self::SubscriptionFetchFailed(_)
            | Self::SubscriptionEmpty
            | Self::InvalidSubscriptionUrl(_)
            | Self::SubscriptionDecodeError(_) => "Subscription",

            Self::WebDavAuthFailed | Self::WebDavNetworkError(_) | Self::WebDavConflict => "Sync",

            Self::TunPrivilegeMissing
            | Self::SystemProxyFailed(_)
            | Self::KeyringError(_)
            | Self::AutostartFailed(_) => "Platform",

            Self::Internal(_) | Self::NetworkTimeout => "Generic",
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct StructuredError {
    pub code: InfiltratorErrorCode,
    pub domain: String,
    pub message: String,
    pub suggestion: String,
    pub troubleshooting_url: Option<String>,
}

pub fn get_localized_error(code: &InfiltratorErrorCode, lang: &str) -> StructuredError {
    let (message_key, suggestion_key, values) = match code {
        InfiltratorErrorCode::PortInUse(port) => (
            "error_port_in_use_message",
            "error_port_in_use_suggestion",
            vec![("port", port.to_string())],
        ),
        InfiltratorErrorCode::KernelCrash => (
            "error_kernel_crash_message",
            "error_kernel_crash_suggestion",
            vec![],
        ),
        InfiltratorErrorCode::KernelNotFound => (
            "error_kernel_not_found_message",
            "error_kernel_not_found_suggestion",
            vec![],
        ),
        InfiltratorErrorCode::ConfigInvalid(err) => (
            "error_config_invalid_message",
            "error_config_invalid_suggestion",
            vec![("reason", err.clone())],
        ),
        InfiltratorErrorCode::ReadinessTimeout => (
            "error_readiness_timeout_message",
            "error_readiness_timeout_suggestion",
            vec![],
        ),
        InfiltratorErrorCode::SecretMismatch => (
            "error_secret_mismatch_message",
            "error_secret_mismatch_suggestion",
            vec![],
        ),
        InfiltratorErrorCode::SubscriptionFetchFailed(err) => (
            "error_subscription_fetch_failed_message",
            "error_subscription_fetch_failed_suggestion",
            vec![("reason", err.clone())],
        ),
        InfiltratorErrorCode::SubscriptionEmpty => (
            "error_subscription_empty_message",
            "error_subscription_empty_suggestion",
            vec![],
        ),
        InfiltratorErrorCode::InvalidSubscriptionUrl(err) => (
            "error_invalid_subscription_url_message",
            "error_invalid_subscription_url_suggestion",
            vec![("reason", err.clone())],
        ),
        InfiltratorErrorCode::SubscriptionDecodeError(err) => (
            "error_subscription_decode_error_message",
            "error_subscription_decode_error_suggestion",
            vec![("reason", err.clone())],
        ),
        InfiltratorErrorCode::WebDavAuthFailed => (
            "error_web_dav_auth_failed_message",
            "error_web_dav_auth_failed_suggestion",
            vec![],
        ),
        InfiltratorErrorCode::WebDavNetworkError(err) => (
            "error_web_dav_network_error_message",
            "error_web_dav_network_error_suggestion",
            vec![("reason", err.clone())],
        ),
        InfiltratorErrorCode::WebDavConflict => (
            "error_web_dav_conflict_message",
            "error_web_dav_conflict_suggestion",
            vec![],
        ),
        InfiltratorErrorCode::TunPrivilegeMissing => (
            "error_tun_privilege_missing_message",
            "error_tun_privilege_missing_suggestion",
            vec![],
        ),
        InfiltratorErrorCode::SystemProxyFailed(err) => (
            "error_system_proxy_failed_message",
            "error_system_proxy_failed_suggestion",
            vec![("reason", err.clone())],
        ),
        InfiltratorErrorCode::KeyringError(err) => (
            "error_keyring_error_message",
            "error_keyring_error_suggestion",
            vec![("reason", err.clone())],
        ),
        InfiltratorErrorCode::AutostartFailed(err) => (
            "error_autostart_failed_message",
            "error_autostart_failed_suggestion",
            vec![("reason", err.clone())],
        ),
        InfiltratorErrorCode::Internal(err) => (
            "error_internal_message",
            "error_internal_suggestion",
            vec![("reason", err.clone())],
        ),
        InfiltratorErrorCode::NetworkTimeout => (
            "error_network_timeout_message",
            "error_network_timeout_suggestion",
            vec![],
        ),
    };
    let locale = resolve_language_code(lang);
    let message = localize(&locale, message_key, &values);
    let suggestion = localize(&locale, suggestion_key, &values);

    StructuredError {
        code: code.clone(),
        domain: code.domain().to_string(),
        message,
        suggestion,
        troubleshooting_url: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locales::{Lang, Localizer};

    #[test]
    fn test_error_code_serialization() {
        let code = InfiltratorErrorCode::PortInUse(7890);
        let serialized = serde_json::to_string(&code).unwrap();
        assert!(serialized.contains("PortInUse"));

        let deserialized: InfiltratorErrorCode = serde_json::from_str(&serialized).unwrap();
        assert_eq!(code, deserialized);

        let code_str = InfiltratorErrorCode::ConfigInvalid("bad syntax".to_string());
        let serialized_str = serde_json::to_string(&code_str).unwrap();
        let deserialized_str: InfiltratorErrorCode = serde_json::from_str(&serialized_str).unwrap();
        assert_eq!(code_str, deserialized_str);
    }

    #[test]
    fn test_localization_zh() {
        let code = InfiltratorErrorCode::PortInUse(7890);
        let error = get_localized_error(&code, "zh-CN");
        assert_eq!(error.domain, "Core");
        assert_eq!(error.message, "端口 7890 已被占用");
        assert_eq!(
            error.suggestion,
            "端口 7890 已被占用，请检查是否已运行其他代理客户端或在设置中更换端口"
        );
    }

    #[test]
    fn test_localization_en() {
        let code = InfiltratorErrorCode::PortInUse(7890);
        let error = get_localized_error(&code, "en-US");
        assert_eq!(error.domain, "Core");
        assert_eq!(error.message, "Port 7890 is already in use");
        assert_eq!(
            error.suggestion,
            "Port 7890 is already in use. Please check if another proxy client is running or change the port in settings."
        );
    }

    #[test]
    fn test_localization_dynamic_params() {
        let code = InfiltratorErrorCode::ConfigInvalid("missing field `port`".to_string());
        let error_zh = get_localized_error(&code, "zh-CN");
        assert_eq!(error_zh.message, "配置文件无效: missing field `port`");

        let error_en = get_localized_error(&code, "en");
        assert_eq!(
            error_en.message,
            "Invalid configuration: missing field `port`"
        );
    }

    #[test]
    fn error_copy_uses_the_same_locale_resources_and_interprets_user_parameters_once() {
        let reason = "user {port} / {reason} / 日本";
        let code = InfiltratorErrorCode::ConfigInvalid(reason.into());
        let chinese = get_localized_error(&code, "ZH_cn");
        let english = get_localized_error(&code, "EN_us");
        assert_eq!(chinese.message, format!("配置文件无效: {reason}"));
        assert_eq!(english.message, format!("Invalid configuration: {reason}"));
        assert_eq!(chinese.code, code);
        assert_eq!(english.code, code);
        assert_eq!(chinese.domain, "Core");
        assert_eq!(english.domain, "Core");
        assert_eq!(
            chinese.suggestion,
            localize("zh-CN", "error_config_invalid_suggestion", &[])
        );
        assert_eq!(
            english.suggestion,
            localize("en-US", "error_config_invalid_suggestion", &[])
        );
        assert_eq!(Lang("en-US").tr("unknown_error_key"), "unknown_error_key");
    }

    #[test]
    fn test_all_variants_localization() {
        let variants = vec![
            InfiltratorErrorCode::PortInUse(1080),
            InfiltratorErrorCode::KernelCrash,
            InfiltratorErrorCode::KernelNotFound,
            InfiltratorErrorCode::ConfigInvalid("err".to_string()),
            InfiltratorErrorCode::ReadinessTimeout,
            InfiltratorErrorCode::SecretMismatch,
            InfiltratorErrorCode::SubscriptionFetchFailed("err".to_string()),
            InfiltratorErrorCode::SubscriptionEmpty,
            InfiltratorErrorCode::InvalidSubscriptionUrl("err".to_string()),
            InfiltratorErrorCode::SubscriptionDecodeError("err".to_string()),
            InfiltratorErrorCode::WebDavAuthFailed,
            InfiltratorErrorCode::WebDavNetworkError("err".to_string()),
            InfiltratorErrorCode::WebDavConflict,
            InfiltratorErrorCode::TunPrivilegeMissing,
            InfiltratorErrorCode::SystemProxyFailed("err".to_string()),
            InfiltratorErrorCode::KeyringError("err".to_string()),
            InfiltratorErrorCode::AutostartFailed("err".to_string()),
            InfiltratorErrorCode::Internal("err".to_string()),
            InfiltratorErrorCode::NetworkTimeout,
        ];

        for variant in variants {
            let error_zh = get_localized_error(&variant, "zh-CN");
            let error_en = get_localized_error(&variant, "en-US");

            assert!(!error_zh.message.is_empty());
            assert!(!error_zh.suggestion.is_empty());
            assert!(!error_en.message.is_empty());
            assert!(!error_en.suggestion.is_empty());
        }
    }
}

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

            Self::WebDavAuthFailed
            | Self::WebDavNetworkError(_)
            | Self::WebDavConflict => "Sync",

            Self::TunPrivilegeMissing
            | Self::SystemProxyFailed(_)
            | Self::KeyringError(_)
            | Self::AutostartFailed(_) => "Platform",

            Self::Internal(_)
            | Self::NetworkTimeout => "Generic",
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
    let is_zh = lang.starts_with("zh");

    let (message, suggestion) = match code {
        InfiltratorErrorCode::PortInUse(port) => {
            if is_zh {
                (
                    format!("端口 {} 已被占用", port),
                    format!("端口 {} 已被占用，请检查是否已运行其他代理客户端或在设置中更换端口", port),
                )
            } else {
                (
                    format!("Port {} is already in use", port),
                    format!("Port {} is already in use. Please check if another proxy client is running or change the port in settings.", port),
                )
            }
        }
        InfiltratorErrorCode::KernelCrash => {
            if is_zh {
                ("内核已崩溃".to_string(), "请尝试重启应用或检查日志获取更多信息。".to_string())
            } else {
                ("Kernel crashed".to_string(), "Please try restarting the application or check logs for more details.".to_string())
            }
        }
        InfiltratorErrorCode::KernelNotFound => {
            if is_zh {
                ("未找到内核文件".to_string(), "请重新安装应用或手动下载内核。".to_string())
            } else {
                ("Kernel executable not found".to_string(), "Please reinstall the application or manually download the kernel.".to_string())
            }
        }
        InfiltratorErrorCode::ConfigInvalid(err) => {
            if is_zh {
                (format!("配置文件无效: {}", err), "请检查您的配置文件语法是否有误，或重置为默认配置。".to_string())
            } else {
                (format!("Invalid configuration: {}", err), "Please check your configuration file for syntax errors or reset to default.".to_string())
            }
        }
        InfiltratorErrorCode::ReadinessTimeout => {
            if is_zh {
                ("内核启动超时".to_string(), "内核启动时间过长，可能是系统资源不足或配置有误。".to_string())
            } else {
                ("Kernel readiness timeout".to_string(), "Kernel took too long to start. Check system resources or configuration.".to_string())
            }
        }
        InfiltratorErrorCode::SecretMismatch => {
            if is_zh {
                ("API 密钥不匹配".to_string(), "请确保客户端与内核使用的 API 密钥一致。".to_string())
            } else {
                ("API secret mismatch".to_string(), "Ensure the client and kernel are using the same API secret.".to_string())
            }
        }
        InfiltratorErrorCode::SubscriptionFetchFailed(err) => {
            if is_zh {
                (format!("订阅获取失败: {}", err), "请检查网络连接或订阅链接是否仍然有效。".to_string())
            } else {
                (format!("Subscription fetch failed: {}", err), "Please check your network connection or verify if the subscription link is still valid.".to_string())
            }
        }
        InfiltratorErrorCode::SubscriptionEmpty => {
            if is_zh {
                ("订阅内容为空".to_string(), "获取到的订阅未包含任何节点，请联系订阅提供商。".to_string())
            } else {
                ("Subscription is empty".to_string(), "The fetched subscription contains no nodes. Please contact your provider.".to_string())
            }
        }
        InfiltratorErrorCode::InvalidSubscriptionUrl(err) => {
            if is_zh {
                (format!("无效的订阅链接: {}", err), "请确保填写的订阅链接格式正确（例如以 http:// 或 https:// 开头）。".to_string())
            } else {
                (format!("Invalid subscription URL: {}", err), "Ensure the subscription URL is properly formatted (e.g., starts with http:// or https://).".to_string())
            }
        }
        InfiltratorErrorCode::SubscriptionDecodeError(err) => {
            if is_zh {
                (format!("订阅解析失败: {}", err), "无法识别订阅格式，可能是该订阅已被加密或格式不受支持。".to_string())
            } else {
                (format!("Subscription decode error: {}", err), "Failed to parse the subscription. The format might be unsupported or encrypted.".to_string())
            }
        }
        InfiltratorErrorCode::WebDavAuthFailed => {
            if is_zh {
                ("WebDAV 认证失败".to_string(), "请检查您的 WebDAV 账号和密码是否正确。".to_string())
            } else {
                ("WebDAV authentication failed".to_string(), "Please verify your WebDAV username and password.".to_string())
            }
        }
        InfiltratorErrorCode::WebDavNetworkError(err) => {
            if is_zh {
                (format!("WebDAV 网络错误: {}", err), "无法连接到 WebDAV 服务器，请检查网络或服务器状态。".to_string())
            } else {
                (format!("WebDAV network error: {}", err), "Unable to connect to the WebDAV server. Check your network or server status.".to_string())
            }
        }
        InfiltratorErrorCode::WebDavConflict => {
            if is_zh {
                ("WebDAV 冲突".to_string(), "远程数据与本地数据发生冲突，请手动解决冲突后重试。".to_string())
            } else {
                ("WebDAV conflict".to_string(), "Remote and local data conflict. Please resolve it manually and try again.".to_string())
            }
        }
        InfiltratorErrorCode::TunPrivilegeMissing => {
            if is_zh {
                ("缺少 TUN 模式权限".to_string(), "启用 TUN 模式需要管理员权限，请以管理员身份运行本程序。".to_string())
            } else {
                ("Missing TUN privileges".to_string(), "TUN mode requires administrator privileges. Please run the program as administrator.".to_string())
            }
        }
        InfiltratorErrorCode::SystemProxyFailed(err) => {
            if is_zh {
                (format!("系统代理设置失败: {}", err), "无法自动配置系统代理，请尝试手动设置或检查系统权限。".to_string())
            } else {
                (format!("Failed to set system proxy: {}", err), "Cannot configure system proxy automatically. Try manual setup or check system permissions.".to_string())
            }
        }
        InfiltratorErrorCode::KeyringError(err) => {
            if is_zh {
                (format!("密钥环错误: {}", err), "无法访问系统安全存储。在 Linux 上请确保安装并启动了 gnome-keyring 或 kwallet。".to_string())
            } else {
                (format!("Keyring error: {}", err), "Cannot access system secure storage. On Linux, ensure gnome-keyring or kwallet is installed and running.".to_string())
            }
        }
        InfiltratorErrorCode::AutostartFailed(err) => {
            if is_zh {
                (format!("开机自启设置失败: {}", err), "无法配置开机自启动，可能是权限不足或系统不支持。".to_string())
            } else {
                (format!("Autostart configuration failed: {}", err), "Cannot configure autostart. This might be due to insufficient permissions or an unsupported system.".to_string())
            }
        }
        InfiltratorErrorCode::Internal(err) => {
            if is_zh {
                (format!("内部错误: {}", err), "发生了未知错误，请报告此问题以便我们修复。".to_string())
            } else {
                (format!("Internal error: {}", err), "An unknown error occurred. Please report this issue.".to_string())
            }
        }
        InfiltratorErrorCode::NetworkTimeout => {
            if is_zh {
                ("网络请求超时".to_string(), "请检查您的网络连接并重试。".to_string())
            } else {
                ("Network request timeout".to_string(), "Please check your internet connection and try again.".to_string())
            }
        }
    };

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
        assert_eq!(error_en.message, "Invalid configuration: missing field `port`");
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

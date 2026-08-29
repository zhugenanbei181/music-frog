/// MihomoError 错误类型测试
///
/// 验证错误的展示格式（Display）、标准错误的自动转换（From impl）、
/// 以及 Result<T> 别名在业务函数中的实际使用语义。
/// 每条错误都有对应的 #[error(...)] 格式模板，格式必须精确匹配，
/// 因为这些信息会直接呈现给用户或写入日志。
#[cfg(test)]
mod tests {
    use crate::error::{MihomoError, Result};
    // ──────────────────────────────────────────────
    // Display 格式：用户可见的错误消息
    // ──────────────────────────────────────────────

    #[test]
    fn config_error_display_includes_detail_message() {
        let err = MihomoError::Config("external-controller 端口冲突".to_string());
        assert_eq!(err.to_string(), "Config error: external-controller 端口冲突");
    }

    #[test]
    fn service_error_display_includes_detail_message() {
        let err = MihomoError::Service("mihomo 进程启动失败".to_string());
        assert_eq!(err.to_string(), "Service error: mihomo 进程启动失败");
    }

    #[test]
    fn version_error_display_includes_detail_message() {
        let err = MihomoError::Version("v1.99.0 未安装".to_string());
        assert_eq!(err.to_string(), "Version error: v1.99.0 未安装");
    }

    #[test]
    fn proxy_error_display_includes_detail_message() {
        let err = MihomoError::Proxy("节点 Node-A 已下线".to_string());
        assert_eq!(err.to_string(), "Proxy error: 节点 Node-A 已下线");
    }

    #[test]
    fn not_found_error_display_includes_detail_message() {
        let err = MihomoError::NotFound("配置文件 default.yaml".to_string());
        assert_eq!(err.to_string(), "Not found: 配置文件 default.yaml");
    }

    // ──────────────────────────────────────────────
    // From 转换：标准错误自动包装
    // ──────────────────────────────────────────────

    #[test]
    fn io_error_converts_to_mihomo_io_variant() {
        // 文件读取失败等 IO 错误应自动包装，不需要调用方手动转换
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "配置文件不存在");
        let mihomo_err: MihomoError = io_err.into();
        assert!(
            matches!(mihomo_err, MihomoError::Io(_)),
            "IO 错误应转换为 MihomoError::Io"
        );
        // 原始消息应在错误链中可见
        assert!(mihomo_err.to_string().contains("配置文件不存在"));
    }

    #[test]
    fn json_parse_failure_converts_to_mihomo_json_variant() {
        // Mihomo API 响应解析失败时应包装为 Json 变体
        let json_err = serde_json::from_str::<serde_json::Value>("{ invalid }").unwrap_err();
        let mihomo_err: MihomoError = json_err.into();
        assert!(
            matches!(mihomo_err, MihomoError::Json(_)),
            "JSON 解析错误应转换为 MihomoError::Json"
        );
    }

    #[test]
    fn invalid_url_converts_to_mihomo_url_parse_variant() {
        // 用户配置了错误格式的 external-controller 地址时应得到此错误
        let url_err = url::Url::parse("not-a-valid-url").unwrap_err();
        let mihomo_err: MihomoError = url_err.into();
        assert!(
            matches!(mihomo_err, MihomoError::UrlParse(_)),
            "URL 解析错误应转换为 MihomoError::UrlParse"
        );
    }

    // ──────────────────────────────────────────────
    // Result<T> 别名：业务函数返回值语义
    // ──────────────────────────────────────────────

    #[test]
    fn result_alias_propagates_errors_in_business_functions() {
        // Result<T> 是 std::result::Result<T, MihomoError> 的别名，
        // 验证它可以正常用于业务函数的返回与 ? 传播
        fn load_config(valid: bool) -> Result<String> {
            if valid {
                Ok("port: 7890".to_string())
            } else {
                Err(MihomoError::Config("配置格式错误".to_string()))
            }
        }

        fn apply_config(valid: bool) -> Result<usize> {
            let content = load_config(valid)?; // ? 应能正确传播 MihomoError
            Ok(content.len())
        }

        assert!(apply_config(true).is_ok());
        let err = apply_config(false).unwrap_err();
        assert!(matches!(err, MihomoError::Config(_)));
        assert!(err.to_string().contains("配置格式错误"));
    }
}

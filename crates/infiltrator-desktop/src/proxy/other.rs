//! 面向 windows/linux/macos 之外目标的兜底：设置代理一律报错（绝不假装
//! 成功），读取状态返回默认值。

use super::SystemProxyState;

pub(super) fn apply(endpoint: Option<&str>, _bypass: Option<&str>) -> anyhow::Result<()> {
    if endpoint.is_some() {
        Err(anyhow::anyhow!("Unsupported platform for system proxy"))
    } else {
        Ok(())
    }
}

pub(super) fn read_state() -> anyhow::Result<SystemProxyState> {
    Ok(SystemProxyState::default())
}

#[cfg(test)]
mod tests {
    use super::super::{apply_system_proxy, read_system_proxy_state};

    #[test]
    fn test_apply_system_proxy_unsupported() {
        let result = apply_system_proxy(Some("127.0.0.1:7890"));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unsupported platform")
        );

        let result = apply_system_proxy(None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_read_system_proxy_state_unsupported() {
        let result = read_system_proxy_state();
        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.enabled, false);
    }
}

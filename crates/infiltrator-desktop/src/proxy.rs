//! 系统代理开关的平台实现。
//!
//! - Windows：注册表直写 + rundll32 刷新（本仓库开发/CI 环境无法验证，
//!   保持现状，不做退出码之外的改动）。
//! - macOS：`networksetup`，所有命令强制检查退出码，非零返回 `Err`
//!   （附 stderr/stdout 摘要）。
//! - Linux：仅支持 GNOME 的 gsettings 后端；KDE 及其它桌面环境不受支持
//!   （刻意不做）。检测不到 `gsettings` 时返回类型化的
//!   [`UnsupportedDesktopError`]，绝不静默假装成功。
//!
//! 各平台实现位于 `windows`/`linux`/`macos` 子模块（以及面向未列出目标
//! 的 `other` 兜底）；本文件只保留平台无关的类型、默认旁路列表与公共
//! 入口。

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
mod other;

#[cfg(target_os = "windows")]
use windows as platform;
#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
use other as platform;

const DEFAULT_BYPASS: &str = "localhost;127.*;10.*;172.16.*;192.168.*;<local>";

#[derive(Clone, Default, Debug, PartialEq)]
pub struct SystemProxyState {
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub bypass: Option<String>,
}

#[cfg(target_os = "linux")]
pub use linux::UnsupportedDesktopError;

pub fn apply_system_proxy(endpoint: Option<&str>) -> anyhow::Result<()> {
    apply_system_proxy_with_bypass(endpoint, Some(DEFAULT_BYPASS))
}

pub fn apply_system_proxy_with_bypass(
    endpoint: Option<&str>,
    bypass: Option<&str>,
) -> anyhow::Result<()> {
    platform::apply(endpoint, bypass)
}

pub fn read_system_proxy_state() -> anyhow::Result<SystemProxyState> {
    platform::read_state()
}

/// 解析 `host:port`。仅 linux/macos 后端使用（windows 直写字符串，
/// 不解析端点）。
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn parse_endpoint(endpoint: &str) -> Option<(&str, u16)> {
    let (host, port_str) = endpoint.rsplit_once(':')?;
    let port = port_str.parse::<u16>().ok()?;
    Some((host, port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_proxy_state_default() {
        let state = SystemProxyState::default();
        assert!(!state.enabled);
        assert_eq!(state.endpoint, None);
        assert_eq!(state.bypass, None);
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn test_parse_endpoint() {
        assert_eq!(parse_endpoint("127.0.0.1:7890"), Some(("127.0.0.1", 7890)));
        assert_eq!(parse_endpoint("localhost:8080"), Some(("localhost", 8080)));
        assert_eq!(parse_endpoint("invalid"), None);
    }

    #[test]
    fn test_bypass_default() {
        assert!(DEFAULT_BYPASS.contains("localhost"));
        assert!(DEFAULT_BYPASS.contains("127.*"));
    }
}

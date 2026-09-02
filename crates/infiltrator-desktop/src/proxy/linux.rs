//! Linux 系统代理实现：支持 GNOME（gsettings）、KDE Plasma（kwriteconfig5/6 / kconfig kioslaverc）、
//! XFCE 及 Generic 桌面环境（环境变量 fallback 模式）。

pub mod env_fallback;
pub mod environment;
pub mod gnome;
pub mod kde;

use super::{SystemProxyState, parse_endpoint};

pub type DesktopEnvironment = environment::DesktopEnvironment;
pub type UnsupportedDesktopError = gnome::UnsupportedDesktopError;

/// 探测当前 Linux 桌面环境。
pub fn detect_desktop_environment() -> DesktopEnvironment {
    environment::detect_desktop_environment()
}

/// 基于自定义环境变量获取函数的桌面环境探测器（便于单元测试与沙箱探测）。
pub fn detect_desktop_environment_with<F>(get_env: F) -> DesktopEnvironment
where
    F: Fn(&str) -> Option<String>,
{
    environment::detect_desktop_environment_with(get_env)
}

/// 从 URL 或 `host:port` 字符串中提取统一的 `host:port` 端点。
pub fn parse_url_to_endpoint(val: &str) -> Option<String> {
    let s = val.trim().trim_matches('\'').trim_matches('"').trim();
    if s.is_empty() {
        return None;
    }
    let without_proto = if let Some(rest) = s.strip_prefix("http://") {
        rest
    } else if let Some(rest) = s.strip_prefix("https://") {
        rest
    } else if let Some(rest) = s.strip_prefix("socks5h://") {
        rest
    } else if let Some(rest) = s.strip_prefix("socks5://") {
        rest
    } else if let Some(rest) = s.strip_prefix("socks://") {
        rest
    } else if let Some(rest) = s.strip_prefix("ftp://") {
        rest
    } else {
        s
    };
    let host_port = match without_proto.split_once('/') {
        Some((hp, _)) => hp,
        None => without_proto,
    };
    let (host, port) = parse_endpoint(host_port)?;
    Some(format!("{host}:{port}"))
}

/// 针对指定桌面环境应用代理配置。
pub fn apply_for_de(
    de: DesktopEnvironment,
    endpoint: Option<&str>,
    bypass: Option<&str>,
) -> anyhow::Result<()> {
    match de {
        DesktopEnvironment::Gnome => {
            if gnome::is_available() {
                gnome::apply(endpoint, bypass)
            } else {
                env_fallback::apply(endpoint, bypass)
            }
        }
        DesktopEnvironment::Kde => {
            if kde::is_available() {
                kde::apply(endpoint, bypass)
            } else {
                env_fallback::apply(endpoint, bypass)
            }
        }
        DesktopEnvironment::Xfce => {
            if gnome::is_available() {
                gnome::apply(endpoint, bypass)
            } else {
                env_fallback::apply(endpoint, bypass)
            }
        }
        DesktopEnvironment::Generic => env_fallback::apply(endpoint, bypass),
    }
}

/// 针对指定桌面环境读取代理状态。
pub fn read_state_for_de(de: DesktopEnvironment) -> anyhow::Result<SystemProxyState> {
    match de {
        DesktopEnvironment::Gnome => {
            if gnome::is_available() {
                gnome::read_state()
            } else {
                env_fallback::read_state()
            }
        }
        DesktopEnvironment::Kde => {
            if kde::is_available() {
                kde::read_state()
            } else {
                env_fallback::read_state()
            }
        }
        DesktopEnvironment::Xfce => {
            if gnome::is_available() {
                gnome::read_state()
            } else {
                env_fallback::read_state()
            }
        }
        DesktopEnvironment::Generic => env_fallback::read_state(),
    }
}

pub(super) fn apply(endpoint: Option<&str>, bypass: Option<&str>) -> anyhow::Result<()> {
    let de = detect_desktop_environment();
    apply_for_de(de, endpoint, bypass)
}

pub(super) fn read_state() -> anyhow::Result<SystemProxyState> {
    let de = detect_desktop_environment();
    read_state_for_de(de)
}

#[cfg(test)]
#[path = "linux_test.rs"]
mod linux_test;

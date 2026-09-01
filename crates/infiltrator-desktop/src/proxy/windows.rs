//! Windows 实现：注册表直写 + rundll32 刷新。（本仓库开发/CI 环境无法
//! 验证，保持现状，不做退出码之外的改动。）

use super::SystemProxyState;
use anyhow::anyhow;
use std::os::windows::process::CommandExt;
use std::process::Command;
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

pub(super) fn apply(endpoint: Option<&str>, bypass: Option<&str>) -> anyhow::Result<()> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_WRITE};

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey_with_flags(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
            KEY_WRITE,
        )
        .map_err(|e| anyhow!(e.to_string()))?;

    let enable: u32 = if endpoint.is_some() { 1 } else { 0 };
    key.set_value("ProxyEnable", &enable)
        .map_err(|e| anyhow!(e.to_string()))?;

    let server = endpoint.unwrap_or("");
    key.set_value("ProxyServer", &server)
        .map_err(|e| anyhow!(e.to_string()))?;

    if let Some(b) = bypass {
        key.set_value("ProxyOverride", &b)
            .map_err(|e| anyhow!(e.to_string()))?;
    } else {
        let _ = key.delete_value("ProxyOverride");
    }

    refresh_internet_settings();

    Ok(())
}

pub(super) fn read_state() -> anyhow::Result<SystemProxyState> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey_with_flags(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
            KEY_READ,
        )
        .map_err(|e| anyhow!(e.to_string()))?;

    let enabled: u32 = key.get_value("ProxyEnable").unwrap_or(0);
    let endpoint: Option<String> = key
        .get_value("ProxyServer")
        .ok()
        .and_then(|v: String| if v.trim().is_empty() { None } else { Some(v) });
    let bypass: Option<String> = key
        .get_value("ProxyOverride")
        .ok()
        .and_then(|v: String| if v.trim().is_empty() { None } else { Some(v) });

    Ok(SystemProxyState {
        enabled: enabled != 0,
        endpoint,
        bypass,
    })
}

fn refresh_internet_settings() {
    let mut command = Command::new("rundll32.exe");
    command.creation_flags(CREATE_NO_WINDOW);
    let status = command
        .args(["user32.dll,UpdatePerUserSystemParameters"])
        .status();
    if let Ok(status) = status {
        if !status.success() {
            log::warn!("刷新系统代理设置失败: {}", status);
        }
    } else if let Err(err) = status {
        log::warn!("刷新系统代理设置失败: {err}");
    }
}

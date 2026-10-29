//! Windows 实现：注册表直写 + rundll32 刷新。Windows 注册表读取应区分
//! 缺省值和实际读取错误；本仓库开发/CI 环境无法执行此平台实现。

use super::SystemProxyState;
use anyhow::anyhow;
use std::io::ErrorKind;
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

    if let Some(server) = endpoint {
        key.set_value("ProxyServer", &server)
            .map_err(|e| anyhow!(e.to_string()))?;
        if let Some(bypass) = bypass {
            key.set_value("ProxyOverride", &bypass)
                .map_err(|e| anyhow!(e.to_string()))?;
        } else {
            match key.delete_value("ProxyOverride") {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(anyhow!(error.to_string())),
            }
        }
    }

    refresh_internet_settings();

    Ok(())
}

pub(super) fn read_state() -> anyhow::Result<SystemProxyState> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = match hkcu.open_subkey_with_flags(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
        KEY_READ,
    ) {
        Ok(key) => key,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(SystemProxyState::default());
        }
        Err(error) => return Err(anyhow!(error.to_string())),
    };

    let enabled: u32 = match key.get_value("ProxyEnable") {
        Ok(value) => value,
        Err(error) if error.kind() == ErrorKind::NotFound => 0,
        Err(error) => return Err(anyhow!(error.to_string())),
    };
    let enabled = enabled != 0;
    let (endpoint, bypass) = if enabled {
        (
            read_optional_string(&key, "ProxyServer")?,
            read_optional_string(&key, "ProxyOverride")?,
        )
    } else {
        (None, None)
    };

    Ok(SystemProxyState {
        enabled,
        endpoint,
        bypass,
    })
}

fn read_optional_string(key: &winreg::RegKey, name: &str) -> anyhow::Result<Option<String>> {
    match key.get_value(name) {
        Ok(value) if value.trim().is_empty() => Ok(None),
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(anyhow!(error.to_string())),
    }
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

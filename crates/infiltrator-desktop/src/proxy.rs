use anyhow::anyhow;
#[cfg(target_os = "windows")]
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

#[derive(Clone, Default)]
pub struct SystemProxyState {
    pub enabled: bool,
    pub endpoint: Option<String>,
}

pub fn apply_system_proxy(endpoint: Option<&str>) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        set_windows_system_proxy(endpoint)
    }
    #[cfg(not(target_os = "windows"))]
    {
        if endpoint.is_some() {
            Err(anyhow!("系统代理切换仅支持 Windows"))
        } else {
            Ok(())
        }
    }
}

pub fn read_system_proxy_state() -> anyhow::Result<SystemProxyState> {
    #[cfg(target_os = "windows")]
    {
        read_windows_system_proxy_state()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(SystemProxyState {
            enabled: false,
            endpoint: None,
        })
    }
}

#[cfg(target_os = "windows")]
fn set_windows_system_proxy(endpoint: Option<&str>) -> anyhow::Result<()> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_WRITE};
    use winreg::RegKey;

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

    refresh_internet_settings();

    Ok(())
}

#[cfg(target_os = "windows")]
fn read_windows_system_proxy_state() -> anyhow::Result<SystemProxyState> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey_with_flags(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
            KEY_READ,
        )
        .map_err(|e| anyhow!(e.to_string()))?;

    let enabled: u32 = key.get_value("ProxyEnable").unwrap_or(0);
    let endpoint: Option<String> = key.get_value("ProxyServer").ok().and_then(|v: String| {
        if v.trim().is_empty() {
            None
        } else {
            Some(v)
        }
    });

    Ok(SystemProxyState {
        enabled: enabled != 0,
        endpoint,
    })
}

#[cfg(target_os = "windows")]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_proxy_state_default() {
        let state = SystemProxyState::default();
        assert!(!state.enabled);
        assert_eq!(state.endpoint, None);
    }

    #[test]
    fn test_system_proxy_state_with_values() {
        let state = SystemProxyState {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_string()),
        };
        assert!(state.enabled);
        assert_eq!(state.endpoint, Some("127.0.0.1:7890".to_string()));
    }

    #[test]
    fn test_system_proxy_state_clone() {
        let state = SystemProxyState {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_string()),
        };
        let cloned = state.clone();
        assert_eq!(cloned.enabled, state.enabled);
        assert_eq!(cloned.endpoint, state.endpoint);
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_apply_system_proxy_none_non_windows() {
        let result = apply_system_proxy(None);
        assert!(result.is_ok());
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_apply_system_proxy_some_non_windows() {
        let result = apply_system_proxy(Some("127.0.0.1:7890"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("仅支持 Windows"));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_read_system_proxy_state_non_windows() {
        let result = read_system_proxy_state();
        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.enabled, false);
        assert_eq!(state.endpoint, None);
    }
}

use anyhow::anyhow;
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

const DEFAULT_BYPASS: &str = "localhost;127.*;10.*;172.16.*;192.168.*;<local>";

#[derive(Clone, Default, Debug, PartialEq)]
pub struct SystemProxyState {
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub bypass: Option<String>,
}

pub fn apply_system_proxy(endpoint: Option<&str>) -> anyhow::Result<()> {
    apply_system_proxy_with_bypass(endpoint, Some(DEFAULT_BYPASS))
}

pub fn apply_system_proxy_with_bypass(endpoint: Option<&str>, bypass: Option<&str>) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        set_windows_system_proxy(endpoint, bypass)
    }
    #[cfg(target_os = "linux")]
    {
        set_linux_system_proxy(endpoint, bypass)
    }
    #[cfg(target_os = "macos")]
    {
        set_macos_system_proxy(endpoint, bypass)
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        if endpoint.is_some() {
            Err(anyhow!("Unsupported platform for system proxy"))
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
    #[cfg(target_os = "linux")]
    {
        read_linux_system_proxy_state()
    }
    #[cfg(target_os = "macos")]
    {
        read_macos_system_proxy_state()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        Ok(SystemProxyState::default())
    }
}

fn parse_endpoint(endpoint: &str) -> Option<(&str, u16)> {
    let (host, port_str) = endpoint.rsplit_once(':')?;
    
    
    let port = port_str.parse::<u16>().ok()?;
    Some((host, port))
}

#[cfg(target_os = "windows")]
fn set_windows_system_proxy(endpoint: Option<&str>, bypass: Option<&str>) -> anyhow::Result<()> {
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

#[cfg(target_os = "windows")]
fn read_windows_system_proxy_state() -> anyhow::Result<SystemProxyState> {
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

#[cfg(target_os = "linux")]
fn run_gsettings(args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new("gsettings").args(args).output()?;
    if !output.status.success() {
        return Err(anyhow!("gsettings failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(target_os = "linux")]
fn set_linux_system_proxy(endpoint: Option<&str>, bypass: Option<&str>) -> anyhow::Result<()> {
    if Command::new("gsettings").arg("--version").output().is_err() {
        return Err(anyhow!("gsettings not found, desktop environment unsupported"));
    }

    if let Some(ep) = endpoint {
        let (host, port) = parse_endpoint(ep).ok_or_else(|| anyhow!("Invalid endpoint format"))?;
        let port_str = port.to_string();
        
        run_gsettings(&["set", "org.gnome.system.proxy", "mode", "'manual'"])?;
        
        for proto in &["http", "https", "socks"] {
            run_gsettings(&["set", &format!("org.gnome.system.proxy.{}", proto), "host", &format!("'{}'", host)])?;
            run_gsettings(&["set", &format!("org.gnome.system.proxy.{}", proto), "port", &port_str])?;
        }

        if let Some(b) = bypass {
            // Linux expects array format: ['localhost', '127.0.0.0/8', ...]
            let mut list = String::new();
            list.push('[');
            let parts: Vec<&str> = b.split(';').filter(|s| !s.is_empty()).collect();
            for (i, p) in parts.iter().enumerate() {
                list.push('\'');
                list.push_str(p);
                list.push('\'');
                if i < parts.len() - 1 { list.push_str(", "); }
            }
            list.push(']');
            run_gsettings(&["set", "org.gnome.system.proxy", "ignore-hosts", &list])?;
        }
    } else {
        run_gsettings(&["set", "org.gnome.system.proxy", "mode", "'none'"])?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn read_linux_system_proxy_state() -> anyhow::Result<SystemProxyState> {
    if Command::new("gsettings").arg("--version").output().is_err() {
        return Err(anyhow!("gsettings not found"));
    }

    let mode = run_gsettings(&["get", "org.gnome.system.proxy", "mode"]).unwrap_or_default();
    let enabled = mode.contains("'manual'");
    
    let mut endpoint = None;
    if enabled
        && let Ok(host) = run_gsettings(&["get", "org.gnome.system.proxy.http", "host"])
            && let Ok(port) = run_gsettings(&["get", "org.gnome.system.proxy.http", "port"]) {
                let h = host.trim_matches('\'');
                if !h.is_empty() {
                    endpoint = Some(format!("{}:{}", h, port));
                }
            }
    
    let mut bypass = None;
    if let Ok(hosts) = run_gsettings(&["get", "org.gnome.system.proxy", "ignore-hosts"])
        && hosts != "@as []" && hosts != "[]" {
            let hosts = hosts.trim_matches('[').trim_matches(']');
            let parts: Vec<String> = hosts.split(',')
                .map(|s| s.trim().trim_matches('\'').to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !parts.is_empty() {
                bypass = Some(parts.join(";"));
            }
        }

    Ok(SystemProxyState { enabled, endpoint, bypass })
}

#[cfg(target_os = "macos")]
fn get_active_network_service() -> anyhow::Result<String> {
    let output = Command::new("networksetup").args(["-listnetworkserviceorder"]).output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.starts_with("(1)") {
            if let Some(service) = line.split(" Hardware Port: ").next() {
                let s = service.trim_start_matches("(1)").trim();
                return Ok(s.to_string());
            }
        }
    }
    Ok("Wi-Fi".to_string())
}

#[cfg(target_os = "macos")]
fn set_macos_system_proxy(endpoint: Option<&str>, bypass: Option<&str>) -> anyhow::Result<()> {
    let service = get_active_network_service()?;
    
    if let Some(ep) = endpoint {
        let (host, port) = parse_endpoint(ep).ok_or_else(|| anyhow!("Invalid endpoint format"))?;
        let port_str = port.to_string();
        
        for proxy_type in &["-setwebproxy", "-setsecurewebproxy", "-setsocksfirewallproxy"] {
            Command::new("networksetup")
                .args([proxy_type, &service, host, &port_str])
                .output()?;
            Command::new("networksetup")
                .args([&format!("{}state", proxy_type), &service, "on"])
                .output()?;
        }
        
        if let Some(b) = bypass {
            let parts: Vec<&str> = b.split(';').filter(|s| !s.is_empty()).collect();
            let mut args = vec!["-setproxybypassdomains", &service];
            args.extend(parts);
            Command::new("networksetup").args(&args).output()?;
        }
    } else {
        for proxy_type in &["-setwebproxystate", "-setsecurewebproxystate", "-setsocksfirewallproxystate"] {
            Command::new("networksetup")
                .args([proxy_type, &service, "off"])
                .output()?;
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn read_macos_system_proxy_state() -> anyhow::Result<SystemProxyState> {
    let service = get_active_network_service()?;
    let output = Command::new("networksetup")
        .args(["-getwebproxy", &service])
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    let mut enabled = false;
    let mut host = String::new();
    let mut port = String::new();
    
    for line in stdout.lines() {
        if line.starts_with("Enabled: Yes") {
            enabled = true;
        } else if line.starts_with("Server:") {
            host = line.trim_start_matches("Server:").trim().to_string();
        } else if line.starts_with("Port:") {
            port = line.trim_start_matches("Port:").trim().to_string();
        }
    }
    
    let endpoint = if enabled && !host.is_empty() && port != "0" {
        Some(format!("{}:{}", host, port))
    } else {
        None
    };
    
    let mut bypass = None;
    let bypass_output = Command::new("networksetup")
        .args(["-getproxybypassdomains", &service])
        .output()?;
    let bypass_stdout = String::from_utf8_lossy(&bypass_output.stdout);
    let parts: Vec<String> = bypass_stdout.lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "There aren't any bypass domains set on Wi-Fi.")
        .collect();
    if !parts.is_empty() {
        bypass = Some(parts.join(";"));
    }
    
    Ok(SystemProxyState { enabled, endpoint, bypass })
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

    // Mocked test cases for non-Windows platforms
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    #[test]
    fn test_apply_system_proxy_unsupported() {
        let result = apply_system_proxy(Some("127.0.0.1:7890"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported platform"));
        
        let result = apply_system_proxy(None);
        assert!(result.is_ok());
    }
    
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    #[test]
    fn test_read_system_proxy_state_unsupported() {
        let result = read_system_proxy_state();
        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.enabled, false);
    }
}

use super::{SystemProxyState, parse_endpoint};
use anyhow::anyhow;
use std::process::Command;

/// Linux 桌面环境不受支持错误（保留兼容）。
#[derive(Debug)]
pub struct UnsupportedDesktopError {
    /// 缺失的后端可执行文件名。
    pub backend: &'static str,
}

impl std::fmt::Display for UnsupportedDesktopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "system proxy control requires GNOME ({0} not found); \
             the current desktop environment is unsupported \
             (KDE and other backends are not implemented)",
            self.backend
        )
    }
}

impl std::error::Error for UnsupportedDesktopError {}

/// GNOME 桌面环境代理后端（基于 `gsettings`）。

/// 检查 `gsettings` 是否在系统中可用。
pub fn is_available() -> bool {
    Command::new("gsettings")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn run_gsettings(args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new("gsettings")
        .args(args)
        .output()
        .map_err(|e| anyhow!("gsettings {} spawn failed: {e}", args.join(" ")))?;
    if !output.status.success() {
        return Err(anyhow!(
            "gsettings failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// 将 bypass 规则格式化为 gsettings `ignore-hosts` 数组字符串。
pub fn format_ignore_hosts_array(bypass: Option<&str>) -> String {
    let mut list = String::new();
    list.push('[');
    if let Some(b) = bypass {
        let parts: Vec<&str> = b
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        for (i, p) in parts.iter().enumerate() {
            list.push('\'');
            list.push_str(p);
            list.push('\'');
            if i < parts.len() - 1 {
                list.push_str(", ");
            }
        }
    }
    list.push(']');
    list
}

/// 生成 gsettings 设置参数序列。
pub fn generate_gsettings_commands(
    endpoint: Option<&str>,
    bypass: Option<&str>,
) -> Vec<Vec<String>> {
    let mut cmds = Vec::new();
    if let Some(ep) = endpoint {
        if let Some((host, port)) = parse_endpoint(ep) {
            let port_str = port.to_string();
            cmds.push(vec![
                "set".into(),
                "org.gnome.system.proxy".into(),
                "mode".into(),
                "'manual'".into(),
            ]);
            for proto in &["http", "https", "socks"] {
                cmds.push(vec![
                    "set".into(),
                    format!("org.gnome.system.proxy.{}", proto),
                    "host".into(),
                    format!("'{}'", host),
                ]);
                cmds.push(vec![
                    "set".into(),
                    format!("org.gnome.system.proxy.{}", proto),
                    "port".into(),
                    port_str.clone(),
                ]);
            }
            if let Some(b) = bypass {
                let ignore_hosts = format_ignore_hosts_array(Some(b));
                cmds.push(vec![
                    "set".into(),
                    "org.gnome.system.proxy".into(),
                    "ignore-hosts".into(),
                    ignore_hosts,
                ]);
            }
        }
    } else {
        cmds.push(vec![
            "set".into(),
            "org.gnome.system.proxy".into(),
            "mode".into(),
            "'none'".into(),
        ]);
    }
    cmds
}

pub fn apply(endpoint: Option<&str>, bypass: Option<&str>) -> anyhow::Result<()> {
    if !is_available() {
        return Err(anyhow!(UnsupportedDesktopError {
            backend: "gsettings"
        }));
    }
    let cmds = generate_gsettings_commands(endpoint, bypass);
    if cmds.is_empty() && endpoint.is_some() {
        return Err(anyhow!("Invalid endpoint format"));
    }
    for cmd in cmds {
        let str_args: Vec<&str> = cmd.iter().map(|s| s.as_str()).collect();
        run_gsettings(&str_args)?;
    }
    Ok(())
}

pub fn read_state() -> anyhow::Result<SystemProxyState> {
    if !is_available() {
        return Err(anyhow!(UnsupportedDesktopError {
            backend: "gsettings"
        }));
    }

    let mode = run_gsettings(&["get", "org.gnome.system.proxy", "mode"]).unwrap_or_default();
    let enabled = mode.contains("'manual'");

    let mut endpoint = None;
    if enabled
        && let Ok(host) = run_gsettings(&["get", "org.gnome.system.proxy.http", "host"])
        && let Ok(port) = run_gsettings(&["get", "org.gnome.system.proxy.http", "port"])
    {
        let h = host.trim_matches('\'');
        if !h.is_empty() {
            endpoint = Some(format!("{}:{}", h, port));
        }
    }

    let mut bypass = None;
    if let Ok(hosts) = run_gsettings(&["get", "org.gnome.system.proxy", "ignore-hosts"])
        && hosts != "@as []"
        && hosts != "[]"
    {
        let hosts = hosts.trim_matches('[').trim_matches(']');
        let parts: Vec<String> = hosts
            .split(',')
            .map(|s| s.trim().trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !parts.is_empty() {
            bypass = Some(parts.join(";"));
        }
    }

    Ok(SystemProxyState {
        enabled,
        endpoint,
        bypass,
    })
}

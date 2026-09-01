//! macOS 实现：`networksetup`，所有命令强制检查退出码，非零返回 `Err`
//! （附 stderr/stdout 摘要）。

use super::{parse_endpoint, SystemProxyState};
use anyhow::anyhow;
use std::process::Command;

/// 运行 `networksetup` 并强制检查退出码：非零必须返回 `Err`（附
/// stderr/stdout 摘要），成功时返回修剪后的 stdout。
fn run_networksetup(args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new("networksetup")
        .args(args)
        .output()
        .map_err(|e| anyhow!("networksetup {} spawn failed: {e}", args.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let summary = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        return Err(anyhow!(
            "networksetup {} failed (exit code {:?}): {}",
            args.join(" "),
            output.status.code(),
            summary
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn get_active_network_service() -> anyhow::Result<String> {
    let stdout = run_networksetup(&["-listnetworkserviceorder"])?;
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

pub(super) fn apply(endpoint: Option<&str>, bypass: Option<&str>) -> anyhow::Result<()> {
    let service = get_active_network_service()?;

    if let Some(ep) = endpoint {
        let (host, port) = parse_endpoint(ep).ok_or_else(|| anyhow!("Invalid endpoint format"))?;
        let port_str = port.to_string();

        for proxy_type in &["-setwebproxy", "-setsecurewebproxy", "-setsocksfirewallproxy"] {
            run_networksetup(&[proxy_type, &service, host, &port_str])?;
            run_networksetup(&[&format!("{proxy_type}state"), &service, "on"])?;
        }

        if let Some(b) = bypass {
            let parts: Vec<&str> = b.split(';').filter(|s| !s.is_empty()).collect();
            let mut args = vec!["-setproxybypassdomains", &service];
            args.extend(parts);
            run_networksetup(&args)?;
        }
    } else {
        for proxy_type in &["-setwebproxystate", "-setsecurewebproxystate", "-setsocksfirewallproxystate"] {
            run_networksetup(&[proxy_type, &service, "off"])?;
        }
    }
    Ok(())
}

pub(super) fn read_state() -> anyhow::Result<SystemProxyState> {
    let service = get_active_network_service()?;
    let stdout = run_networksetup(&["-getwebproxy", &service])?;

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
    let bypass_stdout = run_networksetup(&["-getproxybypassdomains", &service])?;
    let parts: Vec<String> = bypass_stdout.lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "There aren't any bypass domains set on Wi-Fi.")
        .collect();
    if !parts.is_empty() {
        bypass = Some(parts.join(";"));
    }

    Ok(SystemProxyState { enabled, endpoint, bypass })
}

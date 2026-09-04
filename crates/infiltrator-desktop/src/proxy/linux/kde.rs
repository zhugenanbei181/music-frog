use super::{SystemProxyState, parse_endpoint, parse_url_to_endpoint};
use anyhow::anyhow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

/// KDE Plasma 桌面环境代理后端（基于 `kwriteconfig5`/`kwriteconfig6` 与 `kconfig` / `kioslaverc`）。
/// 查找可用的 KDE 配置写入工具（优先 kwriteconfig6，其次 kwriteconfig5）。
pub fn find_kwriteconfig() -> Option<&'static str> {
    ["kwriteconfig6", "kwriteconfig5"]
        .iter()
        .find(|&cmd| {
            Command::new(cmd)
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        })
        .map(|v| v as _)
}

/// 查找可用的 KDE 配置读取工具（优先 kreadconfig6，其次 kreadconfig5）。
pub fn find_kreadconfig() -> Option<&'static str> {
    ["kreadconfig6", "kreadconfig5"]
        .iter()
        .find(|&cmd| {
            Command::new(cmd)
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        })
        .map(|v| v as _)
}

/// 定位 KDE kioslaverc 配置文件路径。
pub fn kioslaverc_path() -> Option<PathBuf> {
    if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME")
        && !config_home.trim().is_empty()
    {
        return Some(PathBuf::from(config_home).join("kioslaverc"));
    }
    if let Ok(home) = std::env::var("HOME")
        && !home.trim().is_empty()
    {
        return Some(PathBuf::from(home).join(".config").join("kioslaverc"));
    }
    None
}

/// KDE 是否可用（存在 kwriteconfig/kreadconfig 或可定位到 kioslaverc 配置文件）。
pub fn is_available() -> bool {
    find_kwriteconfig().is_some() || find_kreadconfig().is_some() || kioslaverc_path().is_some()
}

/// 将分号分隔的 bypass 列表转换为 KDE 逗号分隔格式。
pub fn format_bypass_for_kde(bypass: &str) -> String {
    bypass
        .split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(",")
}

/// 将 KDE 逗号分隔的 bypass 转换为统一的分号分隔格式。
pub fn format_bypass_from_kde(bypass: &str) -> String {
    bypass
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(";")
}

/// 生成 KDE kwriteconfig 命令行参数列表。
pub fn generate_write_commands(
    tool: &str,
    endpoint: Option<&str>,
    bypass: Option<&str>,
) -> Vec<Vec<String>> {
    let mut cmds = Vec::new();
    if let Some(ep) = endpoint {
        if let Some((host, port)) = parse_endpoint(ep) {
            let http_url = format!("http://{host}:{port}");
            let socks_url = format!("socks://{host}:{port}");

            cmds.push(vec![
                tool.into(),
                "--file".into(),
                "kioslaverc".into(),
                "--group".into(),
                "Proxy Settings".into(),
                "--key".into(),
                "ProxyType".into(),
                "1".into(),
            ]);
            cmds.push(vec![
                tool.into(),
                "--file".into(),
                "kioslaverc".into(),
                "--group".into(),
                "Proxy Settings".into(),
                "--key".into(),
                "httpProxy".into(),
                http_url.clone(),
            ]);
            cmds.push(vec![
                tool.into(),
                "--file".into(),
                "kioslaverc".into(),
                "--group".into(),
                "Proxy Settings".into(),
                "--key".into(),
                "httpsProxy".into(),
                http_url.clone(),
            ]);
            cmds.push(vec![
                tool.into(),
                "--file".into(),
                "kioslaverc".into(),
                "--group".into(),
                "Proxy Settings".into(),
                "--key".into(),
                "ftpProxy".into(),
                http_url,
            ]);
            cmds.push(vec![
                tool.into(),
                "--file".into(),
                "kioslaverc".into(),
                "--group".into(),
                "Proxy Settings".into(),
                "--key".into(),
                "socksProxy".into(),
                socks_url,
            ]);
            if let Some(b) = bypass {
                let kde_b = format_bypass_for_kde(b);
                cmds.push(vec![
                    tool.into(),
                    "--file".into(),
                    "kioslaverc".into(),
                    "--group".into(),
                    "Proxy Settings".into(),
                    "--key".into(),
                    "NoProxyFor".into(),
                    kde_b,
                ]);
            }
        }
    } else {
        cmds.push(vec![
            tool.into(),
            "--file".into(),
            "kioslaverc".into(),
            "--group".into(),
            "Proxy Settings".into(),
            "--key".into(),
            "ProxyType".into(),
            "0".into(),
        ]);
    }
    cmds
}

/// 生成 KDE kreadconfig 命令行参数。
pub fn generate_read_commands(tool: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
    (
        vec![
            tool.into(),
            "--file".into(),
            "kioslaverc".into(),
            "--group".into(),
            "Proxy Settings".into(),
            "--key".into(),
            "ProxyType".into(),
        ],
        vec![
            tool.into(),
            "--file".into(),
            "kioslaverc".into(),
            "--group".into(),
            "Proxy Settings".into(),
            "--key".into(),
            "httpProxy".into(),
        ],
        vec![
            tool.into(),
            "--file".into(),
            "kioslaverc".into(),
            "--group".into(),
            "Proxy Settings".into(),
            "--key".into(),
            "NoProxyFor".into(),
        ],
    )
}

/// 更新 INI 格式的 kioslaverc 内容。
pub fn update_kioslaverc_content(
    existing_content: &str,
    endpoint: Option<&str>,
    bypass: Option<&str>,
) -> String {
    let lines: Vec<String> = existing_content.lines().map(|s| s.to_string()).collect();
    let mut in_proxy_settings = false;
    let mut proxy_settings_found = false;
    let mut keys_to_write = HashMap::new();

    if let Some(ep) = endpoint {
        if let Some((host, port)) = parse_endpoint(ep) {
            let http_url = format!("http://{host}:{port}");
            let socks_url = format!("socks://{host}:{port}");
            keys_to_write.insert("ProxyType".to_string(), "1".to_string());
            keys_to_write.insert("httpProxy".to_string(), http_url.clone());
            keys_to_write.insert("httpsProxy".to_string(), http_url.clone());
            keys_to_write.insert("ftpProxy".to_string(), http_url);
            keys_to_write.insert("socksProxy".to_string(), socks_url);
            if let Some(b) = bypass {
                keys_to_write.insert("NoProxyFor".to_string(), format_bypass_for_kde(b));
            }
        }
    } else {
        keys_to_write.insert("ProxyType".to_string(), "0".to_string());
    }

    let mut written_keys = std::collections::HashSet::new();
    let mut new_lines = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section_name = &trimmed[1..trimmed.len() - 1];
            if in_proxy_settings {
                for (k, v) in &keys_to_write {
                    if !written_keys.contains(k) {
                        new_lines.push(format!("{}={}", k, v));
                    }
                }
                in_proxy_settings = false;
            }
            if section_name.trim().eq_ignore_ascii_case("Proxy Settings") {
                in_proxy_settings = true;
                proxy_settings_found = true;
            }
            new_lines.push(line);
        } else if in_proxy_settings {
            if let Some((k, _)) = trimmed.split_once('=') {
                let key = k.trim();
                if let Some(new_val) = keys_to_write.get(key) {
                    new_lines.push(format!("{}={}", key, new_val));
                    written_keys.insert(key.to_string());
                    continue;
                }
            }
            new_lines.push(line);
        } else {
            new_lines.push(line);
        }
    }

    if in_proxy_settings {
        for (k, v) in &keys_to_write {
            if !written_keys.contains(k) {
                new_lines.push(format!("{}={}", k, v));
            }
        }
    } else if !proxy_settings_found {
        if !new_lines.is_empty() && !new_lines.last().unwrap().is_empty() {
            new_lines.push(String::new());
        }
        new_lines.push("[Proxy Settings]".to_string());
        for (k, v) in &keys_to_write {
            new_lines.push(format!("{}={}", k, v));
        }
    }

    let mut res = new_lines.join("\n");
    res.push('\n');
    res
}

/// 解析 INI 格式的 kioslaverc 内容。
pub fn parse_kioslaverc_content(content: &str) -> SystemProxyState {
    let mut in_proxy_settings = false;
    let mut proxy_type = None;
    let mut http_proxy = None;
    let mut socks_proxy = None;
    let mut no_proxy_for = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section_name = &trimmed[1..trimmed.len() - 1];
            in_proxy_settings = section_name.trim().eq_ignore_ascii_case("Proxy Settings");
            continue;
        }
        if in_proxy_settings && let Some((k, v)) = trimmed.split_once('=') {
            let key = k.trim();
            let val = v.trim();
            if key.eq_ignore_ascii_case("ProxyType") {
                proxy_type = Some(val.to_string());
            } else if key.eq_ignore_ascii_case("httpProxy") {
                http_proxy = Some(val.to_string());
            } else if key.eq_ignore_ascii_case("socksProxy") {
                socks_proxy = Some(val.to_string());
            } else if key.eq_ignore_ascii_case("NoProxyFor") {
                no_proxy_for = Some(val.to_string());
            }
        }
    }

    let enabled = matches!(proxy_type.as_deref(), Some("1") | Some("4"));

    let endpoint = if enabled {
        if let Some(ref hp) = http_proxy {
            parse_url_to_endpoint(hp)
        } else if let Some(ref sp) = socks_proxy {
            parse_url_to_endpoint(sp)
        } else {
            None
        }
    } else {
        None
    };

    let bypass = if enabled {
        no_proxy_for
            .map(|b| format_bypass_from_kde(&b))
            .filter(|s| !s.is_empty())
    } else {
        None
    };

    SystemProxyState {
        enabled,
        endpoint,
        bypass,
    }
}

/// 通知 KDE KIO 重新读取代理配置。
pub fn notify_kio() {
    let _ = Command::new("dbus-send")
        .args([
            "--type=signal",
            "/KIO/Scheduler",
            "org.kde.KIO.Scheduler.reparseSlaveConfiguration",
            "string:",
        ])
        .output();
}

/// 应用 KDE 代理设置。
pub fn apply(endpoint: Option<&str>, bypass: Option<&str>) -> anyhow::Result<()> {
    if let Some(ep) = endpoint
        && parse_endpoint(ep).is_none()
    {
        return Err(anyhow!("Invalid endpoint format"));
    }

    if let Some(tool) = find_kwriteconfig() {
        let cmds = generate_write_commands(tool, endpoint, bypass);
        for cmd in cmds {
            let status = Command::new(&cmd[0])
                .args(&cmd[1..])
                .status()
                .map_err(|e| anyhow!("{} spawn failed: {e}", cmd[0]))?;
            if !status.success() {
                return Err(anyhow!(
                    "{} failed with exit code: {:?}",
                    cmd[0],
                    status.code()
                ));
            }
        }
        notify_kio();
        return Ok(());
    }

    // 回退：直接写入 kioslaverc 文件
    if let Some(path) = kioslaverc_path() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        let new_content = update_kioslaverc_content(&existing, endpoint, bypass);
        std::fs::write(&path, new_content)?;
        notify_kio();
        return Ok(());
    }

    Err(anyhow!(
        "Neither kwriteconfig nor kioslaverc path could be found for KDE"
    ))
}

/// 读取 KDE 代理状态。
pub fn read_state() -> anyhow::Result<SystemProxyState> {
    if let Some(tool) = find_kreadconfig() {
        let (cmd_type, cmd_http, cmd_bypass) = generate_read_commands(tool);
        let out_type = Command::new(&cmd_type[0])
            .args(&cmd_type[1..])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();

        let enabled = out_type == "1" || out_type == "4";
        let mut endpoint = None;
        let mut bypass = None;

        if enabled {
            let out_http = Command::new(&cmd_http[0])
                .args(&cmd_http[1..])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            endpoint = parse_url_to_endpoint(&out_http);

            let out_bypass = Command::new(&cmd_bypass[0])
                .args(&cmd_bypass[1..])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            if !out_bypass.is_empty() {
                bypass = Some(format_bypass_from_kde(&out_bypass));
            }
        }

        return Ok(SystemProxyState {
            enabled,
            endpoint,
            bypass,
        });
    }

    // 回退：直接读取 kioslaverc 文件
    if let Some(path) = kioslaverc_path()
        && path.exists()
    {
        let content = std::fs::read_to_string(&path)?;
        return Ok(parse_kioslaverc_content(&content));
    }

    Ok(SystemProxyState::default())
}

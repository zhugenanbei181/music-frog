use super::{SystemProxyState, parse_endpoint, parse_url_to_endpoint};
use anyhow::anyhow;
use std::collections::HashMap;
use std::path::PathBuf;

/// 环境变量代理模式（http_proxy / https_proxy / all_proxy / no_proxy）。
/// 代理环境变量名称列表（大小写全覆盖）。
pub const PROXY_ENV_KEYS: &[&str] = &[
    "http_proxy",
    "HTTP_PROXY",
    "https_proxy",
    "HTTPS_PROXY",
    "all_proxy",
    "ALL_PROXY",
    "no_proxy",
    "NO_PROXY",
];

/// 将分号分隔的 bypass 列表转换为逗号分隔的标准环境变量 no_proxy 格式。
pub fn format_no_proxy(bypass: &str) -> String {
    bypass
        .split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(",")
}

/// 将逗号或分号分隔的 no_proxy 字符串转换为统一的分号分隔格式。
pub fn parse_no_proxy(val: &str) -> String {
    val.split([',', ';'])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(";")
}

/// 生成环境变量键值对映射。
pub fn generate_env_vars(endpoint: Option<&str>, bypass: Option<&str>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Some(ep) = endpoint
        && let Some((host, port)) = parse_endpoint(ep) {
            let http_url = format!("http://{host}:{port}");
            let socks_url = format!("socks5://{host}:{port}");

            map.insert("http_proxy".to_string(), http_url.clone());
            map.insert("HTTP_PROXY".to_string(), http_url.clone());
            map.insert("https_proxy".to_string(), http_url.clone());
            map.insert("HTTPS_PROXY".to_string(), http_url);
            map.insert("all_proxy".to_string(), socks_url.clone());
            map.insert("ALL_PROXY".to_string(), socks_url);

            if let Some(b) = bypass {
                let no_p = format_no_proxy(b);
                map.insert("no_proxy".to_string(), no_p.clone());
                map.insert("NO_PROXY".to_string(), no_p);
            }
        }
    map
}

/// 生成用于 shell 执行的 export/unset 语句脚本。
pub fn generate_shell_export(endpoint: Option<&str>, bypass: Option<&str>) -> String {
    if endpoint.is_some() {
        let vars = generate_env_vars(endpoint, bypass);
        let mut lines = Vec::new();
        for key in PROXY_ENV_KEYS {
            if let Some(val) = vars.get(*key) {
                lines.push(format!("export {}=\"{}\"", key, val));
            }
        }
        lines.join("\n")
    } else {
        "unset http_proxy HTTP_PROXY https_proxy HTTPS_PROXY all_proxy ALL_PROXY no_proxy NO_PROXY"
            .to_string()
    }
}

/// 生成 systemd environment.d 格式的配置内容。
pub fn generate_environment_d(endpoint: Option<&str>, bypass: Option<&str>) -> String {
    if endpoint.is_some() {
        let vars = generate_env_vars(endpoint, bypass);
        let mut lines = Vec::new();
        for key in PROXY_ENV_KEYS {
            if let Some(val) = vars.get(*key) {
                lines.push(format!("{}={}", key, val));
            }
        }
        lines.join("\n")
    } else {
        String::new()
    }
}

/// 定位 environment.d 配置文件路径（`~/.config/environment.d/99-infiltrator-proxy.conf`）。
pub fn environment_d_path() -> Option<PathBuf> {
    if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME")
        && !config_home.trim().is_empty() {
            return Some(
                PathBuf::from(config_home)
                    .join("environment.d")
                    .join("99-infiltrator-proxy.conf"),
            );
        }
    if let Ok(home) = std::env::var("HOME")
        && !home.trim().is_empty() {
            return Some(
                PathBuf::from(home)
                    .join(".config")
                    .join("environment.d")
                    .join("99-infiltrator-proxy.conf"),
            );
        }
    None
}

/// 应用环境变量代理设置（同时设置进程环境变量与 environment.d 配置文件）。
pub fn apply(endpoint: Option<&str>, bypass: Option<&str>) -> anyhow::Result<()> {
    if let Some(ep) = endpoint {
        if parse_endpoint(ep).is_none() {
            return Err(anyhow!("Invalid endpoint format"));
        }
        let vars = generate_env_vars(Some(ep), bypass);
        for (k, v) in &vars {
            unsafe {
                std::env::set_var(k, v);
            }
        }
        // 尽最大努力写入 environment.d
        if let Some(conf_path) = environment_d_path() {
            if let Some(parent) = conf_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let content = generate_environment_d(Some(ep), bypass);
            let _ = std::fs::write(&conf_path, content);
        }
    } else {
        for k in PROXY_ENV_KEYS {
            unsafe {
                std::env::remove_var(k);
            }
        }
        // 尽最大努力移除 environment.d
        if let Some(conf_path) = environment_d_path() {
            let _ = std::fs::remove_file(&conf_path);
        }
    }
    Ok(())
}

/// 基于自定义环境变量获取函数的读取实现。
pub fn read_state_with<F>(get_env: F) -> SystemProxyState
where
    F: Fn(&str) -> Option<String>,
{
    let mut endpoint = None;
    for key in &[
        "http_proxy",
        "HTTP_PROXY",
        "all_proxy",
        "ALL_PROXY",
        "https_proxy",
        "HTTPS_PROXY",
    ] {
        if let Some(val) = get_env(key)
            && let Some(ep) = parse_url_to_endpoint(&val) {
                endpoint = Some(ep);
                break;
            }
    }

    let enabled = endpoint.is_some();
    let mut bypass = None;
    if enabled {
        for key in &["no_proxy", "NO_PROXY"] {
            if let Some(val) = get_env(key) {
                let parsed = parse_no_proxy(&val);
                if !parsed.is_empty() {
                    bypass = Some(parsed);
                    break;
                }
            }
        }
    }

    SystemProxyState {
        enabled,
        endpoint,
        bypass,
    }
}

/// 读取当前环境变量代理状态。
pub fn read_state() -> anyhow::Result<SystemProxyState> {
    Ok(read_state_with(|k| std::env::var(k).ok()))
}

//! Linux 实现：仅支持 GNOME 的 gsettings 后端；KDE 及其它桌面环境不受
//! 支持（刻意不做）。检测不到 `gsettings` 时返回类型化的
//! [`UnsupportedDesktopError`]，绝不静默假装成功。

use super::{parse_endpoint, SystemProxyState};
use anyhow::anyhow;
use std::process::Command;

/// Linux 桌面环境不受支持：系统代理只实现了 GNOME/gsettings 后端，KDE
/// 等其它桌面不做。调用方可经 `anyhow::Error::downcast_ref` 拿回本类型，
/// 据此给出「当前桌面环境不受支持」的针对性提示。
#[derive(Debug)]
pub struct UnsupportedDesktopError {
    /// 缺失的后端可执行文件名（当前固定为 `gsettings`）。
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

fn run_gsettings(args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new("gsettings")
        .args(args)
        .output()
        .map_err(|e| anyhow!("gsettings {} spawn failed: {e}", args.join(" ")))?;
    if !output.status.success() {
        return Err(anyhow!("gsettings failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// 后端可用性检测：`gsettings` 不存在 → 类型化的
/// [`UnsupportedDesktopError`]（当前桌面环境不受支持）；存在但自身报错 →
/// 带 stderr 摘要的普通错误。两种情况都不允许继续假装成功。
fn ensure_gsettings_backend() -> anyhow::Result<()> {
    match Command::new("gsettings").arg("--version").output() {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(anyhow!(
            "gsettings --version failed (exit code {:?}): {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Err(anyhow!(UnsupportedDesktopError { backend: "gsettings" }))
        }
        Err(err) => Err(anyhow!("gsettings spawn failed: {err}")),
    }
}

pub(super) fn apply(endpoint: Option<&str>, bypass: Option<&str>) -> anyhow::Result<()> {
    ensure_gsettings_backend()?;

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

pub(super) fn read_state() -> anyhow::Result<SystemProxyState> {
    ensure_gsettings_backend()?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    /// Linux 检测失败必须给出类型化错误：可经 anyhow 下沉后下cast 回
    /// [`UnsupportedDesktopError`]，且消息明确「当前桌面环境不受支持」。
    #[test]
    fn test_unsupported_desktop_error_is_typed_and_explicit() {
        let err = anyhow!(UnsupportedDesktopError { backend: "gsettings" });
        let typed = err
            .downcast_ref::<UnsupportedDesktopError>()
            .expect("typed error survives the anyhow downcast");
        assert_eq!(typed.backend, "gsettings");

        let message = typed.to_string();
        assert!(message.contains("unsupported"), "{message}");
        assert!(message.contains("desktop environment"), "{message}");
        assert!(message.contains("gsettings"), "{message}");
        assert!(message.contains("KDE"), "{message}");
    }
}

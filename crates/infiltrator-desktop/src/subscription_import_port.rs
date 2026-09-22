//! DUAL-07-01: desktop host adapter for local-file and clipboard imports.
//!
//! This is the concrete [`SubscriptionImportPort`] the desktop composition
//! installs. It reads real files and the platform clipboard (pbpaste /
//! Get-Clipboard / wl-paste / xclip / xsel). A headless session without a
//! display server reports a typed unsupported clipboard rather than pretending
//! it read an empty string.

use async_trait::async_trait;
use infiltrator_contract::capability::Capability;
use infiltrator_ports::error::PortError;
use infiltrator_ports::subscription_import::SubscriptionImportPort;

/// Desktop-backed import source.
#[derive(Clone, Copy, Debug, Default)]
pub struct DesktopSubscriptionImportPort;

#[async_trait]
impl SubscriptionImportPort for DesktopSubscriptionImportPort {
    async fn read_local_file(&self, path: &str) -> Result<String, PortError> {
        let path = path.trim();
        if path.is_empty() {
            return Err(PortError::Io("本地文件路径不能为空".to_string()));
        }
        tokio::fs::read_to_string(path)
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }

    async fn read_clipboard(&self) -> Result<String, PortError> {
        let text = tokio::task::spawn_blocking(read_clipboard_blocking)
            .await
            .map_err(|error| PortError::Failed(error.to_string()))??;
        if text.trim().is_empty() {
            return Err(PortError::unsupported(
                Capability::Profiles,
                "剪贴板内容为空",
            ));
        }
        Ok(text)
    }
}

fn read_clipboard_blocking() -> Result<String, PortError> {
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("pbpaste").output()
            && output.status.success()
            && let Ok(text) = String::from_utf8(output.stdout)
        {
            return Ok(text);
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "Get-Clipboard"])
            .output()
            && output.status.success()
            && let Ok(text) = String::from_utf8(output.stdout)
        {
            return Ok(text);
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if std::env::var_os("WAYLAND_DISPLAY").is_some()
            && let Ok(output) = std::process::Command::new("wl-paste")
                .args(["--no-newline"])
                .output()
            && output.status.success()
            && let Ok(text) = String::from_utf8(output.stdout)
        {
            return Ok(text);
        }
        if std::env::var_os("DISPLAY").is_some() {
            for (program, args) in [
                ("xclip", &["-selection", "clipboard", "-o"][..]),
                ("xsel", &["--clipboard", "--output"][..]),
            ] {
                if let Ok(output) = std::process::Command::new(program).args(args).output()
                    && output.status.success()
                    && let Ok(text) = String::from_utf8(output.stdout)
                {
                    return Ok(text);
                }
            }
        }
    }
    Err(PortError::unsupported(
        Capability::Profiles,
        "当前宿主没有可用的剪贴板后端",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_path_is_rejected_before_touching_the_filesystem() {
        let port = DesktopSubscriptionImportPort;
        assert!(port.read_local_file("   ").await.is_err());
    }

    #[tokio::test]
    async fn missing_file_surfaces_as_io_error() {
        let port = DesktopSubscriptionImportPort;
        let error = port.read_local_file("/definitely/not/here.yaml").await;
        assert!(matches!(error, Err(PortError::Io(_))));
    }
}

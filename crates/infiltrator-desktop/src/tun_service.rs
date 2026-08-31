use anyhow::{Context, Result};
use std::fmt;
use std::path::Path;
use std::process::Command;

/// Represents the status of the TUN service or privilege mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceModeStatus {
    InstalledAndRunning,
    InstalledStopped,
    NotInstalled,
    MissingPrivilege,
    Unsupported,
}

impl fmt::Display for ServiceModeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::InstalledAndRunning => "Installed and Running",
            Self::InstalledStopped => "Installed but Stopped",
            Self::NotInstalled => "Not Installed",
            Self::MissingPrivilege => "Missing Privilege",
            Self::Unsupported => "Unsupported OS",
        };
        write!(f, "{}", msg)
    }
}

/// Controller for TUN Mode privileges and background services.
pub struct TunServiceManager;

impl TunServiceManager {
    pub const SERVICE_NAME: &'static str = "MusicFrogInfiltratorService";

    /// Checks the current status of the service mode.
    pub fn check_status() -> ServiceModeStatus {
        let exe = match std::env::current_exe() {
            Ok(path) => path,
            Err(_) => return ServiceModeStatus::MissingPrivilege,
        };
        Self::check_status_for(&exe)
    }

    /// Checks the service/capability state for the actual mihomo binary.
    /// This is different from [`Self::check_status`] in packaged desktops:
    /// the GUI executable and the core executable are separate files.
    pub fn check_status_for(binary_path: &Path) -> ServiceModeStatus {
        #[cfg(target_os = "windows")]
        {
            Self::check_status_windows()
        }
        #[cfg(target_os = "linux")]
        {
            Self::check_status_linux(binary_path)
        }
        #[cfg(target_os = "macos")]
        {
            Self::check_status_macos()
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            let _ = binary_path;
            ServiceModeStatus::Unsupported
        }
    }

    /// Installs the background service or grants necessary capabilities.
    pub fn install_service(service_bin_path: &Path) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            Self::install_service_windows(service_bin_path)
        }
        #[cfg(target_os = "linux")]
        {
            Self::install_service_linux(service_bin_path)
        }
        #[cfg(target_os = "macos")]
        {
            Self::install_service_macos(service_bin_path)
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            let _ = service_bin_path;
            anyhow::bail!("Unsupported OS for service installation");
        }
    }

    /// Uninstalls the service or removes capabilities.
    pub fn uninstall_service() -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            Self::uninstall_service_windows()
        }
        #[cfg(target_os = "linux")]
        {
            Self::uninstall_service_linux()
        }
        #[cfg(target_os = "macos")]
        {
            Self::uninstall_service_macos()
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            anyhow::bail!("Unsupported OS for service uninstallation");
        }
    }

    /// Starts the installed service.
    pub fn start_service() -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            Self::start_service_windows()
        }
        #[cfg(target_os = "linux")]
        {
            Self::start_service_linux()
        }
        #[cfg(target_os = "macos")]
        {
            Self::start_service_macos()
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            anyhow::bail!("Unsupported OS for service start");
        }
    }

    /// Stops the currently running service.
    pub fn stop_service() -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            Self::stop_service_windows()
        }
        #[cfg(target_os = "linux")]
        {
            Self::stop_service_linux()
        }
        #[cfg(target_os = "macos")]
        {
            Self::stop_service_macos()
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            anyhow::bail!("Unsupported OS for service stop");
        }
    }

    // =========================================================================
    // WINDOWS IMPLEMENTATION
    // =========================================================================

    #[cfg(target_os = "windows")]
    fn check_status_windows() -> ServiceModeStatus {
        let output = Command::new("sc.exe")
            .arg("query")
            .arg(Self::SERVICE_NAME)
            .output();

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.contains("1060") || stdout.contains("does not exist") {
                return ServiceModeStatus::NotInstalled;
            } else if stdout.contains("RUNNING") {
                return ServiceModeStatus::InstalledAndRunning;
            } else if stdout.contains("STOPPED") {
                return ServiceModeStatus::InstalledStopped;
            } else if !out.status.success() {
                return ServiceModeStatus::MissingPrivilege;
            }
        }
        ServiceModeStatus::MissingPrivilege
    }

    #[cfg(target_os = "windows")]
    fn install_service_windows(bin_path: &Path) -> Result<()> {
        let path_str = bin_path.to_str().context("Invalid binary path")?;
        let bin_path_arg = format!("\"{}\"", path_str);
        let status = Command::new("sc.exe")
            .args([
                "create",
                Self::SERVICE_NAME,
                "binPath=",
                &bin_path_arg,
                "start=",
                "auto",
            ])
            .status()?;

        if !status.success() {
            anyhow::bail!("Failed to create Windows service. Requires Administrator privileges.");
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn uninstall_service_windows() -> Result<()> {
        let _ = Self::stop_service_windows();
        let status = Command::new("sc.exe")
            .args(["delete", Self::SERVICE_NAME])
            .status()?;

        if !status.success() {
            anyhow::bail!("Failed to delete Windows service. Requires Administrator privileges.");
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn start_service_windows() -> Result<()> {
        let status = Command::new("sc.exe")
            .args(["start", Self::SERVICE_NAME])
            .status()?;

        if !status.success() {
            anyhow::bail!("Failed to start Windows service. Requires Administrator privileges.");
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn stop_service_windows() -> Result<()> {
        let status = Command::new("sc.exe")
            .args(["stop", Self::SERVICE_NAME])
            .status()?;

        if !status.success() {
            anyhow::bail!("Failed to stop Windows service.");
        }
        Ok(())
    }

    // =========================================================================
    // LINUX IMPLEMENTATION
    // =========================================================================

    #[cfg(target_os = "linux")]
    fn check_status_linux(exe: &Path) -> ServiceModeStatus {
        let output = match Command::new("getcap").arg(exe).output() {
            Ok(output) => output,
            Err(_) => return ServiceModeStatus::MissingPrivilege,
        };

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("cap_net_admin") && stdout.contains("cap_net_bind_service") {
                return ServiceModeStatus::InstalledAndRunning;
            }
        }
        ServiceModeStatus::NotInstalled
    }

    #[cfg(target_os = "linux")]
    fn install_service_linux(bin_path: &Path) -> Result<()> {
        // Do not invoke a shell here. The binary path is user/package
        // controlled and `pkexec setcap ... <path>` preserves spaces and
        // punctuation without introducing shell-quoting hazards.
        let status = Command::new("pkexec")
            .arg("setcap")
            .arg("cap_net_admin,cap_net_bind_service+ep")
            .arg(bin_path)
            .status()?;

        if !status.success() {
            anyhow::bail!("Failed to grant capabilities via pkexec.");
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn uninstall_service_linux() -> Result<()> {
        let exe = std::env::current_exe().context("Failed to get current executable path")?;
        let status = Command::new("pkexec")
            .arg("setcap")
            .arg("-r")
            .arg(exe)
            .status()?;

        if !status.success() {
            anyhow::bail!("Failed to remove capabilities via pkexec.");
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn start_service_linux() -> Result<()> {
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn stop_service_linux() -> Result<()> {
        Ok(())
    }

    // =========================================================================
    // MACOS IMPLEMENTATION
    // =========================================================================

    #[cfg(target_os = "macos")]
    fn check_status_macos() -> ServiceModeStatus {
        let helper_tool = Path::new("/Library/PrivilegedHelperTools/com.musicfrog.infiltrator");
        let launch_daemon = Path::new("/Library/LaunchDaemons/com.musicfrog.infiltrator.plist");

        if helper_tool.exists() && launch_daemon.exists() {
            let output = Command::new("launchctl")
                .args(["list", "com.musicfrog.infiltrator"])
                .output();

            if let Ok(out) = output {
                if out.status.success() {
                    return ServiceModeStatus::InstalledAndRunning;
                }
            }
            ServiceModeStatus::InstalledStopped
        } else {
            ServiceModeStatus::NotInstalled
        }
    }

    #[cfg(target_os = "macos")]
    fn install_service_macos(_bin_path: &Path) -> Result<()> {
        anyhow::bail!("macOS service installation requires SMJobBless or manual plist deployment.");
    }

    #[cfg(target_os = "macos")]
    fn uninstall_service_macos() -> Result<()> {
        anyhow::bail!("macOS uninstall not fully implemented in CLI.");
    }

    #[cfg(target_os = "macos")]
    fn start_service_macos() -> Result<()> {
        let status = Command::new("sudo")
            .args(["launchctl", "start", "com.musicfrog.infiltrator"])
            .status()?;
        if !status.success() {
            anyhow::bail!("Failed to start macOS service.");
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn stop_service_macos() -> Result<()> {
        let status = Command::new("sudo")
            .args(["launchctl", "stop", "com.musicfrog.infiltrator"])
            .status()?;
        if !status.success() {
            anyhow::bail!("Failed to stop macOS service.");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_display() {
        assert_eq!(
            ServiceModeStatus::InstalledAndRunning.to_string(),
            "Installed and Running"
        );
        assert_eq!(
            ServiceModeStatus::InstalledStopped.to_string(),
            "Installed but Stopped"
        );
        assert_eq!(ServiceModeStatus::NotInstalled.to_string(), "Not Installed");
        assert_eq!(
            ServiceModeStatus::MissingPrivilege.to_string(),
            "Missing Privilege"
        );
        assert_eq!(ServiceModeStatus::Unsupported.to_string(), "Unsupported OS");
    }

    #[test]
    fn test_status_transitions() {
        let mut status = ServiceModeStatus::NotInstalled;
        assert_eq!(status, ServiceModeStatus::NotInstalled);
        status = ServiceModeStatus::InstalledAndRunning;
        assert_eq!(status, ServiceModeStatus::InstalledAndRunning);
    }

    #[test]
    fn test_service_manager_dummy_check() {
        let status = TunServiceManager::check_status();
        assert!(matches!(
            status,
            ServiceModeStatus::InstalledAndRunning
                | ServiceModeStatus::InstalledStopped
                | ServiceModeStatus::NotInstalled
                | ServiceModeStatus::MissingPrivilege
                | ServiceModeStatus::Unsupported
        ));
    }
}

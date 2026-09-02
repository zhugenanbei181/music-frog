//! TUN mode privilege / background-service management.
//!
//! Platform contracts (see `docs/PLATFORM_CONTRACTS.md` §2.5):
//!
//! - **Linux — capability model.** Install = `pkexec setcap
//!   cap_net_admin,cap_net_bind_service+ep <core-binary>`; uninstall = the exact
//!   inverse, `pkexec setcap -r <core-binary>`, targeting the *same* core binary
//!   that was passed to [`TunServiceManager::install_service`] (never the GUI
//!   executable). There is no daemon in this model, so start/stop are documented
//!   no-ops.
//! - **Windows — `sc.exe` service management** (unchanged; cannot be compiled or
//!   verified on the current Linux host).
//! - **macOS — explicitly unsupported in 0.20.** A real macOS TUN route needs a
//!   Network Extension (or a `nerds`-style privileged helper campaign); that work
//!   is out of scope for 0.20. The previous `sudo launchctl` calls were a fake
//!   path (no tty in a GUI session, guaranteed to fail, sometimes silently) and
//!   have been removed: every macOS verb returns [`UnsupportedPlatformError`]
//!   and status checks return [`ServiceModeStatus::Unsupported`].
//!
//! Each platform's implementation lives in its own submodule (`windows`,
//! `linux`, `macos`, plus an honest `other` fallback for unlisted targets);
//! this file carries only the platform-neutral types and the dispatching
//! facade.

use anyhow::Result;
use std::fmt;
use std::path::Path;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
mod other;
#[cfg(target_os = "windows")]
mod windows;

/// Typed error for platforms where a TUN privilege route is intentionally not
/// implemented (currently macOS). Callers can downcast to detect "honest
/// unsupported" instead of a generic failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnsupportedPlatformError {
    /// The service verb that was attempted, e.g. `"install_service"`.
    pub action: &'static str,
    /// The platform the verb is unsupported on, e.g. `"macOS"`.
    pub platform: &'static str,
}

impl fmt::Display for UnsupportedPlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TUN service verb `{}` is not supported on {} (requires Network Extension or a privileged helper; not implemented in 0.20)",
            self.action, self.platform
        )
    }
}

impl std::error::Error for UnsupportedPlatformError {}

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
            windows::check_status_for(binary_path)
        }
        #[cfg(target_os = "linux")]
        {
            linux::check_status_for(binary_path)
        }
        #[cfg(target_os = "macos")]
        {
            macos::check_status_for(binary_path)
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            other::check_status_for(binary_path)
        }
    }

    /// Installs the background service or grants necessary capabilities.
    ///
    /// On Linux this grants `cap_net_admin,cap_net_bind_service+ep` on
    /// `service_bin_path` (the mihomo core binary). [`Self::uninstall_service`]
    /// is the exact inverse of this call.
    pub fn install_service(service_bin_path: &Path) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            windows::install_service(service_bin_path)
        }
        #[cfg(target_os = "linux")]
        {
            linux::install_service(service_bin_path)
        }
        #[cfg(target_os = "macos")]
        {
            macos::install_service(service_bin_path)
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            other::install_service(service_bin_path)
        }
    }

    /// Uninstalls the service or removes capabilities.
    ///
    /// On Linux this is the *exact inverse* of [`Self::install_service`]: it
    /// removes the capabilities (`pkexec setcap -r`) from the very same core
    /// binary that install granted them to. It must never target the GUI
    /// executable.
    pub fn uninstall_service(service_bin_path: &Path) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            windows::uninstall_service(service_bin_path)
        }
        #[cfg(target_os = "linux")]
        {
            linux::uninstall_service(service_bin_path)
        }
        #[cfg(target_os = "macos")]
        {
            macos::uninstall_service(service_bin_path)
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            other::uninstall_service(service_bin_path)
        }
    }

    /// Starts the installed service.
    ///
    /// On Linux the capability model has no daemon, so this is a documented
    /// no-op: granting capabilities is the whole "installation".
    pub fn start_service() -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            windows::start_service()
        }
        #[cfg(target_os = "linux")]
        {
            linux::start_service()
        }
        #[cfg(target_os = "macos")]
        {
            macos::start_service()
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            other::start_service()
        }
    }

    /// Stops the currently running service.
    ///
    /// On Linux the capability model has no daemon, so this is a documented
    /// no-op.
    pub fn stop_service() -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            windows::stop_service()
        }
        #[cfg(target_os = "linux")]
        {
            linux::stop_service()
        }
        #[cfg(target_os = "macos")]
        {
            macos::stop_service()
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            other::stop_service()
        }
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

    #[test]
    fn test_unsupported_platform_error_display() {
        let err = UnsupportedPlatformError {
            action: "install_service",
            platform: "macOS",
        };
        assert!(err.to_string().contains("install_service"));
        assert!(err.to_string().contains("macOS"));
    }
}

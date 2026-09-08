//! Desktop adapter for the privileged TUN/service-mode seam.
//!
//! The platform modules remain the only place that knows `sc.exe`, `pkexec`,
//! or launchd details. This adapter converts their status into the shared
//! contract and never reports preparation as successful unless the post-check
//! reaches a ready state.

use infiltrator_contract::capability::Capability;
use infiltrator_contract::service_mode::{
    ServiceModePlatform, ServiceModeSnapshot, ServiceModeState,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::service_mode::ServiceModePort;
use std::path::{Path, PathBuf};

use crate::tun_service::{ServiceModeStatus, TunServiceManager};

pub struct DesktopServiceMode {
    binary_path: PathBuf,
}

impl DesktopServiceMode {
    pub fn new(binary_path: PathBuf) -> Self {
        Self { binary_path }
    }

    pub fn binary_path(&self) -> &Path {
        &self.binary_path
    }

    pub fn snapshot_now(&self) -> ServiceModeSnapshot {
        map_status(TunServiceManager::check_status_for(&self.binary_path))
    }
}

#[async_trait::async_trait]
impl ServiceModePort for DesktopServiceMode {
    async fn snapshot(&self) -> Result<ServiceModeSnapshot, PortError> {
        Ok(self.snapshot_now())
    }

    async fn prepare(&self) -> Result<ServiceModeSnapshot, PortError> {
        let current = self.snapshot_now();
        match current.state {
            ServiceModeState::Ready => return Ok(current),
            ServiceModeState::Unsupported => {
                return Err(PortError::unsupported(
                    Capability::Tun,
                    "this desktop has no service-mode adapter",
                ));
            }
            ServiceModeState::InstalledStopped => {
                TunServiceManager::start_service().map_err(permission_error)?;
            }
            ServiceModeState::NotInstalled | ServiceModeState::MissingPrivilege => {
                TunServiceManager::install_service(&self.binary_path).map_err(permission_error)?;
                TunServiceManager::start_service().map_err(permission_error)?;
            }
            ServiceModeState::Unavailable => {
                return Err(PortError::Failed(
                    "service-mode status is unavailable".to_owned(),
                ));
            }
        }

        let ready = self.snapshot_now();
        if ready.state != ServiceModeState::Ready {
            return Err(PortError::PermissionDenied(format!(
                "service-mode preparation did not reach ready state: {:?}",
                ready.state
            )));
        }
        Ok(ready)
    }
}

fn permission_error(error: anyhow::Error) -> PortError {
    PortError::PermissionDenied(error.to_string())
}

fn map_status(status: ServiceModeStatus) -> ServiceModeSnapshot {
    ServiceModeSnapshot {
        platform: current_platform(),
        state: match status {
            ServiceModeStatus::InstalledAndRunning => ServiceModeState::Ready,
            ServiceModeStatus::InstalledStopped => ServiceModeState::InstalledStopped,
            ServiceModeStatus::NotInstalled => ServiceModeState::NotInstalled,
            ServiceModeStatus::MissingPrivilege => ServiceModeState::MissingPrivilege,
            ServiceModeStatus::Unsupported => ServiceModeState::Unsupported,
        },
    }
}

const fn current_platform() -> ServiceModePlatform {
    #[cfg(target_os = "windows")]
    {
        return ServiceModePlatform::WindowsService;
    }
    #[cfg(target_os = "linux")]
    {
        return ServiceModePlatform::LinuxPolkit;
    }
    #[cfg(target_os = "macos")]
    {
        return ServiceModePlatform::MacosLaunchd;
    }
    #[allow(unreachable_code)]
    ServiceModePlatform::Unsupported
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_platform_is_explicit() {
        #[cfg(target_os = "linux")]
        assert_eq!(current_platform(), ServiceModePlatform::LinuxPolkit);
        #[cfg(target_os = "windows")]
        assert_eq!(current_platform(), ServiceModePlatform::WindowsService);
        #[cfg(target_os = "macos")]
        assert_eq!(current_platform(), ServiceModePlatform::MacosLaunchd);
    }

    #[test]
    fn status_mapping_never_confuses_stopped_with_ready() {
        let mapped = map_status(ServiceModeStatus::InstalledStopped);
        assert_eq!(mapped.state, ServiceModeState::InstalledStopped);
        assert_ne!(mapped.state, ServiceModeState::Ready);
    }
}

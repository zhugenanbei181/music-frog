//! Honest fallback for targets outside windows/linux/macos: status reports
//! [`ServiceModeStatus::Unsupported`] and every verb fails.

use super::ServiceModeStatus;
use anyhow::Result;
use std::path::Path;

pub(super) fn check_status_for(_binary_path: &Path) -> ServiceModeStatus {
    ServiceModeStatus::Unsupported
}

pub(super) fn install_service(_service_bin_path: &Path) -> Result<()> {
    anyhow::bail!("Unsupported OS for service installation");
}

pub(super) fn uninstall_service(_service_bin_path: &Path) -> Result<()> {
    anyhow::bail!("Unsupported OS for service uninstallation");
}

pub(super) fn start_service() -> Result<()> {
    anyhow::bail!("Unsupported OS for service start");
}

pub(super) fn stop_service() -> Result<()> {
    anyhow::bail!("Unsupported OS for service stop");
}

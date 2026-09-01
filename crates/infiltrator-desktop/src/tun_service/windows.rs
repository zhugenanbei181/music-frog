//! Windows implementation: `sc.exe` service management.
//!
//! NOTE: kept as-is; this host cannot compile or test the Windows arm.

use super::{ServiceModeStatus, TunServiceManager};
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub(super) fn check_status_for(_binary_path: &Path) -> ServiceModeStatus {
    let output = Command::new("sc.exe")
        .arg("query")
        .arg(TunServiceManager::SERVICE_NAME)
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

pub(super) fn install_service(bin_path: &Path) -> Result<()> {
    let path_str = bin_path.to_str().context("Invalid binary path")?;
    let bin_path_arg = format!("\"{}\"", path_str);
    let status = Command::new("sc.exe")
        .args([
            "create",
            TunServiceManager::SERVICE_NAME,
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

pub(super) fn uninstall_service(_service_bin_path: &Path) -> Result<()> {
    let _ = stop_service();
    let status = Command::new("sc.exe")
        .args(["delete", TunServiceManager::SERVICE_NAME])
        .status()?;

    if !status.success() {
        anyhow::bail!("Failed to delete Windows service. Requires Administrator privileges.");
    }
    Ok(())
}

pub(super) fn start_service() -> Result<()> {
    let status = Command::new("sc.exe")
        .args(["start", TunServiceManager::SERVICE_NAME])
        .status()?;

    if !status.success() {
        anyhow::bail!("Failed to start Windows service. Requires Administrator privileges.");
    }
    Ok(())
}

pub(super) fn stop_service() -> Result<()> {
    let status = Command::new("sc.exe")
        .args(["stop", TunServiceManager::SERVICE_NAME])
        .status()?;

    if !status.success() {
        anyhow::bail!("Failed to stop Windows service.");
    }
    Ok(())
}

//! Linux implementation: capability model (`pkexec setcap` on the core
//! binary). There is no daemon — the grant *is* the installation, so
//! start/stop are documented no-ops.
//!
//! NOTE: the fake-pkexec regression tests (argv contract, exact-inverse
//! uninstall, loud failures) live with the implementation but are currently
//! gated behind the Mimosa write gate; see the pending follow-up edit that
//! re-appends the `#[cfg(test)]` module.

use super::ServiceModeStatus;
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub(super) fn check_status_for(exe: &Path) -> ServiceModeStatus {
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

/// Builds the argv handed to `pkexec` for a setcap grant (`remove = false`)
/// or a setcap removal (`remove = true`). Pure helper, unit-tested so the
/// argv contract cannot drift between install and uninstall.
fn setcap_argv(bin_path: &Path, remove: bool) -> Vec<std::ffi::OsString> {
    let mut argv = vec![std::ffi::OsString::from("setcap")];
    if remove {
        argv.push(std::ffi::OsString::from("-r"));
    } else {
        argv.push(std::ffi::OsString::from("cap_net_admin,cap_net_bind_service+ep"));
    }
    argv.push(bin_path.as_os_str().to_os_string());
    argv
}

/// Runs `pkexec setcap [...] <bin_path>` and fails loudly on any non-zero
/// exit or spawn error.
///
/// argv-vector form only — no shell, no string interpolation into a command
/// line. Production always passes `Command::new("pkexec")`; tests inject a
/// fake recorder binary at this seam.
fn run_pkexec_setcap(command: &mut Command, bin_path: &Path, remove: bool) -> Result<()> {
    let argv = setcap_argv(bin_path, remove);
    command.args(&argv);
    let output = command.output().with_context(|| {
        format!(
            "failed to spawn pkexec for setcap on {}",
            bin_path.display()
        )
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "Failed to {} capabilities via pkexec on {} (exit {:?}): {}",
            if remove { "remove" } else { "grant" },
            bin_path.display(),
            output.status.code(),
            stderr.trim()
        );
    }
    Ok(())
}

pub(super) fn install_service(bin_path: &Path) -> Result<()> {
    // Fixed program, argv-vector args, no shell: the binary path is
    // user/package controlled and `pkexec setcap ... <path>` preserves
    // spaces and punctuation without shell-quoting hazards.
    run_pkexec_setcap(&mut Command::new("pkexec"), bin_path, false)
}

/// Exact inverse of [`install_service`]: removes the very capabilities that
/// install granted, from the same core binary.
pub(super) fn uninstall_service(bin_path: &Path) -> Result<()> {
    run_pkexec_setcap(&mut Command::new("pkexec"), bin_path, true)
}

// Capability model: there is no daemon to start or stop.
pub(super) fn start_service() -> Result<()> {
    Ok(())
}

pub(super) fn stop_service() -> Result<()> {
    Ok(())
}

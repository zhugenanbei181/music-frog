//! Windows autostart support backed by the per-user `Run` registry key.
//!
//! Registry entries are keyed by `name`, so each desktop client manages its
//! own entry and coexists with the other: the iced client registers as
//! `MusicFrogInfiltrator`, the legacy Tauri client as
//! `MihomoDespicableInfiltrator`. On non-Windows targets this is a no-op
//! (`is_autostart_enabled` returns `false`, `set_autostart_enabled` errors).

#[cfg(target_os = "windows")]
use std::process::Command;

use anyhow::anyhow;

#[cfg(target_os = "windows")]
const REG_RUN_KEY: &str = "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run";

#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

#[cfg(target_os = "windows")]
fn new_hidden_command(program: &str) -> Command {
    let mut command = Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// Returns whether an autostart entry registered under `name` exists.
pub fn is_autostart_enabled(name: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        let output = new_hidden_command("reg")
            .args(["query", REG_RUN_KEY, "/v", name])
            .output();
        output.map(|o| o.status.success()).unwrap_or(false)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = name;
        false
    }
}

/// Creates or removes the autostart entry registered under `name`, pointing
/// it at the current executable with `--autostart`.
pub fn set_autostart_enabled(name: &str, enabled: bool) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        if enabled {
            let exe = std::env::current_exe()?;
            let task_cmd = format!("\"{}\" --autostart", exe.to_string_lossy());
            let status = new_hidden_command("reg")
                .args([
                    "add",
                    REG_RUN_KEY,
                    "/v",
                    name,
                    "/t",
                    "REG_SZ",
                    "/d",
                    &task_cmd,
                    "/f",
                ])
                .status()?;
            if !status.success() {
                return Err(anyhow!("Failed to create registry autostart entry"));
            }
        } else if is_autostart_enabled(name) {
            let status = new_hidden_command("reg")
                .args(["delete", REG_RUN_KEY, "/v", name, "/f"])
                .status()?;
            if !status.success() {
                return Err(anyhow!("Failed to delete registry autostart entry"));
            }
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (name, enabled);
        Err(anyhow!("Autostart is only supported on Windows"))
    }
}

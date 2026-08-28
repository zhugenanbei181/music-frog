use anyhow::anyhow;
use std::process::Command;

const AUTOSTART_NAME: &str = "MusicFrogInfiltrator";
const REG_RUN_KEY: &str = "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run";

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(target_os = "windows")]
fn new_hidden_command(program: &str) -> Command {
    let mut command = Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

pub fn is_autostart_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        let output = new_hidden_command("reg")
            .args(["query", REG_RUN_KEY, "/v", AUTOSTART_NAME])
            .output();
        output.map(|o| o.status.success()).unwrap_or(false)
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

pub fn set_autostart_enabled(enabled: bool) -> anyhow::Result<()> {
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
                    AUTOSTART_NAME,
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
        } else if is_autostart_enabled() {
            let status = new_hidden_command("reg")
                .args(["delete", REG_RUN_KEY, "/v", AUTOSTART_NAME, "/f"])
                .status()?;
            if !status.success() {
                return Err(anyhow!("Failed to delete registry autostart entry"));
            }
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = enabled;
        Err(anyhow!("Autostart is only supported on Windows"))
    }
}

use std::process::Command;

use anyhow::anyhow;

const AUTOSTART_TASK_NAME: &str = "MihomoDespicableInfiltrator";

pub(crate) fn is_autostart_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("schtasks")
            .args(["/Query", "/TN", AUTOSTART_TASK_NAME])
            .output();
        return output.map(|o| o.status.success()).unwrap_or(false);
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

pub(crate) fn set_autostart_enabled(enabled: bool) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        if enabled {
            let exe = std::env::current_exe()?;
            let task_cmd = format!("\"{}\"", exe.to_string_lossy());
            let status = Command::new("schtasks")
                .args([
                    "/Create",
                    "/F",
                    "/SC",
                    "ONLOGON",
                    "/RL",
                    "HIGHEST",
                    "/TN",
                    AUTOSTART_TASK_NAME,
                    "/TR",
                    &task_cmd,
                ])
                .status()?;
            if !status.success() {
                return Err(anyhow!("创建计划任务失败"));
            }
        } else if is_autostart_enabled() {
            let status = Command::new("schtasks")
                .args(["/Delete", "/TN", AUTOSTART_TASK_NAME, "/F"])
                .status()?;
            if !status.success() {
                return Err(anyhow!("删除计划任务失败"));
            }
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = enabled;
        Err(anyhow!("开机自启仅支持 Windows"))
    }
}

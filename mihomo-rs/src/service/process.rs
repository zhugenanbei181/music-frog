use crate::core::{get_home_dir, MihomoError, Result};
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
use sysinfo::{Pid, System};
use tokio::fs;

pub async fn spawn_daemon(binary: &Path, config: &Path) -> Result<u32> {
    if !binary.exists() {
        return Err(MihomoError::NotFound(format!(
            "Binary not found: {}",
            binary.display()
        )));
    }

    if !config.exists() {
        return Err(MihomoError::NotFound(format!(
            "Config not found: {}",
            config.display()
        )));
    }

    let log_path = prepare_log_file().await?;
    // Open two separate file handles for stdout and stderr
    let stdout = open_log_file(&log_path)?;
    let stderr = open_log_file(&log_path)?;
    log::info!("mihomo log file: {}", log_path.display());

    let config_dir = config
        .parent()
        .ok_or_else(|| MihomoError::Config("Config file has no parent directory".to_string()))?;

    let mut command = Command::new(binary);
    command
        .arg("-d")
        .arg(config_dir)
        .arg("-f")
        .arg(config)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));

    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = command
        .spawn()
        .map_err(|e| MihomoError::Service(format!("Failed to spawn process: {}", e)))?;

    // Give it a brief moment to see if it exits immediately (e.g. config error)
    // We can't await it because it's a daemon, but we can check status
    if let Ok(Some(status)) = child.try_wait() {
        return Err(MihomoError::Service(format!(
            "Process exited immediately with status: {}",
            status
        )));
    }

    let pid = child.id();
    Ok(pid)
}

async fn prepare_log_file() -> Result<PathBuf> {
    let home = get_home_dir()?;
    let log_dir = home.join("logs");
    fs::create_dir_all(&log_dir).await?;
    Ok(log_dir.join("mihomo.log"))
}

fn open_log_file(path: &Path) -> Result<std::fs::File> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| {
            MihomoError::Service(format!("Failed to open log file {}: {}", path.display(), e))
        })
}

pub fn kill_process(pid: u32) -> Result<()> {
    let mut system = System::new();
    system.refresh_processes();

    let pid = Pid::from_u32(pid);
    if let Some(process) = system.process(pid) {
        if !process.kill() {
            return Err(MihomoError::Service(format!(
                "Failed to kill process {}",
                pid
            )));
        }
    }

    Ok(())
}

pub fn is_process_alive(pid: u32) -> bool {
    let mut system = System::new();
    system.refresh_processes();
    system.process(Pid::from_u32(pid)).is_some()
}

pub async fn read_pid_file(path: &Path) -> Result<u32> {
    if !path.exists() {
        return Err(MihomoError::NotFound("PID file not found".to_string()));
    }

    let content = fs::read_to_string(path).await?;
    let pid = content
        .trim()
        .parse::<u32>()
        .map_err(|e| MihomoError::Service(format!("Invalid PID in file: {}", e)))?;

    Ok(pid)
}

pub async fn write_pid_file(path: &Path, pid: u32) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::write(path, pid.to_string()).await?;
    Ok(())
}

pub async fn remove_pid_file(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path).await?;
    }
    Ok(())
}

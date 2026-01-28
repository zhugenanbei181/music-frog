use async_trait::async_trait;
use std::path::PathBuf;

use mihomo_api::{MihomoError, Result};
use tokio::time::Duration;

use crate::paths::get_home_dir;
use crate::traits::{CoreController, CredentialStore, DataDirProvider};

pub struct ProcessCoreController {
    binary_path: PathBuf,
    config_path: PathBuf,
    pid_file: PathBuf,
}

impl ProcessCoreController {
    pub fn new(binary_path: PathBuf, config_path: PathBuf) -> Self {
        let home = get_home_dir().unwrap_or_else(|_| PathBuf::from("."));
        let pid_file = home.join("mihomo.pid");

        Self {
            binary_path,
            config_path,
            pid_file,
        }
    }

    pub fn with_home(binary_path: PathBuf, config_path: PathBuf, home: PathBuf) -> Self {
        let pid_file = home.join("mihomo.pid");

        Self {
            binary_path,
            config_path,
            pid_file,
        }
    }

    pub fn with_pid_file(binary_path: PathBuf, config_path: PathBuf, pid_file: PathBuf) -> Self {
        Self {
            binary_path,
            config_path,
            pid_file,
        }
    }

    async fn read_running_pid(&self) -> Result<Option<u32>> {
        match process::read_pid_file(&self.pid_file).await {
            Ok(pid) => {
                if process::is_process_alive(pid) {
                    Ok(Some(pid))
                } else {
                    process::remove_pid_file(&self.pid_file).await?;
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
    }
}

#[async_trait]
impl CoreController for ProcessCoreController {
    async fn start(&self) -> Result<()> {
        if self.is_running().await {
            return Err(MihomoError::Service(
                "Service is already running".to_string(),
            ));
        }

        let pid = process::spawn_daemon(&self.binary_path, &self.config_path).await?;
        process::write_pid_file(&self.pid_file, pid).await?;

        tokio::time::sleep(Duration::from_millis(500)).await;

        if !process::is_process_alive(pid) {
            process::remove_pid_file(&self.pid_file).await?;
            return Err(MihomoError::Service("Service failed to start".to_string()));
        }

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        let pid = process::read_pid_file(&self.pid_file).await?;

        if !process::is_process_alive(pid) {
            process::remove_pid_file(&self.pid_file).await?;
            return Err(MihomoError::Service("Service is not running".to_string()));
        }

        process::kill_process(pid)?;
        process::remove_pid_file(&self.pid_file).await?;

        Ok(())
    }

    async fn is_running(&self) -> bool {
        self.read_running_pid().await.ok().flatten().is_some()
    }

    fn controller_url(&self) -> Option<String> {
        None
    }

    async fn pid(&self) -> Option<u32> {
        match process::read_pid_file(&self.pid_file).await {
            Ok(pid) => Some(pid),
            Err(err) => {
                log::warn!("failed to read pid file: {err}");
                None
            }
        }
    }
}

pub struct KeyringCredentialStore;

impl Default for KeyringCredentialStore {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl CredentialStore for KeyringCredentialStore {
    async fn get(&self, service: &str, key: &str) -> Result<Option<String>> {
        let entry = match keyring::Entry::new(service, key) {
            Ok(entry) => entry,
            Err(err) => {
                log::warn!("keyring init failed: {err}");
                return Ok(None);
            }
        };
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(err) => {
                log::warn!("keyring get failed: {err}");
                Ok(None)
            }
        }
    }

    async fn set(&self, service: &str, key: &str, value: &str) -> Result<()> {
        let entry = keyring::Entry::new(service, key)
            .map_err(|err| MihomoError::Config(format!("Keyring init failed: {err}")))?;
        entry
            .set_password(value)
            .map_err(|err| MihomoError::Config(format!("Keyring set failed: {err}")))?;
        Ok(())
    }

    async fn delete(&self, service: &str, key: &str) -> Result<()> {
        let entry = keyring::Entry::new(service, key)
            .map_err(|err| MihomoError::Config(format!("Keyring init failed: {err}")))?;
        entry
            .delete_credential()
            .map_err(|err| MihomoError::Config(format!("Keyring delete failed: {err}")))?;
        Ok(())
    }
}

pub struct DesktopDataDirProvider;

impl Default for DesktopDataDirProvider {
    fn default() -> Self {
        Self
    }
}

impl DataDirProvider for DesktopDataDirProvider {
    fn data_dir(&self) -> Option<PathBuf> {
        get_home_dir().ok()
    }
}

mod process {
    use mihomo_api::{MihomoError, Result};
    use std::fs::OpenOptions;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use sysinfo::{Pid, ProcessesToUpdate, System};
    use tokio::fs;

    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    #[cfg(windows)]
    use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

    use crate::paths::get_home_dir;

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

        if let Ok(Some(status)) = child.try_wait() {
            return Err(MihomoError::Service(format!(
                "Process exited immediately with status: {}",
                status
            )));
        }

        Ok(child.id())
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
                MihomoError::Service(format!(
                    "Failed to open log file {}: {}",
                    path.display(),
                    e
                ))
            })
    }

    pub fn kill_process(pid: u32) -> Result<()> {
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::All, true);

        let pid = Pid::from_u32(pid);
        if let Some(process) = system.process(pid)
            && !process.kill() {
                return Err(MihomoError::Service(format!(
                    "Failed to kill process {}",
                    pid
                )));
            }

        Ok(())
    }

    pub fn is_process_alive(pid: u32) -> bool {
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::All, true);
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
}

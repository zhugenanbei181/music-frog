use std::path::PathBuf;

use mihomo_api::error::Result;
use mihomo_platform::desktop::ProcessCoreController;
use mihomo_platform::traits::CoreController;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceStatus {
    Running(u32),
    Stopped,
}

pub struct ServiceManager {
    controller: std::sync::Arc<ProcessCoreController>,
    binary_path: PathBuf,
}

impl ServiceManager {
    pub fn new(binary_path: PathBuf, config_path: PathBuf) -> Self {
        Self {
            controller: std::sync::Arc::new(ProcessCoreController::new(
                binary_path.clone(),
                config_path,
            )),
            binary_path,
        }
    }

    pub fn with_home(binary_path: PathBuf, config_path: PathBuf, home: PathBuf) -> Self {
        Self {
            controller: std::sync::Arc::new(ProcessCoreController::with_home(
                binary_path.clone(),
                config_path,
                home,
            )),
            binary_path,
        }
    }

    pub fn with_pid_file(binary_path: PathBuf, config_path: PathBuf, pid_file: PathBuf) -> Self {
        Self {
            controller: std::sync::Arc::new(ProcessCoreController::with_pid_file(
                binary_path.clone(),
                config_path,
                pid_file,
            )),
            binary_path,
        }
    }

    /// The platform controller as a trait object, for hosting inside a
    /// shared [`infiltrator_core::session::CoreSession`].
    pub fn controller(&self) -> std::sync::Arc<dyn CoreController> {
        self.controller.clone()
    }

    pub fn binary_path(&self) -> &std::path::Path {
        &self.binary_path
    }

    pub async fn start(&self) -> Result<()> {
        self.controller.start().await
    }

    pub async fn stop(&self) -> Result<()> {
        self.controller.stop().await
    }

    pub async fn restart(&self) -> Result<()> {
        if self.is_running().await {
            self.stop().await?;
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
        self.start().await
    }

    pub async fn status(&self) -> Result<ServiceStatus> {
        if self.is_running().await {
            let pid = self.controller.pid().await.unwrap_or(0);
            Ok(ServiceStatus::Running(pid))
        } else {
            Ok(ServiceStatus::Stopped)
        }
    }

    pub async fn is_running(&self) -> bool {
        self.controller.is_running().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_service_status_equality() {
        let status1 = ServiceStatus::Running(1234);
        let status2 = ServiceStatus::Running(1234);
        let status3 = ServiceStatus::Running(5678);
        let status4 = ServiceStatus::Stopped;

        assert_eq!(status1, status2);
        assert_ne!(status1, status3);
        assert_ne!(status1, status4);
    }

    #[test]
    fn test_service_manager_new() {
        let temp_dir = TempDir::new().unwrap();
        let binary_path = temp_dir.path().join("mihomo.exe");
        let config_path = temp_dir.path().join("config.yaml");

        let _manager = ServiceManager::new(binary_path.clone(), config_path.clone());
    }

    #[test]
    fn test_service_manager_with_home() {
        let temp_dir = TempDir::new().unwrap();
        let binary_path = temp_dir.path().join("mihomo.exe");
        let config_path = temp_dir.path().join("config.yaml");
        let home = temp_dir.path().join("home");

        let _manager =
            ServiceManager::with_home(binary_path.clone(), config_path.clone(), home.clone());
    }

    #[test]
    fn test_service_manager_with_pid_file() {
        let temp_dir = TempDir::new().unwrap();
        let binary_path = temp_dir.path().join("mihomo.exe");
        let config_path = temp_dir.path().join("config.yaml");
        let pid_file = temp_dir.path().join("pidfile");

        let _manager = ServiceManager::with_pid_file(
            binary_path.clone(),
            config_path.clone(),
            pid_file.clone(),
        );
    }

    #[test]
    fn test_service_status_running_debug() {
        let status = ServiceStatus::Running(1234);
        let debug_str = format!("{:?}", status);
        assert!(debug_str.contains("Running"));
        assert!(debug_str.contains("1234"));
    }

    #[test]
    fn test_service_status_stopped_debug() {
        let status = ServiceStatus::Stopped;
        let debug_str = format!("{:?}", status);
        assert!(debug_str.contains("Stopped"));
    }
}

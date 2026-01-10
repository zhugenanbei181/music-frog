use std::path::PathBuf;

use mihomo_api::Result;
use mihomo_platform::{CoreController, ProcessCoreController};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceStatus {
    Running(u32),
    Stopped,
}

pub struct ServiceManager {
    controller: ProcessCoreController,
}

impl ServiceManager {
    pub fn new(binary_path: PathBuf, config_path: PathBuf) -> Self {
        Self {
            controller: ProcessCoreController::new(binary_path, config_path),
        }
    }

    pub fn with_home(binary_path: PathBuf, config_path: PathBuf, home: PathBuf) -> Self {
        Self {
            controller: ProcessCoreController::with_home(binary_path, config_path, home),
        }
    }

    pub fn with_pid_file(binary_path: PathBuf, config_path: PathBuf, pid_file: PathBuf) -> Self {
        Self {
            controller: ProcessCoreController::with_pid_file(binary_path, config_path, pid_file),
        }
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

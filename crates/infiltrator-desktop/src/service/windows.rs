//! Windows Service installation, SCM controller, and Named Pipe communication.
//!
//! Provides management for standalone Windows background services (`sc.exe` control,
//! Service Control Manager (SCM) operations, and secure Named Pipe IPC configurations).

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::process::Command;

pub const DEFAULT_WINDOWS_SERVICE_NAME: &str = "MusicFrogInfiltratorService";
pub const DEFAULT_WINDOWS_DISPLAY_NAME: &str = "MusicFrog Infiltrator Privileged Service";
pub const DEFAULT_WINDOWS_DESCRIPTION: &str =
    "Provides privileged TUN interface routing and network proxy control for MusicFrog Infiltrator";
pub const DEFAULT_WINDOWS_PIPE_NAME: &str = r"\\.\pipe\musicfrog-infiltrator-service";

/// Default SDDL granting LocalSystem (SY) & Built-in Admins (BA) Generic All (GA),
/// and Authenticated Users (AU) Generic Read + Generic Write (GRGW).
pub const DEFAULT_NAMED_PIPE_SDDL: &str = "D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;AU)";

/// Windows Service startup type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowsServiceStartType {
    Auto,
    Demand,
    Disabled,
}

impl WindowsServiceStartType {
    pub fn as_sc_arg(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Demand => "demand",
            Self::Disabled => "disabled",
        }
    }
}

/// Windows Service execution status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowsServiceStatus {
    NotInstalled,
    Stopped,
    StartPending,
    StopPending,
    Running,
    ContinuePending,
    PausePending,
    Paused,
    Unknown(String),
}

impl WindowsServiceStatus {
    pub fn is_running(&self) -> bool {
        matches!(self, WindowsServiceStatus::Running)
    }

    pub fn is_installed(&self) -> bool {
        !matches!(self, WindowsServiceStatus::NotInstalled)
    }
}

impl fmt::Display for WindowsServiceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInstalled => write!(f, "Not Installed"),
            Self::Stopped => write!(f, "Stopped"),
            Self::StartPending => write!(f, "Start Pending"),
            Self::StopPending => write!(f, "Stop Pending"),
            Self::Running => write!(f, "Running"),
            Self::ContinuePending => write!(f, "Continue Pending"),
            Self::PausePending => write!(f, "Pause Pending"),
            Self::Paused => write!(f, "Paused"),
            Self::Unknown(s) => write!(f, "Unknown ({s})"),
        }
    }
}

/// Configuration descriptor for a Windows background service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowsServiceConfig {
    pub service_name: String,
    pub display_name: String,
    pub description: String,
    pub binary_path: PathBuf,
    pub start_type: WindowsServiceStartType,
    pub account: String,
    pub pipe_name: String,
    pub sddl: String,
}

impl Default for WindowsServiceConfig {
    fn default() -> Self {
        Self {
            service_name: DEFAULT_WINDOWS_SERVICE_NAME.to_string(),
            display_name: DEFAULT_WINDOWS_DISPLAY_NAME.to_string(),
            description: DEFAULT_WINDOWS_DESCRIPTION.to_string(),
            binary_path: PathBuf::from(r"C:\Program Files\MusicFrog\infiltrator-service.exe"),
            start_type: WindowsServiceStartType::Auto,
            account: "LocalSystem".to_string(),
            pipe_name: DEFAULT_WINDOWS_PIPE_NAME.to_string(),
            sddl: DEFAULT_NAMED_PIPE_SDDL.to_string(),
        }
    }
}

impl WindowsServiceConfig {
    pub fn new(binary_path: impl Into<PathBuf>) -> Self {
        Self {
            binary_path: binary_path.into(),
            ..Default::default()
        }
    }

    pub fn with_service_name(mut self, name: impl Into<String>) -> Self {
        self.service_name = name.into();
        self
    }

    pub fn with_pipe_name(mut self, pipe: impl Into<String>) -> Self {
        self.pipe_name = pipe.into();
        self
    }
}

/// Security & DACL utilities for Windows Named Pipes.
pub struct NamedPipeSecurity;

impl NamedPipeSecurity {
    /// Validates that a string is a well-formed Windows named pipe path (`\\.\pipe\...`).
    pub fn is_valid_pipe_name(pipe_name: &str) -> bool {
        pipe_name.starts_with(r"\\.\pipe\") && pipe_name.len() > 9 && !pipe_name[9..].contains('\\')
    }

    /// Generates a Discretionary Access Control List (DACL) in SDDL format.
    pub fn generate_sddl(allow_authenticated_users: bool) -> String {
        if allow_authenticated_users {
            DEFAULT_NAMED_PIPE_SDDL.to_string()
        } else {
            // LocalSystem and Built-in Admins only
            "D:(A;;GA;;;SY)(A;;GA;;;BA)".to_string()
        }
    }
}

/// Windows Service Manager providing service registration, teardown, and status query.
#[derive(Debug, Clone)]
pub struct WindowsServiceManager {
    config: WindowsServiceConfig,
}

impl WindowsServiceManager {
    pub fn new(config: WindowsServiceConfig) -> Self {
        Self { config }
    }

    pub fn for_binary(binary_path: impl Into<PathBuf>) -> Self {
        Self::new(WindowsServiceConfig::new(binary_path))
    }

    pub fn config(&self) -> &WindowsServiceConfig {
        &self.config
    }

    /// Builds command arguments for `sc.exe create`.
    pub fn build_create_args(&self) -> Vec<String> {
        let bin_path_arg = format!("\"{}\"", self.config.binary_path.to_string_lossy());
        vec![
            "create".to_string(),
            self.config.service_name.clone(),
            format!("binPath={}", bin_path_arg),
            format!("DisplayName={}", self.config.display_name),
            format!("start={}", self.config.start_type.as_sc_arg()),
        ]
    }

    /// Builds command arguments for `sc.exe description`.
    pub fn build_description_args(&self) -> Vec<String> {
        vec![
            "description".to_string(),
            self.config.service_name.clone(),
            self.config.description.clone(),
        ]
    }

    /// Builds command arguments for `sc.exe delete`.
    pub fn build_delete_args(&self) -> Vec<String> {
        vec!["delete".to_string(), self.config.service_name.clone()]
    }

    /// Builds command arguments for `sc.exe start`.
    pub fn build_start_args(&self) -> Vec<String> {
        vec!["start".to_string(), self.config.service_name.clone()]
    }

    /// Builds command arguments for `sc.exe stop`.
    pub fn build_stop_args(&self) -> Vec<String> {
        vec!["stop".to_string(), self.config.service_name.clone()]
    }

    /// Builds command arguments for `sc.exe query`.
    pub fn build_query_args(&self) -> Vec<String> {
        vec!["query".to_string(), self.config.service_name.clone()]
    }

    /// Parses the raw stdout of `sc.exe query <service>`.
    pub fn parse_sc_query_output(output: &str) -> WindowsServiceStatus {
        let upper = output.to_uppercase();
        if upper.contains("1060")
            || upper.contains("DOES NOT EXIST")
            || upper.contains("FAILED 1060")
        {
            WindowsServiceStatus::NotInstalled
        } else if upper.contains("STATE") && upper.contains("RUNNING") {
            WindowsServiceStatus::Running
        } else if upper.contains("STATE") && upper.contains("STOPPED") {
            WindowsServiceStatus::Stopped
        } else if upper.contains("START_PENDING") {
            WindowsServiceStatus::StartPending
        } else if upper.contains("STOP_PENDING") {
            WindowsServiceStatus::StopPending
        } else if upper.contains("PAUSE_PENDING") {
            WindowsServiceStatus::PausePending
        } else if upper.contains("CONTINUE_PENDING") {
            WindowsServiceStatus::ContinuePending
        } else if upper.contains("PAUSED") {
            WindowsServiceStatus::Paused
        } else {
            WindowsServiceStatus::Unknown(output.trim().to_string())
        }
    }

    /// Installs the Windows service using `sc.exe`.
    pub fn install_service(&self) -> Result<()> {
        let args = self.build_create_args();
        let status = Command::new("sc.exe")
            .args(&args)
            .status()
            .with_context(|| "Failed to invoke sc.exe create")?;

        if !status.success() {
            return Err(anyhow!(
                "Failed to create Windows service '{}'. Requires Administrator privileges.",
                self.config.service_name
            ));
        }

        // Set description (best-effort)
        let desc_args = self.build_description_args();
        let _ = Command::new("sc.exe").args(&desc_args).status();

        Ok(())
    }

    /// Uninstalls the Windows service using `sc.exe`.
    pub fn uninstall_service(&self) -> Result<()> {
        let _ = self.stop_service();
        let args = self.build_delete_args();
        let status = Command::new("sc.exe")
            .args(&args)
            .status()
            .with_context(|| "Failed to invoke sc.exe delete")?;

        if !status.success() {
            return Err(anyhow!(
                "Failed to delete Windows service '{}'. Requires Administrator privileges.",
                self.config.service_name
            ));
        }
        Ok(())
    }

    /// Starts the Windows service.
    pub fn start_service(&self) -> Result<()> {
        let args = self.build_start_args();
        let status = Command::new("sc.exe")
            .args(&args)
            .status()
            .with_context(|| "Failed to invoke sc.exe start")?;

        if !status.success() {
            return Err(anyhow!(
                "Failed to start Windows service '{}'. Requires Administrator privileges.",
                self.config.service_name
            ));
        }
        Ok(())
    }

    /// Stops the Windows service.
    pub fn stop_service(&self) -> Result<()> {
        let args = self.build_stop_args();
        let status = Command::new("sc.exe")
            .args(&args)
            .status()
            .with_context(|| "Failed to invoke sc.exe stop")?;

        if !status.success() {
            return Err(anyhow!(
                "Failed to stop Windows service '{}'.",
                self.config.service_name
            ));
        }
        Ok(())
    }

    /// Queries the status of the Windows service.
    pub fn query_status(&self) -> Result<WindowsServiceStatus> {
        let args = self.build_query_args();
        let output = Command::new("sc.exe")
            .args(&args)
            .output()
            .with_context(|| "Failed to invoke sc.exe query")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");

        Ok(Self::parse_sc_query_output(&combined))
    }
}

//! Privileged Host Service and Daemon IPC architecture for Infiltrator Desktop.
//!
//! Provides inter-process communication (Named Pipes on Windows, Unix Domain Socket
//! on Linux/macOS) between the unprivileged GUI client and the background privileged
//! service helper (TUN route injection, system proxy override, and power/status telemetry).

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use infiltrator_ports::core_process::CoreProcess;
use mihomo_platform::desktop::ProcessCoreController;
use mihomo_platform::traits::CoreController;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

pub mod client;
pub mod linux;
pub mod macos;
pub mod server;
pub mod state_machine;
pub mod windows;

#[cfg(test)]
#[path = "service_test.rs"]
mod tests;

pub const WINDOWS_PIPE_NAME: &str = r"\\.\pipe\musicfrog-infiltrator-service";
pub const DEFAULT_UNIX_SOCKET_PATH: &str = "/var/run/musicfrog-infiltrator.sock";
pub const FALLBACK_UNIX_SOCKET_NAME: &str = "musicfrog-infiltrator.sock";
pub const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceStatus {
    Running(u32),
    Stopped,
}

pub struct ServiceManager {
    controller: Arc<ProcessCoreController>,
    binary_path: PathBuf,
}

impl ServiceManager {
    pub fn new(binary_path: PathBuf, config_path: PathBuf) -> Self {
        Self {
            controller: Arc::new(ProcessCoreController::new(binary_path.clone(), config_path)),
            binary_path,
        }
    }

    pub fn with_home(binary_path: PathBuf, config_path: PathBuf, home: PathBuf) -> Self {
        Self {
            controller: Arc::new(ProcessCoreController::with_home(
                binary_path.clone(),
                config_path,
                home,
            )),
            binary_path,
        }
    }

    pub fn with_pid_file(binary_path: PathBuf, config_path: PathBuf, pid_file: PathBuf) -> Self {
        Self {
            controller: Arc::new(ProcessCoreController::with_pid_file(
                binary_path.clone(),
                config_path,
                pid_file,
            )),
            binary_path,
        }
    }

    pub fn controller(&self) -> Arc<dyn CoreController> {
        self.controller.clone()
    }

    /// 0.30 process port. New application code should consume this trait
    /// object instead of the legacy `mihomo-platform` controller trait.
    pub fn core_process(&self) -> Arc<dyn CoreProcess> {
        self.controller.clone()
    }
    pub fn binary_path(&self) -> &Path {
        &self.binary_path
    }

    pub async fn is_running(&self) -> bool {
        self.controller.is_running().await
    }

    pub async fn start(&self) -> mihomo_api::error::Result<()> {
        CoreController::start(self.controller.as_ref()).await
    }

    pub async fn stop(&self) -> mihomo_api::error::Result<()> {
        CoreController::stop(self.controller.as_ref()).await
    }

    pub async fn restart(&self) -> mihomo_api::error::Result<()> {
        CoreController::start(self.controller.as_ref()).await
    }

    pub async fn status(&self) -> mihomo_api::error::Result<ServiceStatus> {
        if self.controller.is_running().await {
            Ok(ServiceStatus::Running(std::process::id()))
        } else {
            Ok(ServiceStatus::Stopped)
        }
    }
}

pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthToken(String);

impl AuthToken {
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    pub fn generate() -> Self {
        use std::fmt::Write;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(42);
        let pid = std::process::id();
        let mut s = String::with_capacity(64);
        for i in 0..32 {
            let val = ((now.wrapping_mul(i as u128 + 1) ^ (pid as u128).wrapping_mul(31))
                >> (i % 8)) as u8;
            let _ = write!(s, "{:02x}", val);
        }
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn secret(&self) -> &str {
        &self.0
    }

    pub fn verify(&self, candidate: &str) -> bool {
        constant_time_eq(self.0.as_bytes(), candidate.as_bytes())
    }

    pub async fn save_to_file(&self, path: &Path) -> Result<(), ServiceError> {
        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        tokio::fs::write(path, &self.0)
            .await
            .map_err(|e| ServiceError::Io(e.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    pub async fn load_from_file(path: &Path) -> Result<Self, ServiceError> {
        Self::load_or_create(path).await
    }

    pub async fn load_or_create(path: &Path) -> Result<Self, ServiceError> {
        if path.exists() {
            let content = tokio::fs::read_to_string(path)
                .await
                .map_err(|e| ServiceError::Io(e.to_string()))?;
            let trimmed = content.trim().to_string();
            if !trimmed.is_empty() {
                return Ok(Self(trimmed));
            }
        }
        let token = Self::generate();
        token.save_to_file(path).await?;
        Ok(token)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivilegeLevel {
    Admin,
    Root,
    CapNetAdmin,
    Unprivileged,
}

impl PrivilegeLevel {
    pub fn is_elevated(&self) -> bool {
        matches!(
            self,
            PrivilegeLevel::Admin | PrivilegeLevel::Root | PrivilegeLevel::CapNetAdmin
        )
    }

    pub fn detect() -> Self {
        #[cfg(windows)]
        {
            PrivilegeLevel::Unprivileged
        }
        #[cfg(unix)]
        {
            PrivilegeLevel::Unprivileged
        }
        #[cfg(not(any(windows, unix)))]
        {
            PrivilegeLevel::Unprivileged
        }
    }
}

impl fmt::Display for PrivilegeLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrivilegeLevel::Admin => write!(f, "Administrator"),
            PrivilegeLevel::Root => write!(f, "Root"),
            PrivilegeLevel::CapNetAdmin => write!(f, "cap_net_admin"),
            PrivilegeLevel::Unprivileged => write!(f, "Unprivileged"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceState {
    NotInstalled,
    Stopped,
    Running,
    Error(String),
}

impl ServiceState {
    pub fn is_running(&self) -> bool {
        matches!(self, ServiceState::Running)
    }
}

impl fmt::Display for ServiceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServiceState::NotInstalled => write!(f, "Not Installed"),
            ServiceState::Stopped => write!(f, "Stopped"),
            ServiceState::Running => write!(f, "Running"),
            ServiceState::Error(err) => write!(f, "Error: {err}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceStatusInfo {
    pub state: ServiceState,
    pub version: String,
    pub privilege_level: PrivilegeLevel,
    pub pid: Option<u32>,
    pub tun_active: bool,
    pub system_proxy_active: bool,
}

impl ServiceStatusInfo {
    pub fn fallback_uninstalled() -> Self {
        Self {
            state: ServiceState::NotInstalled,
            version: SERVICE_VERSION.to_string(),
            privilege_level: PrivilegeLevel::Unprivileged,
            pid: None,
            tun_active: false,
            system_proxy_active: false,
        }
    }

    pub fn fallback_stopped() -> Self {
        Self {
            state: ServiceState::Stopped,
            version: SERVICE_VERSION.to_string(),
            privilege_level: PrivilegeLevel::detect(),
            pid: None,
            tun_active: false,
            system_proxy_active: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", content = "params", rename_all = "snake_case")]
pub enum ServiceCommand {
    Ping {
        nonce: u64,
    },
    QueryStatus,
    StartTun {
        tun_interface: Option<String>,
        config_path: Option<String>,
    },
    StopTun,
    SetSystemProxy {
        endpoint: String,
        bypass: Option<String>,
    },
    ClearSystemProxy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceRequest {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub auth_token: String,
    pub token: String,
    #[serde(flatten)]
    pub command: ServiceCommand,
}

impl ServiceRequest {
    pub fn new(
        id: impl Into<String>,
        auth_token: impl Into<String>,
        command: ServiceCommand,
    ) -> Self {
        let token_str = auth_token.into();
        Self {
            id: id.into(),
            auth_token: token_str.clone(),
            token: token_str,
            command,
        }
    }

    pub fn authed(token: &AuthToken, command: ServiceCommand) -> Self {
        Self {
            id: String::new(),
            auth_token: token.as_str().to_string(),
            token: token.as_str().to_string(),
            command,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ServiceResponsePayload {
    Pong { nonce: u64 },
    Status(ServiceStatusInfo),
    TunStarted { interface_name: Option<String> },
    TunStopped,
    SystemProxyApplied,
    SystemProxyCleared,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<ServiceResponsePayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ServiceResponse {
    pub fn ok(payload: ServiceResponsePayload) -> Self {
        Self {
            success: true,
            payload: Some(payload),
            error: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            payload: None,
            error: Some(msg.into()),
        }
    }

    pub fn pong(_id: &str, nonce: u64) -> Self {
        Self::ok(ServiceResponsePayload::Pong { nonce })
    }

    pub fn err(_id: &str, msg: impl Into<String>) -> Self {
        Self::error(msg)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcEndpoint {
    #[cfg(unix)]
    UnixSocket(PathBuf),
    NamedPipe(String),
    Mock,
}

impl IpcEndpoint {
    #[cfg(unix)]
    pub fn from_unix_path(path: impl Into<PathBuf>) -> Self {
        IpcEndpoint::UnixSocket(path.into())
    }

    #[cfg(not(unix))]
    pub fn from_unix_path(_path: impl Into<PathBuf>) -> Self {
        IpcEndpoint::Mock
    }

    pub fn from_named_pipe(name: impl Into<String>) -> Self {
        IpcEndpoint::NamedPipe(name.into())
    }

    pub fn display_target(&self) -> String {
        match self {
            #[cfg(unix)]
            IpcEndpoint::UnixSocket(p) => p.display().to_string(),
            IpcEndpoint::NamedPipe(n) => n.clone(),
            IpcEndpoint::Mock => "mock://in-memory".to_string(),
        }
    }

    pub fn is_available(&self) -> bool {
        match self {
            #[cfg(unix)]
            IpcEndpoint::UnixSocket(p) => p.exists(),
            IpcEndpoint::NamedPipe(_) => true,
            IpcEndpoint::Mock => true,
        }
    }

    pub fn default_for_platform() -> Self {
        Self::default_system()
    }

    pub fn default_system() -> Self {
        #[cfg(windows)]
        {
            IpcEndpoint::NamedPipe(WINDOWS_PIPE_NAME.to_string())
        }
        #[cfg(unix)]
        {
            let default_path = PathBuf::from(DEFAULT_UNIX_SOCKET_PATH);
            if let Ok(mut dir) = std::env::var("XDG_RUNTIME_DIR") {
                dir.push_str(&format!("/{}", FALLBACK_UNIX_SOCKET_NAME));
                IpcEndpoint::UnixSocket(PathBuf::from(dir))
            } else if let Ok(home) = mihomo_platform::paths::get_home_dir() {
                IpcEndpoint::UnixSocket(home.join(FALLBACK_UNIX_SOCKET_NAME))
            } else {
                IpcEndpoint::UnixSocket(default_path)
            }
        }
        #[cfg(not(any(windows, unix)))]
        {
            IpcEndpoint::Mock
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ServiceError {
    NotRunning,
    NotInstalled,
    Unauthorized(String),
    ConnectionFailed(String),
    CommandFailed(String),
    ProtocolError(String),
    Timeout,
    Io(String),
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRunning => write!(f, "Service is not running"),
            Self::NotInstalled => write!(f, "Service is not installed"),
            Self::Unauthorized(msg) => write!(f, "Unauthorized: {msg}"),
            Self::ConnectionFailed(msg) => write!(f, "Connection failed: {msg}"),
            Self::CommandFailed(msg) => write!(f, "Command execution failed: {msg}"),
            Self::ProtocolError(msg) => write!(f, "Protocol error: {msg}"),
            Self::Timeout => write!(f, "Service communication timed out"),
            Self::Io(msg) => write!(f, "I/O error: {msg}"),
        }
    }
}

impl std::error::Error for ServiceError {}

pub async fn send_framed_json<W: AsyncWrite + Unpin, T: Serialize>(
    writer: &mut W,
    value: &T,
) -> Result<(), ServiceError> {
    let json = serde_json::to_string(value)
        .map_err(|e| ServiceError::ProtocolError(format!("Serialization failed: {e}")))?;
    writer
        .write_all(json.as_bytes())
        .await
        .map_err(|e| ServiceError::Io(e.to_string()))?;
    writer
        .write_all(b"\n")
        .await
        .map_err(|e| ServiceError::Io(e.to_string()))?;
    writer
        .flush()
        .await
        .map_err(|e| ServiceError::Io(e.to_string()))?;
    Ok(())
}

pub async fn recv_framed_json<R: AsyncBufReadExt + Unpin, T: serde::de::DeserializeOwned>(
    reader: &mut R,
) -> Result<T, ServiceError> {
    let mut line = String::new();
    let bytes_read = reader
        .read_line(&mut line)
        .await
        .map_err(|e| ServiceError::Io(e.to_string()))?;
    if bytes_read == 0 {
        return Err(ServiceError::ConnectionFailed(
            "Connection closed by peer".to_string(),
        ));
    }
    serde_json::from_str::<T>(line.trim()).map_err(|e| {
        ServiceError::ProtocolError(format!("Deserialization failed: {e} (raw: {line})"))
    })
}

pub trait ServiceCommandHandler: Send + Sync {
    fn handle_command(
        &self,
        command: ServiceCommand,
    ) -> std::result::Result<ServiceResponsePayload, String>;
}

impl<H: ServiceCommandHandler> ServiceCommandHandler for Arc<H> {
    fn handle_command(
        &self,
        command: ServiceCommand,
    ) -> std::result::Result<ServiceResponsePayload, String> {
        (**self).handle_command(command)
    }
}

pub struct DefaultServiceCommandHandler {
    tun_active: std::sync::atomic::AtomicBool,
    system_proxy_active: std::sync::atomic::AtomicBool,
    privilege_level: PrivilegeLevel,
}

impl Default for DefaultServiceCommandHandler {
    fn default() -> Self {
        Self::new(PrivilegeLevel::detect())
    }
}

impl DefaultServiceCommandHandler {
    pub fn new(privilege_level: PrivilegeLevel) -> Self {
        Self {
            tun_active: std::sync::atomic::AtomicBool::new(false),
            system_proxy_active: std::sync::atomic::AtomicBool::new(false),
            privilege_level,
        }
    }

    pub fn is_tun_active(&self) -> bool {
        self.tun_active.load(std::sync::atomic::Ordering::SeqCst)
    }
    pub fn is_system_proxy_active(&self) -> bool {
        self.system_proxy_active
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl ServiceCommandHandler for DefaultServiceCommandHandler {
    fn handle_command(
        &self,
        command: ServiceCommand,
    ) -> std::result::Result<ServiceResponsePayload, String> {
        match command {
            ServiceCommand::Ping { nonce } => Ok(ServiceResponsePayload::Pong { nonce }),
            ServiceCommand::QueryStatus => Ok(ServiceResponsePayload::Status(ServiceStatusInfo {
                state: ServiceState::Running,
                version: SERVICE_VERSION.to_string(),
                privilege_level: self.privilege_level,
                pid: Some(std::process::id()),
                tun_active: self.tun_active.load(std::sync::atomic::Ordering::SeqCst),
                system_proxy_active: self
                    .system_proxy_active
                    .load(std::sync::atomic::Ordering::SeqCst),
            })),
            ServiceCommand::StartTun { tun_interface, .. } => {
                self.tun_active
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(ServiceResponsePayload::TunStarted {
                    interface_name: tun_interface.or_else(|| Some("tun0".to_string())),
                })
            }
            ServiceCommand::StopTun => {
                self.tun_active
                    .store(false, std::sync::atomic::Ordering::SeqCst);
                Ok(ServiceResponsePayload::TunStopped)
            }
            ServiceCommand::SetSystemProxy { .. } => {
                self.system_proxy_active
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(ServiceResponsePayload::SystemProxyApplied)
            }
            ServiceCommand::ClearSystemProxy => {
                self.system_proxy_active
                    .store(false, std::sync::atomic::Ordering::SeqCst);
                Ok(ServiceResponsePayload::SystemProxyCleared)
            }
        }
    }
}

//! Injectable command boundary for native sing-box checks and child startup.

use async_trait::async_trait;
use lurker_core_ports::error::PortError;
use std::io::{self, ErrorKind};
use std::path::Path;
use std::process::Stdio;
use tokio::process::{Child, Command};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeConfigCheck {
    Accepted,
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SingBoxChildPoll {
    Running,
    Exited,
}

#[async_trait]
pub trait SingBoxChildProcess: Send {
    fn pid(&self) -> u32;
    async fn poll(&mut self) -> Result<SingBoxChildPoll, PortError>;
    async fn terminate(&mut self) -> Result<(), PortError>;
}

#[async_trait]
pub trait SingBoxCommandPort: Send + Sync {
    async fn check_config(
        &self,
        binary_path: &Path,
        config_path: &Path,
    ) -> Result<NativeConfigCheck, PortError>;

    async fn spawn_core(
        &self,
        binary_path: &Path,
        config_path: &Path,
    ) -> Result<Box<dyn SingBoxChildProcess>, PortError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TokioSingBoxCommandPort;

#[async_trait]
impl SingBoxCommandPort for TokioSingBoxCommandPort {
    async fn check_config(
        &self,
        binary_path: &Path,
        config_path: &Path,
    ) -> Result<NativeConfigCheck, PortError> {
        let output = Command::new(binary_path)
            .arg("check")
            .arg("-c")
            .arg(config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .output()
            .await
            .map_err(|error| map_command_error(binary_path, error))?;

        match output.status.success() {
            true => Ok(NativeConfigCheck::Accepted),
            false => Ok(NativeConfigCheck::Rejected),
        }
    }

    async fn spawn_core(
        &self,
        binary_path: &Path,
        config_path: &Path,
    ) -> Result<Box<dyn SingBoxChildProcess>, PortError> {
        let mut child = Command::new(binary_path)
            .arg("run")
            .arg("-c")
            .arg(config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| map_command_error(binary_path, error))?;
        let pid = match child.id() {
            Some(pid) => pid,
            None => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                return Err(PortError::Failed(
                    "sing-box child process did not expose a process ID".to_owned(),
                ));
            }
        };

        Ok(Box::new(TokioSingBoxChildProcess { child, pid }))
    }
}

struct TokioSingBoxChildProcess {
    child: Child,
    pid: u32,
}

#[async_trait]
impl SingBoxChildProcess for TokioSingBoxChildProcess {
    fn pid(&self) -> u32 {
        self.pid
    }

    async fn poll(&mut self) -> Result<SingBoxChildPoll, PortError> {
        match self.child.try_wait() {
            Ok(Some(_)) => Ok(SingBoxChildPoll::Exited),
            Ok(None) => Ok(SingBoxChildPoll::Running),
            Err(error) => Err(PortError::Io(format!(
                "sing-box process poll failed: {error}"
            ))),
        }
    }

    async fn terminate(&mut self) -> Result<(), PortError> {
        match self.child.try_wait() {
            Ok(Some(_)) => Ok(()),
            Ok(None) => {
                self.child.start_kill().map_err(|error| {
                    PortError::Io(format!("sing-box process termination failed: {error}"))
                })?;
                self.child.wait().await.map_err(|error| {
                    PortError::Io(format!("sing-box process wait failed: {error}"))
                })?;
                Ok(())
            }
            Err(error) => Err(PortError::Io(format!(
                "sing-box process status before termination failed: {error}"
            ))),
        }
    }
}

fn map_command_error(binary_path: &Path, error: io::Error) -> PortError {
    match error.kind() {
        ErrorKind::NotFound => PortError::NotFound(format!(
            "sing-box executable is unavailable at {}",
            binary_path.display()
        )),
        ErrorKind::PermissionDenied => {
            PortError::PermissionDenied("sing-box executable cannot be started".to_owned())
        }
        _ => PortError::Io(format!("sing-box command failed to start: {error}")),
    }
}

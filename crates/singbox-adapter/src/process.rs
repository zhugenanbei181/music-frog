//! Owner-local sing-box process lifecycle adapter.

use crate::command::{
    NativeConfigCheck, SingBoxChildPoll, SingBoxCommandPort, TokioSingBoxCommandPort,
};
use async_trait::async_trait;
use infiltrator_contract::core_identity::{CoreIdentity, CoreInstanceId};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_process::CoreProcess;
use infiltrator_ports::error::PortError;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

enum ProcessState {
    Stopped,
    Running(Box<dyn crate::command::SingBoxChildProcess>),
    Exited,
}

pub struct SingBoxProcess {
    binary_path: PathBuf,
    config_path: PathBuf,
    identity: CoreIdentity,
    controller_endpoint: String,
    command: Arc<dyn SingBoxCommandPort>,
    state: Mutex<ProcessState>,
}

impl SingBoxProcess {
    pub fn with_tokio(
        binary_path: PathBuf,
        config_path: PathBuf,
        instance_id: CoreInstanceId,
        controller_address: SocketAddr,
    ) -> Result<Self, PortError> {
        Self::new(
            binary_path,
            config_path,
            instance_id,
            controller_address,
            Arc::new(TokioSingBoxCommandPort),
        )
    }

    pub fn new(
        binary_path: PathBuf,
        config_path: PathBuf,
        instance_id: CoreInstanceId,
        controller_address: SocketAddr,
        command: Arc<dyn SingBoxCommandPort>,
    ) -> Result<Self, PortError> {
        if !controller_address.ip().is_loopback() {
            return Err(PortError::Failed(
                "sing-box controller must bind to a loopback address".to_owned(),
            ));
        }

        Ok(Self {
            binary_path,
            config_path,
            identity: CoreIdentity::new(crate::engine_id(), instance_id, None),
            controller_endpoint: format!("http://{controller_address}"),
            command,
            state: Mutex::new(ProcessState::Stopped),
        })
    }

    pub fn binary_path(&self) -> &PathBuf {
        &self.binary_path
    }

    pub fn config_path(&self) -> &PathBuf {
        &self.config_path
    }

    async fn prepare_start(&self, state: &mut ProcessState) -> Result<(), PortError> {
        match std::mem::replace(state, ProcessState::Stopped) {
            ProcessState::Stopped | ProcessState::Exited => Ok(()),
            ProcessState::Running(mut child) => match child.poll().await {
                Ok(SingBoxChildPoll::Running) => {
                    *state = ProcessState::Running(child);
                    Err(PortError::Failed(
                        "sing-box process is already running".to_owned(),
                    ))
                }
                Ok(SingBoxChildPoll::Exited) => {
                    *state = ProcessState::Exited;
                    Ok(())
                }
                Err(error) => {
                    *state = ProcessState::Running(child);
                    Err(error)
                }
            },
        }
    }
}

#[async_trait]
impl CoreProcess for SingBoxProcess {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(self.identity.clone())
    }

    async fn start(&self) -> Result<(), PortError> {
        let mut state = self.state.lock().await;
        self.prepare_start(&mut state).await?;

        match self
            .command
            .check_config(&self.binary_path, &self.config_path)
            .await?
        {
            NativeConfigCheck::Accepted => {}
            NativeConfigCheck::Rejected => {
                return Err(PortError::Rejected(
                    infiltrator_contract::error::Failure::new(
                        infiltrator_contract::error::ErrorCode::Configuration,
                        "sing-box native config check rejected the active document",
                        false,
                    ),
                ));
            }
        }

        let child = self
            .command
            .spawn_core(&self.binary_path, &self.config_path)
            .await?;
        *state = ProcessState::Running(child);
        Ok(())
    }

    async fn stop(&self) -> Result<(), PortError> {
        let mut state = self.state.lock().await;
        match std::mem::replace(&mut *state, ProcessState::Stopped) {
            ProcessState::Stopped | ProcessState::Exited => Ok(()),
            ProcessState::Running(mut child) => match child.terminate().await {
                Ok(()) => Ok(()),
                Err(error) => {
                    *state = ProcessState::Running(child);
                    Err(error)
                }
            },
        }
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        let mut state = self.state.lock().await;
        match std::mem::replace(&mut *state, ProcessState::Stopped) {
            ProcessState::Stopped => Ok(CoreLifecycle::Stopped),
            ProcessState::Exited => {
                *state = ProcessState::Exited;
                Ok(CoreLifecycle::Failed)
            }
            ProcessState::Running(mut child) => match child.poll().await {
                Ok(SingBoxChildPoll::Running) => {
                    *state = ProcessState::Running(child);
                    Ok(CoreLifecycle::Running)
                }
                Ok(SingBoxChildPoll::Exited) => {
                    *state = ProcessState::Exited;
                    Ok(CoreLifecycle::Failed)
                }
                Err(error) => {
                    *state = ProcessState::Running(child);
                    Err(error)
                }
            },
        }
    }

    fn controller_endpoint(&self) -> Option<String> {
        Some(self.controller_endpoint.clone())
    }

    async fn pid(&self) -> Option<u32> {
        match self.status().await {
            Ok(CoreLifecycle::Running) => {
                let state = self.state.lock().await;
                match &*state {
                    ProcessState::Running(child) => Some(child.pid()),
                    ProcessState::Stopped | ProcessState::Exited => None,
                }
            }
            Ok(
                CoreLifecycle::Stopped
                | CoreLifecycle::Starting
                | CoreLifecycle::Ready
                | CoreLifecycle::Stopping
                | CoreLifecycle::Failed,
            )
            | Err(_) => None,
        }
    }
}

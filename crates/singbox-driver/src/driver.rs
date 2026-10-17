//! Owner-local sing-box process lifecycle: spawn, stop, child polling and pid
//! readback behind the neutral `CoreProcess` port.

use crate::process::command::{
    NativeConfigCheck, SingBoxChildPoll, SingBoxChildProcess, SingBoxCommandPort,
    TokioSingBoxCommandPort,
};
use async_trait::async_trait;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreIdentity, CoreInstanceId, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_ports::core_process::CoreProcess;
use lurker_core_ports::error::PortError;
use std::mem;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::OwnedMutexGuard;

/// Serializes process activation with profile-file transactions that commit
/// while the Core is inactive.
#[derive(Default)]
pub struct SingBoxCoreOperationGate {
    lock: Arc<Mutex<()>>,
}

impl SingBoxCoreOperationGate {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn acquire(&self) -> OwnedMutexGuard<()> {
        Arc::clone(&self.lock).lock_owned().await
    }
}

enum ProcessState {
    Stopped,
    Running(Box<dyn SingBoxChildProcess>),
    Exited,
}

pub struct SingBoxProcess {
    binary_path: PathBuf,
    config_path: PathBuf,
    identity: CoreIdentity,
    controller_endpoint: String,
    command: Arc<dyn SingBoxCommandPort>,
    operation_gate: Arc<SingBoxCoreOperationGate>,
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
        Self::new_with_operation_gate(
            binary_path,
            config_path,
            instance_id,
            controller_address,
            command,
            Arc::new(SingBoxCoreOperationGate::new()),
        )
    }

    pub fn new_with_operation_gate(
        binary_path: PathBuf,
        config_path: PathBuf,
        instance_id: CoreInstanceId,
        controller_address: SocketAddr,
        command: Arc<dyn SingBoxCommandPort>,
        operation_gate: Arc<SingBoxCoreOperationGate>,
    ) -> Result<Self, PortError> {
        if !controller_address.ip().is_loopback() || controller_address.port() == 0 {
            return Err(PortError::Failed(
                "sing-box controller must bind to a fixed loopback address".to_owned(),
            ));
        }

        Ok(Self {
            binary_path,
            config_path,
            identity: CoreIdentity::new(EngineId::sing_box(), instance_id, None),
            controller_endpoint: controller_address.to_string(),
            command,
            operation_gate,
            state: Mutex::new(ProcessState::Stopped),
        })
    }

    pub fn binary_path(&self) -> &PathBuf {
        &self.binary_path
    }

    pub fn config_path(&self) -> &PathBuf {
        &self.config_path
    }

    pub fn operation_gate(&self) -> Arc<SingBoxCoreOperationGate> {
        Arc::clone(&self.operation_gate)
    }

    async fn prepare_start(&self, state: &mut ProcessState) -> Result<(), PortError> {
        match mem::replace(state, ProcessState::Stopped) {
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

    async fn start(&self, context: CoreProcessStartContext) -> Result<(), PortError> {
        if !context.identity().same_instance(&self.identity) {
            return Err(PortError::Failed(
                "Core start context targets a different sing-box process".to_owned(),
            ));
        }
        let _gate = self.operation_gate.acquire().await;
        let mut state = self.state.lock().await;
        self.prepare_start(&mut state).await?;

        match self
            .command
            .check_config(&self.binary_path, &self.config_path)
            .await?
        {
            NativeConfigCheck::Accepted => {}
            NativeConfigCheck::Rejected => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::Configuration,
                    "sing-box native config check rejected the active document",
                    false,
                )));
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
        let _gate = self.operation_gate.acquire().await;
        let mut state = self.state.lock().await;
        match mem::replace(&mut *state, ProcessState::Stopped) {
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
        let _gate = self.operation_gate.acquire().await;
        let mut state = self.state.lock().await;
        match mem::replace(&mut *state, ProcessState::Stopped) {
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

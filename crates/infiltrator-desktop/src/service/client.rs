use std::time::Duration;
use tokio::io::BufReader;

use super::state_machine::{CommandSequence, SequenceExecutionResult};
use super::{
    AuthToken, IpcEndpoint, ServiceCommand, ServiceError, ServiceRequest, ServiceResponse,
    ServiceResponsePayload, ServiceState, ServiceStatusInfo, recv_framed_json, send_framed_json,
};

pub struct ServiceClient {
    endpoint: IpcEndpoint,
    auth_token: AuthToken,
    timeout: Duration,
}

impl ServiceClient {
    pub fn new(endpoint: IpcEndpoint, auth_token: AuthToken) -> Self {
        Self {
            endpoint,
            auth_token,
            timeout: Duration::from_secs(3),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn endpoint(&self) -> &IpcEndpoint {
        &self.endpoint
    }
    pub fn auth_token(&self) -> &AuthToken {
        &self.auth_token
    }

    pub async fn is_service_available(&self) -> bool {
        self.ping(0).await.is_ok()
    }

    pub async fn check_state(&self) -> ServiceState {
        match self.query_status().await {
            Ok(status) => status.state,
            Err(ServiceError::NotRunning) => ServiceState::Stopped,
            Err(ServiceError::NotInstalled) => ServiceState::NotInstalled,
            Err(ServiceError::ConnectionFailed(_)) => ServiceState::Stopped,
            Err(e) => ServiceState::Error(e.to_string()),
        }
    }

    pub async fn query_status_or_fallback(&self) -> ServiceStatusInfo {
        match self.query_status().await {
            Ok(status) => status,
            Err(ServiceError::NotInstalled) => ServiceStatusInfo::fallback_uninstalled(),
            Err(_) => ServiceStatusInfo::fallback_stopped(),
        }
    }

    pub async fn send_command(
        &self,
        command: ServiceCommand,
    ) -> Result<ServiceResponsePayload, ServiceError> {
        let request = ServiceRequest::authed(&self.auth_token, command);
        let response = self.execute_request(&request).await?;
        if response.success {
            Ok(response.payload.unwrap_or(ServiceResponsePayload::Empty))
        } else {
            let msg = response
                .error
                .unwrap_or_else(|| "Unknown service error".to_string());
            if msg.contains("Unauthorized") || msg.contains("invalid authentication token") {
                Err(ServiceError::Unauthorized(msg))
            } else {
                Err(ServiceError::CommandFailed(msg))
            }
        }
    }

    pub async fn ping(&self, nonce: u64) -> Result<u64, ServiceError> {
        match self.send_command(ServiceCommand::Ping { nonce }).await? {
            ServiceResponsePayload::Pong { nonce: resp_nonce } => Ok(resp_nonce),
            _ => Err(ServiceError::ProtocolError(
                "Expected Pong payload".to_string(),
            )),
        }
    }

    pub async fn query_status(&self) -> Result<ServiceStatusInfo, ServiceError> {
        match self.send_command(ServiceCommand::QueryStatus).await? {
            ServiceResponsePayload::Status(status) => Ok(status),
            _ => Err(ServiceError::ProtocolError(
                "Expected Status payload".to_string(),
            )),
        }
    }

    pub async fn start_tun(
        &self,
        tun_interface: Option<String>,
        config_path: Option<String>,
    ) -> Result<Option<String>, ServiceError> {
        match self
            .send_command(ServiceCommand::StartTun {
                tun_interface,
                config_path,
            })
            .await?
        {
            ServiceResponsePayload::TunStarted { interface_name } => Ok(interface_name),
            _ => Err(ServiceError::ProtocolError(
                "Expected TunStarted payload".to_string(),
            )),
        }
    }

    pub async fn stop_tun(&self) -> Result<(), ServiceError> {
        match self.send_command(ServiceCommand::StopTun).await? {
            ServiceResponsePayload::TunStopped => Ok(()),
            _ => Err(ServiceError::ProtocolError(
                "Expected TunStopped payload".to_string(),
            )),
        }
    }

    pub async fn set_system_proxy(
        &self,
        endpoint: impl Into<String>,
        bypass: Option<String>,
    ) -> Result<(), ServiceError> {
        match self
            .send_command(ServiceCommand::SetSystemProxy {
                endpoint: endpoint.into(),
                bypass,
            })
            .await?
        {
            ServiceResponsePayload::SystemProxyApplied => Ok(()),
            _ => Err(ServiceError::ProtocolError(
                "Expected SystemProxyApplied payload".to_string(),
            )),
        }
    }

    pub async fn clear_system_proxy(&self) -> Result<(), ServiceError> {
        match self.send_command(ServiceCommand::ClearSystemProxy).await? {
            ServiceResponsePayload::SystemProxyCleared => Ok(()),
            _ => Err(ServiceError::ProtocolError(
                "Expected SystemProxyCleared payload".to_string(),
            )),
        }
    }

    pub async fn execute_sequence(&self, sequence: &CommandSequence) -> SequenceExecutionResult {
        let mut step_results = Vec::new();
        let mut all_ok = true;

        for cmd in &sequence.commands {
            match self.send_command(cmd.clone()).await {
                Ok(payload) => {
                    step_results.push((cmd.clone(), Ok(payload)));
                }
                Err(err) => {
                    all_ok = false;
                    step_results.push((cmd.clone(), Err(err.to_string())));
                    break;
                }
            }
        }

        SequenceExecutionResult {
            sequence_name: sequence.name.clone(),
            step_results,
            success: all_ok,
        }
    }

    async fn execute_request(
        &self,
        request: &ServiceRequest,
    ) -> Result<ServiceResponse, ServiceError> {
        match &self.endpoint {
            #[cfg(unix)]
            IpcEndpoint::UnixSocket(path) => {
                if !path.exists() {
                    return Err(ServiceError::NotRunning);
                }
                let stream =
                    tokio::time::timeout(self.timeout, tokio::net::UnixStream::connect(path))
                        .await
                        .map_err(|_| ServiceError::Timeout)?
                        .map_err(|e| ServiceError::ConnectionFailed(e.to_string()))?;
                let (reader, mut writer) = stream.into_split();
                let mut buf_reader = BufReader::new(reader);
                send_framed_json(&mut writer, request).await?;
                tokio::time::timeout(
                    self.timeout,
                    recv_framed_json::<_, ServiceResponse>(&mut buf_reader),
                )
                .await
                .map_err(|_| ServiceError::Timeout)?
            }
            #[cfg(windows)]
            IpcEndpoint::NamedPipe(pipe_name) => {
                let client = tokio::net::windows::named_pipe::ClientOptions::new()
                    .open(pipe_name)
                    .map_err(|e| ServiceError::ConnectionFailed(e.to_string()))?;
                let (reader, mut writer) = tokio::io::split(client);
                let mut buf_reader = BufReader::new(reader);
                send_framed_json(&mut writer, request).await?;
                tokio::time::timeout(
                    self.timeout,
                    recv_framed_json::<_, ServiceResponse>(&mut buf_reader),
                )
                .await
                .map_err(|_| ServiceError::Timeout)?
            }
            #[cfg(not(windows))]
            IpcEndpoint::NamedPipe(pipe_name) => Err(ServiceError::ConnectionFailed(format!(
                "Named pipes are not supported on this platform: {pipe_name}"
            ))),
            IpcEndpoint::Mock => Err(ServiceError::ConnectionFailed(
                "Mock endpoint cannot connect via execute_request; use MockServiceHarness"
                    .to_string(),
            )),
        }
    }
}

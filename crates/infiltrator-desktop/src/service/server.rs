use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::watch;

use super::state_machine::{CommandSequence, SequenceExecutionResult};
use super::{
    AuthToken, DefaultServiceCommandHandler, IpcEndpoint, PrivilegeLevel, ServiceCommandHandler,
    ServiceError, ServiceRequest, ServiceResponse, ServiceResponsePayload, recv_framed_json,
    send_framed_json,
};

pub struct ServiceServer<H> {
    endpoint: IpcEndpoint,
    auth_token: AuthToken,
    handler: Arc<H>,
}

impl<H: ServiceCommandHandler + 'static> ServiceServer<H> {
    pub fn new(endpoint: IpcEndpoint, auth_token: AuthToken, handler: H) -> Self {
        Self {
            endpoint,
            auth_token,
            handler: Arc::new(handler),
        }
    }

    pub async fn run(&self, mut shutdown_rx: watch::Receiver<bool>) -> Result<(), ServiceError> {
        match &self.endpoint {
            #[cfg(unix)]
            IpcEndpoint::UnixSocket(path) => {
                if path.exists() {
                    let _ = tokio::fs::remove_file(path).await;
                }
                if let Some(parent) = path.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                let listener = tokio::net::UnixListener::bind(path)
                    .map_err(|e| ServiceError::Io(e.to_string()))?;

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
                }

                loop {
                    tokio::select! {
                        _ = shutdown_rx.changed() => {
                            if *shutdown_rx.borrow() {
                                break;
                            }
                        }
                        res = listener.accept() => {
                            match res {
                                Ok((stream, _)) => {
                                    let handler = self.handler.clone();
                                    let token = self.auth_token.clone();
                                    tokio::spawn(async move {
                                        let (reader, writer) = stream.into_split();
                                        let _ = process_connection(reader, writer, &token, handler).await;
                                    });
                                }
                                Err(e) => {
                                    log::warn!("Unix socket accept error: {e}");
                                }
                            }
                        }
                    }
                }
                let _ = tokio::fs::remove_file(path).await;
                Ok(())
            }
            #[cfg(windows)]
            IpcEndpoint::NamedPipe(pipe_name) => {
                let server = tokio::net::windows::named_pipe::ServerOptions::new()
                    .first_pipe_instance(true)
                    .create(pipe_name)
                    .map_err(|e| ServiceError::Io(e.to_string()))?;

                let mut current_server = server;
                loop {
                    tokio::select! {
                        _ = shutdown_rx.changed() => {
                            if *shutdown_rx.borrow() {
                                break;
                            }
                        }
                        connected = current_server.connect() => {
                            if connected.is_ok() {
                                let (reader, writer) = tokio::io::split(current_server);
                                let handler = self.handler.clone();
                                let token = self.auth_token.clone();
                                tokio::spawn(async move {
                                    let _ = process_connection(reader, writer, &token, handler).await;
                                });
                                current_server = tokio::net::windows::named_pipe::ServerOptions::new()
                                    .create(pipe_name)
                                    .map_err(|e| ServiceError::Io(e.to_string()))?;
                            }
                        }
                    }
                }
                Ok(())
            }
            #[cfg(not(windows))]
            IpcEndpoint::NamedPipe(pipe_name) => Err(ServiceError::Io(format!(
                "Named pipes are not supported on this platform: {pipe_name}"
            ))),
            IpcEndpoint::Mock => Ok(()),
        }
    }
}

pub async fn process_connection<R, W, H>(
    reader: R,
    mut writer: W,
    auth_token: &AuthToken,
    handler: Arc<H>,
) -> Result<(), ServiceError>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
    H: ServiceCommandHandler,
{
    let mut buf_reader = BufReader::new(reader);
    while let Ok(request) = recv_framed_json::<_, ServiceRequest>(&mut buf_reader).await {
        let response = if !auth_token.verify(&request.token) {
            ServiceResponse::error("Unauthorized: invalid authentication token")
        } else {
            match handler.handle_command(request.command) {
                Ok(payload) => ServiceResponse::ok(payload),
                Err(err_msg) => ServiceResponse::error(err_msg),
            }
        };

        if send_framed_json(&mut writer, &response).await.is_err() {
            break;
        }
    }
    let _ = writer.flush().await;
    Ok(())
}

pub struct MockServiceHarness<H> {
    pub auth_token: AuthToken,
    pub handler: Arc<H>,
}

impl MockServiceHarness<DefaultServiceCommandHandler> {
    pub fn with_privilege(privilege: PrivilegeLevel) -> Self {
        Self::new(
            AuthToken::generate(),
            DefaultServiceCommandHandler::new(privilege),
        )
    }
}

impl<H: ServiceCommandHandler + 'static> MockServiceHarness<H> {
    pub fn new(auth_token: AuthToken, handler: H) -> Self {
        Self {
            auth_token,
            handler: Arc::new(handler),
        }
    }

    pub fn auth_token(&self) -> &AuthToken {
        &self.auth_token
    }

    pub async fn execute_request(
        &self,
        token: &str,
        command: super::ServiceCommand,
    ) -> Result<ServiceResponse, ServiceError> {
        self.dispatch(token, command).await
    }

    pub async fn dispatch_raw(
        &self,
        request: &ServiceRequest,
    ) -> Result<ServiceResponse, ServiceError> {
        let (client_read, server_write) = tokio::io::duplex(4096);
        let (server_read, client_write) = tokio::io::duplex(4096);

        let handler = self.handler.clone();
        let token = self.auth_token.clone();

        tokio::spawn(async move {
            let _ = process_connection(server_read, server_write, &token, handler).await;
        });

        let mut client_writer = client_write;
        let mut client_reader = BufReader::new(client_read);

        send_framed_json(&mut client_writer, request).await?;
        client_writer
            .flush()
            .await
            .map_err(|e| ServiceError::Io(e.to_string()))?;

        recv_framed_json::<_, ServiceResponse>(&mut client_reader).await
    }

    pub async fn dispatch(
        &self,
        token: &str,
        command: super::ServiceCommand,
    ) -> Result<ServiceResponse, ServiceError> {
        let request = ServiceRequest {
            id: String::new(),
            auth_token: token.to_string(),
            token: token.to_string(),
            command,
        };
        self.dispatch_raw(&request).await
    }

    pub async fn dispatch_authed(
        &self,
        command: super::ServiceCommand,
    ) -> Result<ServiceResponsePayload, ServiceError> {
        let resp = self.dispatch(self.auth_token.as_str(), command).await?;
        if resp.success {
            Ok(resp.payload.unwrap_or(ServiceResponsePayload::Empty))
        } else {
            Err(ServiceError::CommandFailed(resp.error.unwrap_or_default()))
        }
    }

    pub async fn execute_sequence(&self, sequence: &CommandSequence) -> SequenceExecutionResult {
        let mut step_results = Vec::new();
        let mut all_ok = true;

        for cmd in &sequence.commands {
            match self.dispatch_authed(cmd.clone()).await {
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
}

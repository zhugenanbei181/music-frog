use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(not(unix))]
use tokio::net::{TcpListener, TcpStream};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast;
use tokio::time::timeout;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum IpcCommand {
    FocusWindow,
    OpenUrl(String),
    Ping,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum IpcResponse {
    Ack,
    Pong,
    Error(String),
}

fn get_ipc_temp_dir() -> PathBuf {
    std::env::var("TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| env::temp_dir())
}

pub struct SingleInstanceIpcServer {
    running: Arc<AtomicBool>,
    #[cfg(unix)]
    socket_path: PathBuf,
    #[cfg(not(unix))]
    port_file: PathBuf,
}

impl SingleInstanceIpcServer {
    pub fn start(socket_id: &str) -> Result<(Self, broadcast::Receiver<IpcCommand>)> {
        let (tx, rx) = broadcast::channel(32);
        let running = Arc::new(AtomicBool::new(true));

        #[cfg(unix)]
        let (server, _tx) = {
            let socket_path = get_ipc_temp_dir().join(format!("{}.sock", socket_id));
            if socket_path.exists() {
                let _ = std::fs::remove_file(&socket_path);
            }
            let listener =
                UnixListener::bind(&socket_path).context("Failed to bind Unix socket")?;
            let r = running.clone();
            let t = tx.clone();
            tokio::spawn(async move {
                while r.load(Ordering::Relaxed) {
                    if let Ok(result) = timeout(Duration::from_millis(100), listener.accept()).await
                        && let Ok((mut stream, _)) = result
                    {
                        let t = t.clone();
                        tokio::spawn(async move {
                            let (reader, mut writer) = stream.split();
                            let mut reader = BufReader::new(reader);
                            let mut line = String::new();
                            if reader.read_line(&mut line).await.is_ok() && !line.is_empty() {
                                if let Ok(cmd) = serde_json::from_str::<IpcCommand>(&line) {
                                    let resp = match cmd {
                                        IpcCommand::Ping => IpcResponse::Pong,
                                        _ => {
                                            let _ = t.send(cmd);
                                            IpcResponse::Ack
                                        }
                                    };
                                    if let Ok(resp_json) = serde_json::to_string(&resp) {
                                        let _ = writer
                                            .write_all(format!("{}\n", resp_json).as_bytes())
                                            .await;
                                    }
                                } else {
                                    let resp = IpcResponse::Error("Invalid command".into());
                                    if let Ok(resp_json) = serde_json::to_string(&resp) {
                                        let _ = writer
                                            .write_all(format!("{}\n", resp_json).as_bytes())
                                            .await;
                                    }
                                }
                            }
                        });
                    }
                }
            });
            (
                Self {
                    running,
                    socket_path,
                },
                tx,
            )
        };

        #[cfg(not(unix))]
        let (server, _tx) = {
            let port_file = get_ipc_temp_dir().join(format!("{}.port", socket_id));
            let std_listener =
                std::net::TcpListener::bind("127.0.0.1:0").context("Failed to bind TCP socket")?;
            let port = std_listener.local_addr()?.port();
            std::fs::write(&port_file, port.to_string())?;
            std_listener.set_nonblocking(true)?;
            let listener = TcpListener::from_std(std_listener)?;
            let r = running.clone();
            let t = tx.clone();
            tokio::spawn(async move {
                while r.load(Ordering::Relaxed) {
                    if let Ok(result) = timeout(Duration::from_millis(100), listener.accept()).await
                    {
                        if let Ok((mut stream, _)) = result {
                            let t = t.clone();
                            tokio::spawn(async move {
                                let (reader, mut writer) = stream.split();
                                let mut reader = BufReader::new(reader);
                                let mut line = String::new();
                                if reader.read_line(&mut line).await.is_ok() && !line.is_empty() {
                                    if let Ok(cmd) = serde_json::from_str::<IpcCommand>(&line) {
                                        let resp = match cmd {
                                            IpcCommand::Ping => IpcResponse::Pong,
                                            _ => {
                                                let _ = t.send(cmd);
                                                IpcResponse::Ack
                                            }
                                        };
                                        if let Ok(resp_json) = serde_json::to_string(&resp) {
                                            let _ = writer
                                                .write_all(format!("{}\n", resp_json).as_bytes())
                                                .await;
                                        }
                                    } else {
                                        let resp = IpcResponse::Error("Invalid command".into());
                                        if let Ok(resp_json) = serde_json::to_string(&resp) {
                                            let _ = writer
                                                .write_all(format!("{}\n", resp_json).as_bytes())
                                                .await;
                                        }
                                    }
                                }
                            });
                        }
                    }
                }
            });
            (Self { running, port_file }, tx)
        };

        Ok((server, rx))
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        #[cfg(unix)]
        let _ = std::fs::remove_file(&self.socket_path);
        #[cfg(not(unix))]
        let _ = std::fs::remove_file(&self.port_file);
    }
}

impl Drop for SingleInstanceIpcServer {
    fn drop(&mut self) {
        self.stop();
    }
}

pub struct SingleInstanceIpcClient;

impl SingleInstanceIpcClient {
    pub async fn send_command(
        socket_id: &str,
        cmd: IpcCommand,
        timeout: Duration,
    ) -> Result<IpcResponse> {
        let req_json = serde_json::to_string(&cmd)?;

        #[cfg(unix)]
        let mut stream = {
            let socket_path = get_ipc_temp_dir().join(format!("{}.sock", socket_id));
            tokio::time::timeout(timeout, UnixStream::connect(&socket_path))
                .await
                .context("Connection timeout")?
                .context("Failed to connect to Unix socket")?
        };

        #[cfg(not(unix))]
        let mut stream = {
            let port_file = get_ipc_temp_dir().join(format!("{}.port", socket_id));
            let port_str =
                std::fs::read_to_string(&port_file).context("Failed to read port file")?;
            let port: u16 = port_str.trim().parse().context("Invalid port number")?;
            tokio::time::timeout(timeout, TcpStream::connect(format!("127.0.0.1:{}", port)))
                .await
                .context("Connection timeout")?
                .context("Failed to connect to TCP socket")?
        };

        let (reader, mut writer) = stream.split();
        writer
            .write_all(format!("{}\n", req_json).as_bytes())
            .await?;

        let mut reader = BufReader::new(reader);
        let mut line = String::new();
        tokio::time::timeout(timeout, reader.read_line(&mut line))
            .await
            .context("Read timeout")?
            .context("Failed to read response")?;

        let resp: IpcResponse = serde_json::from_str(&line).context("Failed to parse response")?;
        Ok(resp)
    }

    pub async fn notify_primary_to_focus(socket_id: &str) -> Result<()> {
        let resp =
            Self::send_command(socket_id, IpcCommand::FocusWindow, Duration::from_secs(1)).await?;
        if resp != IpcResponse::Ack {
            bail!("Unexpected response: {:?}", resp);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ipc_ping_pong() {
        let socket_id = "test_ipc_ping_pong";
        let (server, _rx) = SingleInstanceIpcServer::start(socket_id).unwrap();

        let resp = SingleInstanceIpcClient::send_command(
            socket_id,
            IpcCommand::Ping,
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        assert_eq!(resp, IpcResponse::Pong);

        server.stop();
    }

    #[tokio::test]
    async fn test_ipc_focus_window() {
        let socket_id = "test_ipc_focus_window";
        let (server, mut rx) = SingleInstanceIpcServer::start(socket_id).unwrap();

        SingleInstanceIpcClient::notify_primary_to_focus(socket_id)
            .await
            .unwrap();

        let cmd = rx.recv().await.unwrap();
        assert_eq!(cmd, IpcCommand::FocusWindow);

        server.stop();
    }

    #[tokio::test]
    async fn test_ipc_timeout() {
        let socket_id = "test_ipc_timeout_missing";

        let result = SingleInstanceIpcClient::send_command(
            socket_id,
            IpcCommand::Ping,
            Duration::from_millis(10),
        )
        .await;
        assert!(result.is_err());
    }
}

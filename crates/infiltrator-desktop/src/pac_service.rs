//! Loopback HTTP server for generated PAC scripts.

use async_trait::async_trait;
use infiltrator_ports::error::PortError;
use infiltrator_ports::pac::PacServicePort;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

struct ServiceHandle {
    url: String,
    shutdown: Option<oneshot::Sender<()>>,
}

#[derive(Clone)]
pub struct DesktopPacServicePort {
    state: Arc<Mutex<Option<ServiceHandle>>>,
}

static SHARED_STATE: OnceLock<Arc<Mutex<Option<ServiceHandle>>>> = OnceLock::new();

impl DesktopPacServicePort {
    pub fn shared() -> Self {
        Self {
            state: Arc::clone(SHARED_STATE.get_or_init(|| Arc::new(Mutex::new(None)))),
        }
    }
}

impl Default for DesktopPacServicePort {
    fn default() -> Self {
        Self::shared()
    }
}

#[async_trait]
impl PacServicePort for DesktopPacServicePort {
    async fn start(&self, script: String) -> Result<String, PortError> {
        self.stop().await?;
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|error| PortError::Io(format!("bind PAC loopback server: {error}")))?;
        let port = listener
            .local_addr()
            .map_err(|error| PortError::Io(format!("read PAC loopback address: {error}")))?
            .port();
        let url = format!("http://127.0.0.1:{port}/proxy.pac");
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        let script = Arc::new(script);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => break,
                    accepted = listener.accept() => match accepted {
                        Ok((stream, _)) => {
                            let script = Arc::clone(&script);
                            tokio::spawn(async move { serve_connection(stream, script).await });
                        }
                        Err(error) => {
                            log::warn!("PAC loopback accept failed: {error}");
                            break;
                        }
                    },
                }
            }
        });
        *self.state.lock().expect("PAC service state lock") = Some(ServiceHandle {
            url: url.clone(),
            shutdown: Some(shutdown_tx),
        });
        Ok(url)
    }

    async fn stop(&self) -> Result<(), PortError> {
        let handle = self.state.lock().expect("PAC service state lock").take();
        if let Some(mut handle) = handle
            && let Some(shutdown) = handle.shutdown.take()
        {
            let _ = shutdown.send(());
        }
        Ok(())
    }

    async fn status(&self) -> Result<Option<String>, PortError> {
        Ok(self
            .state
            .lock()
            .expect("PAC service state lock")
            .as_ref()
            .map(|handle| handle.url.clone()))
    }
}

async fn serve_connection(mut stream: TcpStream, script: Arc<String>) {
    let mut request = [0_u8; 8192];
    let size = match stream.read(&mut request).await {
        Ok(size) => size,
        Err(_) => return,
    };
    let request = String::from_utf8_lossy(&request[..size]);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("")
        .split('?')
        .next()
        .unwrap_or("");
    let (status, content_type, body) = if path == "/proxy.pac" || path == "/" {
        (
            "200 OK",
            "application/x-ns-proxy-autoconfig",
            script.as_str(),
        )
    } else {
        ("404 Not Found", "text/plain; charset=utf-8", "not found")
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn loopback_server_serves_generated_pac_and_stops() {
        let service = DesktopPacServicePort::shared();
        let url = service
            .start(r#"function FindProxyForURL(url, host) { return "DIRECT"; }"#.to_owned())
            .await
            .expect("start PAC service");
        let port = url
            .rsplit(':')
            .next()
            .and_then(|value| value.split('/').next())
            .and_then(|value| value.parse::<u16>().ok())
            .expect("PAC port");
        let mut stream = TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect PAC service");
        stream
            .write_all(b"GET /proxy.pac HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .expect("request PAC script");
        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .await
            .expect("read PAC response");
        let response = String::from_utf8(response).expect("PAC response UTF-8");
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("application/x-ns-proxy-autoconfig"));
        assert!(response.contains("FindProxyForURL"));
        service.stop().await.expect("stop PAC service");
        assert!(service.status().await.expect("PAC status").is_none());
    }
}

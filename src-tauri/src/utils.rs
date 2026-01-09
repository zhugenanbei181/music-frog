use std::net::SocketAddr;
use std::time::Duration;

use tokio::{net::TcpListener, time::sleep};

#[derive(Debug, Clone, Copy)]
pub(crate) struct LaunchPorts {
    pub(crate) static_port: Option<u16>,
    pub(crate) admin_port: Option<u16>,
}

pub(crate) fn parse_launch_ports() -> LaunchPorts {
    let mut static_port = None;
    let mut admin_port = None;
    for arg in std::env::args().skip(1) {
        if let Some(value) = arg.strip_prefix("--static-port=") {
            static_port = value.parse::<u16>().ok();
        } else if let Some(value) = arg.strip_prefix("--admin-port=") {
            admin_port = value.parse::<u16>().ok();
        }
    }
    LaunchPorts {
        static_port,
        admin_port,
    }
}

pub(crate) fn extract_port_from_url(url: &str) -> Option<u16> {
    let host = url.split("://").nth(1)?;
    let host = host.split('/').next()?;
    let port = host.split(':').next_back()?;
    port.parse::<u16>().ok()
}

pub(crate) async fn wait_for_port_release(port: u16, timeout: Duration) {
    let start = std::time::Instant::now();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    loop {
        match TcpListener::bind(addr).await {
            Ok(listener) => {
                drop(listener);
                break;
            }
            Err(_) => {
                if start.elapsed() >= timeout {
                    break;
                }
                sleep(Duration::from_millis(150)).await;
            }
        }
    }
}

pub(crate) async fn is_port_available(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpListener::bind(addr).await.is_ok()
}

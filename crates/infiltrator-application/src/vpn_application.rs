//! Runtime-neutral VpnService lifecycle use-case.

use futures_util::lock::Mutex;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::vpn::{
    VpnConfiguration, VpnSessionSnapshot, VpnSessionState, VpnStartRequest,
};
use infiltrator_ports::vpn_service::VpnServicePort;
use std::sync::Arc;

struct State {
    snapshot: VpnSessionSnapshot,
    next_revision: u64,
}

#[derive(Clone)]
pub struct VpnServiceApplication {
    port: Arc<dyn VpnServicePort>,
    state: Arc<Mutex<State>>,
}

impl VpnServiceApplication {
    pub fn new(port: Arc<dyn VpnServicePort>) -> Self {
        Self {
            port,
            state: Arc::new(Mutex::new(State {
                snapshot: VpnSessionSnapshot::default(),
                next_revision: 1,
            })),
        }
    }

    pub async fn snapshot(&self) -> VpnSessionSnapshot {
        match self.port.snapshot().await {
            Ok(snapshot) => self.store(snapshot).await,
            Err(error) => {
                let failure = Failure::from(error);
                self.store(VpnSessionSnapshot::failed(
                    self.next_revision().await,
                    failure,
                ))
                .await
            }
        }
    }

    /// Ask the native host to obtain/verify VpnService consent and promote its
    /// service to the foreground. Establishing the TUN FD remains a native
    /// callback followed by the start operation.
    pub async fn request_start(&self) -> Result<VpnSessionSnapshot, Failure> {
        let snapshot = self.port.request_start().await.map_err(Failure::from)?;
        self.reject_terminal_state(snapshot).await
    }

    /// Apply native Builder configuration before the host establishes the TUN
    /// interface. The later FD callback only starts packet processing.
    pub async fn prepare(
        &self,
        configuration: VpnConfiguration,
    ) -> Result<VpnSessionSnapshot, Failure> {
        infiltrator_domain::vpn_policy::validate_configuration(&configuration)
            .map_err(|message| Failure::new(ErrorCode::InvalidInput, message, false))?;
        let snapshot = self
            .port
            .prepare(configuration)
            .await
            .map_err(Failure::from)?;
        self.reject_terminal_state(snapshot).await
    }

    /// Start packet processing after the native VpnService has established a
    /// tunnel FD. Both the request and the host-reported Running/foreground
    /// state are validated before success is returned.
    pub async fn start(&self, request: VpnStartRequest) -> Result<VpnSessionSnapshot, Failure> {
        infiltrator_domain::vpn_policy::validate_start_request(&request)
            .map_err(|message| Failure::new(ErrorCode::InvalidInput, message, false))?;
        let reported = self.port.start(request).await.map_err(Failure::from)?;
        let observed = self.port.snapshot().await.map_err(Failure::from)?;
        if reported.state != observed.state
            || reported.foreground != observed.foreground
            || observed.state != VpnSessionState::Running
            || !observed.foreground
        {
            let failure = Failure::new(
                ErrorCode::InvalidState,
                format!(
                    "VpnService start readback mismatch: reported={:?}, observed={:?}",
                    reported.state, observed.state
                ),
                true,
            );
            self.store(VpnSessionSnapshot::failed(
                self.next_revision().await,
                failure.clone(),
            ))
            .await;
            return Err(failure);
        }
        Ok(self.store(observed).await)
    }

    pub async fn stop(&self) -> Result<VpnSessionSnapshot, Failure> {
        let reported = self.port.stop().await.map_err(Failure::from)?;
        let observed = self.port.snapshot().await.map_err(Failure::from)?;
        if reported.state != observed.state
            || matches!(
                observed.state,
                VpnSessionState::Running | VpnSessionState::Starting
            )
        {
            let failure = Failure::new(
                ErrorCode::InvalidState,
                format!(
                    "VpnService stop readback mismatch: reported={:?}, observed={:?}",
                    reported.state, observed.state
                ),
                true,
            );
            self.store(VpnSessionSnapshot::failed(
                self.next_revision().await,
                failure.clone(),
            ))
            .await;
            return Err(failure);
        }
        Ok(self.store(observed).await)
    }

    pub async fn revoke(&self) -> Result<VpnSessionSnapshot, Failure> {
        let reported = self.port.revoke().await.map_err(Failure::from)?;
        let observed = self.port.snapshot().await.map_err(Failure::from)?;
        if reported.state != observed.state || observed.state != VpnSessionState::Revoked {
            let failure = Failure::new(
                ErrorCode::InvalidState,
                format!(
                    "VpnService revoke readback mismatch: reported={:?}, observed={:?}",
                    reported.state, observed.state
                ),
                true,
            );
            self.store(VpnSessionSnapshot::failed(
                self.next_revision().await,
                failure.clone(),
            ))
            .await;
            return Err(failure);
        }
        Ok(self.store(observed).await)
    }

    async fn reject_terminal_state(
        &self,
        snapshot: VpnSessionSnapshot,
    ) -> Result<VpnSessionSnapshot, Failure> {
        let snapshot = self.store(snapshot).await;
        match &snapshot.state {
            VpnSessionState::Unsupported { reason } => Err(Failure::unsupported(reason.clone())),
            VpnSessionState::Failed { failure } => Err(failure.clone()),
            _ => Ok(snapshot),
        }
    }

    async fn next_revision(&self) -> u64 {
        self.state.lock().await.next_revision
    }

    async fn store(&self, mut snapshot: VpnSessionSnapshot) -> VpnSessionSnapshot {
        let mut state = self.state.lock().await;
        snapshot.revision = state.next_revision;
        state.next_revision = state.next_revision.saturating_add(1);
        state.snapshot = snapshot.clone();
        snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::vpn::{VpnConfiguration, VpnRoute, VpnSessionState, VpnStartRequest};
    use infiltrator_ports::error::PortError;

    struct FakePort {
        snapshot: std::sync::Mutex<VpnSessionSnapshot>,
        starts: std::sync::Mutex<usize>,
    }

    #[async_trait]
    impl VpnServicePort for FakePort {
        async fn request_start(&self) -> Result<VpnSessionSnapshot, PortError> {
            Ok(VpnSessionSnapshot {
                state: VpnSessionState::Starting,
                foreground: true,
                revision: 0,
                ..VpnSessionSnapshot::default()
            })
        }

        async fn prepare(
            &self,
            _configuration: VpnConfiguration,
        ) -> Result<VpnSessionSnapshot, PortError> {
            Ok(VpnSessionSnapshot {
                state: VpnSessionState::Starting,
                foreground: true,
                revision: 0,
                ..VpnSessionSnapshot::default()
            })
        }

        async fn start(&self, request: VpnStartRequest) -> Result<VpnSessionSnapshot, PortError> {
            *self.starts.lock().expect("start counter lock") += 1;
            let snapshot = VpnSessionSnapshot::running(
                0,
                request.mtu,
                request.routes.len(),
                request.dns_servers,
                request.ipv6,
                true,
            );
            *self.snapshot.lock().expect("VPN snapshot lock") = snapshot.clone();
            Ok(snapshot)
        }

        async fn stop(&self) -> Result<VpnSessionSnapshot, PortError> {
            let snapshot = VpnSessionSnapshot {
                state: VpnSessionState::Stopped,
                revision: 0,
                ..VpnSessionSnapshot::default()
            };
            *self.snapshot.lock().expect("VPN snapshot lock") = snapshot.clone();
            Ok(snapshot)
        }

        async fn revoke(&self) -> Result<VpnSessionSnapshot, PortError> {
            let snapshot = VpnSessionSnapshot {
                state: VpnSessionState::Revoked,
                revision: 0,
                ..VpnSessionSnapshot::default()
            };
            *self.snapshot.lock().expect("VPN snapshot lock") = snapshot.clone();
            Ok(snapshot)
        }

        async fn snapshot(&self) -> Result<VpnSessionSnapshot, PortError> {
            Ok(self.snapshot.lock().expect("VPN snapshot lock").clone())
        }
    }

    fn request() -> VpnStartRequest {
        VpnStartRequest {
            tun_fd: 7,
            proxy_endpoint: "socks5://127.0.0.1:7891".to_owned(),
            mtu: 1500,
            routes: vec![VpnRoute {
                address: "0.0.0.0".to_owned(),
                prefix: 0,
                exclude: false,
            }],
            dns_servers: vec!["1.1.1.1".to_owned()],
            ipv6: true,
            foreground_requested: true,
        }
    }

    #[tokio::test]
    async fn application_requires_foreground_running_readback() {
        let port = Arc::new(FakePort {
            snapshot: std::sync::Mutex::new(VpnSessionSnapshot::default()),
            starts: std::sync::Mutex::new(0),
        });
        let application = VpnServiceApplication::new(port.clone());
        let requested = application.request_start().await.expect("request start");
        assert_eq!(requested.state, VpnSessionState::Starting);
        let prepared = application
            .prepare(request().configuration())
            .await
            .expect("VPN Builder prepare");
        assert_eq!(prepared.state, VpnSessionState::Starting);
        let started = application.start(request()).await.expect("VPN start");
        assert!(started.is_running());
        assert_eq!(*port.starts.lock().expect("start counter lock"), 1);
        let stopped = application.stop().await.expect("VPN stop");
        assert_eq!(stopped.state, VpnSessionState::Stopped);
        let revoked = application.revoke().await.expect("VPN revoke");
        assert_eq!(revoked.state, VpnSessionState::Revoked);
    }

    #[tokio::test]
    async fn invalid_fd_is_rejected_before_host_start() {
        let port = Arc::new(FakePort {
            snapshot: std::sync::Mutex::new(VpnSessionSnapshot::default()),
            starts: std::sync::Mutex::new(0),
        });
        let application = VpnServiceApplication::new(port.clone());
        let mut request = request();
        request.tun_fd = 0;
        let failure = application
            .start(request)
            .await
            .expect_err("invalid FD must fail");
        assert_eq!(failure.code, ErrorCode::InvalidInput);
        assert_eq!(*port.starts.lock().expect("start counter lock"), 0);
    }
}

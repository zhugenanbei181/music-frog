//! Runtime-neutral VpnService lifecycle use-case.

use infiltrator_contract::android_control::VpnAuthorizationState;
use infiltrator_contract::vpn::{
    PreparedVpnConfiguration, VpnSessionSnapshot, VpnSessionState, VpnStartRequest,
    VpnSurfaceObservation,
};
use infiltrator_domain::vpn_policy::{validate_configuration, validate_start_request};
use infiltrator_ports::vpn_service::{VpnServicePort, VpnStartReceipt};
use lurker_core_contract::error::{ErrorCode, Failure};
use std::sync::Arc;

#[derive(Clone)]
pub struct VpnServiceApplication {
    port: Arc<dyn VpnServicePort>,
}

impl VpnServiceApplication {
    pub fn new(port: Arc<dyn VpnServicePort>) -> Self {
        Self { port }
    }

    /// Return the complete read model authored by the platform VPN owner.
    /// This method never projects lifecycle booleans into a local snapshot.
    pub async fn owner_observation(&self) -> VpnSurfaceObservation {
        self.port.owner_observation().await
    }

    /// Return the port's owner-authored revision. An unavailable owner remains
    /// unavailable instead of receiving a local synthetic revision.
    pub async fn snapshot(&self) -> VpnSessionSnapshot {
        match self.port.snapshot().await {
            Ok(snapshot) => snapshot,
            Err(error) => VpnSessionSnapshot::unsupported(
                0,
                format!("VPN owner snapshot is unavailable: {error}"),
            ),
        }
    }

    pub async fn authorization_state(&self) -> Result<VpnAuthorizationState, Failure> {
        self.port.authorization_state().await.map_err(Failure::from)
    }

    pub async fn request_start(&self) -> Result<VpnSessionSnapshot, Failure> {
        let snapshot = self.port.request_start().await.map_err(Failure::from)?;
        Self::reject_terminal_state(snapshot)
    }

    pub async fn prepare(
        &self,
        prepared: PreparedVpnConfiguration,
    ) -> Result<VpnSessionSnapshot, Failure> {
        if prepared.preparation_id.value() == 0 || prepared.vpn_generation.value() == 0 {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "prepared VPN configuration requires a non-zero transaction identity",
                false,
            ));
        }
        validate_configuration(&prepared.configuration)
            .map_err(|message| Failure::new(ErrorCode::InvalidInput, message, false))?;
        if !prepared
            .core_scope
            .session_token
            .is_some_and(|token| token.is_valid())
        {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "prepared VPN configuration has no active Core session token",
                true,
            ));
        }
        let snapshot = self.port.prepare(prepared).await.map_err(Failure::from)?;
        Self::reject_terminal_state(snapshot)
    }

    pub async fn start(&self, request: VpnStartRequest) -> Result<VpnSessionSnapshot, Failure> {
        validate_start_request(&request)
            .map_err(|message| Failure::new(ErrorCode::InvalidInput, message, false))?;
        let receipt = self.port.start(request).await.map_err(Failure::from)?;
        let snapshot = match receipt {
            VpnStartReceipt::Started(snapshot) | VpnStartReceipt::AlreadyRunning(snapshot) => {
                snapshot
            }
        };
        let observed = self.port.snapshot().await.map_err(Failure::from)?;
        if snapshot.state != observed.state
            || snapshot.foreground != observed.foreground
            || !snapshot.is_running()
            || !observed.is_running()
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                format!(
                    "VpnService typed start receipt/readback did not agree: receipt={:?}, owner={:?}",
                    snapshot.state, observed.state
                ),
                true,
            ));
        }
        Ok(observed)
    }

    pub async fn stop(&self) -> Result<VpnSessionSnapshot, Failure> {
        let snapshot = self.port.stop().await.map_err(Failure::from)?;
        if !matches!(
            snapshot.state,
            VpnSessionState::Stopped | VpnSessionState::Revoked
        ) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                format!(
                    "VpnService Stop IPC returned a nonterminal snapshot: {:?}",
                    snapshot.state
                ),
                true,
            ));
        }
        Ok(snapshot)
    }

    pub async fn revoke(&self) -> Result<VpnSessionSnapshot, Failure> {
        let snapshot = self.port.revoke().await.map_err(Failure::from)?;
        if snapshot.state != VpnSessionState::Revoked {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                format!(
                    "VpnService Revoke readback was not Revoked: {:?}",
                    snapshot.state
                ),
                true,
            ));
        }
        Ok(snapshot)
    }

    fn reject_terminal_state(snapshot: VpnSessionSnapshot) -> Result<VpnSessionSnapshot, Failure> {
        match &snapshot.state {
            VpnSessionState::Unsupported { reason } => Err(Failure::unsupported(reason.clone())),
            VpnSessionState::Failed { failure } => Err(failure.clone()),
            _ => Ok(snapshot),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::vpn::{
        PreparedVpnConfiguration, VpnConfiguration, VpnGeneration, VpnPreparationId, VpnRoute,
        VpnSessionState, VpnStartRequest,
    };
    use infiltrator_ports::error::PortError;
    use lurker_core_contract::identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
    };
    use lurker_core_contract::session::SessionToken;
    #[cfg(test)]
    use std::sync;

    struct FakePort {
        snapshot: sync::Mutex<VpnSessionSnapshot>,
        starts: sync::Mutex<usize>,
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
            _prepared: PreparedVpnConfiguration,
        ) -> Result<VpnSessionSnapshot, PortError> {
            Ok(VpnSessionSnapshot {
                state: VpnSessionState::Starting,
                foreground: true,
                revision: 0,
                ..VpnSessionSnapshot::default()
            })
        }

        async fn start(&self, request: VpnStartRequest) -> Result<VpnStartReceipt, PortError> {
            *self.starts.lock().expect("start counter lock") += 1;
            let configuration = &request.prepared.configuration;
            let snapshot = VpnSessionSnapshot::running(
                0,
                configuration.mtu,
                configuration.routes.len(),
                configuration.dns_servers.clone(),
                configuration.ipv6,
                true,
            );
            *self.snapshot.lock().expect("VPN snapshot lock") = snapshot.clone();
            Ok(VpnStartReceipt::Started(snapshot))
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

    fn core_scope() -> CoreScope {
        CoreScope {
            identity: CoreIdentity::new(
                EngineId::new("mihomo").expect("engine ID"),
                CoreInstanceId::new("primary").expect("instance ID"),
                Some(CoreVersion::new("1.19.12").expect("Core version")),
            ),
            generation: CoreGeneration::new(9),
            session_token: Some(SessionToken::new(71)),
        }
    }

    fn configuration() -> VpnConfiguration {
        VpnConfiguration {
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

    fn prepared_configuration() -> PreparedVpnConfiguration {
        PreparedVpnConfiguration {
            preparation_id: VpnPreparationId::new(5),
            vpn_generation: VpnGeneration::new(3),
            owner_data_revision: 44,
            core_scope: core_scope(),
            configuration: configuration(),
        }
    }

    fn request() -> VpnStartRequest {
        VpnStartRequest {
            tun_fd: 7,
            prepared: prepared_configuration(),
        }
    }

    #[tokio::test]
    async fn application_accepts_typed_foreground_start_receipt() {
        let port = Arc::new(FakePort {
            snapshot: sync::Mutex::new(VpnSessionSnapshot::default()),
            starts: sync::Mutex::new(0),
        });
        let application = VpnServiceApplication::new(port.clone());
        let requested = application.request_start().await.expect("request start");
        assert_eq!(requested.state, VpnSessionState::Starting);
        let prepared = application
            .prepare(prepared_configuration())
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

    struct DelayedOwnerPublicationPort {
        start_receipts: sync::Mutex<usize>,
        snapshot_reads: sync::Mutex<usize>,
    }

    #[async_trait]
    impl VpnServicePort for DelayedOwnerPublicationPort {
        async fn request_start(&self) -> Result<VpnSessionSnapshot, PortError> {
            Ok(VpnSessionSnapshot {
                state: VpnSessionState::Starting,
                foreground: true,
                ..VpnSessionSnapshot::default()
            })
        }

        async fn prepare(
            &self,
            _prepared: PreparedVpnConfiguration,
        ) -> Result<VpnSessionSnapshot, PortError> {
            Ok(VpnSessionSnapshot {
                state: VpnSessionState::Starting,
                foreground: true,
                ..VpnSessionSnapshot::default()
            })
        }

        async fn start(&self, request: VpnStartRequest) -> Result<VpnStartReceipt, PortError> {
            *self.start_receipts.lock().expect("start receipt count") += 1;
            let configuration = &request.prepared.configuration;
            Ok(VpnStartReceipt::Started(VpnSessionSnapshot::running(
                0,
                configuration.mtu,
                configuration.routes.len(),
                configuration.dns_servers.clone(),
                configuration.ipv6,
                true,
            )))
        }

        async fn stop(&self) -> Result<VpnSessionSnapshot, PortError> {
            Ok(VpnSessionSnapshot {
                state: VpnSessionState::Stopped,
                ..VpnSessionSnapshot::default()
            })
        }

        async fn revoke(&self) -> Result<VpnSessionSnapshot, PortError> {
            Ok(VpnSessionSnapshot {
                state: VpnSessionState::Revoked,
                ..VpnSessionSnapshot::default()
            })
        }

        async fn snapshot(&self) -> Result<VpnSessionSnapshot, PortError> {
            *self.snapshot_reads.lock().expect("snapshot read count") += 1;
            // Kotlin publishes owner `Running` only after this native start
            // callback returns to the service executor.
            Ok(VpnSessionSnapshot {
                state: VpnSessionState::Starting,
                foreground: true,
                ..VpnSessionSnapshot::default()
            })
        }
    }

    #[tokio::test]
    async fn start_receipt_with_unpublished_owner_running_state_fails_closed() {
        let port = Arc::new(DelayedOwnerPublicationPort {
            start_receipts: sync::Mutex::new(0),
            snapshot_reads: sync::Mutex::new(0),
        });
        let application = VpnServiceApplication::new(port.clone());

        let failure = application
            .start(request())
            .await
            .expect_err("unconfirmed owner readback must fail closed");

        assert_eq!(failure.code, ErrorCode::InvalidState);
        assert!(
            failure
                .message
                .contains("VpnService typed start receipt/readback")
        );
        assert_eq!(*port.start_receipts.lock().expect("start receipt count"), 1);
        assert_eq!(
            *port.snapshot_reads.lock().expect("snapshot read count"),
            1,
            "start completion must read the owner state and reject an unpublished Running state",
        );
    }

    struct NonterminalStopReadbackPort {
        snapshot: VpnSessionSnapshot,
        events: sync::Mutex<Vec<&'static str>>,
    }

    #[async_trait]
    impl VpnServicePort for NonterminalStopReadbackPort {
        async fn request_start(&self) -> Result<VpnSessionSnapshot, PortError> {
            Ok(self.snapshot.clone())
        }

        async fn prepare(
            &self,
            _prepared: PreparedVpnConfiguration,
        ) -> Result<VpnSessionSnapshot, PortError> {
            Ok(self.snapshot.clone())
        }

        async fn start(&self, _request: VpnStartRequest) -> Result<VpnStartReceipt, PortError> {
            Err(PortError::Failed("start is outside this test".to_owned()))
        }

        async fn stop(&self) -> Result<VpnSessionSnapshot, PortError> {
            self.events
                .lock()
                .expect("stop event lock")
                .push("owner_stop");
            Ok(self.snapshot.clone())
        }

        async fn revoke(&self) -> Result<VpnSessionSnapshot, PortError> {
            Ok(self.snapshot.clone())
        }

        async fn snapshot(&self) -> Result<VpnSessionSnapshot, PortError> {
            self.events
                .lock()
                .expect("stop event lock")
                .push("owner_readback");
            Ok(self.snapshot.clone())
        }
    }

    #[tokio::test]
    async fn dedicated_stop_receipt_must_be_a_terminal_owner_state() {
        let port = Arc::new(NonterminalStopReadbackPort {
            snapshot: VpnSessionSnapshot {
                state: VpnSessionState::Stopping,
                foreground: true,
                ..VpnSessionSnapshot::default()
            },
            events: sync::Mutex::new(Vec::new()),
        });
        let application = VpnServiceApplication::new(port.clone());

        let failure = application
            .stop()
            .await
            .expect_err("a matching but nonterminal owner state is not a completed stop");

        assert_eq!(failure.code, ErrorCode::InvalidState);
        assert!(
            failure
                .message
                .contains("VpnService Stop IPC returned a nonterminal snapshot")
        );
        assert_eq!(
            *port.events.lock().expect("stop event lock"),
            vec!["owner_stop"],
            "the Stop receipt already carries the owner snapshot",
        );
    }

    struct OwnerUnavailableStopPort {
        events: sync::Mutex<Vec<&'static str>>,
    }

    #[async_trait]
    impl VpnServicePort for OwnerUnavailableStopPort {
        async fn request_start(&self) -> Result<VpnSessionSnapshot, PortError> {
            Err(PortError::Failed("unused start".to_owned()))
        }

        async fn prepare(
            &self,
            _prepared: PreparedVpnConfiguration,
        ) -> Result<VpnSessionSnapshot, PortError> {
            Err(PortError::Failed("unused prepare".to_owned()))
        }

        async fn start(&self, _request: VpnStartRequest) -> Result<VpnStartReceipt, PortError> {
            Err(PortError::Failed("unused start".to_owned()))
        }

        async fn stop(&self) -> Result<VpnSessionSnapshot, PortError> {
            self.events
                .lock()
                .expect("stop event lock")
                .push("owner_stop");
            Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "VpnService owner is unavailable",
                true,
            )))
        }

        async fn revoke(&self) -> Result<VpnSessionSnapshot, PortError> {
            Err(PortError::Failed("unused revoke".to_owned()))
        }

        async fn snapshot(&self) -> Result<VpnSessionSnapshot, PortError> {
            self.events
                .lock()
                .expect("stop event lock")
                .push("owner_readback");
            Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "VpnService owner is unavailable",
                true,
            )))
        }
    }

    #[tokio::test]
    async fn owner_stop_failure_is_not_projected_as_running() {
        let port = Arc::new(OwnerUnavailableStopPort {
            events: sync::Mutex::new(Vec::new()),
        });
        let application = VpnServiceApplication::new(port.clone());

        let failure = application
            .stop()
            .await
            .expect_err("unavailable owner must remain a typed stop failure");
        assert_eq!(failure.code, ErrorCode::NotReady);
        assert_eq!(
            *port.events.lock().expect("stop event lock"),
            vec!["owner_stop"],
            "owner failure prevents a readback request and the application records Failed",
        );
        assert!(matches!(
            application.snapshot().await.state,
            VpnSessionState::Unsupported { .. }
        ));
    }

    #[tokio::test]
    async fn invalid_fd_is_rejected_before_host_start() {
        let port = Arc::new(FakePort {
            snapshot: sync::Mutex::new(VpnSessionSnapshot::default()),
            starts: sync::Mutex::new(0),
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

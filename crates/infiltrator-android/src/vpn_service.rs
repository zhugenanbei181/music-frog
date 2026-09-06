//! Android VpnService host adapter.
//!
//! Kotlin owns Context, VpnService.prepare, Builder.establish and the
//! foreground notification. Rust owns request validation, tun2proxy packet
//! processing and the typed lifecycle readback.

use async_trait::async_trait;
#[cfg(not(target_os = "android"))]
use infiltrator_contract::capability::Capability;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::vpn::{
    VpnConfiguration, VpnSessionSnapshot, VpnSessionState, VpnStartRequest,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::vpn_service::VpnServicePort;
use mihomo_platform::android_bridge::{AndroidBridge, get_android_bridge};
use std::sync::{Arc, Mutex, OnceLock};

static SHARED_VPN_STATE: OnceLock<Arc<Mutex<VpnSessionSnapshot>>> = OnceLock::new();

#[allow(dead_code)]
#[derive(Clone)]
pub struct AndroidVpnServicePort {
    state: Arc<Mutex<VpnSessionSnapshot>>,
}

impl Default for AndroidVpnServicePort {
    fn default() -> Self {
        Self::shared()
    }
}

#[allow(dead_code)]
impl AndroidVpnServicePort {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(VpnSessionSnapshot::default())),
        }
    }

    pub fn shared() -> Self {
        Self {
            state: SHARED_VPN_STATE
                .get_or_init(|| Arc::new(Mutex::new(VpnSessionSnapshot::default())))
                .clone(),
        }
    }

    fn bridge() -> Result<Arc<dyn AndroidBridge>, PortError> {
        get_android_bridge()
            .ok_or_else(|| PortError::Failed("android bridge is not ready".to_owned()))
    }

    fn store(&self, mut snapshot: VpnSessionSnapshot) -> VpnSessionSnapshot {
        let mut state = self.state.lock().expect("VPN session state lock");
        snapshot.revision = state.revision.saturating_add(1).max(1);
        *state = snapshot.clone();
        snapshot
    }

    fn store_worker_exit(&self, exit_code: i32) {
        let mut state = self.state.lock().expect("VPN session state lock");
        state.revision = state.revision.saturating_add(1).max(1);
        if exit_code == 0 {
            state.state = VpnSessionState::Stopped;
            state.foreground = false;
        } else {
            state.state = VpnSessionState::Failed {
                failure: Failure::new(
                    ErrorCode::InvalidState,
                    format!("tun2proxy exited with code {exit_code}"),
                    true,
                ),
            };
            state.foreground = false;
        }
    }

    #[cfg(target_os = "android")]
    async fn start_tun2proxy(
        &self,
        request: VpnStartRequest,
        bridge: Arc<dyn AndroidBridge>,
    ) -> Result<VpnSessionSnapshot, PortError> {
        if !bridge
            .vpn_is_foreground()
            .await
            .map_err(|error| PortError::Failed(error.to_string()))?
        {
            return Err(PortError::PermissionDenied(
                "VpnService is not running as a foreground service".to_owned(),
            ));
        }

        let proxy = tun2proxy::ArgProxy::try_from(request.proxy_endpoint.as_str())
            .map_err(|error| PortError::Failed(format!("invalid tun2proxy endpoint: {error}")))?;
        let mut args = tun2proxy::Args::default();
        args.proxy = proxy;
        args.tun_fd = Some(request.tun_fd);
        args.close_fd_on_drop = Some(true);
        args.ipv6_enabled = request.ipv6;
        args.setup = false;
        if let Some(server) = request.dns_servers.first()
            && let Ok(address) = server.parse()
        {
            args.dns_addr = address;
        }
        let mtu = u16::try_from(request.mtu)
            .map_err(|_| PortError::Failed("VPN MTU does not fit tun2proxy".to_owned()))?;
        let running = self.store(VpnSessionSnapshot::running(
            0,
            request.mtu,
            request.routes.len(),
            request.dns_servers.clone(),
            request.ipv6,
            true,
        ));
        let state = self.clone();
        let spawn_result = std::thread::Builder::new()
            .name("infiltrator-android-vpn".to_owned())
            .spawn(move || {
                let exit_code = tun2proxy::mobile_run(args, mtu, false);
                state.store_worker_exit(exit_code);
            });
        if let Err(error) = spawn_result {
            let failure = Failure::new(
                ErrorCode::Internal,
                format!("failed to spawn tun2proxy: {error}"),
                true,
            );
            self.store(VpnSessionSnapshot::failed(0, failure.clone()));
            return Err(PortError::Io(failure.message));
        }

        Ok(running)
    }
}

#[async_trait]
impl VpnServicePort for AndroidVpnServicePort {
    async fn request_start(&self) -> Result<VpnSessionSnapshot, PortError> {
        #[cfg(target_os = "android")]
        {
            let bridge = Self::bridge()?;
            let accepted = bridge
                .vpn_start()
                .await
                .map_err(|error| PortError::Failed(error.to_string()))?;
            if !accepted {
                return Ok(self.store(VpnSessionSnapshot {
                    state: VpnSessionState::PermissionRequired,
                    ..VpnSessionSnapshot::default()
                }));
            }
            let foreground = bridge
                .vpn_is_foreground()
                .await
                .map_err(|error| PortError::Failed(error.to_string()))?;
            return Ok(self.store(VpnSessionSnapshot {
                state: VpnSessionState::Starting,
                foreground,
                ..VpnSessionSnapshot::default()
            }));
        }

        #[cfg(not(target_os = "android"))]
        {
            Err(PortError::unsupported(
                Capability::VpnService,
                "Android VpnService is unavailable on this host",
            ))
        }
    }

    async fn prepare(
        &self,
        configuration: VpnConfiguration,
    ) -> Result<VpnSessionSnapshot, PortError> {
        #[cfg(target_os = "android")]
        {
            let bridge = Self::bridge()?;
            let configuration_json = serde_json::to_string(&configuration).map_err(|error| {
                PortError::Failed(format!("serialize VPN configuration failed: {error}"))
            })?;
            if !bridge
                .vpn_apply_configuration(&configuration_json)
                .await
                .map_err(|error| PortError::Failed(error.to_string()))?
            {
                return Err(PortError::Failed(
                    "native VpnService rejected route/DNS/MTU configuration".to_owned(),
                ));
            }
            let foreground = bridge
                .vpn_is_foreground()
                .await
                .map_err(|error| PortError::Failed(error.to_string()))?;
            return Ok(self.store(VpnSessionSnapshot {
                state: VpnSessionState::Starting,
                foreground,
                mtu: Some(configuration.mtu),
                route_count: configuration.routes.len(),
                dns_servers: configuration.dns_servers,
                ipv6: configuration.ipv6,
                ..VpnSessionSnapshot::default()
            }));
        }

        #[cfg(not(target_os = "android"))]
        {
            let _ = configuration;
            Err(PortError::unsupported(
                Capability::VpnService,
                "Android VpnService is unavailable on this host",
            ))
        }
    }

    async fn start(&self, request: VpnStartRequest) -> Result<VpnSessionSnapshot, PortError> {
        #[cfg(target_os = "android")]
        {
            let current = self.state.lock().expect("VPN session state lock").clone();
            if current.state == VpnSessionState::Running && current.foreground {
                return Ok(current);
            }
            return self.start_tun2proxy(request, Self::bridge()?).await;
        }

        #[cfg(not(target_os = "android"))]
        {
            let _ = request;
            Err(PortError::unsupported(
                Capability::VpnService,
                "Android VpnService is unavailable on this host",
            ))
        }
    }

    async fn stop(&self) -> Result<VpnSessionSnapshot, PortError> {
        #[cfg(target_os = "android")]
        {
            let bridge = Self::bridge()?;
            let exit_code = tun2proxy::mobile_stop();
            if exit_code != 0 {
                log::debug!("tun2proxy stop returned {exit_code}; continuing native stop");
            }
            let native_stopped = bridge
                .vpn_stop()
                .await
                .map_err(|error| PortError::Failed(error.to_string()))?;
            if !native_stopped
                && bridge
                    .vpn_is_running()
                    .await
                    .map_err(|error| PortError::Failed(error.to_string()))?
            {
                return Err(PortError::Failed(
                    "native VpnService remained active after stop".to_owned(),
                ));
            }
            return Ok(self.store(VpnSessionSnapshot {
                state: VpnSessionState::Stopped,
                ..VpnSessionSnapshot::default()
            }));
        }

        #[cfg(not(target_os = "android"))]
        {
            Err(PortError::unsupported(
                Capability::VpnService,
                "Android VpnService is unavailable on this host",
            ))
        }
    }

    async fn revoke(&self) -> Result<VpnSessionSnapshot, PortError> {
        #[cfg(target_os = "android")]
        {
            let stopped = self.stop().await?;
            return Ok(self.store(VpnSessionSnapshot {
                state: VpnSessionState::Revoked,
                foreground: false,
                mtu: stopped.mtu,
                route_count: stopped.route_count,
                dns_servers: stopped.dns_servers,
                ipv6: stopped.ipv6,
                ..VpnSessionSnapshot::default()
            }));
        }

        #[cfg(not(target_os = "android"))]
        {
            Err(PortError::unsupported(
                Capability::VpnService,
                "Android VpnService is unavailable on this host",
            ))
        }
    }

    async fn snapshot(&self) -> Result<VpnSessionSnapshot, PortError> {
        #[cfg(target_os = "android")]
        {
            let bridge = Self::bridge()?;
            let running = bridge
                .vpn_is_running()
                .await
                .map_err(|error| PortError::Failed(error.to_string()))?;
            let foreground = if running {
                bridge
                    .vpn_is_foreground()
                    .await
                    .map_err(|error| PortError::Failed(error.to_string()))?
            } else {
                false
            };
            let current = self.state.lock().expect("VPN session state lock").clone();
            if !running {
                return Ok(self.store(VpnSessionSnapshot {
                    state: if matches!(current.state, VpnSessionState::Revoked) {
                        VpnSessionState::Revoked
                    } else {
                        VpnSessionState::Stopped
                    },
                    foreground: false,
                    mtu: current.mtu,
                    route_count: current.route_count,
                    dns_servers: current.dns_servers,
                    ipv6: current.ipv6,
                    ..VpnSessionSnapshot::default()
                }));
            }
            if !foreground {
                return Ok(self.store(VpnSessionSnapshot::failed(
                    0,
                    Failure::new(
                        ErrorCode::InvalidState,
                        "VpnService is running without foreground protection",
                        true,
                    ),
                )));
            }
            return Ok(self.store(VpnSessionSnapshot {
                state: current.state.clone(),
                foreground,
                ..current
            }));
        }

        #[cfg(not(target_os = "android"))]
        {
            Err(PortError::unsupported(
                Capability::VpnService,
                "Android VpnService is unavailable on this host",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::vpn::{VpnRoute, VpnStartRequest};

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

    #[test]
    fn shared_port_starts_with_an_idle_snapshot() {
        let port = AndroidVpnServicePort::new();
        let snapshot = port.state.lock().expect("VPN session state lock").clone();
        assert_eq!(snapshot.state, VpnSessionState::Idle);
        assert!(!snapshot.foreground);
    }

    #[test]
    fn local_state_preserves_route_and_dns_metadata() {
        let port = AndroidVpnServicePort::new();
        let request = request();
        let snapshot = port.store(VpnSessionSnapshot::running(
            0,
            request.mtu,
            request.routes.len(),
            request.dns_servers,
            true,
            true,
        ));
        assert_eq!(snapshot.route_count, 1);
        assert_eq!(snapshot.dns_servers, vec!["1.1.1.1".to_owned()]);
        assert!(snapshot.is_running());
    }

    #[cfg(not(target_os = "android"))]
    #[tokio::test]
    async fn non_android_host_returns_typed_vpn_unsupported() {
        let port = AndroidVpnServicePort::new();
        let failure = port
            .request_start()
            .await
            .expect_err("desktop test host must not emulate Android VpnService");
        assert!(matches!(
            failure,
            PortError::Unsupported {
                capability: Capability::VpnService,
                ..
            }
        ));
    }
}

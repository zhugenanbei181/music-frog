//! Host-service state reads kept outside the large surface assembler.

use infiltrator_contract::controller::{ControllerAuthSnapshot, ControllerAuthStatus};

use super::ApplicationSurfaceReader;

impl ApplicationSurfaceReader {
    pub(super) async fn read_controller_auth(&self) -> ControllerAuthSnapshot {
        let Some(source) = &self.endpoint_source else {
            return ControllerAuthSnapshot::default();
        };
        match source.resolve().await {
            Ok(endpoint) => ControllerAuthSnapshot {
                status: if endpoint
                    .secret
                    .as_deref()
                    .is_some_and(|secret| !secret.trim().is_empty())
                {
                    ControllerAuthStatus::Secured
                } else {
                    ControllerAuthStatus::Missing
                },
            },
            Err(_) => ControllerAuthSnapshot {
                status: ControllerAuthStatus::Unavailable,
            },
        }
    }

    pub(super) async fn read_service_mode(
        &self,
    ) -> infiltrator_contract::service_mode::ServiceModeSnapshot {
        match &self.service_mode {
            Some(application) => application.snapshot().await.unwrap_or_default(),
            None => Default::default(),
        }
    }

    pub(super) async fn read_port_conflicts(
        &self,
    ) -> infiltrator_contract::port_conflict::PortConflictSnapshot {
        match &self.port_conflicts {
            Some(application) => application.snapshot().await.unwrap_or_default(),
            None => Default::default(),
        }
    }

    pub(super) async fn read_resources(
        &self,
    ) -> infiltrator_contract::resources::CoreResourceSnapshot {
        match &self.resources {
            Some(application) => application.poll().await.unwrap_or_default(),
            None => Default::default(),
        }
    }

    pub(super) async fn read_offline_startup(
        &self,
    ) -> infiltrator_contract::offline_startup::OfflineStartupSnapshot {
        match &self.offline_startup {
            Some(application) => application.snapshot().await,
            None => Default::default(),
        }
    }

    pub(super) async fn read_mtu(
        &self,
    ) -> infiltrator_contract::mtu::MtuNegotiationSnapshot {
        match &self.mtu {
            Some(application) => application.probe_cached().await,
            None => Default::default(),
        }
    }

    pub(super) async fn read_system_proxy(
        &self,
    ) -> infiltrator_contract::system_proxy::SystemProxySnapshot {
        match &self.system_proxy {
            Some(application) => application.snapshot_cached().await,
            None => Default::default(),
        }
    }

    pub(super) fn read_system_proxy_recovery(
        &self,
    ) -> infiltrator_contract::system_proxy::SystemProxyRecoverySnapshot {
        match &self.system_proxy {
            Some(application) => application.recovery_snapshot(),
            None => Default::default(),
        }
    }

    pub(super) async fn read_uwp_loopback(
        &self,
    ) -> infiltrator_contract::uwp::UwpLoopbackSnapshot {
        match &self.uwp_loopback {
            Some(application) => application.snapshot_cached().await,
            None => Default::default(),
        }
    }

    pub(super) async fn read_pac(&self) -> infiltrator_contract::pac::PacSnapshot {
        match &self.pac {
            Some(application) => application.snapshot().await,
            None => Default::default(),
        }
    }

    pub(super) async fn read_network_roaming(
        &self,
    ) -> infiltrator_contract::network_roaming::NetworkRoamingSnapshot {
        match &self.network_roaming {
            Some(application) => application.refresh().await,
            None => match self
                .capabilities
                .availability(infiltrator_contract::capability::Capability::NetworkRoaming)
            {
                infiltrator_contract::capability::Availability::Unsupported { reason } => {
                    infiltrator_contract::network_roaming::NetworkRoamingSnapshot::unsupported(
                        0, reason,
                    )
                }
                _ => Default::default(),
            },
        }
    }

    pub(super) async fn read_vpn(
        &self,
    ) -> infiltrator_contract::vpn::VpnSessionSnapshot {
        match &self.vpn {
            Some(application) => application.snapshot().await,
            None => match self
                .capabilities
                .availability(infiltrator_contract::capability::Capability::VpnService)
            {
                infiltrator_contract::capability::Availability::Unsupported { reason } => {
                    infiltrator_contract::vpn::VpnSessionSnapshot::unsupported(0, reason)
                }
                _ => Default::default(),
            },
        }
    }
}

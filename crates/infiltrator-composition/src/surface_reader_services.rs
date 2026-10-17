//! Host-service state reads kept outside the large surface assembler.

use super::ProductSurfaceReader;
use infiltrator_contract::android_control::VpnAuthorizationState;
use infiltrator_contract::capability::Availability;
use infiltrator_contract::controller::{ControllerAuthSnapshot, ControllerAuthStatus};
use infiltrator_contract::mtu::MtuNegotiationSnapshot;
use infiltrator_contract::network_roaming::NetworkRoamingSnapshot;
use infiltrator_contract::offline_startup::OfflineStartupSnapshot;
use infiltrator_contract::pac::PacSnapshot;
use infiltrator_contract::port_conflict::PortConflictSnapshot;
use infiltrator_contract::privileged_network::PrivilegedNetworkSnapshot;
use infiltrator_contract::resources::CoreResourceSnapshot;
use infiltrator_contract::runtime_status::VpnAuthorizationObservation;
use infiltrator_contract::service_mode::ServiceModeSnapshot;
use infiltrator_contract::surface::HostKind;
use infiltrator_contract::system_proxy::{SystemProxyRecoverySnapshot, SystemProxySnapshot};
use infiltrator_contract::uwp::UwpLoopbackSnapshot;
use infiltrator_contract::vpn::VpnSurfaceObservation;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};

impl ProductSurfaceReader {
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

    pub(super) async fn read_service_mode(&self) -> ServiceModeSnapshot {
        match &self.service_mode {
            Some(application) => application.snapshot().await.unwrap_or_default(),
            None => Default::default(),
        }
    }

    pub(super) async fn read_port_conflicts(&self) -> PortConflictSnapshot {
        match &self.port_conflicts {
            Some(application) => application.snapshot().await.unwrap_or_default(),
            None => Default::default(),
        }
    }

    pub(super) async fn read_resources(&self) -> CoreResourceSnapshot {
        match &self.resources {
            Some(application) => application.poll().await.unwrap_or_default(),
            None => Default::default(),
        }
    }

    pub(super) async fn read_offline_startup(&self) -> OfflineStartupSnapshot {
        match &self.offline_startup {
            Some(application) => application.snapshot().await,
            None => Default::default(),
        }
    }

    pub(super) async fn read_mtu(&self) -> MtuNegotiationSnapshot {
        match &self.mtu {
            Some(application) => application.probe_cached().await,
            None => Default::default(),
        }
    }

    pub(super) async fn read_system_proxy(&self) -> SystemProxySnapshot {
        match &self.system_proxy {
            Some(application) => application.snapshot_cached().await,
            None => Default::default(),
        }
    }

    pub(super) fn read_system_proxy_recovery(&self) -> SystemProxyRecoverySnapshot {
        match &self.system_proxy {
            Some(application) => application.recovery_snapshot(),
            None => Default::default(),
        }
    }

    pub(super) async fn read_uwp_loopback(&self) -> UwpLoopbackSnapshot {
        match &self.uwp_loopback {
            Some(application) => application.snapshot_cached().await,
            None => Default::default(),
        }
    }

    pub(super) async fn read_pac(&self) -> PacSnapshot {
        match &self.pac {
            Some(application) => application.snapshot().await,
            None => Default::default(),
        }
    }

    pub(super) async fn read_network_roaming(&self) -> NetworkRoamingSnapshot {
        match &self.network_roaming {
            Some(application) => application.refresh().await,
            None => match self.capabilities.availability(Capability::NetworkRoaming) {
                Availability::Unsupported { reason } => {
                    NetworkRoamingSnapshot::unsupported(0, reason)
                }
                _ => Default::default(),
            },
        }
    }

    pub(super) async fn read_vpn(&self) -> VpnSurfaceObservation {
        match &self.vpn {
            Some(application) => application.owner_observation().await,
            None => match self.capabilities.availability(Capability::VpnService) {
                Availability::Unsupported { reason } => {
                    VpnSurfaceObservation::Unsupported { reason }
                }
                _ => VpnSurfaceObservation::NotApplicable,
            },
        }
    }

    pub(super) async fn read_vpn_authorization(&self) -> VpnAuthorizationObservation {
        if self.capabilities.host != HostKind::Android {
            return VpnAuthorizationObservation::NotApplicable;
        }
        match &self.vpn {
            Some(application) => match application.authorization_state().await {
                Ok(VpnAuthorizationState::Granted) => VpnAuthorizationObservation::Granted,
                Ok(VpnAuthorizationState::Required) => VpnAuthorizationObservation::Required,
                Err(failure) if failure.code == ErrorCode::Unsupported => {
                    VpnAuthorizationObservation::Unsupported {
                        reason: failure.message,
                    }
                }
                Err(failure) => VpnAuthorizationObservation::Unavailable { failure },
            },
            None => match self.capabilities.availability(Capability::VpnService) {
                Availability::Unsupported { reason } => {
                    VpnAuthorizationObservation::Unsupported { reason }
                }
                Availability::Unavailable { reason } => VpnAuthorizationObservation::Unavailable {
                    failure: Failure::new(ErrorCode::NotReady, reason, true),
                },
                Availability::Supported | Availability::Experimental => {
                    VpnAuthorizationObservation::Unavailable {
                        failure: Failure::new(
                            ErrorCode::NotReady,
                            "Android VPN authorization port is not composed",
                            true,
                        ),
                    }
                }
            },
        }
    }

    pub(super) async fn read_privileged_network(&self) -> PrivilegedNetworkSnapshot {
        match &self.privileged_network {
            Some(application) => application.snapshot().await,
            None => PrivilegedNetworkSnapshot::unsupported(
                0,
                "privileged network regression port is not composed for this host",
            ),
        }
    }
}

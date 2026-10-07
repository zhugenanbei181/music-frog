//! Default shared Settings projection used by demo and empty compositions.

use super::settings_core::SettingsProjection;
use infiltrator_contract::lan::DEFAULT_BIND_ADDRESS;
use infiltrator_contract::network_roaming::{
    NetworkInterfaceKind, NetworkInterfaceSnapshot, NetworkRoamingEvent, NetworkRoamingSnapshot,
    NetworkRoamingStatus,
};
use infiltrator_contract::privileged_network::PrivilegedNetworkSnapshot;
use infiltrator_contract::runtime_control::RuntimeControlStatus;
use infiltrator_contract::surface_snapshot::PageStatus;
use infiltrator_contract::system_proxy::{SystemProxyRecoverySnapshot, SystemProxySnapshot};
use infiltrator_contract::tun::TunStack;
use infiltrator_contract::version::CoreVersionSnapshot;
use infiltrator_contract::vpn::VpnSessionSnapshot;

impl SettingsProjection {
    pub fn demo() -> Self {
        Self {
            close_to_tray: Some(true),
            notifications_enabled: Some(true),
            preference_status: PageStatus::Ready,
            runtime_status: RuntimeControlStatus::Ready,
            autostart: true,
            system_proxy: true,
            system_proxy_snapshot: SystemProxySnapshot::default(),
            system_proxy_recovery: SystemProxyRecoverySnapshot::default(),
            mixed_port: Some(7890),
            allow_lan: Some(false),
            lan_bind_address: Some(DEFAULT_BIND_ADDRESS.to_owned()),
            lan_security: Some(Default::default()),
            ipv6_routing: Some(Default::default()),
            pac: Default::default(),
            network_roaming: demo_network_roaming(),
            vpn: VpnSessionSnapshot::unsupported(
                1,
                "Android VpnService is not part of the desktop demo host",
            ),
            privileged_network: PrivilegedNetworkSnapshot::unsupported(
                1,
                "privileged network regression is a host-test capability",
            ),
            tun_enabled: Some(true),
            tun_stack: Some(TunStack::Gvisor.as_str().to_owned()),
            tun_auto_route: Some(true),
            tun_strict_route: Some(false),
            controller_port: Some(9090),
            log_level: Some("info".to_owned()),
            core_channel: "stable".to_owned(),
            core_versions: CoreVersionSnapshot::default(),
            core_integrity: Default::default(),
            controller_auth: Default::default(),
            service_mode: Default::default(),
            port_conflicts: Default::default(),
            core_resources: Default::default(),
            offline_startup: Default::default(),
            mtu: Default::default(),
            mini_hud: Default::default(),
        }
    }
}

fn demo_network_roaming() -> NetworkRoamingSnapshot {
    NetworkRoamingSnapshot {
        status: NetworkRoamingStatus::Stable,
        interfaces: vec![NetworkInterfaceSnapshot {
            name: "eth0".to_owned(),
            kind: NetworkInterfaceKind::Ethernet,
            is_up: true,
            is_default_gateway: true,
            gateway_ip: Some("192.168.1.1".to_owned()),
            ip_addresses: vec!["192.168.1.10/24".to_owned()],
            mtu: Some(1500),
            metric: Some(100),
            dns_servers: vec!["192.168.1.1".to_owned()],
        }],
        active_interface: Some("eth0".to_owned()),
        default_gateway: Some("192.168.1.1".to_owned()),
        tun_interface: Some("Meta".to_owned()),
        physical_mtu: Some(1500),
        recommended_tun_mtu: Some(1420),
        tcp_mss: Some(1380),
        last_event: Some(NetworkRoamingEvent::InitialObservation {
            interface: Some("eth0".to_owned()),
            gateway_ip: Some("192.168.1.1".to_owned()),
        }),
        revision: 1,
        ..NetworkRoamingSnapshot::default()
    }
}

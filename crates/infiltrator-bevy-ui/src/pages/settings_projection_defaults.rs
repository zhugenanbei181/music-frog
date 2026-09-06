//! Default shared Settings projection used by demo and empty compositions.

use super::settings_core::SettingsProjection;
use infiltrator_contract::version::CoreVersionSnapshot;
use infiltrator_contract::system_proxy::{
    SystemProxyRecoverySnapshot, SystemProxySnapshot,
};
use infiltrator_contract::network_roaming::{
    NetworkInterfaceKind, NetworkInterfaceSnapshot, NetworkRoamingEvent,
    NetworkRoamingSnapshot, NetworkRoamingStatus,
};

impl SettingsProjection {
    pub fn demo() -> Self {
        Self {
            autostart: true,
            system_proxy: true,
            system_proxy_snapshot: SystemProxySnapshot::default(),
            system_proxy_recovery: SystemProxyRecoverySnapshot::default(),
            mixed_port: 7890,
            allow_lan: false,
            lan_bind_address: infiltrator_contract::lan::DEFAULT_BIND_ADDRESS.to_owned(),
            lan_security: Default::default(),
            ipv6_routing: Default::default(),
            pac: Default::default(),
            network_roaming: demo_network_roaming(),
            vpn: infiltrator_contract::vpn::VpnSessionSnapshot::unsupported(
                1,
                "Android VpnService is not part of the desktop demo host",
            ),
            privileged_network:
                infiltrator_contract::privileged_network::PrivilegedNetworkSnapshot::unsupported(
                    1,
                    "privileged network regression is a host-test capability",
                ),
            tun_enabled: true,
            tun_stack: "gVisor (高性能用户态协议栈)".to_owned(),
            tun_auto_route: true,
            tun_strict_route: false,
            controller_port: 9090,
            log_level: "info".to_owned(),
            core_channel: "stable".to_owned(),
            core_versions: CoreVersionSnapshot::default(),
            core_integrity: Default::default(),
            controller_auth: Default::default(),
            service_mode: Default::default(),
            port_conflicts: Default::default(),
            core_resources: Default::default(),
            offline_startup: Default::default(),
            mtu: Default::default(),
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

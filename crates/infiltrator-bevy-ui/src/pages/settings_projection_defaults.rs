//! Default shared Settings projection used by demo and empty compositions.

use super::settings_core::SettingsProjection;
use infiltrator_contract::version::CoreVersionSnapshot;
use infiltrator_contract::system_proxy::{
    SystemProxyRecoverySnapshot, SystemProxySnapshot,
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

//! Settings snapshot construction belongs to the settings adapter.
use crate::pages::settings::settings_core::SettingsProjection;
use infiltrator_contract::runtime_control::RuntimeControlStatus;
use infiltrator_contract::surface_snapshot;
use infiltrator_contract::surface_snapshot::PageStatus;

pub(crate) fn empty_settings() -> SettingsProjection {
    SettingsProjection {
        close_to_tray: None,
        notifications_enabled: None,
        preference_status: PageStatus::Loading,
        runtime_status: RuntimeControlStatus::Unobserved,
        autostart: false,
        system_proxy: false,
        system_proxy_snapshot: Default::default(),
        system_proxy_recovery: Default::default(),
        mixed_port: None,
        allow_lan: None,
        lan_bind_address: None,
        lan_security: None,
        ipv6_routing: None,
        pac: Default::default(),
        network_roaming: Default::default(),
        vpn: Default::default(),
        privileged_network: Default::default(),
        tun_enabled: None,
        tun_stack: None,
        tun_auto_route: None,
        tun_strict_route: None,
        controller_port: None,
        log_level: None,
        core_channel: String::new(),
        core_versions: Default::default(),
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

impl From<SettingsProjection> for surface_snapshot::SettingsPageSnapshot {
    fn from(value: SettingsProjection) -> Self {
        Self {
            close_to_tray: value.close_to_tray,
            notifications_enabled: value.notifications_enabled,
            language: "zh-CN".into(),
            autostart: value.autostart,
            system_proxy: value.system_proxy,
            mixed_port: value.mixed_port,
            allow_lan: value.allow_lan,
            lan_bind_address: value.lan_bind_address,
            lan_security: value.lan_security,
            ipv6_routing: value.ipv6_routing,
            pac: value.pac,
            tun_enabled: value.tun_enabled,
            tun_stack: value.tun_stack,
            tun_auto_route: value.tun_auto_route,
            tun_strict_route: value.tun_strict_route,
            controller_port: value.controller_port,
            log_level: value.log_level,
            core_channel: value.core_channel,
            mini_hud: value.mini_hud,
        }
    }
}

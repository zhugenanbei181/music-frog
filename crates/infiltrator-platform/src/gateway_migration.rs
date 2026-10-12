use super::{
    GatewayMigrationAction, GatewayMigrationDetector, GatewayMigrationEvent,
    NetworkInterfaceSnapshot,
};

impl GatewayMigrationDetector {
    pub fn new(tun_interface_name: Option<String>) -> Self {
        Self {
            tun_interface_name,
            current_physical_gateway: None,
            current_gateway_ip: None,
            current_physical_mtu: None,
            is_tun_active: false,
        }
    }

    pub fn set_tun_interface(&mut self, tun_name: Option<String>) {
        self.tun_interface_name = tun_name;
    }

    pub fn set_tun_active(&mut self, active: bool) {
        self.is_tun_active = active;
    }

    pub fn is_tun_active(&self) -> bool {
        self.is_tun_active
    }

    pub fn tun_interface_name(&self) -> Option<&str> {
        self.tun_interface_name.as_deref()
    }

    pub fn current_physical_gateway(&self) -> Option<&str> {
        self.current_physical_gateway.as_deref()
    }

    pub fn current_physical_mtu(&self) -> Option<u32> {
        self.current_physical_mtu
    }

    pub fn is_tun_like_interface(&self, iface: &str) -> bool {
        if self
            .tun_interface_name
            .as_deref()
            .is_some_and(|tun| iface.eq_ignore_ascii_case(tun))
        {
            return true;
        }
        let lower = iface.to_ascii_lowercase();
        lower.starts_with("tun")
            || lower.starts_with("utun")
            || lower.starts_with("wintun")
            || lower.starts_with("meta")
            || lower.starts_with("clash")
            || lower.starts_with("sing")
            || lower.contains("tap")
    }

    pub fn detect_migration(
        &mut self,
        before: &[NetworkInterfaceSnapshot],
        after: &[NetworkInterfaceSnapshot],
    ) -> Option<GatewayMigrationEvent> {
        let old_gw = before.iter().find(|i| i.is_default_gateway);
        let new_gw = after.iter().find(|i| i.is_default_gateway);

        let old_gw_name = old_gw.map(|i| i.name.clone());
        let old_gw_ip = old_gw.and_then(|i| i.gateway_ip.clone());
        let new_gw_name = new_gw.map(|i| i.name.clone());
        let new_gw_ip = new_gw.and_then(|i| i.gateway_ip.clone());
        let new_gw_is_up = new_gw.map(|i| i.is_up).unwrap_or(false);

        let gateway_changed = old_gw_name != new_gw_name || old_gw_ip != new_gw_ip;

        if !gateway_changed {
            if let (true, Some(phys_gw)) = (self.is_tun_active, &self.current_physical_gateway) {
                let phys_now_down = after
                    .iter()
                    .find(|i| i.name == *phys_gw)
                    .map(|i| !i.is_up)
                    .unwrap_or(true);

                if phys_now_down {
                    let fallback = after
                        .iter()
                        .find(|i| i.is_up && !self.is_tun_like_interface(&i.name) && !i.is_loopback)
                        .map(|i| i.name.clone());

                    return Some(GatewayMigrationEvent {
                        old_gateway_interface: Some(phys_gw.clone()),
                        new_gateway_interface: fallback,
                        old_gateway_ip: self.current_gateway_ip.clone(),
                        new_gateway_ip: None,
                        action_required: GatewayMigrationAction::DeadTunMitigation {
                            tun_interface: self
                                .tun_interface_name
                                .clone()
                                .unwrap_or_else(|| "Meta".to_string()),
                            reason: format!(
                                "Physical gateway interface '{}' went down; dead TUN route risk",
                                phys_gw
                            ),
                        },
                    });
                }
            }
            return None;
        }

        let action = if let Some(ref new_name) = new_gw_name {
            if !new_gw_is_up {
                if self.is_tun_active {
                    GatewayMigrationAction::DeadTunMitigation {
                        tun_interface: self
                            .tun_interface_name
                            .clone()
                            .unwrap_or_else(|| "Meta".to_string()),
                        reason: format!(
                            "Gateway interface '{}' is down; dead TUN route risk",
                            new_name
                        ),
                    }
                } else {
                    GatewayMigrationAction::None
                }
            } else if self.is_tun_like_interface(new_name) {
                if self.current_physical_gateway.is_none() {
                    let fallback = after
                        .iter()
                        .find(|i| i.is_up && !self.is_tun_like_interface(&i.name) && !i.is_loopback)
                        .map(|i| i.name.clone());

                    GatewayMigrationAction::PreventRoutingLoop {
                        tun_interface: new_name.clone(),
                        fallback_interface: fallback,
                        reason:
                            "Default gateway set to TUN interface without physical gateway bypass"
                                .to_string(),
                    }
                } else {
                    GatewayMigrationAction::None
                }
            } else {
                let new_mtu = new_gw.map(|i| i.effective_mtu()).unwrap_or(1500);
                self.current_physical_gateway = Some(new_name.clone());
                self.current_gateway_ip = new_gw_ip.clone();
                self.current_physical_mtu = Some(new_mtu);

                if self.is_tun_active {
                    GatewayMigrationAction::UpdateTunRoutes {
                        old_gateway_iface: old_gw_name.clone(),
                        new_gateway_iface: new_name.clone(),
                        new_gateway_ip: new_gw_ip.clone(),
                    }
                } else {
                    GatewayMigrationAction::RebindGateway {
                        interface: new_name.clone(),
                        gateway_ip: new_gw_ip.clone(),
                    }
                }
            }
        } else if self.is_tun_active {
            GatewayMigrationAction::DeadTunMitigation {
                tun_interface: self
                    .tun_interface_name
                    .clone()
                    .unwrap_or_else(|| "Meta".to_string()),
                reason: "Default gateway removed; no active route to Internet".to_string(),
            }
        } else {
            GatewayMigrationAction::None
        };

        Some(GatewayMigrationEvent {
            old_gateway_interface: old_gw_name,
            new_gateway_interface: new_gw_name,
            old_gateway_ip: old_gw_ip,
            new_gateway_ip: new_gw_ip,
            action_required: action,
        })
    }
}

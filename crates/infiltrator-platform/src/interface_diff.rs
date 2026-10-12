use super::{
    GatewayMigrationAction, GatewayMigrationDetector, NetworkEvent, NetworkInterfaceSnapshot,
};

/// Computes differences between interface snapshots.
pub struct InterfaceDiffDetector;

impl InterfaceDiffDetector {
    pub fn compute_diff(
        before: &[NetworkInterfaceSnapshot],
        after: &[NetworkInterfaceSnapshot],
    ) -> Vec<NetworkEvent> {
        let mut events = Vec::new();

        let old_gw = before
            .iter()
            .find(|i| i.is_default_gateway)
            .map(|i| i.name.clone());
        let new_gw = after
            .iter()
            .find(|i| i.is_default_gateway)
            .map(|i| i.name.clone());

        if old_gw != new_gw {
            events.push(NetworkEvent::DefaultGatewayChanged {
                old: old_gw,
                new: new_gw,
            });
        }

        for next_iface in after {
            let prev_iface = before.iter().find(|i| i.name == next_iface.name);

            if let Some(prev) = prev_iface {
                if !prev.is_up && next_iface.is_up {
                    events.push(NetworkEvent::InterfaceUp(next_iface.name.clone()));
                } else if prev.is_up && !next_iface.is_up {
                    events.push(NetworkEvent::InterfaceDown(next_iface.name.clone()));
                }

                if prev.ip_addresses != next_iface.ip_addresses {
                    events.push(NetworkEvent::IpAddressChanged {
                        iface: next_iface.name.clone(),
                        new_ips: next_iface.ip_addresses.clone(),
                    });
                }
            } else if next_iface.is_up {
                events.push(NetworkEvent::InterfaceUp(next_iface.name.clone()));
            }
        }

        for prev_iface in before {
            if !after.iter().any(|i| i.name == prev_iface.name) && prev_iface.is_up {
                events.push(NetworkEvent::InterfaceDown(prev_iface.name.clone()));
            }
        }

        events
    }

    pub fn compute_diff_with_detector(
        before: &[NetworkInterfaceSnapshot],
        after: &[NetworkInterfaceSnapshot],
        detector: &mut GatewayMigrationDetector,
    ) -> Vec<NetworkEvent> {
        let mut events = Self::compute_diff(before, after);

        if let Some(migration) = detector.detect_migration(before, after) {
            if let GatewayMigrationAction::PreventRoutingLoop {
                ref tun_interface,
                ref reason,
                ..
            } = migration.action_required
            {
                events.push(NetworkEvent::RoutingLoopRiskDetected {
                    tun_interface: tun_interface.clone(),
                    gateway_interface: migration.new_gateway_interface.clone().unwrap_or_default(),
                    details: reason.clone(),
                });
            }
            events.push(NetworkEvent::GatewayMigration(Box::new(migration)));
        }

        events
    }

    pub fn should_trigger_core_reconnect(events: &[NetworkEvent]) -> bool {
        events.iter().any(|e| match e {
            NetworkEvent::DefaultGatewayChanged { .. } => true,
            NetworkEvent::GatewayMigration(mig) => match mig.action_required {
                GatewayMigrationAction::UpdateTunRoutes { .. }
                | GatewayMigrationAction::PreventRoutingLoop { .. }
                | GatewayMigrationAction::DeadTunMitigation { .. }
                | GatewayMigrationAction::RebindGateway { .. }
                | GatewayMigrationAction::PurgeStaleConnections { .. }
                | GatewayMigrationAction::FlushDnsCache { .. } => true,
                GatewayMigrationAction::ClampMtu { .. } | GatewayMigrationAction::None => false,
            },
            NetworkEvent::RoutingLoopRiskDetected { .. } => true,
            NetworkEvent::InterfaceUp(_) | NetworkEvent::InterfaceDown(_) => true,
            NetworkEvent::IpAddressChanged { .. } => false,
            NetworkEvent::MtuClampingSuggested { .. }
            | NetworkEvent::InterfaceFlapDetected { .. } => false,
        })
    }
}

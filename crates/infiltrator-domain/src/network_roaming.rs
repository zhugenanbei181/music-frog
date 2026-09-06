//! Pure selection and migration decisions for physical network roaming.

use crate::runtime::TunSnapshot;
use infiltrator_contract::network_roaming::{
    NetworkInterfaceKind, NetworkInterfaceSnapshot, NetworkObservation,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkRoamingDecision {
    pub active_interface: Option<String>,
    pub gateway_ip: Option<String>,
    pub previous_interface: Option<String>,
    pub previous_gateway_ip: Option<String>,
    pub changed: bool,
    pub active_address_changed: bool,
    pub route_repair_required: bool,
    pub reason: String,
}

/// Select the best live physical egress. An OS-confirmed default route wins;
/// route metric and then name make multi-homing selection deterministic.
pub fn select_active_interface(
    interfaces: &[NetworkInterfaceSnapshot],
) -> Option<&NetworkInterfaceSnapshot> {
    interfaces
        .iter()
        .filter(|interface| {
            interface.is_up
                && interface.kind.is_physical()
                && !matches!(interface.kind, NetworkInterfaceKind::Loopback)
                && !interface.ip_addresses.is_empty()
        })
        .min_by_key(|interface| {
            (
                !interface.is_default_gateway,
                interface
                    .metric
                    .unwrap_or_else(|| interface.kind.default_metric()),
                interface.name.clone(),
            )
        })
}

/// Decide whether a physical-link observation requires a TUN route repair.
/// The initial observation is deliberately non-destructive; only a transition
/// after a known baseline can trigger automatic host changes.
pub fn decide(
    previous: Option<&NetworkObservation>,
    current: &NetworkObservation,
    tun: Option<&TunSnapshot>,
    tun_interface: Option<&str>,
) -> NetworkRoamingDecision {
    let selected = select_active_interface(&current.interfaces);
    let previous_selected =
        previous.and_then(|observation| select_active_interface(&observation.interfaces));
    let active_interface = selected.map(|interface| interface.name.clone());
    let gateway_ip = selected.and_then(|interface| interface.gateway_ip.clone());
    let previous_interface = previous_selected.map(|interface| interface.name.clone());
    let previous_gateway_ip = previous_selected.and_then(|interface| interface.gateway_ip.clone());
    let active_address_changed = match (previous_selected, selected) {
        (Some(previous), Some(current)) if previous.name == current.name => {
            previous.ip_addresses != current.ip_addresses
        }
        _ => false,
    };
    let changed = previous.is_some()
        && (active_interface != previous_interface
            || gateway_ip != previous_gateway_ip
            || active_address_changed);
    let route_repair_required = changed
        && selected.is_some()
        && tun.is_some_and(|tun| tun.enable && tun.auto_route)
        && tun_interface.is_some();

    let reason = if previous.is_none() {
        "initial physical-link observation".to_owned()
    } else if selected.is_none() {
        "no live physical egress interface is available".to_owned()
    } else if route_repair_required {
        "physical egress changed while TUN auto-route is active".to_owned()
    } else if changed {
        "physical egress changed while TUN route repair was not required".to_owned()
    } else {
        "physical egress is stable".to_owned()
    };

    NetworkRoamingDecision {
        active_interface,
        gateway_ip,
        previous_interface,
        previous_gateway_ip,
        changed,
        active_address_changed,
        route_repair_required,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::network_roaming::{
        NetworkInterfaceKind, NetworkInterfaceSnapshot, NetworkObservation,
    };

    fn link(
        name: &str,
        kind: NetworkInterfaceKind,
        gateway: &str,
        metric: u32,
    ) -> NetworkInterfaceSnapshot {
        NetworkInterfaceSnapshot {
            name: name.to_owned(),
            kind,
            is_up: true,
            is_default_gateway: true,
            gateway_ip: Some(gateway.to_owned()),
            ip_addresses: vec![format!("{gateway}/24")],
            mtu: Some(1500),
            metric: Some(metric),
            dns_servers: Vec::new(),
        }
    }

    fn tun() -> TunSnapshot {
        TunSnapshot {
            enable: true,
            stack: "gvisor".to_owned(),
            auto_route: true,
            strict_route: true,
            mtu: Some(1420),
        }
    }

    #[test]
    fn ethernet_beats_wifi_when_both_are_default_candidates() {
        let interfaces = vec![
            link("wlan0", NetworkInterfaceKind::Wifi, "192.168.2.1", 200),
            link("eth0", NetworkInterfaceKind::Ethernet, "192.168.1.1", 100),
        ];
        assert_eq!(select_active_interface(&interfaces).unwrap().name, "eth0");
    }

    #[test]
    fn gateway_migration_requests_repair_when_tun_auto_route_is_live() {
        let previous = NetworkObservation {
            interfaces: vec![link(
                "wlan0",
                NetworkInterfaceKind::Wifi,
                "192.168.2.1",
                200,
            )],
            observed_at_epoch_ms: Some(1),
        };
        let current = NetworkObservation {
            interfaces: vec![link(
                "eth0",
                NetworkInterfaceKind::Ethernet,
                "192.168.1.1",
                100,
            )],
            observed_at_epoch_ms: Some(2),
        };
        let tun = tun();
        let decision = decide(Some(&previous), &current, Some(&tun), Some("Meta"));
        assert!(decision.changed);
        assert!(decision.route_repair_required);
        assert_eq!(decision.previous_interface.as_deref(), Some("wlan0"));
        assert_eq!(decision.active_interface.as_deref(), Some("eth0"));
    }

    #[test]
    fn migration_is_observed_but_not_repaired_when_tun_is_inactive() {
        let previous = NetworkObservation {
            interfaces: vec![link(
                "wlan0",
                NetworkInterfaceKind::Wifi,
                "192.168.2.1",
                200,
            )],
            observed_at_epoch_ms: Some(1),
        };
        let current = NetworkObservation {
            interfaces: vec![link(
                "eth0",
                NetworkInterfaceKind::Ethernet,
                "192.168.1.1",
                100,
            )],
            observed_at_epoch_ms: Some(2),
        };
        let tun = TunSnapshot {
            enable: false,
            ..tun()
        };
        let decision = decide(Some(&previous), &current, Some(&tun), Some("Meta"));
        assert!(decision.changed);
        assert!(!decision.route_repair_required);
    }

    #[test]
    fn no_physical_route_is_not_an_empty_successful_repair_plan() {
        let observation = NetworkObservation {
            interfaces: vec![NetworkInterfaceSnapshot {
                name: "Meta".to_owned(),
                kind: NetworkInterfaceKind::Tun,
                is_up: true,
                ..NetworkInterfaceSnapshot::default()
            }],
            observed_at_epoch_ms: Some(1),
        };
        let tun = tun();
        let decision = decide(Some(&observation), &observation, Some(&tun), Some("Meta"));
        assert!(decision.active_interface.is_none());
        assert!(!decision.route_repair_required);
    }
}

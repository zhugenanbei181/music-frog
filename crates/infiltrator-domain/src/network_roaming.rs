//! Pure selection and migration decisions for physical network roaming.

use infiltrator_contract::network_roaming::{
    NetworkInterfaceKind, NetworkInterfaceSnapshot, NetworkObservation,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetworkRoamingDecision {
    InitialObservation {
        active_interface: Option<String>,
        gateway_ip: Option<String>,
    },
    Stable {
        active_interface: Option<String>,
        gateway_ip: Option<String>,
    },
    InterfaceAddressChanged {
        interface: String,
        gateway_ip: Option<String>,
    },
    GatewayChanged {
        previous_interface: Option<String>,
        active_interface: Option<String>,
        previous_gateway_ip: Option<String>,
        gateway_ip: Option<String>,
    },
}

#[derive(Clone, Copy)]
pub enum NetworkRoamingBaseline<'a> {
    AwaitingInitialObservation,
    Observed(&'a NetworkObservation),
}

impl NetworkRoamingDecision {
    pub fn reason(&self) -> &'static str {
        match self {
            Self::InitialObservation { .. } => "initial physical-link observation",
            Self::Stable {
                active_interface: None,
                ..
            } => "no live physical egress interface is available",
            Self::Stable { .. } => "physical egress is stable",
            Self::GatewayChanged {
                active_interface: None,
                ..
            } => "no live physical egress interface is available",
            Self::InterfaceAddressChanged { .. } | Self::GatewayChanged { .. } => {
                "physical egress changed while TUN route repair was not required"
            }
        }
    }

    pub fn active_interface(&self) -> Option<&str> {
        match self {
            Self::InitialObservation {
                active_interface, ..
            }
            | Self::Stable {
                active_interface, ..
            }
            | Self::GatewayChanged {
                active_interface, ..
            } => active_interface.as_deref(),
            Self::InterfaceAddressChanged { interface, .. } => Some(interface),
        }
    }

    pub fn gateway_ip(&self) -> Option<&str> {
        match self {
            Self::InitialObservation { gateway_ip, .. }
            | Self::Stable { gateway_ip, .. }
            | Self::InterfaceAddressChanged { gateway_ip, .. }
            | Self::GatewayChanged { gateway_ip, .. } => gateway_ip.as_deref(),
        }
    }

    pub fn previous_interface(&self) -> Option<&str> {
        match self {
            Self::GatewayChanged {
                previous_interface, ..
            } => previous_interface.as_deref(),
            Self::InitialObservation { .. }
            | Self::Stable { .. }
            | Self::InterfaceAddressChanged { .. } => None,
        }
    }

    pub fn previous_gateway_ip(&self) -> Option<&str> {
        match self {
            Self::GatewayChanged {
                previous_gateway_ip,
                ..
            } => previous_gateway_ip.as_deref(),
            Self::InitialObservation { .. }
            | Self::Stable { .. }
            | Self::InterfaceAddressChanged { .. } => None,
        }
    }
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

/// Classify the physical-link transition. The initial observation is
/// deliberately non-destructive; only a later typed transition can trigger
/// automatic host changes.
pub fn decide(
    baseline: NetworkRoamingBaseline<'_>,
    current: &NetworkObservation,
) -> NetworkRoamingDecision {
    let selected = select_active_interface(&current.interfaces);
    let active_interface = selected.map(|interface| interface.name.clone());
    let gateway_ip = selected.and_then(|interface| interface.gateway_ip.clone());
    match baseline {
        NetworkRoamingBaseline::AwaitingInitialObservation => {
            NetworkRoamingDecision::InitialObservation {
                active_interface,
                gateway_ip,
            }
        }
        NetworkRoamingBaseline::Observed(previous) => {
            let previous_selected = select_active_interface(&previous.interfaces);
            let previous_interface = previous_selected.map(|interface| interface.name.clone());
            let previous_gateway_ip =
                previous_selected.and_then(|interface| interface.gateway_ip.clone());
            match (previous_selected, selected) {
                (Some(previous), Some(current))
                    if previous.name == current.name
                        && previous.gateway_ip == current.gateway_ip
                        && previous.ip_addresses != current.ip_addresses =>
                {
                    NetworkRoamingDecision::InterfaceAddressChanged {
                        interface: current.name.clone(),
                        gateway_ip,
                    }
                }
                _ if active_interface == previous_interface
                    && gateway_ip == previous_gateway_ip =>
                {
                    NetworkRoamingDecision::Stable {
                        active_interface,
                        gateway_ip,
                    }
                }
                _ => NetworkRoamingDecision::GatewayChanged {
                    previous_interface,
                    active_interface,
                    previous_gateway_ip,
                    gateway_ip,
                },
            }
        }
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

    #[test]
    fn ethernet_beats_wifi_when_both_are_default_candidates() {
        let interfaces = vec![
            link("wlan0", NetworkInterfaceKind::Wifi, "192.168.2.1", 200),
            link("eth0", NetworkInterfaceKind::Ethernet, "192.168.1.1", 100),
        ];
        assert_eq!(select_active_interface(&interfaces).unwrap().name, "eth0");
    }

    #[test]
    fn gateway_migration_is_a_typed_transition() {
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
        let decision = decide(NetworkRoamingBaseline::Observed(&previous), &current);
        assert!(matches!(
            decision,
            NetworkRoamingDecision::GatewayChanged {
                previous_interface: Some(ref previous),
                active_interface: Some(ref active),
                ..
            } if previous == "wlan0" && active == "eth0"
        ));
    }

    #[test]
    fn unchanged_path_is_a_typed_stable_transition() {
        let previous = NetworkObservation {
            interfaces: vec![link(
                "wlan0",
                NetworkInterfaceKind::Wifi,
                "192.168.2.1",
                200,
            )],
            observed_at_epoch_ms: Some(1),
        };
        let decision = decide(NetworkRoamingBaseline::Observed(&previous), &previous);
        assert!(matches!(decision, NetworkRoamingDecision::Stable { .. }));
    }

    #[test]
    fn changed_address_on_the_same_path_has_its_own_transition() {
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
            interfaces: vec![NetworkInterfaceSnapshot {
                ip_addresses: vec!["192.168.2.20/24".to_owned()],
                ..previous.interfaces[0].clone()
            }],
            observed_at_epoch_ms: Some(2),
        };
        let decision = decide(NetworkRoamingBaseline::Observed(&previous), &current);
        assert!(matches!(
            decision,
            NetworkRoamingDecision::InterfaceAddressChanged { ref interface, .. }
                if interface == "wlan0"
        ));
    }

    #[test]
    fn initial_observation_is_distinct_from_stable_path() {
        let current = NetworkObservation {
            interfaces: vec![link(
                "eth0",
                NetworkInterfaceKind::Ethernet,
                "192.168.1.1",
                100,
            )],
            observed_at_epoch_ms: Some(2),
        };
        let decision = decide(NetworkRoamingBaseline::AwaitingInitialObservation, &current);
        assert!(matches!(
            decision,
            NetworkRoamingDecision::InitialObservation {
                active_interface: Some(ref active), ..
            } if active == "eth0"
        ));
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
        let decision = decide(NetworkRoamingBaseline::Observed(&observation), &observation);
        assert!(matches!(decision, NetworkRoamingDecision::Stable { .. }));
        assert_eq!(decision.active_interface(), None);
    }
}

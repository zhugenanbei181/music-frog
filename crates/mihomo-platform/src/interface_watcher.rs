use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkInterfaceSnapshot {
    pub name: String,
    pub is_up: bool,
    pub ip_addresses: Vec<String>,
    pub is_default_gateway: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterfaceChangeEvent {
    InterfaceUp(String),
    InterfaceDown(String),
    DefaultGatewayChanged {
        old: Option<String>,
        new: Option<String>,
    },
    IpAddressChanged {
        iface: String,
        new_ips: Vec<String>,
    },
}

pub struct InterfaceDiffDetector;

impl InterfaceDiffDetector {
    pub fn compute_diff(
        before: &[NetworkInterfaceSnapshot],
        after: &[NetworkInterfaceSnapshot],
    ) -> Vec<InterfaceChangeEvent> {
        let mut events = Vec::new();

        // Check for gateway changes
        let old_gw = before
            .iter()
            .find(|i| i.is_default_gateway)
            .map(|i| i.name.clone());
        let new_gw = after
            .iter()
            .find(|i| i.is_default_gateway)
            .map(|i| i.name.clone());

        if old_gw != new_gw {
            events.push(InterfaceChangeEvent::DefaultGatewayChanged {
                old: old_gw,
                new: new_gw,
            });
        }

        // Check for interface state and IP changes
        for next_iface in after {
            let prev_iface = before.iter().find(|i| i.name == next_iface.name);

            if let Some(prev) = prev_iface {
                if !prev.is_up && next_iface.is_up {
                    events.push(InterfaceChangeEvent::InterfaceUp(next_iface.name.clone()));
                } else if prev.is_up && !next_iface.is_up {
                    events.push(InterfaceChangeEvent::InterfaceDown(next_iface.name.clone()));
                }

                if prev.ip_addresses != next_iface.ip_addresses {
                    events.push(InterfaceChangeEvent::IpAddressChanged {
                        iface: next_iface.name.clone(),
                        new_ips: next_iface.ip_addresses.clone(),
                    });
                }
            } else {
                // Completely new interface
                if next_iface.is_up {
                    events.push(InterfaceChangeEvent::InterfaceUp(next_iface.name.clone()));
                }
            }
        }

        // Check for removed interfaces
        for prev_iface in before {
            if !after.iter().any(|i| i.name == prev_iface.name) {
                if prev_iface.is_up {
                    events.push(InterfaceChangeEvent::InterfaceDown(prev_iface.name.clone()));
                }
            }
        }

        events
    }

    pub fn should_trigger_core_reconnect(events: &[InterfaceChangeEvent]) -> bool {
        events.iter().any(|e| match e {
            InterfaceChangeEvent::DefaultGatewayChanged { .. } => true,
            InterfaceChangeEvent::InterfaceUp(_) | InterfaceChangeEvent::InterfaceDown(_) => true,
            InterfaceChangeEvent::IpAddressChanged { .. } => false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_snapshot(
        name: &str,
        is_up: bool,
        is_default_gateway: bool,
        ips: Vec<&str>,
    ) -> NetworkInterfaceSnapshot {
        NetworkInterfaceSnapshot {
            name: name.to_string(),
            is_up,
            is_default_gateway,
            ip_addresses: ips.into_iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn test_interface_up_down_detection() {
        let before = vec![
            create_snapshot("eth0", false, false, vec![]),
            create_snapshot("wlan0", true, false, vec!["192.168.1.5"]),
        ];
        let after = vec![
            create_snapshot("eth0", true, false, vec!["10.0.0.5"]),
            create_snapshot("wlan0", false, false, vec![]),
        ];

        let diff = InterfaceDiffDetector::compute_diff(&before, &after);
        
        assert!(diff.contains(&InterfaceChangeEvent::InterfaceUp("eth0".to_string())));
        assert!(diff.contains(&InterfaceChangeEvent::InterfaceDown("wlan0".to_string())));
    }

    #[test]
    fn test_default_gateway_migration_detection() {
        let before = vec![
            create_snapshot("eth0", true, true, vec!["10.0.0.5"]),
            create_snapshot("wlan0", true, false, vec!["192.168.1.5"]),
        ];
        let after = vec![
            create_snapshot("eth0", true, false, vec!["10.0.0.5"]),
            create_snapshot("wlan0", true, true, vec!["192.168.1.5"]),
        ];

        let diff = InterfaceDiffDetector::compute_diff(&before, &after);

        assert!(diff.contains(&InterfaceChangeEvent::DefaultGatewayChanged {
            old: Some("eth0".to_string()),
            new: Some("wlan0".to_string()),
        }));
    }

    #[test]
    fn test_ip_address_change_detection() {
        let before = vec![create_snapshot("eth0", true, false, vec!["10.0.0.5"])];
        let after = vec![create_snapshot(
            "eth0",
            true,
            false,
            vec!["10.0.0.5", "10.0.0.6"],
        )];

        let diff = InterfaceDiffDetector::compute_diff(&before, &after);

        assert!(diff.contains(&InterfaceChangeEvent::IpAddressChanged {
            iface: "eth0".to_string(),
            new_ips: vec!["10.0.0.5".to_string(), "10.0.0.6".to_string()],
        }));
    }

    #[test]
    fn test_core_reconnect_trigger() {
        let ev1 = InterfaceChangeEvent::InterfaceUp("eth0".to_string());
        let ev2 = InterfaceChangeEvent::InterfaceDown("wlan0".to_string());
        let ev3 = InterfaceChangeEvent::DefaultGatewayChanged {
            old: Some("eth0".to_string()),
            new: Some("wlan0".to_string()),
        };
        let ev4 = InterfaceChangeEvent::IpAddressChanged {
            iface: "eth0".to_string(),
            new_ips: vec!["10.0.0.5".to_string()],
        };

        assert!(InterfaceDiffDetector::should_trigger_core_reconnect(&[ev1]));
        assert!(InterfaceDiffDetector::should_trigger_core_reconnect(&[ev2]));
        assert!(InterfaceDiffDetector::should_trigger_core_reconnect(&[ev3]));
        assert!(!InterfaceDiffDetector::should_trigger_core_reconnect(&[ev4]));
    }
}

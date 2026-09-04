//! Virtual bridge detection and route rule compilation.

use infiltrator_domain::rules::RuleEntry;
use std::net::{IpAddr, Ipv4Addr};

use super::{
    BridgeRoutingMode, VirtualBridgeType, VirtualNetworkBridge, VirtualNetworkBridgeDetector,
    matches_cidr,
};

impl VirtualNetworkBridgeDetector {
    /// Standard well-known default virtual subnets
    pub const DOCKER_DEFAULT_CIDR: &'static str = "172.17.0.0/16";
    pub const DOCKER_CUSTOM_RANGE: &'static str = "172.18.0.0/16";
    pub const PODMAN_DEFAULT_CIDR: &'static str = "10.88.0.0/16";
    pub const HYPERV_DEFAULT_CIDR: &'static str = "172.28.0.0/16";

    /// Creates a new detector with standard default virtual bridges.
    pub fn new() -> Self {
        let mut detector = Self {
            bridges: Vec::new(),
        };
        detector.add_default_bridges();
        detector
    }

    /// Adds standard default virtual network bridge configurations.
    pub fn add_default_bridges(&mut self) {
        self.bridges.push(VirtualNetworkBridge {
            bridge_type: VirtualBridgeType::DockerDefaultBridge,
            interface_name: "docker0".to_string(),
            subnet_cidr: Self::DOCKER_DEFAULT_CIDR.to_string(),
            gateway_ip: Some(IpAddr::V4(Ipv4Addr::new(172, 17, 0, 1))),
            is_mirrored_mode: false,
        });
        self.bridges.push(VirtualNetworkBridge {
            bridge_type: VirtualBridgeType::DockerCustomBridge,
            interface_name: "br-docker".to_string(),
            subnet_cidr: Self::DOCKER_CUSTOM_RANGE.to_string(),
            gateway_ip: Some(IpAddr::V4(Ipv4Addr::new(172, 18, 0, 1))),
            is_mirrored_mode: false,
        });
        self.bridges.push(VirtualNetworkBridge {
            bridge_type: VirtualBridgeType::PodmanBridge,
            interface_name: "podman0".to_string(),
            subnet_cidr: Self::PODMAN_DEFAULT_CIDR.to_string(),
            gateway_ip: Some(IpAddr::V4(Ipv4Addr::new(10, 88, 0, 1))),
            is_mirrored_mode: false,
        });
        self.bridges.push(VirtualNetworkBridge {
            bridge_type: VirtualBridgeType::Wsl2Nat,
            interface_name: "vEthernet (WSL)".to_string(),
            subnet_cidr: "172.16.0.0/12".to_string(),
            gateway_ip: None,
            is_mirrored_mode: false,
        });
    }

    /// Registers a discovered virtual network bridge.
    pub fn register_bridge(&mut self, bridge: VirtualNetworkBridge) {
        if !self
            .bridges
            .iter()
            .any(|b| b.subnet_cidr == bridge.subnet_cidr)
        {
            self.bridges.push(bridge);
        }
    }

    /// Returns the currently tracked virtual bridges.
    pub fn bridges(&self) -> &[VirtualNetworkBridge] {
        &self.bridges
    }

    /// Checks if a target IP belongs to a tracked virtual bridge network.
    pub fn classify_virtual_ip(&self, ip: &IpAddr) -> Option<VirtualBridgeType> {
        for bridge in &self.bridges {
            if matches_cidr(&bridge.subnet_cidr, *ip) {
                return Some(bridge.bridge_type);
            }
        }
        None
    }

    /// Generates Mihomo / Clash routing rules for virtual bridge isolation.
    pub fn generate_bridge_rules(
        &self,
        mode: BridgeRoutingMode,
        proxy_outbound: &str,
    ) -> Vec<RuleEntry> {
        let mut rules = Vec::new();

        // 1. Process-level rules for WSL & Docker daemons
        match mode {
            BridgeRoutingMode::BypassAllBridges => {
                rules.push(RuleEntry {
                    rule: "PROCESS-NAME,wsl.exe,DIRECT".to_string(),
                    enabled: true,
                });
                rules.push(RuleEntry {
                    rule: "PROCESS-NAME,wslservice.exe,DIRECT".to_string(),
                    enabled: true,
                });
                rules.push(RuleEntry {
                    rule: "PROCESS-NAME,dockerd.exe,DIRECT".to_string(),
                    enabled: true,
                });
                rules.push(RuleEntry {
                    rule: "PROCESS-NAME,dockerd,DIRECT".to_string(),
                    enabled: true,
                });
                rules.push(RuleEntry {
                    rule: "PROCESS-NAME,docker-proxy,DIRECT".to_string(),
                    enabled: true,
                });
            }
            BridgeRoutingMode::ProxyWslOnly => {
                rules.push(RuleEntry {
                    rule: format!("PROCESS-NAME,wsl.exe,{proxy_outbound}"),
                    enabled: true,
                });
                rules.push(RuleEntry {
                    rule: format!("PROCESS-NAME,wslservice.exe,{proxy_outbound}"),
                    enabled: true,
                });
                rules.push(RuleEntry {
                    rule: "PROCESS-NAME,dockerd.exe,DIRECT".to_string(),
                    enabled: true,
                });
                rules.push(RuleEntry {
                    rule: "PROCESS-NAME,dockerd,DIRECT".to_string(),
                    enabled: true,
                });
            }
            BridgeRoutingMode::ProxyDockerOnly => {
                rules.push(RuleEntry {
                    rule: "PROCESS-NAME,wsl.exe,DIRECT".to_string(),
                    enabled: true,
                });
                rules.push(RuleEntry {
                    rule: format!("PROCESS-NAME,dockerd.exe,{proxy_outbound}"),
                    enabled: true,
                });
                rules.push(RuleEntry {
                    rule: format!("PROCESS-NAME,dockerd,{proxy_outbound}"),
                    enabled: true,
                });
            }
            BridgeRoutingMode::ProxyAllBridges => {
                rules.push(RuleEntry {
                    rule: format!("PROCESS-NAME,wsl.exe,{proxy_outbound}"),
                    enabled: true,
                });
                rules.push(RuleEntry {
                    rule: format!("PROCESS-NAME,dockerd.exe,{proxy_outbound}"),
                    enabled: true,
                });
            }
        }

        // 2. CIDR-level subnet isolation rules
        for bridge in &self.bridges {
            let target = match (mode, bridge.bridge_type) {
                (BridgeRoutingMode::BypassAllBridges, _) => "DIRECT",
                (
                    BridgeRoutingMode::ProxyWslOnly,
                    VirtualBridgeType::Wsl2Nat | VirtualBridgeType::Wsl2Mirrored,
                ) => proxy_outbound,
                (BridgeRoutingMode::ProxyWslOnly, _) => "DIRECT",
                (
                    BridgeRoutingMode::ProxyDockerOnly,
                    VirtualBridgeType::DockerDefaultBridge | VirtualBridgeType::DockerCustomBridge,
                ) => proxy_outbound,
                (BridgeRoutingMode::ProxyDockerOnly, _) => "DIRECT",
                (BridgeRoutingMode::ProxyAllBridges, _) => proxy_outbound,
            };

            let rule_type = if bridge.subnet_cidr.contains(':') {
                "IP-CIDR6"
            } else {
                "IP-CIDR"
            };

            rules.push(RuleEntry {
                rule: format!("{},{},{},no-resolve", rule_type, bridge.subnet_cidr, target),
                enabled: true,
            });
        }

        rules
    }
}

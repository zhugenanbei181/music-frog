//! Desktop physical-link MTU observation.

use async_trait::async_trait;
use infiltrator_contract::mtu::PhysicalMtuSnapshot;
use infiltrator_ports::error::PortError;
use infiltrator_ports::mtu_probe::MtuProbePort;
use mihomo_platform::interface_watcher::{GatewayPriorityArbiter, NetworkInterfaceSnapshot, NetworkInterfaceWatcher};
use std::collections::HashMap;
use std::process::Command;

/// Reads the active physical interface and its OS-reported link MTU. The
/// application layer performs the virtual-TUN subtraction and MSS math.
#[derive(Clone, Copy, Debug, Default)]
pub struct DesktopMtuProbe;

impl DesktopMtuProbe {
    pub fn new() -> Self {
        Self
    }

    fn probe_sync() -> Result<PhysicalMtuSnapshot, PortError> {
        let mut interfaces = NetworkInterfaceWatcher::poll_interfaces();
        apply_link_mtu(&mut interfaces, &link_mtu_table());
        let selected = default_route_interface()
            .as_deref()
            .and_then(|name| {
                interfaces.iter().find(|interface| {
                    interface.name == name
                        && interface.is_up
                        && !interface.is_loopback
                        && !interface.inferred_type().is_tun()
                        && !interface.ip_addresses.is_empty()
                })
            })
            .or_else(|| GatewayPriorityArbiter::select_best_candidate(&interfaces))
            .ok_or_else(|| {
                PortError::NotFound("no active physical interface with an address".to_owned())
            })?;
        let mtu = selected.mtu.ok_or_else(|| {
            PortError::unsupported(
                infiltrator_contract::capability::Capability::Tun,
                format!("OS did not expose link MTU for interface {}", selected.name),
            )
        })?;
        Ok(PhysicalMtuSnapshot {
            interface: selected.name.clone(),
            mtu,
        })
    }
}

#[async_trait]
impl MtuProbePort for DesktopMtuProbe {
    async fn probe_physical_mtu(&self) -> Result<PhysicalMtuSnapshot, PortError> {
        tokio::task::spawn_blocking(Self::probe_sync)
            .await
            .map_err(|error| PortError::Io(format!("physical MTU probe worker failed: {error}")))?
    }
}

fn apply_link_mtu(interfaces: &mut [NetworkInterfaceSnapshot], mtus: &HashMap<String, u32>) {
    for interface in interfaces {
        if let Some(mtu) = mtus.get(&interface.name) {
            interface.mtu = Some(*mtu);
        }
    }
}

#[allow(clippy::needless_return)]
fn link_mtu_table() -> HashMap<String, u32> {
    #[cfg(target_os = "linux")]
    {
        return Command::new("ip")
            .args(["-o", "link", "show"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| parse_linux_link_mtu(&String::from_utf8_lossy(&output.stdout)))
            .unwrap_or_default();
    }

    #[cfg(target_os = "macos")]
    {
        return Command::new("ifconfig")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| parse_ifconfig_mtu(&String::from_utf8_lossy(&output.stdout)))
            .unwrap_or_default();
    }

    #[cfg(target_os = "windows")]
    {
        return Command::new("netsh")
            .args(["interface", "ipv4", "show", "subinterfaces"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| parse_netsh_mtu(&String::from_utf8_lossy(&output.stdout)))
            .unwrap_or_default();
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    HashMap::new()
}

#[allow(clippy::needless_return)]
fn default_route_interface() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        return Command::new("ip")
            .args(["route", "show", "default"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| parse_linux_default_route(&String::from_utf8_lossy(&output.stdout)));
    }

    #[cfg(target_os = "macos")]
    {
        return Command::new("route")
            .args(["-n", "get", "default"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| parse_macos_default_route(&String::from_utf8_lossy(&output.stdout)));
    }

    #[cfg(target_os = "windows")]
    {
        return Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "(Get-NetRoute -AddressFamily IPv4 -DestinationPrefix '0.0.0.0/0' | Sort-Object RouteMetric | Select-Object -First 1).ifAlias",
            ])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| parse_windows_default_route(&String::from_utf8_lossy(&output.stdout)));
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    None
}

fn parse_linux_link_mtu(output: &str) -> HashMap<String, u32> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let _index = fields.next()?;
            let name = fields.next()?.trim_end_matches(':').split('@').next()?;
            let fields: Vec<&str> = fields.collect();
            let mtu_index = fields.iter().position(|field| *field == "mtu")?;
            let mtu = fields.get(mtu_index + 1)?.parse().ok()?;
            Some((name.to_owned(), mtu))
        })
        .collect()
}

fn parse_linux_default_route(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        (fields.next()? == "default").then(|| {
            fields
                .collect::<Vec<_>>()
                .windows(2)
                .find(|pair| pair[0] == "dev")
                .map(|pair| pair[1].to_owned())
        })?
    })
}

#[allow(dead_code)]
fn parse_ifconfig_mtu(output: &str) -> HashMap<String, u32> {
    output
        .lines()
        .filter_map(|line| {
            let name = line.split(':').next()?.trim();
            if name.is_empty() || !line.contains(" mtu ") {
                return None;
            }
            let fields: Vec<&str> = line.split_whitespace().collect();
            let mtu_index = fields.iter().position(|field| *field == "mtu")?;
            Some((name.to_owned(), fields.get(mtu_index + 1)?.parse().ok()?))
        })
        .collect()
}

#[allow(dead_code)]
fn parse_macos_default_route(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == "interface").then(|| value.trim().to_owned())
    })
}

#[allow(dead_code)]
fn parse_netsh_mtu(output: &str) -> HashMap<String, u32> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let mtu = fields.next()?.parse().ok()?;
            let _metric = fields.next()?;
            let _bytes_in = fields.next()?;
            let _bytes_out = fields.next()?;
            let name = fields.collect::<Vec<_>>().join(" ");
            (!name.is_empty()).then_some((name, mtu))
        })
        .collect()
}

#[allow(dead_code)]
fn parse_windows_default_route(output: &str) -> Option<String> {
    output.lines().map(str::trim).find(|line| !line.is_empty()).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_link_parser_reads_interface_mtu_without_shell_interpolation() {
        let output = "2: eth0@if3: <BROADCAST,UP> mtu 1500 qdisc fq_codel state UP\n3: wlan0: <BROADCAST> mtu 1400 state UP";
        let mtus = parse_linux_link_mtu(output);
        assert_eq!(mtus.get("eth0"), Some(&1500));
        assert_eq!(mtus.get("wlan0"), Some(&1400));
    }

    #[test]
    fn ifconfig_parser_reads_mtu_lines() {
        let output = "en0: flags=8863<UP> mtu 1500\n\tinet 192.168.1.4";
        assert_eq!(parse_ifconfig_mtu(output).get("en0"), Some(&1500));
    }

    #[test]
    fn linux_default_route_parser_prefers_the_real_egress_interface() {
        let output = "default via 192.0.2.1 dev wlan0 proto dhcp metric 600\n";
        assert_eq!(parse_linux_default_route(output).as_deref(), Some("wlan0"));
    }

    #[test]
    fn macos_default_route_parser_reads_interface_field() {
        let output = "   gateway: 192.0.2.1\n interface: en0\n";
        assert_eq!(parse_macos_default_route(output).as_deref(), Some("en0"));
    }

    #[test]
    fn windows_default_route_parser_reads_alias_output() {
        assert_eq!(
            parse_windows_default_route("\nEthernet 2\n").as_deref(),
            Some("Ethernet 2")
        );
    }

    #[test]
    fn netsh_parser_ignores_header_and_keeps_interface_name() {
        let output = "MTU  MediaSenseState   Bytes In  Bytes Out  Interface\n1500  1                10         20         Ethernet";
        assert_eq!(parse_netsh_mtu(output).get("Ethernet"), Some(&1500));
    }

    #[test]
    fn link_mtu_overrides_category_fallback() {
        let mut interfaces = vec![
            NetworkInterfaceSnapshot::new("eth0", true, true, vec!["192.0.2.1".to_owned()]),
        ];
        let mut mtus = HashMap::new();
        mtus.insert("eth0".to_owned(), 1492);
        apply_link_mtu(&mut interfaces, &mtus);
        assert_eq!(interfaces[0].effective_mtu(), 1492);
    }
}
